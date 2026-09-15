//! Combines the SQLite-owned live eligibility proof and the object capability.
use std::{error::Error, fmt};

use radishmemory_sqlite::{
    SourceVaultKeyDatabase, SqliteError, SqliteErrorCode, SqliteStorageReason,
};

use crate::{
    KeyEncryptionKey, ObjectDirectory, PROVIDER_PROFILE, SourceVaultError, SourceVaultErrorCode,
};

/// Bounded diagnostics only; never retain SQLite's arbitrary SQL/source chain.
#[derive(Debug)]
pub enum KeyInitializationError {
    Database {
        code: SqliteErrorCode,
        reason: Option<SqliteStorageReason>,
        sqlite_extended_code: Option<i32>,
    },
    Vault(SourceVaultError),
}
impl From<SqliteError> for KeyInitializationError {
    fn from(error: SqliteError) -> Self {
        Self::Database {
            code: error.code(),
            reason: error.storage_reason(),
            sqlite_extended_code: error.sqlite_extended_code(),
        }
    }
}
impl From<SourceVaultError> for KeyInitializationError {
    fn from(error: SourceVaultError) -> Self {
        Self::Vault(error)
    }
}
impl fmt::Display for KeyInitializationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database {
                code,
                reason,
                sqlite_extended_code,
            } => write!(
                f,
                "key initialization database failure: {code:?} ({reason:?}, SQLite {sqlite_extended_code:?})"
            ),
            Self::Vault(error) => write!(f, "key initialization failed: {error}"),
        }
    }
}
impl Error for KeyInitializationError {}

pub(crate) fn initialize(
    directory: &ObjectDirectory,
    namespace: &str,
    device: &str,
    key_operation: impl FnOnce(bool) -> Result<KeyEncryptionKey, SourceVaultError>,
) -> Result<KeyEncryptionKey, KeyInitializationError> {
    let (path, reference) = directory.prepare_key_database()?;
    let mut database = SourceVaultKeyDatabase::open(&path)?;
    reference.verify_identity(&path)?;
    initialize_in_database(&mut database, directory, namespace, device, key_operation)
}

fn initialize_in_database(
    database: &mut SourceVaultKeyDatabase,
    directory: &ObjectDirectory,
    namespace: &str,
    device: &str,
    key_operation: impl FnOnce(bool) -> Result<KeyEncryptionKey, SourceVaultError>,
) -> Result<KeyEncryptionKey, KeyInitializationError> {
    let path = directory.key_database_path()?;
    let reference = crate::filesystem_support::Observation::open(&path)?;
    let transaction = database.begin_key_initialization(namespace, device, PROVIDER_PROFILE)?;
    reference.verify_identity(&path)?;
    // A nonempty directory is evidence, never a caller assertion of eligibility.
    // Route it to read-only provider access so a missing key stays key_missing.
    let empty = directory.is_empty_for_key_initialization()?;
    let key = key_operation(transaction.key_already_initialized() || !empty)?;
    if !empty || !directory.is_empty_for_key_initialization()? {
        return Err(SourceVaultError::new(
            SourceVaultErrorCode::AttemptMismatch,
            "object facts require migration reconciliation",
        )
        .into());
    }
    directory.key_database_path()?;
    reference.verify_identity(&path)?;
    transaction.commit()?;
    reference.verify_identity(&path)?;
    directory.key_database_path()?;
    Ok(key)
}

#[cfg(test)]
mod tests;
