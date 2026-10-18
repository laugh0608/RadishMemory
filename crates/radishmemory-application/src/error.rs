use std::error::Error;
use std::fmt;

/// Stable production host operation names for UI matching.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplicationOperation {
    OpenLibrary,
    InitializeLibraryKey,
    MigrateLibraryBodies,
    InspectRecovery,
    AbandonCapture,
    ImportNewSource,
    UpdateSource,
    ListSources,
    GetSource,
    SearchSources,
    ExportSource,
    DeleteSourceLineage,
    GetDeletionEvidence,
    VerifyLibrary,
    RebuildRecall,
}

/// Stable top-level categories returned by the Phase 1 application service.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplicationErrorCode {
    InvalidRequest,
    Runtime,
    FileEntry,
    Canonical,
    Storage,
    SourceVault,
    NotFound,
}

/// Stable application failure details without path or content retention.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplicationErrorReason {
    InvalidConfiguration,
    InvalidRuntimeIdentifier,
    IdentifierGenerationFailed,
    ClockFailed,
    FileEntryRejected,
    CanonicalInvariant,
    StorageFailure,
    SourceVaultFailure,
    LineageNotFound,
    SourceNotFound,
    OriginalRequestRequired,
    RequestProfileMismatch,
}

/// Redacted application failure with a local source chain.
pub struct ApplicationError {
    operation: ApplicationOperation,
    code: ApplicationErrorCode,
    reason: ApplicationErrorReason,
    retryable: bool,
    source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

impl ApplicationError {
    pub(crate) const fn original_request_required(operation: ApplicationOperation) -> Self {
        Self::without_source(
            operation,
            ApplicationErrorCode::InvalidRequest,
            ApplicationErrorReason::OriginalRequestRequired,
            false,
        )
    }
    pub(crate) const fn request_profile_mismatch(operation: ApplicationOperation) -> Self {
        Self::without_source(
            operation,
            ApplicationErrorCode::InvalidRequest,
            ApplicationErrorReason::RequestProfileMismatch,
            false,
        )
    }
    pub(crate) const fn invalid_configuration() -> Self {
        Self::without_source(
            ApplicationOperation::OpenLibrary,
            ApplicationErrorCode::InvalidRequest,
            ApplicationErrorReason::InvalidConfiguration,
            false,
        )
    }

    pub(crate) const fn invalid_runtime_identifier(operation: ApplicationOperation) -> Self {
        Self::without_source(
            operation,
            ApplicationErrorCode::Runtime,
            ApplicationErrorReason::InvalidRuntimeIdentifier,
            false,
        )
    }

    pub(crate) fn identifier_generation<E>(operation: ApplicationOperation, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self::with_source(
            operation,
            ApplicationErrorCode::Runtime,
            ApplicationErrorReason::IdentifierGenerationFailed,
            true,
            source,
        )
    }

    pub(crate) fn clock<E>(operation: ApplicationOperation, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self::with_source(
            operation,
            ApplicationErrorCode::Runtime,
            ApplicationErrorReason::ClockFailed,
            true,
            source,
        )
    }

    pub(crate) fn file_entry(
        operation: ApplicationOperation,
        source: radishmemory_file_entry::FileEntryError,
    ) -> Self {
        let retryable = source.retryable();
        Self::with_source(
            operation,
            ApplicationErrorCode::FileEntry,
            ApplicationErrorReason::FileEntryRejected,
            retryable,
            source,
        )
    }

    pub(crate) fn canonical(
        operation: ApplicationOperation,
        source: radishmemory_core::CoreError,
    ) -> Self {
        Self::with_source(
            operation,
            ApplicationErrorCode::Canonical,
            ApplicationErrorReason::CanonicalInvariant,
            false,
            source,
        )
    }

    pub(crate) fn storage(
        operation: ApplicationOperation,
        source: radishmemory_sqlite::SqliteError,
    ) -> Self {
        Self::with_source(
            operation,
            ApplicationErrorCode::Storage,
            ApplicationErrorReason::StorageFailure,
            false,
            source,
        )
    }

    pub(crate) const fn lineage_not_found(operation: ApplicationOperation) -> Self {
        Self::without_source(
            operation,
            ApplicationErrorCode::NotFound,
            ApplicationErrorReason::LineageNotFound,
            false,
        )
    }

    pub(crate) fn vault(
        operation: ApplicationOperation,
        source: radishmemory_source_vault::VaultMaintenanceError,
    ) -> Self {
        // Retrying maintenance is an explicit decision based on the bounded cause;
        // a generic retry must never bootstrap a key or invent a new request.
        Self::with_source(
            operation,
            ApplicationErrorCode::SourceVault,
            ApplicationErrorReason::SourceVaultFailure,
            false,
            source,
        )
    }

    /// Bounded database/key/object cause, with no raw SQL or credential diagnostics.
    #[must_use]
    pub fn vault_failure(&self) -> Option<&radishmemory_source_vault::VaultMaintenanceError> {
        self.source.as_deref()?.downcast_ref()
    }

    pub(crate) const fn source_not_found(operation: ApplicationOperation) -> Self {
        Self::without_source(
            operation,
            ApplicationErrorCode::NotFound,
            ApplicationErrorReason::SourceNotFound,
            false,
        )
    }

    const fn without_source(
        operation: ApplicationOperation,
        code: ApplicationErrorCode,
        reason: ApplicationErrorReason,
        retryable: bool,
    ) -> Self {
        Self {
            operation,
            code,
            reason,
            retryable,
            source: None,
        }
    }

    fn with_source<E>(
        operation: ApplicationOperation,
        code: ApplicationErrorCode,
        reason: ApplicationErrorReason,
        retryable: bool,
        source: E,
    ) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self {
            operation,
            code,
            reason,
            retryable,
            source: Some(Box::new(source)),
        }
    }

    #[must_use]
    pub const fn operation(&self) -> ApplicationOperation {
        self.operation
    }

    #[must_use]
    pub const fn code(&self) -> ApplicationErrorCode {
        self.code
    }

    #[must_use]
    pub const fn reason(&self) -> ApplicationErrorReason {
        self.reason
    }

    #[must_use]
    pub const fn retryable(&self) -> bool {
        self.retryable
    }
}

impl fmt::Debug for ApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationError")
            .field("operation", &self.operation)
            .field("code", &self.code)
            .field("reason", &self.reason)
            .field("retryable", &self.retryable)
            .field("has_source", &self.source.is_some())
            .finish()
    }
}

impl fmt::Display for ApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.code {
            ApplicationErrorCode::InvalidRequest => "invalid local library request",
            ApplicationErrorCode::Runtime => "local application runtime failed",
            ApplicationErrorCode::FileEntry => "local file operation was rejected",
            ApplicationErrorCode::Canonical => "canonical application invariant failed",
            ApplicationErrorCode::Storage => "local library storage failed",
            ApplicationErrorCode::SourceVault => "encrypted local library operation failed",
            ApplicationErrorCode::NotFound => "local library object was not found",
        })
    }
}

impl Error for ApplicationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}
