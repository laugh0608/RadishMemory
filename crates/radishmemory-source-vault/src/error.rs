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
}

#[derive(Clone, Eq, PartialEq)]
pub struct SourceVaultError {
    code: SourceVaultErrorCode,
    reason: &'static str,
}

impl SourceVaultError {
    pub(crate) const fn new(code: SourceVaultErrorCode, reason: &'static str) -> Self {
        Self { code, reason }
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
            .finish()
    }
}

impl fmt::Display for SourceVaultError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "source vault {:?}: {}", self.code, self.reason)
    }
}

impl Error for SourceVaultError {}
