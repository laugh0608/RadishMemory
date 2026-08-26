//! Explicitly opt-in controls used only by the synthetic M0 fixture runner.

use radishmemory_core::{
    ComponentResult, DeletionStore, Identifier, LocalDeletionExecution, NonEmptyText,
};
use rusqlite::Connection;

use crate::deletion_store::{execute_request_with_fixture_failure, validate_request_profile};
use crate::{SqliteDatabase, SqliteError, SqliteStorageReason};

impl SqliteDatabase {
    /// Opens one isolated, non-persistent database for the synthetic fixture runner.
    ///
    /// The connection still applies the production capability, migration, integrity,
    /// and synchronous-write policy. Keeping it in memory avoids filesystem sync cost
    /// from becoming part of a deterministic application-contract fixture.
    pub fn open_fixture_isolated() -> Result<Self, SqliteError> {
        let connection = Connection::open_in_memory().map_err(SqliteError::open)?;
        Self::initialize(connection)
    }
}

/// One deterministic component failure injected by the synthetic fixture runner.
///
/// This type is absent unless the `fixture-runner` feature is enabled. It does not
/// alter the production [`DeletionStore`] port or accept arbitrary storage actions.
#[derive(Clone)]
pub struct FixtureDeletionFailure {
    component_key: Identifier,
    error_code: NonEmptyText,
    retryable: bool,
}

impl FixtureDeletionFailure {
    #[must_use]
    pub const fn new(component_key: Identifier, error_code: NonEmptyText, retryable: bool) -> Self {
        Self {
            component_key,
            error_code,
            retryable,
        }
    }

    pub(crate) const fn component_key(&self) -> &Identifier {
        &self.component_key
    }

    pub(crate) const fn error_code(&self) -> &NonEmptyText {
        &self.error_code
    }

    pub(crate) const fn retryable(&self) -> bool {
        self.retryable
    }
}

impl SqliteDatabase {
    /// Executes a real deletion attempt while failing exactly one frozen component.
    pub fn execute_deletion_with_fixture_failure(
        &mut self,
        namespace_id: &Identifier,
        delete_request_id: &Identifier,
        execution: &LocalDeletionExecution,
        failure: &FixtureDeletionFailure,
    ) -> Result<Vec<ComponentResult>, SqliteError> {
        let request = self
            .load_delete_request(namespace_id, delete_request_id)?
            .ok_or_else(|| {
                SqliteError::deletion_invariant(SqliteStorageReason::MissingDeleteRequest)
            })?;
        validate_request_profile(&request)?;
        if !request
            .params()
            .planned_components
            .iter()
            .any(|component| component.component_key() == failure.component_key())
        {
            return Err(SqliteError::deletion_invariant(
                SqliteStorageReason::DeletionPlan,
            ));
        }
        execute_request_with_fixture_failure(
            &mut self.connection,
            &request,
            execution,
            failure.component_key(),
            failure.error_code(),
            failure.retryable(),
        )
    }
}
