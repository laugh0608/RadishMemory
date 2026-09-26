use std::collections::BTreeMap;

use radishmemory_core::{
    Identifier, SourceArtifact, SourceCapture, SourceCaptureOutcome, SourceCaptureResult,
    SourceCaptureStore, SourceOriginKind, source_origin_binding_id_is_valid,
    validate_complete_source_fragment_set,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::source_store::{
    identifier, insert_source_artifact, insert_source_fragments, load_source_artifact,
};
use crate::{SqliteDatabase, SqliteError, SqliteStorageReason};

impl SourceCaptureStore for SqliteDatabase {
    type Error = SqliteError;

    fn capture_source(
        &mut self,
        capture: &SourceCapture,
    ) -> Result<SourceCaptureResult, Self::Error> {
        capture_source_with_before_commit(self, capture, |_| Ok(()))
    }
}

fn capture_source_with_before_commit<BeforeCommit>(
    database: &mut SqliteDatabase,
    capture: &SourceCapture,
    before_commit: BeforeCommit,
) -> Result<SourceCaptureResult, SqliteError>
where
    BeforeCommit: FnOnce(&Connection) -> Result<(), SqliteError>,
{
    let transaction = database
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(SqliteError::storage)?;
    crate::derived_index::verify(&transaction)?;
    verify_origin_bindings(&transaction)?;

    let (result_source, outcome) = decide_capture(&transaction, capture, &load_source_artifact)?;
    if outcome != SourceCaptureOutcome::Idempotent {
        insert_capture_facts(&transaction, capture, outcome, true)?;
    }

    crate::derived_index::verify(&transaction)?;
    verify_origin_bindings(&transaction)?;
    before_commit(&transaction)?;
    transaction.commit().map_err(SqliteError::storage)?;
    Ok(SourceCaptureResult::from_source(&result_source, outcome))
}

pub(crate) fn decide_capture(
    connection: &Connection,
    capture: &SourceCapture,
    load_source: crate::source_store::SourceLoader<'_>,
) -> Result<(SourceArtifact, SourceCaptureOutcome), SqliteError> {
    let candidate = capture.source();
    let value = candidate.params();
    if let Some(lineage_id) =
        load_origin_lineage(connection, &value.namespace_id, capture.origin_binding_id())?
    {
        if lineage_id != value.lineage_id {
            return Err(capture_mismatch(SqliteStorageReason::OriginBindingMismatch));
        }
        let current =
            load_current_source_with(connection, &value.namespace_id, &lineage_id, load_source)?;
        validate_existing_capture(connection, capture.origin_binding_id(), &current)?;
        if value.governance != current.params().governance {
            return Err(capture_mismatch(SqliteStorageReason::CaptureStateMismatch));
        }
        if same_capture_bytes(candidate, &current) {
            if value.source_kind != current.params().source_kind
                || value.media_type != current.params().media_type
            {
                return Err(capture_mismatch(SqliteStorageReason::CaptureStateMismatch));
            }
            return Ok((current, SourceCaptureOutcome::Idempotent));
        }
        Ok((candidate.clone(), SourceCaptureOutcome::Versioned))
    } else {
        require_unused_lineage(connection, candidate)?;
        Ok((candidate.clone(), SourceCaptureOutcome::Created))
    }
}

pub(crate) fn insert_capture_facts(
    connection: &Connection,
    capture: &SourceCapture,
    outcome: SourceCaptureOutcome,
    inline: bool,
) -> Result<(), SqliteError> {
    if outcome == SourceCaptureOutcome::Idempotent {
        return Err(capture_mismatch(SqliteStorageReason::CaptureStateMismatch));
    }
    if inline {
        insert_source_artifact(connection, capture.source())?;
    } else {
        crate::source_store::insert_source_metadata(connection, capture.source())?;
    }
    advance_lineage_tip(connection, capture.source())?;
    if outcome == SourceCaptureOutcome::Created {
        insert_origin_binding(connection, capture.origin_binding_id(), capture.source())?;
    }
    if inline {
        insert_source_fragments(connection, capture.fragments())?;
    } else {
        crate::source_store::insert_fragments_for_source(
            connection,
            capture.fragments(),
            capture.source(),
        )?;
    }
    insert_capture_audit(
        connection,
        capture.origin_binding_id(),
        capture.source(),
        outcome,
    )
}

pub(crate) fn advance_lineage_tip(
    connection: &Connection,
    source: &SourceArtifact,
) -> Result<(), SqliteError> {
    let value = source.params();
    let current = connection
        .query_row(
            "SELECT source_id, version
             FROM radishmemory_source_lineage_tips
             WHERE namespace_id = ?1 AND lineage_id = ?2",
            params![value.namespace_id.as_str(), value.lineage_id.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(SqliteError::storage)?;
    let version = i64::try_from(value.version.get())
        .map_err(|_| capture_mismatch(SqliteStorageReason::LineageTipMismatch))?;
    match current {
        None => {
            if value.version.get() != 1 || !value.supersedes_source_ids.is_empty() {
                return Err(capture_mismatch(SqliteStorageReason::LineageTipMismatch));
            }
            connection
                .execute(
                    "INSERT INTO radishmemory_source_lineage_tips (
                         namespace_id, lineage_id, source_id, version
                     ) VALUES (?1, ?2, ?3, ?4)",
                    params![
                        value.namespace_id.as_str(),
                        value.lineage_id.as_str(),
                        value.source_id.as_str(),
                        version,
                    ],
                )
                .map_err(SqliteError::storage)?;
        }
        Some((current_source_id, current_version)) => {
            let expected_version = current_version
                .checked_add(1)
                .ok_or_else(|| capture_mismatch(SqliteStorageReason::LineageTipMismatch))?;
            if version != expected_version
                || value.supersedes_source_ids.len() != 1
                || value.supersedes_source_ids[0].as_str() != current_source_id
            {
                return Err(capture_mismatch(SqliteStorageReason::LineageTipMismatch));
            }
            crate::derived_index::remove_source_fragments(
                connection,
                &Identifier::new(current_source_id.clone()).map_err(|source| {
                    SqliteError::invalid_stored_with_source(
                        SqliteStorageReason::StoredIntegrityMismatch,
                        source,
                    )
                })?,
            )?;
            let changed = connection
                .execute(
                    "UPDATE radishmemory_source_lineage_tips
                     SET source_id = ?1, version = ?2
                     WHERE namespace_id = ?3 AND lineage_id = ?4
                       AND source_id = ?5 AND version = ?6",
                    params![
                        value.source_id.as_str(),
                        version,
                        value.namespace_id.as_str(),
                        value.lineage_id.as_str(),
                        current_source_id,
                        current_version,
                    ],
                )
                .map_err(SqliteError::storage)?;
            if changed != 1 {
                return Err(capture_mismatch(SqliteStorageReason::LineageTipMismatch));
            }
        }
    }
    Ok(())
}

pub(crate) fn verify_origin_bindings(connection: &Connection) -> Result<(), SqliteError> {
    verify_origin_binding_rows(connection)?;
    verify_active_file_captures(connection)
}

pub(crate) fn verify_origin_binding_rows(connection: &Connection) -> Result<(), SqliteError> {
    // Bindings are canonical: their validation must not depend on repairable tips.
    let expected = expected_origin_bindings(connection)?;
    let actual = actual_origin_bindings(connection)?;
    if expected
        .iter()
        .any(|(key, lineage_id)| actual.get(key) != Some(lineage_id))
    {
        return Err(SqliteError::invalid_stored(
            SqliteStorageReason::OriginBindingMismatch,
        ));
    }
    for ((namespace_id, binding_id), lineage_id) in &actual {
        if expected.get(&(namespace_id.clone(), binding_id.clone())) == Some(lineage_id) {
            continue;
        }
        let belongs_to_closed_plan: bool = connection
            .query_row(
                "SELECT EXISTS (
                     SELECT 1 FROM radishmemory_source_artifacts
                     WHERE namespace_id = ?1 AND lineage_id = ?2
                       AND origin_kind = 'explicit_user_input'
                       AND deletion_state IN ('pending', 'failed')
                 )",
                params![namespace_id, lineage_id],
                |row| row.get(0),
            )
            .map_err(SqliteError::storage)?;
        if !belongs_to_closed_plan {
            return Err(SqliteError::invalid_stored(
                SqliteStorageReason::OriginBindingMismatch,
            ));
        }
    }
    Ok(())
}

fn verify_active_file_captures(connection: &Connection) -> Result<(), SqliteError> {
    let mut statement = connection
        .prepare(
            "SELECT namespace_id, source_id, origin_ref
             FROM radishmemory_source_artifacts
             WHERE origin_kind = 'explicit_user_input' AND deletion_state = 'active'
             ORDER BY namespace_id, source_id",
        )
        .map_err(SqliteError::storage)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })
        .map_err(SqliteError::storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(SqliteError::storage)?;
    drop(statement);

    for (namespace_id, source_id, origin_binding_id) in rows {
        let Some(origin_binding_id) = origin_binding_id else {
            return Err(SqliteError::invalid_stored(
                SqliteStorageReason::OriginBindingMismatch,
            ));
        };
        if !source_origin_binding_id_is_valid(&origin_binding_id) {
            return Err(SqliteError::invalid_stored(
                SqliteStorageReason::OriginBindingMismatch,
            ));
        }
        let namespace_id = identifier(namespace_id)?;
        let source_id = identifier(source_id)?;
        let origin_binding_id = identifier(origin_binding_id)?;
        let source =
            load_source_artifact(connection, &namespace_id, &source_id)?.ok_or_else(|| {
                SqliteError::invalid_stored(SqliteStorageReason::StoredIntegrityMismatch)
            })?;
        validate_existing_capture(connection, &origin_binding_id, &source)?;
    }
    Ok(())
}

pub(crate) fn validate_existing_capture(
    connection: &Connection,
    origin_binding_id: &Identifier,
    source: &SourceArtifact,
) -> Result<(), SqliteError> {
    if source.params().origin_kind != SourceOriginKind::ExplicitUserInput
        || source
            .params()
            .origin_ref
            .as_ref()
            .map(|value| value.as_str())
            != Some(origin_binding_id.as_str())
    {
        return Err(SqliteError::invalid_stored(
            SqliteStorageReason::OriginBindingMismatch,
        ));
    }
    let fragments = crate::source_store::load_fragments_for_source(connection, source)?;
    validate_complete_source_fragment_set(source, &fragments).map_err(|source| {
        SqliteError::invalid_stored_with_source(
            SqliteStorageReason::StoredIntegrityMismatch,
            source,
        )
    })?;
    let expected_outcome = if source.params().version.get() == 1 {
        "created"
    } else {
        "versioned"
    };
    let audit_matches: bool = connection
        .query_row(
            "SELECT EXISTS (
                 SELECT 1 FROM radishmemory_source_capture_audit
                 WHERE source_id = ?1 AND namespace_id = ?2 AND origin_binding_id = ?3
                   AND outcome = ?4 AND recorded_at = ?5
             )",
            params![
                source.params().source_id.as_str(),
                source.params().namespace_id.as_str(),
                origin_binding_id.as_str(),
                expected_outcome,
                source.params().captured_at.original(),
            ],
            |row| row.get(0),
        )
        .map_err(SqliteError::storage)?;
    if !audit_matches {
        return Err(SqliteError::invalid_stored(
            SqliteStorageReason::CaptureStateMismatch,
        ));
    }
    Ok(())
}

fn same_capture_bytes(candidate: &SourceArtifact, current: &SourceArtifact) -> bool {
    candidate.params().content_length == current.params().content_length
        && candidate.params().content_digest == current.params().content_digest
        && candidate.params().content == current.params().content
}

fn load_origin_lineage(
    connection: &Connection,
    namespace_id: &Identifier,
    origin_binding_id: &Identifier,
) -> Result<Option<Identifier>, SqliteError> {
    let value = connection
        .query_row(
            "SELECT lineage_id FROM radishmemory_source_origin_bindings
             WHERE namespace_id = ?1 AND origin_binding_id = ?2",
            params![namespace_id.as_str(), origin_binding_id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(SqliteError::storage)?;
    value.map(Identifier::new).transpose().map_err(|source| {
        SqliteError::invalid_stored_with_source(
            SqliteStorageReason::StoredIntegrityMismatch,
            source,
        )
    })
}

fn load_current_source_with(
    connection: &Connection,
    namespace_id: &Identifier,
    lineage_id: &Identifier,
    load_source: crate::source_store::SourceLoader<'_>,
) -> Result<SourceArtifact, SqliteError> {
    let source_id = connection
        .query_row(
            "SELECT source_id FROM radishmemory_source_lineage_tips
             WHERE namespace_id = ?1 AND lineage_id = ?2",
            params![namespace_id.as_str(), lineage_id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(SqliteError::storage)?
        .ok_or_else(|| SqliteError::invalid_stored(SqliteStorageReason::LineageTipMismatch))?;
    let source_id = Identifier::new(source_id).map_err(|source| {
        SqliteError::invalid_stored_with_source(
            SqliteStorageReason::StoredIntegrityMismatch,
            source,
        )
    })?;
    load_source(connection, namespace_id, &source_id)?
        .ok_or_else(|| SqliteError::invalid_stored(SqliteStorageReason::LineageTipMismatch))
}

fn require_unused_lineage(
    connection: &Connection,
    source: &SourceArtifact,
) -> Result<(), SqliteError> {
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM radishmemory_source_lineage_tips
             WHERE namespace_id = ?1 AND lineage_id = ?2",
            params![
                source.params().namespace_id.as_str(),
                source.params().lineage_id.as_str(),
            ],
            |row| row.get(0),
        )
        .map_err(SqliteError::storage)?;
    if count != 0 {
        return Err(capture_mismatch(SqliteStorageReason::OriginBindingMismatch));
    }
    Ok(())
}

fn insert_origin_binding(
    connection: &Connection,
    origin_binding_id: &Identifier,
    source: &SourceArtifact,
) -> Result<(), SqliteError> {
    let changed = connection
        .execute(
            "INSERT INTO radishmemory_source_origin_bindings (
                 namespace_id, origin_binding_id, lineage_id
             ) VALUES (?1, ?2, ?3)",
            params![
                source.params().namespace_id.as_str(),
                origin_binding_id.as_str(),
                source.params().lineage_id.as_str(),
            ],
        )
        .map_err(SqliteError::storage)?;
    if changed != 1 {
        return Err(capture_mismatch(SqliteStorageReason::OriginBindingMismatch));
    }
    Ok(())
}

fn insert_capture_audit(
    connection: &Connection,
    origin_binding_id: &Identifier,
    source: &SourceArtifact,
    outcome: SourceCaptureOutcome,
) -> Result<(), SqliteError> {
    let outcome = match outcome {
        SourceCaptureOutcome::Created => "created",
        SourceCaptureOutcome::Versioned => "versioned",
        SourceCaptureOutcome::Idempotent => {
            return Err(capture_mismatch(SqliteStorageReason::CaptureStateMismatch));
        }
    };
    let changed = connection
        .execute(
            "INSERT INTO radishmemory_source_capture_audit (
                 source_id, namespace_id, origin_binding_id, outcome, recorded_at
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                source.params().source_id.as_str(),
                source.params().namespace_id.as_str(),
                origin_binding_id.as_str(),
                outcome,
                source.params().captured_at.original(),
            ],
        )
        .map_err(SqliteError::storage)?;
    if changed != 1 {
        return Err(capture_mismatch(SqliteStorageReason::CaptureStateMismatch));
    }
    Ok(())
}

fn expected_origin_bindings(
    connection: &Connection,
) -> Result<BTreeMap<(String, String), String>, SqliteError> {
    let mut statement = connection
        .prepare(
            "SELECT source.namespace_id, source.origin_ref, source.lineage_id
             FROM radishmemory_source_artifacts AS source
             WHERE source.origin_kind = 'explicit_user_input'
               AND source.origin_ref IS NOT NULL
               AND source.deletion_state = 'active'
               AND NOT EXISTS (
                   SELECT 1 FROM radishmemory_source_artifacts AS newer
                   WHERE newer.namespace_id = source.namespace_id
                     AND newer.lineage_id = source.lineage_id
                     AND newer.version > source.version
               )
             ORDER BY source.namespace_id, source.origin_ref",
        )
        .map_err(SqliteError::storage)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                (row.get::<_, String>(0)?, row.get::<_, String>(1)?),
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(SqliteError::storage)?;
    let bindings = collect_unique_bindings(rows)?;
    if bindings
        .keys()
        .any(|(_, binding_id)| !source_origin_binding_id_is_valid(binding_id))
    {
        return Err(SqliteError::invalid_stored(
            SqliteStorageReason::OriginBindingMismatch,
        ));
    }
    Ok(bindings)
}

fn actual_origin_bindings(
    connection: &Connection,
) -> Result<BTreeMap<(String, String), String>, SqliteError> {
    let mut statement = connection
        .prepare(
            "SELECT namespace_id, origin_binding_id, lineage_id
             FROM radishmemory_source_origin_bindings
             ORDER BY namespace_id, origin_binding_id",
        )
        .map_err(SqliteError::storage)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                (row.get::<_, String>(0)?, row.get::<_, String>(1)?),
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(SqliteError::storage)?;
    collect_unique_bindings(rows)
}

fn collect_unique_bindings(
    rows: impl Iterator<Item = rusqlite::Result<((String, String), String)>>,
) -> Result<BTreeMap<(String, String), String>, SqliteError> {
    let mut values = BTreeMap::new();
    for row in rows {
        let (key, value) = row.map_err(SqliteError::storage)?;
        if values.insert(key, value).is_some() {
            return Err(SqliteError::invalid_stored(
                SqliteStorageReason::OriginBindingMismatch,
            ));
        }
    }
    Ok(values)
}

fn capture_mismatch(reason: SqliteStorageReason) -> SqliteError {
    SqliteError::source_invariant(reason)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::error::Error as _;

    use radishmemory_core::{
        DeletionState, EgressPolicy, Governance, MediaType, NonEmptyText, ProducerRef,
        ProducerType, RetentionMode, RetentionRule, Sensitivity, SourceArtifactParams,
        SourceFragment, SourceFragmentParams, SourceKind, Timestamp, Version,
        compute_exact_bytes_digest,
    };

    use super::*;
    use crate::{SqliteErrorCode, SqliteStorageReason};

    fn id(value: &str) -> Identifier {
        Identifier::new(value).expect("synthetic identifier must be valid")
    }

    fn text(value: &str) -> NonEmptyText {
        NonEmptyText::new(value).expect("synthetic text must be nonempty")
    }

    fn timestamp(value: &str) -> Timestamp {
        Timestamp::parse(value).expect("synthetic timestamp must be valid")
    }

    fn governance() -> Governance {
        Governance::new(
            Sensitivity::Personal,
            EgressPolicy::LocalOnly,
            RetentionRule::new(RetentionMode::UntilDeleted, None, None)
                .expect("synthetic retention must be valid"),
            DeletionState::Active,
            id("policy-local-only"),
        )
        .expect("synthetic governance must be valid")
    }

    fn producer(producer_type: ProducerType, producer_id: &str) -> ProducerRef {
        ProducerRef::new(producer_type, id(producer_id), text("1"))
    }

    pub(crate) fn capture(
        source_id: &str,
        fragment_id: &str,
        version: u64,
        content: &str,
        captured_at: &str,
    ) -> SourceCapture {
        let content = text(content);
        let content_digest = compute_exact_bytes_digest(content.as_str().as_bytes());
        let supersedes_source_ids = if version == 1 {
            vec![]
        } else {
            vec![id("source-commit-1")]
        };
        let source = SourceArtifact::new(SourceArtifactParams {
            source_id: id(source_id),
            lineage_id: id("lineage-commit-1"),
            version: Version::new(version).expect("synthetic version must be valid"),
            namespace_id: id("namespace-commit-1"),
            source_kind: SourceKind::Text,
            media_type: MediaType::TextPlain,
            content: content.clone(),
            content_length: content.utf8_len() as u64,
            content_digest: content_digest.clone(),
            title: None,
            origin_kind: SourceOriginKind::ExplicitUserInput,
            origin_ref: Some(text("origin-binding-commit-1")),
            observed_at: timestamp(captured_at),
            captured_at: timestamp(captured_at),
            supersedes_source_ids,
            governance: governance(),
            producer: producer(ProducerType::Parser, "file-entry-parser"),
            created_at: timestamp(captured_at),
        })
        .expect("synthetic source must be valid");
        let fragment = SourceFragment::new(SourceFragmentParams {
            fragment_id: id(fragment_id),
            namespace_id: id("namespace-commit-1"),
            source_id: id(source_id),
            ordinal: 0,
            byte_start: 0,
            byte_end: content.utf8_len() as u64,
            heading_path: None,
            content,
            content_digest,
            segmenter: producer(ProducerType::Rule, "whole-file-segmenter"),
            governance: governance(),
            created_at: timestamp(captured_at),
        })
        .expect("synthetic fragment must be valid");
        SourceCapture::new(id("origin-binding-commit-1"), source, vec![fragment])
            .expect("synthetic capture must be valid")
    }

    fn capture_row_counts(connection: &Connection) -> [i64; 8] {
        let mut counts = [0; 8];
        for (index, table) in [
            "radishmemory_source_artifacts",
            "radishmemory_source_bodies",
            "radishmemory_source_supersedes",
            "radishmemory_source_fragments",
            "radishmemory_source_lineage_tips",
            "radishmemory_source_origin_bindings",
            "radishmemory_source_capture_audit",
            "radishmemory_recall_fts",
        ]
        .iter()
        .enumerate()
        {
            counts[index] = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("synthetic row count must be queryable");
        }
        counts
    }

    #[test]
    fn p1_f16_capture_commit_failure_rolls_back_all_facts_and_preserves_real_error() {
        let connection = Connection::open_in_memory().expect("in-memory SQLite must open");
        let mut database =
            SqliteDatabase::initialize(connection).expect("database must initialize");
        let first = capture(
            "source-commit-1",
            "fragment-commit-1",
            1,
            "Stable committed capture marker.\n",
            "2026-08-30T08:00:00Z",
        );
        database
            .capture_source(&first)
            .expect("baseline capture must commit");
        let counts_before = capture_row_counts(database.connection());
        let tip_before: (String, i64) = database
            .connection()
            .query_row(
                "SELECT source_id, version FROM radishmemory_source_lineage_tips",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("baseline tip must be queryable");

        let second = capture(
            "source-commit-2",
            "fragment-commit-2",
            2,
            "Uncommitted replacement capture marker.\n",
            "2026-08-30T09:00:00Z",
        );
        let error = capture_source_with_before_commit(&mut database, &second, |connection| {
            connection
                .execute(
                    "INSERT INTO radishmemory_missing_capture_commit_point VALUES (1)",
                    [],
                )
                .map(|_| ())
                .map_err(SqliteError::storage)
        })
        .expect_err("commit-point failure must reject the complete capture");

        assert_eq!(error.code(), SqliteErrorCode::Storage);
        assert!(
            error
                .source()
                .expect("real SQLite cause must be retained")
                .to_string()
                .contains("no such table")
        );
        assert!(!format!("{error:?}").contains("Uncommitted replacement"));
        assert!(database.connection().is_autocommit());
        assert_eq!(capture_row_counts(database.connection()), counts_before);
        let tip_after: (String, i64) = database
            .connection()
            .query_row(
                "SELECT source_id, version FROM radishmemory_source_lineage_tips",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("tip after rollback must be queryable");
        assert_eq!(tip_after, tip_before);
        let rejected_source_count: i64 = database
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM radishmemory_source_artifacts WHERE source_id = ?1",
                params!["source-commit-2"],
                |row| row.get(0),
            )
            .expect("rejected source count must be queryable");
        assert_eq!(rejected_source_count, 0);
        crate::derived_index::verify(database.connection())
            .expect("rollback must preserve exact recall derivations");
        verify_origin_bindings(database.connection())
            .expect("rollback must preserve exact origin bindings and audit");
    }

    #[test]
    fn library_verify_and_rebuild_reject_missing_origin_binding_without_repair() {
        let connection = Connection::open_in_memory().expect("in-memory SQLite must open");
        let mut database =
            SqliteDatabase::initialize(connection).expect("database must initialize");
        database
            .capture_source(&capture(
                "source-commit-1",
                "fragment-commit-1",
                1,
                "Binding integrity marker.\n",
                "2026-08-30T10:00:00Z",
            ))
            .expect("baseline capture must commit");

        database
            .connection()
            .execute("DELETE FROM radishmemory_source_origin_bindings", [])
            .expect("synthetic binding corruption must be applied");

        let verify_error = database
            .verify_recall_derivations()
            .expect_err("library verification must reject a missing binding");
        assert_eq!(verify_error.code(), SqliteErrorCode::InvalidStoredData);
        assert_eq!(
            verify_error.storage_reason(),
            Some(SqliteStorageReason::OriginBindingMismatch)
        );
        let rebuild_error = database
            .rebuild_recall_derivations()
            .expect_err("rebuild must not synthesize a missing canonical binding");
        assert_eq!(rebuild_error.code(), SqliteErrorCode::InvalidStoredData);
        assert_eq!(
            rebuild_error.storage_reason(),
            Some(SqliteStorageReason::OriginBindingMismatch)
        );
        let binding_count: i64 = database
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM radishmemory_source_origin_bindings",
                [],
                |row| row.get(0),
            )
            .expect("binding count must remain queryable");
        assert_eq!(binding_count, 0);
    }

    #[test]
    fn library_verify_and_rebuild_reject_tampered_body_without_repair() {
        let connection = Connection::open_in_memory().expect("in-memory SQLite must open");
        let mut database =
            SqliteDatabase::initialize(connection).expect("database must initialize");
        database
            .capture_source(&capture(
                "source-commit-1",
                "fragment-commit-1",
                1,
                "Canonical body integrity marker.\n",
                "2026-08-30T10:01:00Z",
            ))
            .expect("baseline capture must commit");
        let recall_before: String = database
            .connection()
            .query_row("SELECT content FROM radishmemory_recall_fts", [], |row| {
                row.get(0)
            })
            .expect("baseline recall row must be queryable");
        let tampered = b"Tampered managed body marker.\n";
        database
            .connection()
            .execute(
                "UPDATE radishmemory_source_bodies SET content = ?1 WHERE source_id = ?2",
                params![tampered.as_slice(), "source-commit-1"],
            )
            .expect("synthetic body corruption must be applied");

        let verify_error = database
            .verify_recall_derivations()
            .expect_err("library verification must reject tampered canonical bytes");
        assert_eq!(verify_error.code(), SqliteErrorCode::InvalidStoredData);
        assert_eq!(
            verify_error.storage_reason(),
            Some(SqliteStorageReason::InvalidCanonicalObject)
        );
        let rebuild_error = database
            .rebuild_recall_derivations()
            .expect_err("rebuild must not overwrite tampered canonical bytes");
        assert_eq!(rebuild_error.code(), SqliteErrorCode::InvalidStoredData);
        assert_eq!(
            rebuild_error.storage_reason(),
            Some(SqliteStorageReason::InvalidCanonicalObject)
        );
        let body_after: Vec<u8> = database
            .connection()
            .query_row(
                "SELECT content FROM radishmemory_source_bodies WHERE source_id = ?1",
                ["source-commit-1"],
                |row| row.get(0),
            )
            .expect("tampered body must remain present for explicit recovery");
        let recall_after: String = database
            .connection()
            .query_row("SELECT content FROM radishmemory_recall_fts", [], |row| {
                row.get(0)
            })
            .expect("recall row must remain queryable after failed rebuild");
        assert_eq!(body_after, tampered);
        assert_eq!(recall_after, recall_before);
    }
}
