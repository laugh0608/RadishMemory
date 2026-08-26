use std::error::Error;
use std::fmt;

/// Stable top-level SQLite adapter failure categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SqliteErrorCode {
    Open,
    ConnectionConfiguration,
    CapabilityProbe,
    UnsupportedCapability,
    UnsupportedSchemaVersion,
    Migration,
    SchemaDrift,
    Storage,
    Search,
    Conflict,
    InvalidStoredData,
    SourceInvariant,
    MemoryInvariant,
    DeletionInvariant,
}

/// Runtime capabilities that the adapter requires before touching schema data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SqliteCapability {
    ReviewedBundledVersion,
    Fts5,
}

/// Stable connection-policy failures without retaining paths or SQL text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SqliteConfigurationReason {
    ForeignKeysDisabled,
    TrustedSchemaEnabled,
    SynchronousNotFull,
    PersistentJournalMode,
}

/// Stable storage detail without retaining identifiers or content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SqliteStorageReason {
    EmptyFragmentBatch,
    MixedFragmentBatch,
    DuplicateFragment,
    DuplicateObject,
    NumericRange,
    MissingSource,
    NamespaceMismatch,
    SourceResolution,
    UnknownEnum,
    InvalidUtf8,
    InvalidCanonicalObject,
    StoredIntegrityMismatch,
    DuplicateProposal,
    MissingFragment,
    ProposalSourceResolution,
    MissingProposal,
    DecisionChain,
    TerminalDecision,
    MissingDecision,
    Materialization,
    MissingMemory,
    MemoryReference,
    EventChain,
    UnsupportedCause,
    DerivedDataMismatch,
    MissingDeleteTarget,
    DeletionPlan,
    MissingDeleteRequest,
    DeletionExecution,
    EvidenceChain,
}

/// SQLite adapter failure that does not display database paths or SQL text.
pub struct SqliteError {
    code: SqliteErrorCode,
    capability: Option<SqliteCapability>,
    configuration_reason: Option<SqliteConfigurationReason>,
    storage_reason: Option<SqliteStorageReason>,
    found_schema_version: Option<i64>,
    supported_schema_version: Option<u32>,
    source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

impl SqliteError {
    pub(crate) fn open(source: rusqlite::Error) -> Self {
        Self::with_source(SqliteErrorCode::Open, source)
    }

    pub(crate) fn configuration(reason: SqliteConfigurationReason) -> Self {
        Self {
            code: SqliteErrorCode::ConnectionConfiguration,
            capability: None,
            configuration_reason: Some(reason),
            storage_reason: None,
            found_schema_version: None,
            supported_schema_version: None,
            source: None,
        }
    }

    pub(crate) fn configuration_source(source: rusqlite::Error) -> Self {
        Self::with_source(SqliteErrorCode::ConnectionConfiguration, source)
    }

    pub(crate) fn capability_probe(source: rusqlite::Error) -> Self {
        Self::with_source(SqliteErrorCode::CapabilityProbe, source)
    }

    pub(crate) fn unsupported_capability(
        capability: SqliteCapability,
        source: Option<rusqlite::Error>,
    ) -> Self {
        Self {
            code: SqliteErrorCode::UnsupportedCapability,
            capability: Some(capability),
            configuration_reason: None,
            storage_reason: None,
            found_schema_version: None,
            supported_schema_version: None,
            source: source.map(|source| Box::new(source) as Box<_>),
        }
    }

    pub(crate) fn unsupported_schema_version(found: i64, supported: u32) -> Self {
        Self {
            code: SqliteErrorCode::UnsupportedSchemaVersion,
            capability: None,
            configuration_reason: None,
            storage_reason: None,
            found_schema_version: Some(found),
            supported_schema_version: Some(supported),
            source: None,
        }
    }

    pub(crate) fn migration(source: rusqlite::Error) -> Self {
        Self::with_source(SqliteErrorCode::Migration, source)
    }

    pub(crate) fn schema_drift(source: Option<rusqlite::Error>) -> Self {
        Self {
            code: SqliteErrorCode::SchemaDrift,
            capability: None,
            configuration_reason: None,
            storage_reason: None,
            found_schema_version: None,
            supported_schema_version: None,
            source: source.map(|source| Box::new(source) as Box<_>),
        }
    }

    pub(crate) fn storage(source: rusqlite::Error) -> Self {
        let conflict = matches!(
            source.sqlite_extended_error_code(),
            Some(rusqlite::ffi::SQLITE_CONSTRAINT_PRIMARYKEY)
                | Some(rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE)
        );
        Self {
            code: if conflict {
                SqliteErrorCode::Conflict
            } else {
                SqliteErrorCode::Storage
            },
            capability: None,
            configuration_reason: None,
            storage_reason: conflict.then_some(SqliteStorageReason::DuplicateObject),
            found_schema_version: None,
            supported_schema_version: None,
            source: Some(Box::new(source)),
        }
    }

    pub(crate) fn search(source: rusqlite::Error) -> Self {
        Self::with_source(SqliteErrorCode::Search, source)
    }

    pub(crate) fn conflict(reason: SqliteStorageReason) -> Self {
        Self::without_source(SqliteErrorCode::Conflict, reason)
    }

    pub(crate) fn source_invariant(reason: SqliteStorageReason) -> Self {
        Self::without_source(SqliteErrorCode::SourceInvariant, reason)
    }

    pub(crate) fn source_invariant_with_core(
        reason: SqliteStorageReason,
        source: radishmemory_core::CoreError,
    ) -> Self {
        Self::with_storage_source(SqliteErrorCode::SourceInvariant, reason, source)
    }

    pub(crate) fn memory_invariant(reason: SqliteStorageReason) -> Self {
        Self::without_source(SqliteErrorCode::MemoryInvariant, reason)
    }

    pub(crate) fn memory_invariant_with_core(
        reason: SqliteStorageReason,
        source: radishmemory_core::CoreError,
    ) -> Self {
        Self::with_storage_source(SqliteErrorCode::MemoryInvariant, reason, source)
    }

    pub(crate) fn deletion_invariant(reason: SqliteStorageReason) -> Self {
        Self::without_source(SqliteErrorCode::DeletionInvariant, reason)
    }

    pub(crate) fn deletion_invariant_with_core(
        reason: SqliteStorageReason,
        source: radishmemory_core::CoreError,
    ) -> Self {
        Self::with_storage_source(SqliteErrorCode::DeletionInvariant, reason, source)
    }

    pub(crate) fn invalid_stored(reason: SqliteStorageReason) -> Self {
        Self::without_source(SqliteErrorCode::InvalidStoredData, reason)
    }

    pub(crate) fn invalid_stored_with_source<E>(reason: SqliteStorageReason, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self::with_storage_source(SqliteErrorCode::InvalidStoredData, reason, source)
    }

    fn without_source(code: SqliteErrorCode, reason: SqliteStorageReason) -> Self {
        Self {
            code,
            capability: None,
            configuration_reason: None,
            storage_reason: Some(reason),
            found_schema_version: None,
            supported_schema_version: None,
            source: None,
        }
    }

    fn with_storage_source<E>(code: SqliteErrorCode, reason: SqliteStorageReason, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self {
            code,
            capability: None,
            configuration_reason: None,
            storage_reason: Some(reason),
            found_schema_version: None,
            supported_schema_version: None,
            source: Some(Box::new(source)),
        }
    }

    fn with_source(code: SqliteErrorCode, source: rusqlite::Error) -> Self {
        Self {
            code,
            capability: None,
            configuration_reason: None,
            storage_reason: None,
            found_schema_version: None,
            supported_schema_version: None,
            source: Some(Box::new(source)),
        }
    }

    /// Returns the stable category intended for programmatic matching.
    #[must_use]
    pub const fn code(&self) -> SqliteErrorCode {
        self.code
    }

    /// Returns the required capability when initialization rejected the runtime.
    #[must_use]
    pub const fn capability(&self) -> Option<SqliteCapability> {
        self.capability
    }

    /// Returns stable connection-policy detail when available.
    #[must_use]
    pub const fn configuration_reason(&self) -> Option<SqliteConfigurationReason> {
        self.configuration_reason
    }

    /// Returns stable storage detail when available.
    #[must_use]
    pub const fn storage_reason(&self) -> Option<SqliteStorageReason> {
        self.storage_reason
    }

    /// Returns the unsupported on-disk schema version when available.
    #[must_use]
    pub const fn found_schema_version(&self) -> Option<i64> {
        self.found_schema_version
    }

    /// Returns the newest schema version understood by this adapter.
    #[must_use]
    pub const fn supported_schema_version(&self) -> Option<u32> {
        self.supported_schema_version
    }
}

impl fmt::Debug for SqliteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SqliteError")
            .field("code", &self.code)
            .field("capability", &self.capability)
            .field("configuration_reason", &self.configuration_reason)
            .field("storage_reason", &self.storage_reason)
            .field("found_schema_version", &self.found_schema_version)
            .field("supported_schema_version", &self.supported_schema_version)
            .field("has_source", &self.source.is_some())
            .finish()
    }
}

impl fmt::Display for SqliteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.code {
            SqliteErrorCode::Open => "SQLite database open failed",
            SqliteErrorCode::ConnectionConfiguration => "SQLite connection configuration failed",
            SqliteErrorCode::CapabilityProbe => "SQLite capability probe failed",
            SqliteErrorCode::UnsupportedCapability => "required SQLite capability is unavailable",
            SqliteErrorCode::UnsupportedSchemaVersion => "unsupported SQLite schema version",
            SqliteErrorCode::Migration => "SQLite schema migration failed",
            SqliteErrorCode::SchemaDrift => "SQLite schema metadata is inconsistent",
            SqliteErrorCode::Storage => "SQLite storage operation failed",
            SqliteErrorCode::Search => "SQLite local search failed",
            SqliteErrorCode::Conflict => "immutable SQLite object already exists",
            SqliteErrorCode::InvalidStoredData => "stored SQLite object is invalid",
            SqliteErrorCode::SourceInvariant => "source storage invariant violation",
            SqliteErrorCode::MemoryInvariant => "memory storage invariant violation",
            SqliteErrorCode::DeletionInvariant => "deletion storage invariant violation",
        };
        formatter.write_str(message)
    }
}

impl Error for SqliteError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}
