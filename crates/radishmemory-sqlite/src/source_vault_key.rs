//! Maintenance-only key checkpoint. No ordinary library operations are exposed.
use std::{fmt, path::Path};

use radishmemory_core::compute_exact_bytes_digest;
use rusqlite::{Connection, OpenFlags, Transaction, TransactionBehavior, params};

use crate::{SqliteError, SqliteStorageReason, migration};

/// A dedicated maintenance connection. The host must suspend ordinary library
/// operations and supply its verified database path before starting preparation.
/// Opening does not upgrade the database. The regular adapter still rejects v7.
pub struct SourceVaultKeyDatabase {
    connection: Connection,
}

impl SourceVaultKeyDatabase {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqliteError> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(SqliteError::open)?;
        // Reject unsupported/drifted databases before changing journal policy.
        migration::preflight_to(&connection, migration::KEY_INITIALIZATION_SCHEMA_VERSION)?;
        crate::configure_connection(&connection)?;
        crate::capability::probe(&connection)?;
        Ok(Self { connection })
    }

    /// Acquires the writer lock before deriving eligibility from persisted facts.
    /// Identifiers must come from the verified host profile; no caller-supplied
    /// eligibility flag or reusable creation permit is accepted.
    pub fn begin_key_initialization(
        &mut self,
        namespace_id: &str,
        device_id: &str,
        provider_profile: &str,
    ) -> Result<KeyInitializationTransaction<'_>, SqliteError> {
        if namespace_id.is_empty() || device_id.is_empty() || provider_profile.is_empty() {
            return Err(profile_mismatch());
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SqliteError::storage)?;
        let found =
            migration::preflight_to(&transaction, migration::KEY_INITIALIZATION_SCHEMA_VERSION)?;
        // Earlier historical versions must first pass the existing v6 migration.
        if !matches!(found, 0 | 6 | 7) {
            return Err(SqliteError::unsupported_schema_version(
                found,
                migration::KEY_INITIALIZATION_SCHEMA_VERSION,
            ));
        }
        if found == 0 {
            migration::apply_pending(&transaction, 0, migration::SQLITE_SCHEMA_VERSION)?;
        }
        let version = if found == 0 { 6 } else { found as u32 };
        migration::verify_schema_definition(&transaction, version)?;
        verify_plaintext_facts(&transaction, namespace_id)?;
        let initialized = found == i64::from(migration::KEY_INITIALIZATION_SCHEMA_VERSION);
        if initialized {
            let mut statement = transaction
                .prepare(
                    "SELECT singleton, namespace_id, device_id, provider_profile, state
                 FROM radishmemory_source_vault_key_profile",
                )
                .map_err(SqliteError::storage)?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })
                .map_err(SqliteError::storage)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(SqliteError::storage)?;
            if rows
                != vec![(
                    1,
                    namespace_id.into(),
                    device_id.into(),
                    provider_profile.into(),
                    "key_ready".into(),
                )]
            {
                return Err(profile_mismatch());
            }
        }
        Ok(KeyInitializationTransaction {
            transaction,
            initialized,
            namespace_id: namespace_id.into(),
            device_id: device_id.into(),
            provider_profile: provider_profile.into(),
        })
    }
}

/// A non-cloneable proof tied to the live SQLite writer transaction. Dropping
/// rolls back. SQL handles remain private; this checkpoint stores no key material.
pub struct KeyInitializationTransaction<'db> {
    transaction: Transaction<'db>,
    initialized: bool,
    namespace_id: String,
    device_id: String,
    provider_profile: String,
}

impl KeyInitializationTransaction<'_> {
    pub const fn key_already_initialized(&self) -> bool {
        self.initialized
    }

    /// Records the checkpoint only after the coordinating adapter has verified
    /// the key and object directory. This consumes the transaction and its proof.
    pub fn commit(self) -> Result<(), SqliteError> {
        if !self.initialized {
            migration::apply_pending(
                &self.transaction,
                6,
                migration::KEY_INITIALIZATION_SCHEMA_VERSION,
            )?;
            self.transaction.execute(
                "INSERT INTO radishmemory_source_vault_key_profile
                 (singleton, namespace_id, device_id, provider_profile, state) VALUES (1, ?1, ?2, ?3, 'key_ready')",
                params![self.namespace_id, self.device_id, self.provider_profile],
            ).map_err(SqliteError::storage)?;
        }
        self.transaction.commit().map_err(SqliteError::storage)
    }
}

impl fmt::Debug for SourceVaultKeyDatabase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SourceVaultKeyDatabase([REDACTED])")
    }
}
impl fmt::Debug for KeyInitializationTransaction<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyInitializationTransaction")
            .field("initialized", &self.initialized)
            .finish_non_exhaustive()
    }
}

fn profile_mismatch() -> SqliteError {
    SqliteError::invalid_stored(SqliteStorageReason::KeyProfileMismatch)
}

fn verify_plaintext_facts(connection: &Connection, namespace: &str) -> Result<(), SqliteError> {
    let integrity: String = connection
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(SqliteError::storage)?;
    let foreign_key_failure = connection
        .prepare("PRAGMA foreign_key_check")
        .map_err(SqliteError::storage)?
        .exists([])
        .map_err(SqliteError::storage)?;
    if integrity != "ok" || foreign_key_failure {
        return Err(profile_mismatch());
    }
    // Include all canonical namespaces, not only sources selected by current FTS.
    for table in [
        "radishmemory_source_artifacts",
        "radishmemory_source_fragments",
        "radishmemory_memory_proposals",
        "radishmemory_memory_decisions",
        "radishmemory_memory_records",
        "radishmemory_memory_state_events",
        "radishmemory_delete_requests",
        "radishmemory_deletion_evidence",
    ] {
        let mismatch = connection
            .prepare(&format!(
                "SELECT 1 FROM {table} WHERE namespace_id != ?1 LIMIT 1"
            ))
            .map_err(SqliteError::storage)?
            .exists([namespace])
            .map_err(SqliteError::storage)?;
        if mismatch {
            return Err(profile_mismatch());
        }
    }
    // Verify digest and length of every remaining BLOB, including old versions and deletion
    // residuals that ordinary active/current source reads intentionally exclude.
    let mut statement = connection.prepare(
        "SELECT a.source_id, a.namespace_id, a.deletion_state, a.content_length,
                a.content_digest_algorithm, a.content_digest_profile, a.content_digest_value, b.content
         FROM radishmemory_source_artifacts a LEFT JOIN radishmemory_source_bodies b ON b.source_id = a.source_id"
    ).map_err(SqliteError::storage)?;
    let mut rows = statement.query([]).map_err(SqliteError::storage)?;
    while let Some(row) = rows.next().map_err(SqliteError::storage)? {
        let state: String = row.get(2).map_err(SqliteError::storage)?;
        let body: Option<Vec<u8>> = row.get(7).map_err(SqliteError::storage)?;
        if let Some(body) = body {
            let length: i64 = row.get(3).map_err(SqliteError::storage)?;
            let algorithm: String = row.get(4).map_err(SqliteError::storage)?;
            let profile: String = row.get(5).map_err(SqliteError::storage)?;
            let value: String = row.get(6).map_err(SqliteError::storage)?;
            let digest = crate::source_store::digest(&algorithm, &profile, &value)?;
            if u64::try_from(length).ok() != Some(body.len() as u64)
                || compute_exact_bytes_digest(&body) != digest
            {
                return Err(SqliteError::invalid_stored(
                    SqliteStorageReason::StoredIntegrityMismatch,
                ));
            }
        } else if state == "active" {
            return Err(SqliteError::invalid_stored(
                SqliteStorageReason::StoredIntegrityMismatch,
            ));
        }
        if state == "active" {
            let id = crate::source_store::identifier(row.get(0).map_err(SqliteError::storage)?)?;
            let ns = crate::source_store::identifier(row.get(1).map_err(SqliteError::storage)?)?;
            crate::source_store::load_source_artifact(connection, &ns, &id)?
                .ok_or_else(profile_mismatch)?;
            crate::source_store::load_source_fragments(connection, &ns, &id)?
                .ok_or_else(profile_mismatch)?;
        }
    }
    crate::source_capture::verify_origin_bindings(connection)?;
    crate::derived_index::verify(connection)
}
