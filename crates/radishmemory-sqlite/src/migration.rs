use std::collections::BTreeSet;

use radishmemory_core::M0_SCHEMA_VERSION;
use rusqlite::{Connection, Transaction, TransactionBehavior, params};

use crate::SqliteError;

/// Newest on-disk schema version accepted for ordinary library operations.
pub const SQLITE_SCHEMA_VERSION: u32 = 6;
pub(crate) const KEY_INITIALIZATION_SCHEMA_VERSION: u32 = 7;

struct Migration {
    version: u32,
    name: &'static str,
    sql: &'static str,
    tables_created: &'static [&'static str],
}

pub(crate) const OBJECT_MIGRATION_SCHEMA_VERSION: u32 = 8;

pub(crate) const CAPTURE_SCHEMA_VERSION: u32 = 9;

pub(crate) const ABANDONMENT_SCHEMA_VERSION: u32 = 10;

pub(crate) const OBJECT_DELETION_SCHEMA_VERSION: u32 = 11;

pub(crate) const LEGACY_DELETION_SCHEMA_VERSION: u32 = 12;

const MIGRATIONS: [Migration; 12] = [
    Migration {
        version: 1,
        name: "0001_sqlite_entry",
        sql: include_str!("../migrations/0001_sqlite_entry.sql"),
        tables_created: &["radishmemory_schema_migrations"],
    },
    Migration {
        version: 2,
        name: "0002_source_storage",
        sql: include_str!("../migrations/0002_source_storage.sql"),
        tables_created: &[
            "radishmemory_fragment_heading_path",
            "radishmemory_source_artifacts",
            "radishmemory_source_bodies",
            "radishmemory_source_fragments",
            "radishmemory_source_supersedes",
        ],
    },
    Migration {
        version: 3,
        name: "0003_memory_storage",
        sql: include_str!("../migrations/0003_memory_storage.sql"),
        tables_created: &[
            "radishmemory_event_related_memories",
            "radishmemory_memory_decisions",
            "radishmemory_memory_proposals",
            "radishmemory_memory_records",
            "radishmemory_memory_state_events",
            "radishmemory_proposal_source_fragments",
            "radishmemory_proposal_targets",
            "radishmemory_record_contradicts",
            "radishmemory_record_source_fragments",
            "radishmemory_record_supersedes",
        ],
    },
    Migration {
        version: 4,
        name: "0004_local_recall",
        sql: include_str!("../migrations/0004_local_recall.sql"),
        tables_created: &[
            "radishmemory_memory_current_projection",
            "radishmemory_recall_fts",
            "radishmemory_recall_fts_config",
            "radishmemory_recall_fts_content",
            "radishmemory_recall_fts_data",
            "radishmemory_recall_fts_docsize",
            "radishmemory_recall_fts_idx",
        ],
    },
    Migration {
        version: 5,
        name: "0005_local_deletion",
        sql: include_str!("../migrations/0005_local_deletion.sql"),
        tables_created: &[
            "radishmemory_delete_component_targets",
            "radishmemory_delete_execution_closure",
            "radishmemory_delete_request_components",
            "radishmemory_delete_request_targets",
            "radishmemory_delete_requests",
            "radishmemory_deletion_evidence",
            "radishmemory_deletion_execution_attempts",
            "radishmemory_deletion_execution_results",
        ],
    },
    Migration {
        version: 6,
        name: "0006_source_capture",
        sql: include_str!("../migrations/0006_source_capture.sql"),
        tables_created: &[
            "radishmemory_source_capture_audit",
            "radishmemory_source_lineage_tips",
            "radishmemory_source_origin_bindings",
        ],
    },
    Migration {
        version: 7,
        name: "0007_source_vault_key",
        sql: include_str!("../migrations/0007_source_vault_key.sql"),
        tables_created: &["radishmemory_source_vault_key_profile"],
    },
    Migration {
        version: 8,
        name: "0008_source_vault_objects",
        sql: include_str!("../migrations/0008_source_vault_objects.sql"),
        tables_created: &[
            "radishmemory_source_vault_migration",
            "radishmemory_source_vault_attempts",
            "radishmemory_source_vault_references",
        ],
    },
    Migration {
        version: 9,
        name: "0009_source_vault_capture",
        sql: include_str!("../migrations/0009_source_vault_capture.sql"),
        tables_created: &[],
    },
    Migration {
        version: 10,
        name: "0010_capture_abandonment",
        sql: include_str!("../migrations/0010_capture_abandonment.sql"),
        tables_created: &[],
    },
    Migration {
        version: 11,
        name: "0011_source_vault_deletion",
        sql: include_str!("../migrations/0011_source_vault_deletion.sql"),
        tables_created: &[
            "radishmemory_source_vault_delete_plans",
            "radishmemory_source_vault_deletions",
        ],
    },
    Migration {
        version: 12,
        name: "0012_legacy_body_retirement",
        sql: include_str!("../migrations/0012_legacy_body_retirement.sql"),
        tables_created: &["radishmemory_legacy_body_retirements"],
    },
];

const EVIDENCE_V2: Migration = Migration {
    version: 13,
    name: "0013_deletion_evidence_v2",
    sql: include_str!("../migrations/0013_deletion_evidence_v2.sql"),
    tables_created: &[],
};

pub(crate) fn evidence_v2_enabled(connection: &Connection) -> Result<bool, SqliteError> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM radishmemory_schema_migrations
             WHERE version=?1 AND migration_name=?2 AND canonical_schema_version=?3)",
            params![EVIDENCE_V2.version, EVIDENCE_V2.name, M0_SCHEMA_VERSION],
            |row| row.get(0),
        )
        .map_err(SqliteError::migration)
}

pub(crate) fn require_evidence_v2(connection: &Connection) -> Result<(), SqliteError> {
    if !evidence_v2_enabled(connection)? {
        return Err(SqliteError::deletion_invariant(
            crate::SqliteStorageReason::EvidenceUpgradeRequired,
        ));
    }
    Ok(())
}

pub(crate) fn upgrade_evidence(
    transaction: &Transaction<'_>,
    version: u32,
) -> Result<(), SqliteError> {
    validate_migration_history(transaction, version)?;
    crate::deletion_store::verify_all_evidence(transaction)?;
    if !evidence_v2_enabled(transaction)? {
        rebuild_evidence_table(transaction)?;
        record_migration(transaction, &EVIDENCE_V2)?;
        validate_migration_history(transaction, version)?;
    }
    Ok(())
}

fn rebuild_evidence_table(connection: &Connection) -> Result<(), SqliteError> {
    connection
        .execute_batch(EVIDENCE_V2.sql)
        .map_err(SqliteError::migration)?;
    connection.execute_batch(
        "INSERT INTO radishmemory_deletion_evidence_v2 SELECT * FROM radishmemory_deletion_evidence;",
    ).map_err(SqliteError::migration)?;
    let matches: bool = connection
        .query_row(
            "SELECT NOT EXISTS(SELECT * FROM radishmemory_deletion_evidence
                           EXCEPT SELECT * FROM radishmemory_deletion_evidence_v2)
            AND (SELECT COUNT(*) FROM radishmemory_deletion_evidence) =
                (SELECT COUNT(*) FROM radishmemory_deletion_evidence_v2)",
            [],
            |row| row.get(0),
        )
        .map_err(SqliteError::migration)?;
    if !matches {
        return Err(SqliteError::schema_drift(None));
    }
    let mut statement = connection
        .prepare(
            "SELECT deletion_evidence_id FROM radishmemory_deletion_evidence
         ORDER BY execution_ordinal DESC",
        )
        .map_err(SqliteError::migration)?;
    let receipts = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(SqliteError::migration)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(SqliteError::migration)?;
    drop(statement);
    // The original table's RESTRICT self-reference requires child-first removal.
    for id in receipts {
        connection
            .execute(
                "DELETE FROM radishmemory_deletion_evidence WHERE deletion_evidence_id=?1",
                [id],
            )
            .map_err(SqliteError::migration)?;
    }
    connection
        .execute_batch(
            "DROP TABLE radishmemory_deletion_evidence;
         ALTER TABLE radishmemory_deletion_evidence_v2 RENAME TO radishmemory_deletion_evidence;",
        )
        .map_err(SqliteError::migration)?;
    let clean: bool = connection
        .query_row(
            "SELECT NOT EXISTS(SELECT 1 FROM pragma_foreign_key_check)",
            [],
            |row| row.get(0),
        )
        .map_err(SqliteError::migration)?;
    if !clean {
        return Err(SqliteError::schema_drift(None));
    }
    Ok(())
}

pub(crate) fn preflight(connection: &Connection) -> Result<i64, SqliteError> {
    preflight_to(connection, SQLITE_SCHEMA_VERSION)
}

pub(crate) fn preflight_to(connection: &Connection, supported: u32) -> Result<i64, SqliteError> {
    let found = user_version(connection).map_err(SqliteError::migration)?;
    if found < 0 || found > i64::from(supported) {
        return Err(SqliteError::unsupported_schema_version(found, supported));
    }

    if found == 0
        && !main_schema_is_empty(connection)
            .map_err(|source| SqliteError::schema_drift(Some(source)))?
    {
        return Err(SqliteError::schema_drift(None));
    }

    if found > 0 {
        validate_migration_history(
            connection,
            u32::try_from(found).map_err(|_| {
                SqliteError::unsupported_schema_version(found, SQLITE_SCHEMA_VERSION)
            })?,
        )?;
    }

    Ok(found)
}

pub(crate) fn migrate_from(connection: &mut Connection, found: i64) -> Result<(), SqliteError> {
    if found == i64::from(SQLITE_SCHEMA_VERSION) {
        return Ok(());
    }

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(SqliteError::migration)?;
    apply_pending(&transaction, found, SQLITE_SCHEMA_VERSION)?;
    if found == 0 {
        crate::recall_profile::replace_schema(&transaction, crate::RecallProfile::HanPhraseV1)?;
        crate::derived_index::rebuild(&transaction)?;
        upgrade_evidence(&transaction, SQLITE_SCHEMA_VERSION)?;
    }
    validate_migration_history(&transaction, SQLITE_SCHEMA_VERSION)?;
    transaction.commit().map_err(SqliteError::migration)
}

#[cfg(test)]
fn migrate(connection: &mut Connection) -> Result<(), SqliteError> {
    let found = preflight(connection)?;
    migrate_from(connection, found)
}

pub(crate) fn apply_pending(
    transaction: &Transaction<'_>,
    found: i64,
    target: u32,
) -> Result<(), SqliteError> {
    for migration in MIGRATIONS
        .iter()
        .filter(|migration| i64::from(migration.version) > found && migration.version <= target)
    {
        transaction
            .execute_batch(migration.sql)
            .map_err(SqliteError::migration)?;
        if migration.version == 6 {
            crate::derived_index::rebuild(transaction)?;
        }
        record_migration(transaction, migration)?;
        transaction
            .pragma_update(None, "user_version", migration.version)
            .map_err(SqliteError::migration)?;
    }

    Ok(())
}

fn record_migration(connection: &Connection, migration: &Migration) -> Result<(), SqliteError> {
    connection.execute(
        "INSERT INTO radishmemory_schema_migrations (version, migration_name, canonical_schema_version)
         VALUES (?1, ?2, ?3)",
        params![migration.version, migration.name, M0_SCHEMA_VERSION],
    ).map_err(SqliteError::migration)?;
    Ok(())
}

fn validate_migration_history(
    connection: &Connection,
    expected_version: u32,
) -> Result<(), SqliteError> {
    let current =
        user_version(connection).map_err(|source| SqliteError::schema_drift(Some(source)))?;
    if current != i64::from(expected_version) {
        return Err(SqliteError::schema_drift(None));
    }

    let mut statement = connection
        .prepare(
            "SELECT version, migration_name, canonical_schema_version
             FROM radishmemory_schema_migrations
             ORDER BY version",
        )
        .map_err(|source| SqliteError::schema_drift(Some(source)))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, u32>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|source| SqliteError::schema_drift(Some(source)))?;
    let actual = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| SqliteError::schema_drift(Some(source)))?;
    let has_evidence_v2 =
        expected_version >= SQLITE_SCHEMA_VERSION && evidence_v2_enabled(connection)?;
    let expected = MIGRATIONS
        .iter()
        .filter(|migration| migration.version <= expected_version)
        .chain(has_evidence_v2.then_some(&EVIDENCE_V2))
        .map(|migration| {
            (
                migration.version,
                migration.name.to_owned(),
                M0_SCHEMA_VERSION.to_owned(),
            )
        })
        .collect::<Vec<_>>();

    if actual != expected {
        return Err(SqliteError::schema_drift(None));
    }

    validate_schema_tables(connection, expected_version)?;
    verify_schema(connection, expected_version, has_evidence_v2)
}

fn validate_schema_tables(
    connection: &Connection,
    expected_version: u32,
) -> Result<(), SqliteError> {
    let mut statement = connection
        .prepare(
            "SELECT name FROM main.sqlite_schema
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )
        .map_err(|source| SqliteError::schema_drift(Some(source)))?;
    let actual = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|source| SqliteError::schema_drift(Some(source)))?
        .collect::<Result<BTreeSet<_>, _>>()
        .map_err(|source| SqliteError::schema_drift(Some(source)))?;
    let mut expected = MIGRATIONS
        .iter()
        .filter(|migration| migration.version <= expected_version)
        .flat_map(|migration| migration.tables_created.iter().copied())
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if crate::recall_profile::metadata(connection)? == crate::RecallProfile::HanPhraseV1 {
        if expected_version < 6 {
            return Err(SqliteError::schema_drift(None));
        }
        expected.insert(crate::recall_profile::PROFILE_TABLE.to_owned());
    }
    if actual != expected {
        return Err(SqliteError::schema_drift(None));
    }
    Ok(())
}

fn user_version(connection: &Connection) -> rusqlite::Result<i64> {
    connection.pragma_query_value(None, "user_version", |row| row.get(0))
}

// The bootstrap gate must reject hidden views/triggers and altered columns as
// well as unknown tables. Derive the expected schema from the same migrations.
pub(crate) fn verify_schema_definition(
    connection: &Connection,
    version: u32,
) -> Result<(), SqliteError> {
    verify_schema(connection, version, evidence_v2_enabled(connection)?)
}

fn verify_schema(
    connection: &Connection,
    version: u32,
    has_evidence_v2: bool,
) -> Result<(), SqliteError> {
    let expected = Connection::open_in_memory().map_err(SqliteError::migration)?;
    for migration in MIGRATIONS
        .iter()
        .filter(|migration| migration.version <= version)
    {
        expected
            .execute_batch(migration.sql)
            .map_err(SqliteError::migration)?;
    }
    if crate::recall_profile::metadata(connection)? == crate::RecallProfile::HanPhraseV1 {
        if version < 6 {
            return Err(SqliteError::schema_drift(None));
        }
        crate::recall_profile::replace_schema(&expected, crate::RecallProfile::HanPhraseV1)?;
    }
    if has_evidence_v2 {
        rebuild_evidence_table(&expected)?;
    }
    fn schema(
        connection: &Connection,
    ) -> Result<Vec<(String, String, Option<String>)>, SqliteError> {
        let mut statement = connection.prepare(
            "SELECT type, name, sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name"
        ).map_err(SqliteError::migration)?;
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .map_err(SqliteError::migration)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(SqliteError::migration)
    }
    if schema(connection)? != schema(&expected)? {
        return Err(SqliteError::schema_drift(None));
    }
    Ok(())
}

fn main_schema_is_empty(connection: &Connection) -> rusqlite::Result<bool> {
    connection.query_row(
        "SELECT NOT EXISTS (
             SELECT 1 FROM main.sqlite_schema WHERE name NOT LIKE 'sqlite_%'
         )",
        [],
        |row| row.get(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_records_exact_adapter_and_canonical_versions() {
        let mut connection = Connection::open_in_memory().expect("in-memory SQLite must open");
        migrate(&mut connection).expect("migration must succeed");

        let mut statement = connection
            .prepare(
                "SELECT version, migration_name, canonical_schema_version
                 FROM radishmemory_schema_migrations ORDER BY version",
            )
            .expect("migration metadata must be queryable");
        let applied = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .expect("migration rows must be queryable")
            .collect::<Result<Vec<_>, _>>()
            .expect("migration rows must decode");
        let expected = MIGRATIONS
            .iter()
            .filter(|migration| migration.version <= SQLITE_SCHEMA_VERSION)
            .chain([&EVIDENCE_V2])
            .map(|migration| {
                (
                    migration.version,
                    migration.name.to_owned(),
                    M0_SCHEMA_VERSION.to_owned(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(applied, expected);
    }

    #[test]
    fn migration_is_idempotent_on_current_schema() {
        let mut connection = Connection::open_in_memory().expect("in-memory SQLite must open");
        migrate(&mut connection).expect("first migration must succeed");
        migrate(&mut connection).expect("current schema must validate without mutation");

        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM radishmemory_schema_migrations",
                [],
                |row| row.get(0),
            )
            .expect("migration metadata must be queryable");
        assert_eq!(count, i64::from(SQLITE_SCHEMA_VERSION) + 1);
    }

    #[test]
    fn receipt_upgrade_rolls_back_and_commutes_with_each_body_schema() {
        let mut before = Connection::open_in_memory().unwrap();
        let mut after = Connection::open_in_memory().unwrap();
        for connection in [&mut before, &mut after] {
            crate::configure_connection(connection).unwrap();
            apply_reviewed_prefix(connection, SQLITE_SCHEMA_VERSION as usize);
        }
        let tx = before.transaction().unwrap();
        upgrade_evidence(&tx, SQLITE_SCHEMA_VERSION).unwrap();
        tx.rollback().unwrap();
        assert_eq!(
            preflight(&before).unwrap(),
            i64::from(SQLITE_SCHEMA_VERSION)
        );
        assert!(!evidence_v2_enabled(&before).unwrap());
        let tx = before.transaction().unwrap();
        upgrade_evidence(&tx, SQLITE_SCHEMA_VERSION).unwrap();
        tx.commit().unwrap();

        for version in KEY_INITIALIZATION_SCHEMA_VERSION..=LEGACY_DELETION_SCHEMA_VERSION {
            let tx = before.transaction().unwrap();
            apply_pending(&tx, i64::from(version - 1), version).unwrap();
            validate_migration_history(&tx, version).unwrap();
            assert!(evidence_v2_enabled(&tx).unwrap());
            tx.commit().unwrap();

            let tx = after.transaction().unwrap();
            apply_pending(&tx, i64::from(version - 1), version).unwrap();
            tx.commit().unwrap();
            let tx = after.transaction().unwrap();
            upgrade_evidence(&tx, version).unwrap();
            let ddl = |connection: &Connection| {
                connection
                    .query_row(
                        "SELECT sql FROM sqlite_schema WHERE name='radishmemory_deletion_evidence'",
                        [],
                        |row| row.get::<_, String>(0),
                    )
                    .unwrap()
            };
            assert_eq!(ddl(&before), ddl(&tx));
            tx.rollback().unwrap();
            validate_migration_history(&after, version).unwrap();
        }
        before
            .execute(
                "DELETE FROM radishmemory_schema_migrations WHERE version=13",
                [],
            )
            .unwrap();
        assert_eq!(
            preflight_to(&before, LEGACY_DELETION_SCHEMA_VERSION)
                .unwrap_err()
                .code(),
            crate::SqliteErrorCode::SchemaDrift
        );
        after
            .execute(
                "INSERT INTO radishmemory_schema_migrations VALUES(13,?1,?2)",
                params![EVIDENCE_V2.name, M0_SCHEMA_VERSION],
            )
            .unwrap();
        assert_eq!(
            preflight_to(&after, LEGACY_DELETION_SCHEMA_VERSION)
                .unwrap_err()
                .code(),
            crate::SqliteErrorCode::SchemaDrift
        );
    }

    #[test]
    fn version_one_upgrades_through_memory_storage_atomically() {
        let mut connection = Connection::open_in_memory().expect("in-memory SQLite must open");
        apply_reviewed_prefix(&connection, 1);

        migrate(&mut connection).expect("pending storage migrations must apply");

        let version = user_version(&connection).expect("schema version must be queryable");
        assert_eq!(version, i64::from(SQLITE_SCHEMA_VERSION));
        validate_migration_history(&connection, SQLITE_SCHEMA_VERSION)
            .expect("upgraded schema must be exact");
    }

    #[test]
    fn version_two_upgrades_to_memory_storage_atomically() {
        let mut connection = Connection::open_in_memory().expect("in-memory SQLite must open");
        apply_reviewed_prefix(&connection, 2);

        migrate(&mut connection).expect("memory storage migration must apply");

        let version = user_version(&connection).expect("schema version must be queryable");
        assert_eq!(version, i64::from(SQLITE_SCHEMA_VERSION));
        validate_migration_history(&connection, SQLITE_SCHEMA_VERSION)
            .expect("upgraded memory schema must be exact");
    }

    #[test]
    fn version_three_upgrades_to_local_recall_atomically() {
        let mut connection = Connection::open_in_memory().expect("in-memory SQLite must open");
        apply_reviewed_prefix(&connection, 3);

        migrate(&mut connection).expect("local recall migration must apply");

        let version = user_version(&connection).expect("schema version must be queryable");
        assert_eq!(version, i64::from(SQLITE_SCHEMA_VERSION));
        validate_migration_history(&connection, SQLITE_SCHEMA_VERSION)
            .expect("upgraded local recall schema must be exact");
    }

    #[test]
    fn version_four_upgrades_to_local_deletion_atomically() {
        let mut connection = Connection::open_in_memory().expect("in-memory SQLite must open");
        apply_reviewed_prefix(&connection, 4);

        migrate(&mut connection).expect("local deletion migration must apply");

        let version = user_version(&connection).expect("schema version must be queryable");
        assert_eq!(version, i64::from(SQLITE_SCHEMA_VERSION));
        validate_migration_history(&connection, SQLITE_SCHEMA_VERSION)
            .expect("upgraded local deletion schema must be exact");
    }

    #[test]
    fn version_five_upgrades_to_atomic_source_capture() {
        let mut connection = Connection::open_in_memory().expect("in-memory SQLite must open");
        apply_reviewed_prefix(&connection, 5);

        migrate(&mut connection).expect("source capture migration must apply");

        let version = user_version(&connection).expect("schema version must be queryable");
        assert_eq!(version, i64::from(SQLITE_SCHEMA_VERSION));
        validate_migration_history(&connection, SQLITE_SCHEMA_VERSION)
            .expect("upgraded source capture schema must be exact");
    }

    fn apply_reviewed_prefix(connection: &Connection, count: usize) {
        for migration in MIGRATIONS.iter().take(count) {
            connection
                .execute_batch(migration.sql)
                .expect("reviewed migration must apply");
            connection
                .execute(
                    "INSERT INTO radishmemory_schema_migrations (
                         version, migration_name, canonical_schema_version
                     ) VALUES (?1, ?2, ?3)",
                    params![migration.version, migration.name, M0_SCHEMA_VERSION],
                )
                .expect("reviewed migration history must be recorded");
            connection
                .pragma_update(None, "user_version", migration.version)
                .expect("reviewed schema version must be recorded");
        }
    }

    #[test]
    fn current_schema_rejects_untracked_tables() {
        let mut connection = Connection::open_in_memory().expect("in-memory SQLite must open");
        migrate(&mut connection).expect("migration must succeed");
        connection
            .execute_batch("CREATE TABLE synthetic_untracked (value TEXT) STRICT;")
            .expect("synthetic drift table must be created");

        let error = preflight(&connection).expect_err("untracked table must fail closed");
        assert_eq!(error.code(), crate::SqliteErrorCode::SchemaDrift);
    }
}
