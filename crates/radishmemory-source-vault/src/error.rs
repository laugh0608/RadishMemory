use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceVaultErrorCode {
    InvalidMetadata,
    PlaintextTooLarge,
    LengthMismatch,
    DigestMismatch,
    RandomSourceUnavailable,
    EncryptionFailed,
    MalformedCiphertext,
    AuthenticationFailed,
    InvalidEnvelope,
    MetadataMismatch,
    InvalidLocator,
    InvalidDirectory,
    InvalidFile,
    FilesystemChanged,
    ObjectExists,
    ObjectMissing,
    AttemptMismatch,
    Io,
}

#[derive(Clone, Eq, PartialEq)]
pub struct SourceVaultError {
    code: SourceVaultErrorCode,
    reason: &'static str,
    io_kind: Option<std::io::ErrorKind>,
    os_code: Option<i32>,
}

impl SourceVaultError {
    pub(crate) const fn new(code: SourceVaultErrorCode, reason: &'static str) -> Self {
        Self {
            code,
            reason,
            io_kind: None,
            os_code: None,
        }
    }

    pub(crate) fn io(reason: &'static str, error: std::io::Error) -> Self {
        Self {
            code: SourceVaultErrorCode::Io,
            reason,
            io_kind: Some(error.kind()),
            os_code: error.raw_os_error(),
        }
    }

    pub fn io_kind(&self) -> Option<std::io::ErrorKind> {
        self.io_kind
    }
    pub fn os_code(&self) -> Option<i32> {
        self.os_code
    }

    pub const fn code(&self) -> SourceVaultErrorCode {
        self.code
    }

    pub const fn reason(&self) -> &'static str {
        self.reason
    }
}

impl fmt::Debug for SourceVaultError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SourceVaultError")
            .field("code", &self.code)
            .field("reason", &self.reason)
            .field("io_kind", &self.io_kind)
            .field("os_code", &self.os_code)
            .finish()
    }
}

impl fmt::Display for SourceVaultError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "source vault {:?}: {}", self.code, self.reason)
    }
}

impl Error for SourceVaultError {}
