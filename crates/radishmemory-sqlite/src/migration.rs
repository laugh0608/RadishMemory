use std::collections::BTreeSet;

use radishmemory_core::M0_SCHEMA_VERSION;
use rusqlite::{Connection, Transaction, TransactionBehavior, params};

use crate::SqliteError;

/// Newest on-disk schema version understood by this adapter.
pub const SQLITE_SCHEMA_VERSION: u32 = 5;

struct Migration {
    version: u32,
    name: &'static str,
    sql: &'static str,
    tables_created: &'static [&'static str],
}

const MIGRATIONS: [Migration; 5] = [
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
];

pub(crate) fn preflight(connection: &Connection) -> Result<i64, SqliteError> {
    let found = user_version(connection).map_err(SqliteError::migration)?;
    if found < 0 || found > i64::from(SQLITE_SCHEMA_VERSION) {
        return Err(SqliteError::unsupported_schema_version(
            found,
            SQLITE_SCHEMA_VERSION,
        ));
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
    apply_pending(&transaction, found)?;
    validate_migration_history(&transaction, SQLITE_SCHEMA_VERSION)?;
    transaction.commit().map_err(SqliteError::migration)
}

#[cfg(test)]
fn migrate(connection: &mut Connection) -> Result<(), SqliteError> {
    let found = preflight(connection)?;
    migrate_from(connection, found)
}

fn apply_pending(transaction: &Transaction<'_>, found: i64) -> Result<(), SqliteError> {
    for migration in MIGRATIONS
        .iter()
        .filter(|migration| i64::from(migration.version) > found)
    {
        transaction
            .execute_batch(migration.sql)
            .map_err(SqliteError::migration)?;
        if migration.version == 4 {
            crate::derived_index::rebuild(transaction)?;
        }
        let changed = transaction
            .execute(
                "INSERT INTO radishmemory_schema_migrations (
                     version, migration_name, canonical_schema_version
                 ) VALUES (?1, ?2, ?3)",
                params![migration.version, migration.name, M0_SCHEMA_VERSION],
            )
            .map_err(SqliteError::migration)?;
        if changed != 1 {
            return Err(SqliteError::schema_drift(None));
        }
        transaction
            .pragma_update(None, "user_version", migration.version)
            .map_err(SqliteError::migration)?;
    }

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
    let expected = MIGRATIONS
        .iter()
        .filter(|migration| migration.version <= expected_version)
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

    validate_schema_tables(connection, expected_version)
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
    let expected = MIGRATIONS
        .iter()
        .filter(|migration| migration.version <= expected_version)
        .flat_map(|migration| migration.tables_created.iter().copied())
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(SqliteError::schema_drift(None));
    }
    Ok(())
}

fn user_version(connection: &Connection) -> rusqlite::Result<i64> {
    connection.pragma_query_value(None, "user_version", |row| row.get(0))
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
        assert_eq!(count, i64::from(SQLITE_SCHEMA_VERSION));
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
