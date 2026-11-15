//! SQLite adapter entry for RadishMemory.
//!
//! This crate owns connection policy, capability checks, and embedded schema
//! migrations. SQL handles and database row identifiers remain private to the
//! adapter boundary.

mod capability;
mod capture_fingerprint;
mod deletion_actions;
mod deletion_store;
mod derived_index;
mod encrypted_capture;
mod error;
#[cfg(feature = "fixture-runner")]
mod fixture_runner;
mod legacy_deletion;
mod library_format;
mod memory_store;
mod migration;
#[doc(hidden)]
pub mod read_cost;
mod recall_codec;
mod recall_profile;
mod recall_repair;
mod source_capture;
mod source_catalog;
mod source_citation;
mod source_store;
mod source_vault_key;
mod source_vault_migration;
mod vault_session;

use std::fmt;
use std::path::Path;

use radishmemory_core::{LocalSearch, LocalSearchHit, LocalSearchRequest};
#[cfg(test)]
use radishmemory_file_entry as _;
use rusqlite::{Connection, TransactionBehavior};

pub use capability::{REVIEWED_BUNDLED_SQLITE_VERSION, SqliteCapabilities};
pub use encrypted_capture::{
    CaptureAbandonmentTarget, CaptureObject, CaptureObjectState, EncryptedCaptureDatabase,
    ObjectReadView,
};
pub use error::{
    SqliteCapability, SqliteConfigurationReason, SqliteError, SqliteErrorCode, SqliteStorageReason,
};
#[cfg(feature = "fixture-runner")]
pub use fixture_runner::FixtureDeletionFailure;
pub use library_format::{LibraryFormat, inspect_library_format};
pub use migration::SQLITE_SCHEMA_VERSION;
pub use recall_profile::RecallProfile;
pub use source_citation::ResolvedSourceCitation;
pub use source_vault_key::{KeyInitializationTransaction, SourceVaultKeyDatabase};
pub use source_vault_migration::{BodyMigrationDatabase, BodyMigrationItem, BodyMigrationState};

/// An initialized RadishMemory SQLite database.
pub struct SqliteDatabase {
    connection: Connection,
    capabilities: SqliteCapabilities,
}

impl SqliteDatabase {
    pub fn deletion_evidence_upgrade_required(&self) -> Result<bool, SqliteError> {
        Ok(!migration::evidence_v2_enabled(&self.connection)?)
    }

    /// Upgrade an existing bound plaintext library under its writer lock.
    pub fn upgrade_deletion_evidence(
        &mut self,
        namespace: &radishmemory_core::Identifier,
    ) -> Result<(), SqliteError> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Exclusive)
            .map_err(SqliteError::storage)?;
        library_format::require_v6(&tx)?;
        source_vault_key::verify_plaintext_facts(&tx, namespace.as_str())?;
        migration::upgrade_evidence(&tx, SQLITE_SCHEMA_VERSION)?;
        tx.commit().map_err(SqliteError::storage)
    }

    /// Opens or creates a database, verifies the runtime, and applies migrations.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqliteError> {
        let _cost = read_cost::span(read_cost::Stage::ReaderOpen);
        let connection = Connection::open(path).map_err(SqliteError::open)?;
        Self::initialize(connection)
    }

    fn initialize(connection: Connection) -> Result<Self, SqliteError> {
        let schema_version = migration::preflight(&connection)?;
        Self::initialize_from(connection, schema_version)
    }

    fn initialize_from(
        mut connection: Connection,
        schema_version: i64,
    ) -> Result<Self, SqliteError> {
        configure_connection(&connection)?;
        let capabilities = capability::probe(&connection)?;
        migration::migrate_from(&mut connection, schema_version)?;
        Self::verify_initialized(&connection)?;

        Ok(Self {
            connection,
            capabilities,
        })
    }

    /// Opens an existing v6 library without creating or upgrading a database.
    pub fn open_current(path: &Path) -> Result<Self, SqliteError> {
        let mut connection =
            library_format::open_existing(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        library_format::require_delete_journal(&connection)?;
        configure_connection(&connection)?;
        let capabilities = capability::probe(&connection)?;
        let readback = connection.transaction().map_err(SqliteError::storage)?;
        library_format::require_v6(&readback)?;
        Self::verify_initialized(&readback)?;
        readback.commit().map_err(SqliteError::storage)?;
        Ok(Self {
            connection,
            capabilities,
        })
    }

    /// Initializes a new file, failing if its name is already occupied.
    pub fn create_new(path: &Path) -> Result<Self, SqliteError> {
        let path = library_format::resolve_database_path(path)?;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(SqliteError::open_path)?;
        let connection =
            library_format::open_existing(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        let version = migration::preflight(&connection)?;
        if version != 0 {
            return Err(SqliteError::unsupported_schema_version(version, 0));
        }
        Self::initialize_from(connection, version)
    }

    fn verify_initialized(connection: &Connection) -> Result<(), SqliteError> {
        derived_index::verify(connection)?;
        source_capture::verify_origin_bindings(connection)
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

    /// Rebuilds all M0 local-recall derivations from validated canonical facts.
    pub fn rebuild_recall_derivations(&mut self) -> Result<(), SqliteError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SqliteError::storage)?;
        source_capture::verify_origin_bindings(&transaction)?;
        derived_index::rebuild(&transaction)?;
        source_capture::verify_origin_bindings(&transaction)?;
        transaction.commit().map_err(SqliteError::storage)
    }

    /// Verifies active file-capture facts, bindings, current projection, and FTS rows.
    pub fn verify_recall_derivations(&self) -> Result<(), SqliteError> {
        source_capture::verify_origin_bindings(&self.connection)?;
        derived_index::verify(&self.connection)
    }

    pub fn recall_profile(&self) -> Result<RecallProfile, SqliteError> {
        recall_profile::read(&self.connection)
    }

    /// Explicit atomic upgrade/downgrade of a healthy library's derived FTS.
    /// Historical body versions, facts and receipts are never changed.
    pub fn change_recall_profile(
        &mut self,
        target: RecallProfile,
    ) -> Result<RecallProfile, SqliteError> {
        self.change_recall_profile_with_check(target, |_| Ok(()))
    }

    fn change_recall_profile_with_check(
        &mut self,
        target: RecallProfile,
        before_commit: impl FnOnce(&Connection) -> Result<(), SqliteError>,
    ) -> Result<RecallProfile, SqliteError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SqliteError::storage)?;
        if migration::preflight(&transaction)? != i64::from(SQLITE_SCHEMA_VERSION) {
            return Err(SqliteError::schema_drift(None));
        }
        source_vault_key::verify_plaintext_bodies(&transaction)?;
        recall_profile::require_completed_deletions(&transaction)?;
        recall_profile::verify_index(&transaction)?;
        if recall_profile::read(&transaction)? != target {
            recall_profile::replace_schema(&transaction, target)?;
            derived_index::rebuild(&transaction)?;
        }
        migration::verify_schema_definition(&transaction, SQLITE_SCHEMA_VERSION)?;
        derived_index::verify(&transaction)?;
        recall_profile::verify_index(&transaction)?;
        before_commit(&transaction)?;
        transaction.commit().map_err(SqliteError::storage)?;
        let readback = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(SqliteError::storage)?;
        migration::preflight(&readback)?;
        source_capture::verify_origin_bindings(&readback)?;
        derived_index::verify(&readback)?;
        let actual = recall_profile::read(&readback)?;
        if actual != target {
            return Err(SqliteError::conflict(
                SqliteStorageReason::RecallProfileChanged,
            ));
        }
        readback.commit().map_err(SqliteError::storage)?;
        Ok(actual)
    }

    #[cfg(test)]
    const fn connection(&self) -> &Connection {
        &self.connection
    }
}

impl LocalSearch for SqliteDatabase {
    type Error = SqliteError;

    fn search(&self, request: &LocalSearchRequest) -> Result<Vec<LocalSearchHit>, Self::Error> {
        derived_index::search(&self.connection, request)
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
