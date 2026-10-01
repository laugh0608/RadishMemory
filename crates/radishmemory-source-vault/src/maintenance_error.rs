use crate::SourceVaultError;
use radishmemory_sqlite::{SqliteError, SqliteErrorCode, SqliteStorageReason};
use std::{error::Error, fmt};

/// Bounded diagnostics only; never retain SQLite's arbitrary SQL/source chain.
#[derive(Debug)]
pub enum VaultMaintenanceError {
    Database {
        code: SqliteErrorCode,
        reason: Option<SqliteStorageReason>,
        sqlite_extended_code: Option<i32>,
    },
    Vault(SourceVaultError),
}
impl From<SqliteError> for VaultMaintenanceError {
    fn from(error: SqliteError) -> Self {
        Self::Database {
            code: error.code(),
            reason: error.storage_reason(),
            sqlite_extended_code: error.sqlite_extended_code(),
        }
    }
}
impl From<SourceVaultError> for VaultMaintenanceError {
    fn from(error: SourceVaultError) -> Self {
        Self::Vault(error)
    }
}
impl fmt::Display for VaultMaintenanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database {
                code,
                reason,
                sqlite_extended_code,
            } => write!(
                f,
                "vault maintenance database failure: {code:?} ({reason:?}, SQLite {sqlite_extended_code:?})"
            ),
            Self::Vault(error) => write!(f, "vault maintenance failed: {error}"),
        }
    }
}
impl Error for VaultMaintenanceError {}
