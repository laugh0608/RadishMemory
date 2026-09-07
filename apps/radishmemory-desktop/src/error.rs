use std::error::Error;
use std::fmt;
use std::io;

use radishmemory_application::{
    ApplicationError, ApplicationErrorCode, ApplicationErrorReason, ApplicationOperation,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopErrorCode {
    ApplicationDirectory,
    HostProfile,
    Runtime,
    LocalLibrary,
    Picker,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopErrorReason {
    ProjectDirectoryUnavailable,
    DataDirectoryInvalid,
    DataDirectoryCreateFailed,
    DatabasePathInvalid,
    ProfileMissingForExistingDatabase,
    ProfileInvalid,
    ProfileReadFailed,
    ProfileWriteFailed,
    IdentityGenerationFailed,
    ClockFailed,
    ApplicationFailed,
    SelectionInvalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApplicationFailureSummary {
    operation: ApplicationOperation,
    code: ApplicationErrorCode,
    reason: ApplicationErrorReason,
    retryable: bool,
}

impl ApplicationFailureSummary {
    #[must_use]
    pub const fn operation(self) -> ApplicationOperation {
        self.operation
    }

    #[must_use]
    pub const fn code(self) -> ApplicationErrorCode {
        self.code
    }

    #[must_use]
    pub const fn reason(self) -> ApplicationErrorReason {
        self.reason
    }

    #[must_use]
    pub const fn retryable(self) -> bool {
        self.retryable
    }
}

pub struct DesktopError {
    code: DesktopErrorCode,
    reason: DesktopErrorReason,
    retryable: bool,
    os_error_code: Option<i32>,
    application: Option<ApplicationFailureSummary>,
}

impl DesktopError {
    pub(crate) const fn without_source(
        code: DesktopErrorCode,
        reason: DesktopErrorReason,
        retryable: bool,
    ) -> Self {
        Self {
            code,
            reason,
            retryable,
            os_error_code: None,
            application: None,
        }
    }

    pub(crate) fn io(
        code: DesktopErrorCode,
        reason: DesktopErrorReason,
        retryable: bool,
        source: &io::Error,
    ) -> Self {
        Self {
            code,
            reason,
            retryable,
            os_error_code: source.raw_os_error(),
            application: None,
        }
    }

    pub(crate) fn application(source: &ApplicationError) -> Self {
        Self {
            code: DesktopErrorCode::LocalLibrary,
            reason: DesktopErrorReason::ApplicationFailed,
            retryable: source.retryable(),
            os_error_code: None,
            application: Some(ApplicationFailureSummary {
                operation: source.operation(),
                code: source.code(),
                reason: source.reason(),
                retryable: source.retryable(),
            }),
        }
    }

    #[must_use]
    pub const fn code(&self) -> DesktopErrorCode {
        self.code
    }

    #[must_use]
    pub const fn reason(&self) -> DesktopErrorReason {
        self.reason
    }

    #[must_use]
    pub const fn retryable(&self) -> bool {
        self.retryable
    }

    #[must_use]
    pub const fn os_error_code(&self) -> Option<i32> {
        self.os_error_code
    }

    #[must_use]
    pub const fn application_failure(&self) -> Option<ApplicationFailureSummary> {
        self.application
    }
}

impl fmt::Debug for DesktopError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DesktopError")
            .field("code", &self.code)
            .field("reason", &self.reason)
            .field("retryable", &self.retryable)
            .field("os_error_code", &self.os_error_code)
            .field("application", &self.application)
            .finish()
    }
}

impl fmt::Display for DesktopError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.code {
            DesktopErrorCode::ApplicationDirectory => "local application directory is unavailable",
            DesktopErrorCode::HostProfile => "local host profile is unavailable",
            DesktopErrorCode::Runtime => "local desktop runtime failed",
            DesktopErrorCode::LocalLibrary => "local library operation failed",
            DesktopErrorCode::Picker => "local file selection failed",
        })
    }
}

impl Error for DesktopError {}
