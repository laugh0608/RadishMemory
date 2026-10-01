//! Shared exclusive session for internal object maintenance/capture coordination.
use crate::{SqliteError, SqliteStorageReason, migration};
use rusqlite::{Connection, OpenFlags, TransactionBehavior};
use std::path::Path;

pub(crate) fn open(
    path: impl AsRef<Path>,
    namespace: &str,
    device: &str,
    provider: &str,
    supported: u32,
    allowed: &[u32],
) -> Result<(Connection, u32), SqliteError> {
    let mut connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .map_err(SqliteError::open)?;
    migration::preflight_to(&connection, supported)?;
    crate::configure_connection(&connection)?;
    crate::capability::probe(&connection)?;
    let mode: String = connection
        .pragma_update_and_check(None, "locking_mode", "EXCLUSIVE", |r| r.get(0))
        .map_err(SqliteError::storage)?;
    if mode != "exclusive" {
        return Err(SqliteError::invalid_stored(
            SqliteStorageReason::StoredIntegrityMismatch,
        ));
    }
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .map_err(SqliteError::storage)?;
    let version = migration::preflight_to(&transaction, supported)?;
    if !allowed.contains(&(version as u32)) {
        return Err(SqliteError::unsupported_schema_version(version, supported));
    }
    migration::verify_schema_definition(&transaction, version as u32)?;
    let profile: (String, String, String, String) = transaction.query_row(
            "SELECT namespace_id, device_id, provider_profile, state FROM radishmemory_source_vault_key_profile WHERE singleton = 1",
            [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        ).map_err(SqliteError::storage)?;
    if profile
        != (
            namespace.into(),
            device.into(),
            provider.into(),
            "key_ready".into(),
        )
    {
        return Err(SqliteError::invalid_stored(
            SqliteStorageReason::KeyProfileMismatch,
        ));
    }
    transaction.commit().map_err(SqliteError::storage)?;
    Ok((connection, version as u32))
}
