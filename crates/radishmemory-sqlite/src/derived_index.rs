use std::collections::{BTreeMap, BTreeSet};

use radishmemory_core::{
    DeletionState, GovernedCanonicalObject, Identifier, LocalSearchHit, LocalSearchRequest,
    MemoryRecord, MemoryState, RetentionMode, Sensitivity, SourceArtifact, SourceFragment,
    Timestamp,
};
use rusqlite::{Connection, params};

use crate::memory_store::load_memory_record_closure;
use crate::source_store::{identifier, load_resolved_source_fragment, sensitivity_str};
use crate::{SqliteError, SqliteStorageReason};

const SOURCE_FRAGMENT_KIND: &str = "source_fragment";
const MEMORY_RECORD_KIND: &str = "memory_record";

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct RecallKey {
    object_kind: String,
    object_id: String,
}

impl RecallKey {
    fn source(fragment: &SourceFragment) -> Self {
        Self {
            object_kind: SOURCE_FRAGMENT_KIND.to_owned(),
            object_id: fragment.params().fragment_id.as_str().to_owned(),
        }
    }

    fn memory(record: &MemoryRecord) -> Self {
        Self {
            object_kind: MEMORY_RECORD_KIND.to_owned(),
            object_id: record.params().memory_id.as_str().to_owned(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RecallRow {
    namespace_id: String,
    sensitivity: String,
    content: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProjectionRow {
    namespace_id: String,
    current_state: String,
    last_state_event_id: String,
}

#[derive(Clone)]
struct Candidate {
    hit: LocalSearchHit,
    namespace_id: Identifier,
    sensitivity: Sensitivity,
    available_at: Timestamp,
    retention_mode: RetentionMode,
    retention_expires_at: Option<Timestamp>,
}

impl Candidate {
    fn source(fragment: SourceFragment, source: &SourceArtifact) -> Self {
        let governance = fragment.governance();
        Self {
            namespace_id: fragment.params().namespace_id.clone(),
            sensitivity: governance.sensitivity(),
            available_at: source.params().captured_at.clone(),
            retention_mode: governance.retention().mode(),
            retention_expires_at: governance.retention().expires_at().cloned(),
            hit: LocalSearchHit::SourceFragment(Box::new(fragment)),
        }
    }

    fn memory(record: MemoryRecord) -> Self {
        let value = record.params();
        let governance = &value.governance;
        Self {
            namespace_id: value.namespace_id.clone(),
            sensitivity: governance.sensitivity(),
            available_at: value.created_at.clone(),
            retention_mode: governance.retention().mode(),
            retention_expires_at: governance.retention().expires_at().cloned(),
            hit: LocalSearchHit::MemoryRecord(Box::new(record)),
        }
    }

    fn is_eligible(&self, request: &LocalSearchRequest) -> bool {
        if &self.namespace_id != request.namespace_id()
            || !request.allows_sensitivity(self.sensitivity)
            || self.available_at > *request.as_of()
        {
            return false;
        }
        if self.retention_mode == RetentionMode::UntilTime
            && self
                .retention_expires_at
                .as_ref()
                .is_none_or(|expires_at| expires_at <= request.as_of())
        {
            return false;
        }
        match &self.hit {
            LocalSearchHit::SourceFragment(_) => true,
            LocalSearchHit::MemoryRecord(record) => {
                record.params().valid_time.contains(request.as_of())
            }
        }
    }
}

struct Catalog {
    recall_rows: BTreeMap<RecallKey, RecallRow>,
    projections: BTreeMap<String, ProjectionRow>,
    candidates: BTreeMap<RecallKey, Candidate>,
}

impl Catalog {
    fn load(connection: &Connection) -> Result<Self, SqliteError> {
        let mut catalog = Self {
            recall_rows: BTreeMap::new(),
            projections: BTreeMap::new(),
            candidates: BTreeMap::new(),
        };
        catalog.load_sources(connection)?;
        catalog.load_memories(connection)?;
        Ok(catalog)
    }

    fn load_sources(&mut self, connection: &Connection) -> Result<(), SqliteError> {
        let mut statement = connection
            .prepare(
                "SELECT namespace_id, fragment_id
                 FROM radishmemory_source_fragments
                 ORDER BY namespace_id, fragment_id",
            )
            .map_err(SqliteError::storage)?;
        let stored = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(SqliteError::storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(SqliteError::storage)?;
        drop(statement);

        for (namespace, fragment_id) in stored {
            let namespace = identifier(namespace)?;
            let fragment_id = identifier(fragment_id)?;
            let (fragment, source) =
                load_resolved_source_fragment(connection, &namespace, &fragment_id)?
                    .ok_or_else(derived_mismatch)?;
            if fragment.governance().deletion_state() != DeletionState::Active
                || source.governance().deletion_state() != DeletionState::Active
            {
                continue;
            }
            let key = RecallKey::source(&fragment);
            let row = RecallRow {
                namespace_id: fragment.params().namespace_id.as_str().to_owned(),
                sensitivity: sensitivity_str(fragment.governance().sensitivity()).to_owned(),
                content: fragment.params().content.as_str().to_owned(),
            };
            insert_unique(&mut self.recall_rows, key.clone(), row)?;
            insert_unique(
                &mut self.candidates,
                key,
                Candidate::source(fragment, &source),
            )?;
        }
        Ok(())
    }

    fn load_memories(&mut self, connection: &Connection) -> Result<(), SqliteError> {
        let mut statement = connection
            .prepare(
                "SELECT namespace_id, memory_id
                 FROM radishmemory_memory_records
                 ORDER BY namespace_id, memory_id",
            )
            .map_err(SqliteError::storage)?;
        let stored = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(SqliteError::storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(SqliteError::storage)?;
        drop(statement);

        for (namespace, memory_id) in stored {
            let namespace = identifier(namespace)?;
            let memory_id = identifier(memory_id)?;
            let (record, _) = load_memory_record_closure(connection, &namespace, &memory_id)?
                .ok_or_else(derived_mismatch)?;
            let value = record.params();
            insert_unique(
                &mut self.projections,
                value.memory_id.as_str().to_owned(),
                ProjectionRow {
                    namespace_id: value.namespace_id.as_str().to_owned(),
                    current_state: memory_state_str(value.current_state).to_owned(),
                    last_state_event_id: value.last_state_event_id.as_str().to_owned(),
                },
            )?;
            if value.current_state != MemoryState::Confirmed
                || record.governance().deletion_state() != DeletionState::Active
            {
                continue;
            }
            let key = RecallKey::memory(&record);
            let row = RecallRow {
                namespace_id: value.namespace_id.as_str().to_owned(),
                sensitivity: sensitivity_str(record.governance().sensitivity()).to_owned(),
                content: value.content.text().as_str().to_owned(),
            };
            insert_unique(&mut self.recall_rows, key.clone(), row)?;
            insert_unique(&mut self.candidates, key, Candidate::memory(record))?;
        }
        Ok(())
    }
}

pub(crate) fn rebuild(connection: &Connection) -> Result<(), SqliteError> {
    let expected = Catalog::load(connection)?;
    connection
        .execute("DELETE FROM radishmemory_recall_fts", [])
        .map_err(SqliteError::storage)?;
    connection
        .execute("DELETE FROM radishmemory_memory_current_projection", [])
        .map_err(SqliteError::storage)?;
    for (memory_id, row) in &expected.projections {
        connection
            .execute(
                "INSERT INTO radishmemory_memory_current_projection (
                     memory_id, namespace_id, current_state, last_state_event_id
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    memory_id,
                    row.namespace_id,
                    row.current_state,
                    row.last_state_event_id,
                ],
            )
            .map_err(SqliteError::storage)?;
    }
    for (key, row) in &expected.recall_rows {
        insert_recall_row(connection, key, row)?;
    }
    verify_catalog(connection, &expected)
}

pub(crate) fn verify(connection: &Connection) -> Result<(), SqliteError> {
    let expected = Catalog::load(connection)?;
    verify_catalog(connection, &expected)
}

pub(crate) fn search(
    connection: &Connection,
    request: &LocalSearchRequest,
) -> Result<Vec<LocalSearchHit>, SqliteError> {
    let catalog = Catalog::load(connection)?;
    verify_catalog(connection, &catalog)?;
    let eligible = catalog
        .candidates
        .iter()
        .filter(|(_, candidate)| candidate.is_eligible(request))
        .map(|(key, _)| key.clone())
        .collect::<BTreeSet<_>>();
    if eligible.is_empty() {
        return Ok(Vec::new());
    }
    populate_search_eligibility(connection, &eligible)?;

    let expression = fts_expression(request.query().as_str());
    let query_result = (|| {
        let mut statement = connection
            .prepare(
                "SELECT recall.object_kind, recall.object_id
                 FROM radishmemory_recall_fts AS recall
                 INNER JOIN temp.radishmemory_search_eligible AS eligible
                     ON eligible.object_kind = recall.object_kind
                    AND eligible.object_id = recall.object_id
                 WHERE radishmemory_recall_fts MATCH ?1
                 ORDER BY bm25(radishmemory_recall_fts), recall.object_kind, recall.object_id",
            )
            .map_err(SqliteError::search)?;
        statement
            .query_map(params![expression], |row| {
                Ok(RecallKey {
                    object_kind: row.get(0)?,
                    object_id: row.get(1)?,
                })
            })
            .map_err(SqliteError::search)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(SqliteError::search)
    })();
    let cleanup_result = connection
        .execute("DELETE FROM temp.radishmemory_search_eligible", [])
        .map_err(SqliteError::search);
    let keys = query_result?;
    cleanup_result?;

    let mut hits = Vec::with_capacity(request.top_k().min(eligible.len()));
    for key in keys {
        let candidate = catalog.candidates.get(&key).ok_or_else(derived_mismatch)?;
        hits.push(candidate.hit.clone());
        if hits.len() == request.top_k() {
            break;
        }
    }
    Ok(hits)
}

fn populate_search_eligibility(
    connection: &Connection,
    eligible: &BTreeSet<RecallKey>,
) -> Result<(), SqliteError> {
    connection
        .execute_batch(
            "CREATE TEMP TABLE IF NOT EXISTS radishmemory_search_eligible (
                 object_kind TEXT NOT NULL,
                 object_id TEXT NOT NULL,
                 PRIMARY KEY (object_kind, object_id)
             ) WITHOUT ROWID;
             DELETE FROM temp.radishmemory_search_eligible;",
        )
        .map_err(SqliteError::search)?;
    for key in eligible {
        let changed = connection
            .execute(
                "INSERT INTO temp.radishmemory_search_eligible (object_kind, object_id)
                 VALUES (?1, ?2)",
                params![key.object_kind, key.object_id],
            )
            .map_err(SqliteError::search)?;
        if changed != 1 {
            return Err(derived_mismatch());
        }
    }
    Ok(())
}

pub(crate) fn insert_source_fragment(
    connection: &Connection,
    fragment: &SourceFragment,
    source: &SourceArtifact,
) -> Result<(), SqliteError> {
    if fragment.governance().deletion_state() != DeletionState::Active
        || source.governance().deletion_state() != DeletionState::Active
    {
        return Ok(());
    }
    let key = RecallKey::source(fragment);
    require_recall_row_absent(connection, &key)?;
    insert_recall_row(
        connection,
        &key,
        &RecallRow {
            namespace_id: fragment.params().namespace_id.as_str().to_owned(),
            sensitivity: sensitivity_str(fragment.governance().sensitivity()).to_owned(),
            content: fragment.params().content.as_str().to_owned(),
        },
    )
}

pub(crate) fn insert_memory_record(
    connection: &Connection,
    record: &MemoryRecord,
) -> Result<(), SqliteError> {
    let value = record.params();
    let changed = connection
        .execute(
            "INSERT INTO radishmemory_memory_current_projection (
                 memory_id, namespace_id, current_state, last_state_event_id
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                value.memory_id.as_str(),
                value.namespace_id.as_str(),
                memory_state_str(value.current_state),
                value.last_state_event_id.as_str(),
            ],
        )
        .map_err(SqliteError::storage)?;
    if changed != 1 {
        return Err(derived_mismatch());
    }
    if value.current_state == MemoryState::Confirmed
        && record.governance().deletion_state() == DeletionState::Active
    {
        let key = RecallKey::memory(record);
        require_recall_row_absent(connection, &key)?;
        insert_recall_row(
            connection,
            &key,
            &RecallRow {
                namespace_id: value.namespace_id.as_str().to_owned(),
                sensitivity: sensitivity_str(record.governance().sensitivity()).to_owned(),
                content: value.content.text().as_str().to_owned(),
            },
        )?;
    }
    Ok(())
}

pub(crate) fn update_memory_record(
    connection: &Connection,
    record: &MemoryRecord,
) -> Result<(), SqliteError> {
    let value = record.params();
    let changed = connection
        .execute(
            "UPDATE radishmemory_memory_current_projection
             SET current_state = ?1, last_state_event_id = ?2
             WHERE memory_id = ?3 AND namespace_id = ?4 AND current_state = 'confirmed'",
            params![
                memory_state_str(value.current_state),
                value.last_state_event_id.as_str(),
                value.memory_id.as_str(),
                value.namespace_id.as_str(),
            ],
        )
        .map_err(SqliteError::storage)?;
    if changed != 1 {
        return Err(derived_mismatch());
    }
    let key = RecallKey::memory(record);
    let removed = connection
        .execute(
            "DELETE FROM radishmemory_recall_fts
             WHERE object_kind = ?1 AND object_id = ?2",
            params![key.object_kind, key.object_id],
        )
        .map_err(SqliteError::storage)?;
    if removed != 1 {
        return Err(derived_mismatch());
    }
    Ok(())
}

fn verify_catalog(connection: &Connection, expected: &Catalog) -> Result<(), SqliteError> {
    let actual_projections = load_actual_projections(connection)?;
    let actual_recall_rows = load_actual_recall_rows(connection)?;
    if actual_projections != expected.projections || actual_recall_rows != expected.recall_rows {
        return Err(derived_mismatch());
    }
    Ok(())
}

fn load_actual_projections(
    connection: &Connection,
) -> Result<BTreeMap<String, ProjectionRow>, SqliteError> {
    let mut statement = connection
        .prepare(
            "SELECT memory_id, namespace_id, current_state, last_state_event_id
             FROM radishmemory_memory_current_projection
             ORDER BY memory_id",
        )
        .map_err(SqliteError::storage)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                ProjectionRow {
                    namespace_id: row.get(1)?,
                    current_state: row.get(2)?,
                    last_state_event_id: row.get(3)?,
                },
            ))
        })
        .map_err(SqliteError::storage)?;
    collect_unique(rows)
}

fn load_actual_recall_rows(
    connection: &Connection,
) -> Result<BTreeMap<RecallKey, RecallRow>, SqliteError> {
    let mut statement = connection
        .prepare(
            "SELECT object_kind, object_id, namespace_id, sensitivity, content
             FROM radishmemory_recall_fts
             ORDER BY object_kind, object_id",
        )
        .map_err(SqliteError::storage)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                RecallKey {
                    object_kind: row.get(0)?,
                    object_id: row.get(1)?,
                },
                RecallRow {
                    namespace_id: row.get(2)?,
                    sensitivity: row.get(3)?,
                    content: row.get(4)?,
                },
            ))
        })
        .map_err(SqliteError::storage)?;
    collect_unique(rows)
}

fn collect_unique<K, V>(
    rows: impl Iterator<Item = rusqlite::Result<(K, V)>>,
) -> Result<BTreeMap<K, V>, SqliteError>
where
    K: Ord,
{
    let mut values = BTreeMap::new();
    for row in rows {
        let (key, value) = row.map_err(SqliteError::storage)?;
        insert_unique(&mut values, key, value)?;
    }
    Ok(values)
}

fn insert_unique<K, V>(values: &mut BTreeMap<K, V>, key: K, value: V) -> Result<(), SqliteError>
where
    K: Ord,
{
    if values.insert(key, value).is_some() {
        return Err(derived_mismatch());
    }
    Ok(())
}

fn insert_recall_row(
    connection: &Connection,
    key: &RecallKey,
    row: &RecallRow,
) -> Result<(), SqliteError> {
    let changed = connection
        .execute(
            "INSERT INTO radishmemory_recall_fts (
                 object_kind, object_id, namespace_id, sensitivity, content
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                key.object_kind,
                key.object_id,
                row.namespace_id,
                row.sensitivity,
                row.content,
            ],
        )
        .map_err(SqliteError::storage)?;
    if changed != 1 {
        return Err(derived_mismatch());
    }
    Ok(())
}

fn require_recall_row_absent(connection: &Connection, key: &RecallKey) -> Result<(), SqliteError> {
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM radishmemory_recall_fts
             WHERE object_kind = ?1 AND object_id = ?2",
            params![key.object_kind, key.object_id],
            |row| row.get(0),
        )
        .map_err(SqliteError::storage)?;
    if count != 0 {
        return Err(derived_mismatch());
    }
    Ok(())
}

fn fts_expression(query: &str) -> String {
    query
        .split_whitespace()
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
}

fn memory_state_str(value: MemoryState) -> &'static str {
    match value {
        MemoryState::Confirmed => "confirmed",
        MemoryState::Superseded => "superseded",
        MemoryState::Contradicted => "contradicted",
        MemoryState::Retracted => "retracted",
        MemoryState::Expired => "expired",
    }
}

fn derived_mismatch() -> SqliteError {
    SqliteError::invalid_stored(SqliteStorageReason::DerivedDataMismatch)
}
