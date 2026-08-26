use rusqlite::Connection;

use crate::{SqliteCapability, SqliteError};

/// SQLite version carried by the reviewed `libsqlite3-sys 0.38.2` source.
pub const REVIEWED_BUNDLED_SQLITE_VERSION: &str = "3.53.2";

/// Runtime facts proven before migrations execute.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SqliteCapabilities {
    sqlite_version: Box<str>,
    fts5: bool,
}

impl SqliteCapabilities {
    /// Returns the SQLite runtime version reported by the opened connection.
    #[must_use]
    pub fn sqlite_version(&self) -> &str {
        &self.sqlite_version
    }

    /// Reports whether both the FTS5 compile option and a live probe succeeded.
    #[must_use]
    pub const fn fts5(&self) -> bool {
        self.fts5
    }
}

pub(crate) fn probe(connection: &Connection) -> Result<SqliteCapabilities, SqliteError> {
    let sqlite_version: String = connection
        .query_row("SELECT sqlite_version()", [], |row| row.get(0))
        .map_err(SqliteError::capability_probe)?;
    require_capability(
        sqlite_version == REVIEWED_BUNDLED_SQLITE_VERSION,
        SqliteCapability::ReviewedBundledVersion,
    )?;

    let compile_option_enabled: i64 = connection
        .query_row(
            "SELECT sqlite_compileoption_used('ENABLE_FTS5')",
            [],
            |row| row.get(0),
        )
        .map_err(SqliteError::capability_probe)?;
    require_capability(compile_option_enabled == 1, SqliteCapability::Fts5)?;

    connection
        .execute_batch(
            "DROP TABLE IF EXISTS temp.radishmemory_fts5_capability_probe;
             CREATE VIRTUAL TABLE temp.radishmemory_fts5_capability_probe USING fts5(content);
             DROP TABLE temp.radishmemory_fts5_capability_probe;",
        )
        .map_err(|source| {
            SqliteError::unsupported_capability(SqliteCapability::Fts5, Some(source))
        })?;

    Ok(SqliteCapabilities {
        sqlite_version: sqlite_version.into_boxed_str(),
        fts5: true,
    })
}

fn require_capability(available: bool, capability: SqliteCapability) -> Result<(), SqliteError> {
    if !available {
        return Err(SqliteError::unsupported_capability(capability, None));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_runtime_proves_fts5_and_cleans_up_probe_table() {
        let connection = Connection::open_in_memory().expect("in-memory SQLite must open");
        let capabilities =
            probe(&connection).expect("bundled runtime must satisfy capability probe");

        assert_eq!(
            capabilities.sqlite_version(),
            REVIEWED_BUNDLED_SQLITE_VERSION
        );
        assert!(capabilities.fts5());
        let remaining: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM temp.sqlite_schema
                 WHERE name = 'radishmemory_fts5_capability_probe'",
                [],
                |row| row.get(0),
            )
            .expect("temporary schema must be queryable");
        assert_eq!(remaining, 0);
    }

    #[test]
    fn missing_fts5_has_stable_unsupported_capability_error() {
        let error = require_capability(false, SqliteCapability::Fts5)
            .expect_err("missing FTS5 must fail closed");

        assert_eq!(error.code(), crate::SqliteErrorCode::UnsupportedCapability);
        assert_eq!(error.capability(), Some(SqliteCapability::Fts5));
    }
}
