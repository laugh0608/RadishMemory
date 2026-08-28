use std::collections::BTreeMap;

use radishmemory_core::{
    Identifier, SourceArtifact, SourceCapture, SourceCaptureOutcome, SourceCaptureResult,
    SourceCaptureStore, SourceOriginKind, source_origin_binding_id_is_valid,
    validate_complete_source_fragment_set,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::source_store::{
    identifier, insert_source_artifact, insert_source_fragments, load_source_artifact,
    load_source_fragments,
};
use crate::{SqliteDatabase, SqliteError, SqliteStorageReason};

impl SourceCaptureStore for SqliteDatabase {
    type Error = SqliteError;

    fn capture_source(
        &mut self,
        capture: &SourceCapture,
    ) -> Result<SourceCaptureResult, Self::Error> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SqliteError::storage)?;
        crate::derived_index::verify(&transaction)?;
        verify_origin_bindings(&transaction)?;

        let candidate = capture.source();
        let value = candidate.params();
        let existing_lineage = load_origin_lineage(
            &transaction,
            &value.namespace_id,
            capture.origin_binding_id(),
        )?;

        let (result_source, outcome) = if let Some(lineage_id) = existing_lineage {
            if lineage_id != value.lineage_id {
                return Err(capture_mismatch(SqliteStorageReason::OriginBindingMismatch));
            }
            let current = load_current_source(&transaction, &value.namespace_id, &lineage_id)?;
            validate_existing_capture(&transaction, capture.origin_binding_id(), &current)?;
            if same_capture_bytes(candidate, &current) {
                if candidate.params().source_kind != current.params().source_kind
                    || candidate.params().media_type != current.params().media_type
                    || candidate.params().governance != current.params().governance
                {
                    return Err(capture_mismatch(SqliteStorageReason::CaptureStateMismatch));
                }
                (current, SourceCaptureOutcome::Idempotent)
            } else {
                if candidate.params().governance != current.params().governance {
                    return Err(capture_mismatch(SqliteStorageReason::CaptureStateMismatch));
                }
                insert_source_artifact(&transaction, candidate)?;
                advance_lineage_tip(&transaction, candidate)?;
                insert_source_fragments(&transaction, capture.fragments())?;
                insert_capture_audit(
                    &transaction,
                    capture.origin_binding_id(),
                    candidate,
                    SourceCaptureOutcome::Versioned,
                )?;
                (candidate.clone(), SourceCaptureOutcome::Versioned)
            }
        } else {
            require_unused_lineage(&transaction, candidate)?;
            insert_source_artifact(&transaction, candidate)?;
            advance_lineage_tip(&transaction, candidate)?;
            insert_origin_binding(&transaction, capture.origin_binding_id(), candidate)?;
            insert_source_fragments(&transaction, capture.fragments())?;
            insert_capture_audit(
                &transaction,
                capture.origin_binding_id(),
                candidate,
                SourceCaptureOutcome::Created,
            )?;
            (candidate.clone(), SourceCaptureOutcome::Created)
        };

        crate::derived_index::verify(&transaction)?;
        verify_origin_bindings(&transaction)?;
        transaction.commit().map_err(SqliteError::storage)?;
        Ok(SourceCaptureResult::from_source(&result_source, outcome))
    }
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
    verify_active_file_captures(connection)
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

fn validate_existing_capture(
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
    let fragments = load_source_fragments(
        connection,
        &source.params().namespace_id,
        &source.params().source_id,
    )?
    .ok_or_else(|| SqliteError::invalid_stored(SqliteStorageReason::StoredIntegrityMismatch))?;
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

fn load_current_source(
    connection: &Connection,
    namespace_id: &Identifier,
    lineage_id: &Identifier,
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
    load_source_artifact(connection, namespace_id, &source_id)?
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
            "SELECT tip.namespace_id, source.origin_ref, tip.lineage_id
             FROM radishmemory_source_lineage_tips AS tip
             JOIN radishmemory_source_artifacts AS source ON source.source_id = tip.source_id
             WHERE source.origin_kind = 'explicit_user_input'
               AND source.origin_ref IS NOT NULL
             ORDER BY tip.namespace_id, source.origin_ref",
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
