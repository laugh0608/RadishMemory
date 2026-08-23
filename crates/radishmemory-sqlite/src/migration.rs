use radishmemory_core::M0_SCHEMA_VERSION;
use rusqlite::{Connection, TransactionBehavior, params};

use crate::SqliteError;

/// Newest on-disk schema version understood by this adapter.
pub const SQLITE_SCHEMA_VERSION: u32 = 1;

struct Migration {
    version: u32,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: [Migration; 1] = [Migration {
    version: 1,
    name: "0001_sqlite_entry",
    sql: include_str!("../migrations/0001_sqlite_entry.sql"),
}];

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

    if found == i64::from(SQLITE_SCHEMA_VERSION) {
        validate_migration_history(connection)?;
    }

    Ok(found)
}

pub(crate) fn migrate_from(connection: &mut Connection, found: i64) -> Result<(), SqliteError> {
    if found < i64::from(SQLITE_SCHEMA_VERSION) {
        apply_pending(connection, found)?;
    }

    validate_migration_history(connection)
}

#[cfg(test)]
fn migrate(connection: &mut Connection) -> Result<(), SqliteError> {
    let found = preflight(connection)?;
    migrate_from(connection, found)
}

fn apply_pending(connection: &mut Connection, found: i64) -> Result<(), SqliteError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(SqliteError::migration)?;

    for migration in MIGRATIONS
        .iter()
        .filter(|migration| i64::from(migration.version) > found)
    {
        transaction
            .execute_batch(migration.sql)
            .map_err(SqliteError::migration)?;
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

    transaction.commit().map_err(SqliteError::migration)
}

fn validate_migration_history(connection: &Connection) -> Result<(), SqliteError> {
    let current =
        user_version(connection).map_err(|source| SqliteError::schema_drift(Some(source)))?;
    if current != i64::from(SQLITE_SCHEMA_VERSION) {
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

        let applied: (u32, String, String) = connection
            .query_row(
                "SELECT version, migration_name, canonical_schema_version
                 FROM radishmemory_schema_migrations",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("migration metadata must exist");
        assert_eq!(
            applied,
            (
                SQLITE_SCHEMA_VERSION,
                "0001_sqlite_entry".to_owned(),
                M0_SCHEMA_VERSION.to_owned(),
            )
        );
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
        assert_eq!(count, 1);
    }
}
