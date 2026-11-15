//! Explicit v6 maintenance after normal open rejects logical recall drift.
use crate::{
    SqliteCapabilities, SqliteDatabase, SqliteError, capability, configure_connection,
    derived_index, library_format::require_v6, memory_store, recall_profile, source_vault_key,
};
use radishmemory_core::Identifier;
use rusqlite::{Connection, OpenFlags, TransactionBehavior};
use std::path::Path;

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
    let connection = crate::library_format::open_existing(path, access)?;
    require_v6(&connection)?;
    crate::library_format::require_delete_journal(&connection)?;
    configure_connection(&connection)?;
    let capabilities = capability::probe(&connection)?;
    Ok((connection, capabilities))
}

fn verify_inputs(connection: &Connection, namespace: &Identifier) -> Result<(), SqliteError> {
    require_v6(connection)?;
    source_vault_key::verify_database_facts(connection, namespace.as_str())?;
    source_vault_key::verify_plaintext_source_facts(connection)?;
    recall_profile::require_completed_deletions(connection)?;
    memory_store::verify_active_proposals(connection, namespace)?;
    derived_index::verify_rebuild_inputs(connection)
}
