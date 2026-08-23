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

/// SQLite adapter failure that does not display database paths or SQL text.
pub struct SqliteError {
    code: SqliteErrorCode,
    capability: Option<SqliteCapability>,
    configuration_reason: Option<SqliteConfigurationReason>,
    found_schema_version: Option<i64>,
    supported_schema_version: Option<u32>,
    source: Option<rusqlite::Error>,
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
            found_schema_version: None,
            supported_schema_version: None,
            source,
        }
    }

    pub(crate) fn unsupported_schema_version(found: i64, supported: u32) -> Self {
        Self {
            code: SqliteErrorCode::UnsupportedSchemaVersion,
            capability: None,
            configuration_reason: None,
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
            found_schema_version: None,
            supported_schema_version: None,
            source,
        }
    }

    fn with_source(code: SqliteErrorCode, source: rusqlite::Error) -> Self {
        Self {
            code,
            capability: None,
            configuration_reason: None,
            found_schema_version: None,
            supported_schema_version: None,
            source: Some(source),
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
        };
        formatter.write_str(message)
    }
}

impl Error for SqliteError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_ref()
            .map(|source| source as &(dyn Error + 'static))
    }
}
