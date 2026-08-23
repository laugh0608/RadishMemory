//! SQLite adapter entry for RadishMemory.
//!
//! This crate owns connection policy, capability checks, and embedded schema
//! migrations. SQL handles and database row identifiers remain private to the
//! adapter boundary.

mod capability;
mod error;
mod migration;
mod source_store;

use std::fmt;
use std::path::Path;

use rusqlite::Connection;

pub use capability::{REVIEWED_BUNDLED_SQLITE_VERSION, SqliteCapabilities};
pub use error::{
    SqliteCapability, SqliteConfigurationReason, SqliteError, SqliteErrorCode, SqliteStorageReason,
};
pub use migration::SQLITE_SCHEMA_VERSION;

/// An initialized RadishMemory SQLite database.
pub struct SqliteDatabase {
    connection: Connection,
    capabilities: SqliteCapabilities,
}

impl SqliteDatabase {
    /// Opens or creates a database, verifies the runtime, and applies migrations.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqliteError> {
        let connection = Connection::open(path).map_err(SqliteError::open)?;
        Self::initialize(connection)
    }

    fn initialize(mut connection: Connection) -> Result<Self, SqliteError> {
        let schema_version = migration::preflight(&connection)?;
        configure_connection(&connection)?;
        let capabilities = capability::probe(&connection)?;
        migration::migrate_from(&mut connection, schema_version)?;

        Ok(Self {
            connection,
            capabilities,
        })
    }

    /// Returns the adapter schema version established by migration.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        SQLITE_SCHEMA_VERSION
    }

    /// Returns the runtime capabilities proven during initialization.
    #[must_use]
    pub const fn capabilities(&self) -> &SqliteCapabilities {
        &self.capabilities
    }

    #[cfg(test)]
    const fn connection(&self) -> &Connection {
        &self.connection
    }
}

impl fmt::Debug for SqliteDatabase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SqliteDatabase")
            .field("schema_version", &SQLITE_SCHEMA_VERSION)
            .field("capabilities", &self.capabilities)
            .field("is_autocommit", &self.connection.is_autocommit())
            .finish_non_exhaustive()
    }
}

fn configure_connection(connection: &Connection) -> Result<(), SqliteError> {
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(SqliteError::configuration_source)?;
    require_pragma_i64(
        connection,
        "foreign_keys",
        1,
        SqliteConfigurationReason::ForeignKeysDisabled,
    )?;

    connection
        .pragma_update(None, "trusted_schema", false)
        .map_err(SqliteError::configuration_source)?;
    require_pragma_i64(
        connection,
        "trusted_schema",
        0,
        SqliteConfigurationReason::TrustedSchemaEnabled,
    )?;

    connection
        .pragma_update(None, "synchronous", "FULL")
        .map_err(SqliteError::configuration_source)?;
    require_pragma_i64(
        connection,
        "synchronous",
        2,
        SqliteConfigurationReason::SynchronousNotFull,
    )?;

    let journal_mode: String = connection
        .pragma_update_and_check(None, "journal_mode", "DELETE", |row| row.get(0))
        .map_err(SqliteError::configuration_source)?;
    if !matches!(journal_mode.as_str(), "delete" | "memory") {
        return Err(SqliteError::configuration(
            SqliteConfigurationReason::PersistentJournalMode,
        ));
    }

    Ok(())
}

fn require_pragma_i64(
    connection: &Connection,
    name: &str,
    expected: i64,
    reason: SqliteConfigurationReason,
) -> Result<(), SqliteError> {
    let actual: i64 = connection
        .pragma_query_value(None, name, |row| row.get(0))
        .map_err(SqliteError::configuration_source)?;
    if actual != expected {
        return Err(SqliteError::configuration(reason));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_policy_is_applied_and_verified() {
        let connection = Connection::open_in_memory().expect("in-memory SQLite must open");
        let database = SqliteDatabase::initialize(connection).expect("initialization must succeed");

        for (name, expected) in [
            ("foreign_keys", 1),
            ("trusted_schema", 0),
            ("synchronous", 2),
        ] {
            let actual: i64 = database
                .connection()
                .pragma_query_value(None, name, |row| row.get(0))
                .expect("connection policy pragma must be queryable");
            assert_eq!(actual, expected, "unexpected {name} value");
        }
        let journal_mode: String = database
            .connection()
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .expect("journal mode must be queryable");
        assert_eq!(journal_mode, "memory");
    }
}
