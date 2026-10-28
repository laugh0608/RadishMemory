//! Adapter-owned, rebuildable recall format, independent of body schema versions.
use crate::{SqliteError, SqliteStorageReason};
use rusqlite::{Connection, OptionalExtension};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecallProfile {
    LegacyUnicode61,
    HanPhraseV1,
}

pub const HAN_PROFILE_ID: &str = "radishmemory.recall.han-basic-a/1";
pub(crate) const PROFILE_TABLE: &str = "radishmemory_recall_profile";
const PROFILE_SQL: &str = "CREATE TABLE radishmemory_recall_profile (
    singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1),
    profile TEXT NOT NULL CHECK (profile = 'radishmemory.recall.han-basic-a/1')
) STRICT";
const HAN_FTS_SQL: &str = "CREATE VIRTUAL TABLE radishmemory_recall_fts USING fts5(
    object_kind UNINDEXED,
    object_id UNINDEXED,
    namespace_id UNINDEXED,
    sensitivity UNINDEXED,
    content UNINDEXED,
    han_terms,
    word_terms,
    tokenize = 'unicode61'
)";

fn legacy_fts_sql() -> &'static str {
    let sql = include_str!("../migrations/0004_local_recall.sql");
    &sql[sql
        .find("CREATE VIRTUAL TABLE")
        .expect("embedded legacy FTS definition")..]
}

fn definition(connection: &Connection, name: &str) -> Result<Option<String>, SqliteError> {
    connection
        .query_row(
            "SELECT sql FROM main.sqlite_schema WHERE type='table' AND name=?1",
            [name],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| SqliteError::schema_drift(Some(error)))
}

fn same_sql(actual: &str, expected: &str) -> bool {
    actual.trim().trim_end_matches(';') == expected.trim().trim_end_matches(';')
}

/// Metadata only, for the complete schema verifier (also handles pre-FTS schemas).
pub(crate) fn metadata(connection: &Connection) -> Result<RecallProfile, SqliteError> {
    let Some(sql) = definition(connection, PROFILE_TABLE)? else {
        return Ok(RecallProfile::LegacyUnicode61);
    };
    if !same_sql(&sql, PROFILE_SQL) {
        return Err(SqliteError::schema_drift(None));
    }
    let rows = connection
        .prepare("SELECT singleton, profile FROM radishmemory_recall_profile")
        .map_err(|error| SqliteError::schema_drift(Some(error)))?
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| SqliteError::schema_drift(Some(error)))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| SqliteError::schema_drift(Some(error)))?;
    if rows != [(1, HAN_PROFILE_ID.to_owned())] {
        return Err(SqliteError::schema_drift(None));
    }
    Ok(RecallProfile::HanPhraseV1)
}

/// Absence is legacy only when the FTS definition is actually the legacy one.
pub(crate) fn read(connection: &Connection) -> Result<RecallProfile, SqliteError> {
    let profile = metadata(connection)?;
    let sql = definition(connection, "radishmemory_recall_fts")?
        .ok_or_else(|| SqliteError::schema_drift(None))?;
    let expected = match profile {
        RecallProfile::LegacyUnicode61 => legacy_fts_sql(),
        RecallProfile::HanPhraseV1 => HAN_FTS_SQL,
    };
    if !same_sql(&sql, expected) {
        return Err(SqliteError::schema_drift(None));
    }
    Ok(profile)
}

/// Schema-only operation. Call only within a caller-owned transaction, or on
/// the isolated expected-schema connection. Facts are rebuilt by derived_index.
pub(crate) fn replace_schema(
    connection: &Connection,
    target: RecallProfile,
) -> Result<(), SqliteError> {
    connection
        .execute_batch("DROP TABLE radishmemory_recall_fts")
        .map_err(SqliteError::migration)?;
    match target {
        RecallProfile::HanPhraseV1 => {
            connection
                .execute_batch(HAN_FTS_SQL)
                .map_err(SqliteError::migration)?;
            connection
                .execute_batch(PROFILE_SQL)
                .map_err(SqliteError::migration)?;
            connection
                .execute(
                    "INSERT INTO radishmemory_recall_profile VALUES (1, ?1)",
                    [HAN_PROFILE_ID],
                )
                .map_err(SqliteError::migration)?;
        }
        RecallProfile::LegacyUnicode61 => {
            connection
                .execute_batch(legacy_fts_sql())
                .map_err(SqliteError::migration)?;
            connection
                .execute_batch("DROP TABLE radishmemory_recall_profile")
                .map_err(SqliteError::migration)?;
        }
    }
    Ok(())
}

pub(crate) fn verify_index(connection: &Connection) -> Result<(), SqliteError> {
    connection.execute(
        "INSERT INTO radishmemory_recall_fts(radishmemory_recall_fts) VALUES('integrity-check')", [],
    ).map_err(SqliteError::storage)?;
    Ok(())
}

pub(crate) fn pending() -> SqliteError {
    SqliteError::invalid_stored(SqliteStorageReason::RecallTransitionPending)
}

/// Pending requests include execution completed before durable evidence. A
/// status cell is insufficient: reload and validate the canonical receipt.
pub(crate) fn require_completed_deletions(connection: &Connection) -> Result<(), SqliteError> {
    let pending_state: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM radishmemory_source_artifacts WHERE deletion_state IN ('pending','failed'))
         OR EXISTS(SELECT 1 FROM radishmemory_memory_records WHERE deletion_state IN ('pending','failed'))",
        [], |row| row.get(0),
    ).map_err(SqliteError::storage)?;
    if pending_state {
        return Err(pending());
    }
    let requests = connection.prepare(
        "SELECT namespace_id, delete_request_id FROM radishmemory_delete_requests ORDER BY namespace_id, delete_request_id",
    ).map_err(SqliteError::storage)?
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(SqliteError::storage)?.collect::<Result<Vec<_>, _>>().map_err(SqliteError::storage)?;
    for (namespace, request) in requests {
        let namespace = crate::source_store::identifier(namespace)?;
        let request = crate::source_store::identifier(request)?;
        crate::deletion_store::load_request(connection, &namespace, &request)?
            .ok_or_else(pending)?;
        let evidence: Option<String> = connection.query_row(
            "SELECT deletion_evidence_id FROM radishmemory_deletion_evidence WHERE namespace_id=?1 AND delete_request_id=?2 ORDER BY execution_ordinal DESC LIMIT 1",
            rusqlite::params![namespace.as_str(), request.as_str()], |row| row.get(0),
        ).optional().map_err(SqliteError::storage)?;
        let evidence = evidence.ok_or_else(pending)?;
        let evidence = crate::deletion_store::load_evidence(
            connection,
            &namespace,
            &crate::source_store::identifier(evidence)?,
        )?
        .ok_or_else(pending)?;
        if evidence.params().overall_status != radishmemory_core::DeletionOverallStatus::Completed {
            return Err(pending());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SqliteDatabase, SqliteErrorCode};

    fn database() -> SqliteDatabase {
        SqliteDatabase::initialize(Connection::open_in_memory().unwrap()).unwrap()
    }

    fn schema(connection: &Connection) -> Vec<(String, String)> {
        connection
            .prepare("SELECT name, sql FROM sqlite_schema WHERE sql IS NOT NULL ORDER BY name")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn real_commit_failure_rolls_back_both_recall_schema_directions() {
        for target in [RecallProfile::LegacyUnicode61, RecallProfile::HanPhraseV1] {
            let mut db = database();
            if target == RecallProfile::HanPhraseV1 {
                db.change_recall_profile(RecallProfile::LegacyUnicode61)
                    .unwrap();
            }
            let before = schema(&db.connection);
            let original = db.recall_profile().unwrap();
            let error = db.change_recall_profile_with_check(target, |connection| {
                connection.execute_batch("PRAGMA defer_foreign_keys=ON;
                    INSERT INTO radishmemory_fragment_heading_path VALUES('missing-synthetic-fragment',0,'Synthetic heading')")
                    .map_err(SqliteError::storage)
            }).unwrap_err();
            assert_eq!(
                error.sqlite_extended_code(),
                Some(rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY)
            );
            assert_eq!(db.recall_profile().unwrap(), original);
            assert_eq!(schema(&db.connection), before);
            db.verify_recall_derivations().unwrap();
            assert_eq!(db.change_recall_profile(target).unwrap(), target);
            assert_eq!(db.change_recall_profile(target).unwrap(), target);
        }
    }

    #[test]
    fn unknown_partial_or_extra_profile_structures_never_fall_back() {
        for sql in [
            "DROP TABLE radishmemory_recall_profile",
            "DELETE FROM radishmemory_recall_profile",
            "PRAGMA ignore_check_constraints=ON; UPDATE radishmemory_recall_profile SET profile='unrecognized';",
            "PRAGMA ignore_check_constraints=ON; INSERT INTO radishmemory_recall_profile VALUES(2,'radishmemory.recall.han-basic-a/1');",
            "CREATE VIEW unexpected_recall_view AS SELECT 1",
            "CREATE TRIGGER unexpected_recall_trigger AFTER INSERT ON radishmemory_recall_profile BEGIN SELECT 1; END",
        ] {
            let mut db = database();
            db.connection.execute_batch(sql).unwrap();
            assert_eq!(
                crate::migration::preflight(&db.connection)
                    .unwrap_err()
                    .code(),
                SqliteErrorCode::SchemaDrift
            );
            assert_eq!(
                db.change_recall_profile(RecallProfile::LegacyUnicode61)
                    .unwrap_err()
                    .code(),
                SqliteErrorCode::SchemaDrift
            );
        }
    }

    #[test]
    fn a_new_profile_requires_exact_derived_fields_not_only_original_content() {
        let mut db = database();
        // Unknown row with a valid-looking codec is also rejected by canonical comparison.
        db.connection.execute(
            "INSERT INTO radishmemory_recall_fts VALUES('source_fragment','missing','synthetic','personal','蓝色','h84dd h8272',' 界 ')", [],
        ).unwrap();
        assert_eq!(
            db.verify_recall_derivations().unwrap_err().storage_reason(),
            Some(SqliteStorageReason::DerivedDataMismatch)
        );
        assert!(
            db.change_recall_profile(RecallProfile::LegacyUnicode61)
                .is_err()
        );
        db.rebuild_recall_derivations().unwrap();
        assert_eq!(db.recall_profile().unwrap(), RecallProfile::HanPhraseV1);
    }
}
