use std::error::Error;
use std::fmt;
use std::io;

/// Stable top-level categories for the Phase 1 file-entry application contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileEntryErrorCode {
    InvalidRequest,
    PathRejected,
    TypeRejected,
    ContentRejected,
    SourceChanged,
    DestinationRejected,
    Integrity,
    Io,
    Conflict,
}

/// Stable reasons that never retain rejected paths or file content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileEntryErrorReason {
    PathNotAllowed,
    SymlinkNotAllowed,
    NotRegularFile,
    UnsupportedFileType,
    EmptyFile,
    FileTooLarge,
    InvalidUtf8,
    NulByteNotAllowed,
    SourceChangedDuringCapture,
    DestinationNotAllowed,
    DestinationExists,
    IntegrityMismatch,
    IoFailure,
    CanonicalConflict,
}

/// File-entry failure with stable matching fields and redacted formatting.
pub struct FileEntryError {
    code: FileEntryErrorCode,
    reason: FileEntryErrorReason,
    retryable: bool,
    source: Option<io::Error>,
}

impl FileEntryError {
    pub(crate) const fn path_not_allowed() -> Self {
        Self::without_source(
            FileEntryErrorCode::PathRejected,
            FileEntryErrorReason::PathNotAllowed,
            false,
        )
    }

    pub(crate) const fn symlink_not_allowed() -> Self {
        Self::without_source(
            FileEntryErrorCode::PathRejected,
            FileEntryErrorReason::SymlinkNotAllowed,
            false,
        )
    }

    pub(crate) const fn not_regular_file() -> Self {
        Self::without_source(
            FileEntryErrorCode::PathRejected,
            FileEntryErrorReason::NotRegularFile,
            false,
        )
    }

    pub(crate) const fn unsupported_file_type() -> Self {
        Self::without_source(
            FileEntryErrorCode::TypeRejected,
            FileEntryErrorReason::UnsupportedFileType,
            false,
        )
    }

    pub(crate) const fn empty_file() -> Self {
        Self::without_source(
            FileEntryErrorCode::ContentRejected,
            FileEntryErrorReason::EmptyFile,
            false,
        )
    }

    pub(crate) const fn file_too_large() -> Self {
        Self::without_source(
            FileEntryErrorCode::ContentRejected,
            FileEntryErrorReason::FileTooLarge,
            false,
        )
    }

    pub(crate) const fn invalid_utf8() -> Self {
        Self::without_source(
            FileEntryErrorCode::ContentRejected,
            FileEntryErrorReason::InvalidUtf8,
            false,
        )
    }

    pub(crate) const fn nul_byte_not_allowed() -> Self {
        Self::without_source(
            FileEntryErrorCode::ContentRejected,
            FileEntryErrorReason::NulByteNotAllowed,
            false,
        )
    }

    pub(crate) const fn source_changed() -> Self {
        Self::without_source(
            FileEntryErrorCode::SourceChanged,
            FileEntryErrorReason::SourceChangedDuringCapture,
            true,
        )
    }

    pub(crate) const fn integrity_mismatch() -> Self {
        Self::without_source(
            FileEntryErrorCode::Integrity,
            FileEntryErrorReason::IntegrityMismatch,
            false,
        )
    }

    pub(crate) fn io(source: io::Error) -> Self {
        let retryable = matches!(
            source.kind(),
            io::ErrorKind::Interrupted
                | io::ErrorKind::WouldBlock
                | io::ErrorKind::TimedOut
                | io::ErrorKind::OutOfMemory
        );
        Self {
            code: FileEntryErrorCode::Io,
            reason: FileEntryErrorReason::IoFailure,
            retryable,
            source: Some(source),
        }
    }

    const fn without_source(
        code: FileEntryErrorCode,
        reason: FileEntryErrorReason,
        retryable: bool,
    ) -> Self {
        Self {
            code,
            reason,
            retryable,
            source: None,
        }
    }

    #[must_use]
    pub const fn code(&self) -> FileEntryErrorCode {
        self.code
    }

    #[must_use]
    pub const fn reason(&self) -> FileEntryErrorReason {
        self.reason
    }

    #[must_use]
    pub const fn retryable(&self) -> bool {
        self.retryable
    }
}

impl fmt::Debug for FileEntryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FileEntryError")
            .field("code", &self.code)
            .field("reason", &self.reason)
            .field("retryable", &self.retryable)
            .field("has_source", &self.source.is_some())
            .finish()
    }
}

impl fmt::Display for FileEntryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.code {
            FileEntryErrorCode::InvalidRequest => "invalid file-entry request",
            FileEntryErrorCode::PathRejected => "file path rejected",
            FileEntryErrorCode::TypeRejected => "file type rejected",
            FileEntryErrorCode::ContentRejected => "file content rejected",
            FileEntryErrorCode::SourceChanged => "file changed during capture",
            FileEntryErrorCode::DestinationRejected => "export destination rejected",
            FileEntryErrorCode::Integrity => "file-entry integrity failure",
            FileEntryErrorCode::Io => "file-entry IO failure",
            FileEntryErrorCode::Conflict => "file-entry canonical conflict",
        };
        formatter.write_str(message)
    }
}

impl Error for FileEntryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_ref()
            .map(|source| source as &(dyn Error + 'static))
    }
}
