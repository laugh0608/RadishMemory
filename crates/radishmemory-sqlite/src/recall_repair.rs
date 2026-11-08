//! Explicit v6 maintenance after normal open rejects logical recall drift.
use crate::{
    SQLITE_SCHEMA_VERSION, SqliteCapabilities, SqliteConfigurationReason, SqliteDatabase,
    SqliteError, capability, configure_connection, derived_index, memory_store, migration,
    recall_profile, source_vault_key,
};
use radishmemory_core::Identifier;
use rusqlite::{Connection, OpenFlags, TransactionBehavior};
use std::{fs, io, path::Path};

impl SqliteDatabase {
    /// Read only the inputs to an explicit repair on an existing v6 database.
    /// No database handle or authorization is retained between user actions.
    pub fn check_recall_repair(path: &Path, namespace: &Identifier) -> Result<(), SqliteError> {
        let (mut connection, _) = open_existing(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let tx = connection.transaction().map_err(SqliteError::storage)?;
        verify_inputs(&tx, namespace)?;
        tx.commit().map_err(SqliteError::storage)
    }

    /// Recheck the inputs under the writer lock, rebuild only derived rows and
    /// return a usable library only after committed readback.
    pub fn repair_recall(path: &Path, namespace: &Identifier) -> Result<Self, SqliteError> {
        let (mut connection, capabilities) =
            open_existing(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SqliteError::storage)?;
        verify_inputs(&tx, namespace)?;
        // Structural FTS damage is outside this logical repair.
        recall_profile::verify_index(&tx)?;
        derived_index::rebuild(&tx)?;
        recall_profile::verify_index(&tx)?;
        tx.commit().map_err(SqliteError::storage)?;

        verified_open(connection, capabilities, namespace)
    }

    /// Retry a failed v6 open without creating or migrating a library.
    pub fn reopen_after_recall_failure(
        path: &Path,
        namespace: &Identifier,
    ) -> Result<Self, SqliteError> {
        let (connection, capabilities) = open_existing(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        verified_open(connection, capabilities, namespace)
    }
}

fn verified_open(
    mut connection: Connection,
    capabilities: SqliteCapabilities,
    namespace: &Identifier,
) -> Result<SqliteDatabase, SqliteError> {
    let readback = connection.transaction().map_err(SqliteError::storage)?;
    verify_inputs(&readback, namespace)?;
    derived_index::verify(&readback)?;
    recall_profile::verify_index(&readback)?;
    readback.commit().map_err(SqliteError::storage)?;
    Ok(SqliteDatabase {
        connection,
        capabilities,
    })
}

fn open_existing(
    path: &Path,
    access: OpenFlags,
) -> Result<(Connection, SqliteCapabilities), SqliteError> {
    let invalid_path = || SqliteError::open_path(io::ErrorKind::InvalidInput.into());
    let name = path.file_name().ok_or_else(invalid_path)?;
    let directory = path.parent().ok_or_else(invalid_path)?;
    if !path.is_absolute()
        || !fs::symlink_metadata(directory)
            .map_err(SqliteError::open_path)?
            .is_dir()
    {
        return Err(invalid_path());
    }
    // Resolve platform ancestor aliases such as macOS /var. The application
    // directory and database themselves must not be symlinks.
    let path = directory
        .canonicalize()
        .map_err(SqliteError::open_path)?
        .join(name);
    if !fs::symlink_metadata(&path)
        .map_err(SqliteError::open_path)?
        .is_file()
    {
        return Err(invalid_path());
    }
    let connection = Connection::open_with_flags(
        path,
        access | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .map_err(SqliteError::open)?;
    require_v6(&connection)?;
    // Reject unexpected journal modes without converting an existing file.
    let journal: String = connection
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .map_err(SqliteError::configuration_source)?;
    if journal != "delete" {
        return Err(SqliteError::configuration(
            SqliteConfigurationReason::PersistentJournalMode,
        ));
    }
    configure_connection(&connection)?;
    let capabilities = capability::probe(&connection)?;
    Ok((connection, capabilities))
}

fn require_v6(connection: &Connection) -> Result<(), SqliteError> {
    let version = migration::preflight(connection)?;
    if version != i64::from(SQLITE_SCHEMA_VERSION) {
        return Err(SqliteError::unsupported_schema_version(
            version,
            SQLITE_SCHEMA_VERSION,
        ));
    }
    Ok(())
}

fn verify_inputs(connection: &Connection, namespace: &Identifier) -> Result<(), SqliteError> {
    require_v6(connection)?;
    source_vault_key::verify_database_facts(connection, namespace.as_str())?;
    source_vault_key::verify_plaintext_source_facts(connection)?;
    recall_profile::require_completed_deletions(connection)?;
    memory_store::verify_active_proposals(connection, namespace)?;
    derived_index::verify_rebuild_inputs(connection)
}
