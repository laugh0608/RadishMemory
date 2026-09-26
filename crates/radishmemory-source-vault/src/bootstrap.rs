//! Combines the SQLite-owned live eligibility proof and the object capability.
use radishmemory_sqlite::SourceVaultKeyDatabase;

use crate::{
    KeyEncryptionKey, KeyInitializationError, ObjectDirectory, PROVIDER_PROFILE, SourceVaultError,
    SourceVaultErrorCode,
};

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
use radishmemory_sqlite::SqliteErrorCode;
#[cfg(test)]
mod tests;
