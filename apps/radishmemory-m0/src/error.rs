use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunnerErrorCode {
    InvalidFixture,
    UnsupportedOperation,
    OperationFailed,
    Storage,
}

impl RunnerErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidFixture => "invalid-fixture",
            Self::UnsupportedOperation => "unsupported-operation",
            Self::OperationFailed => "operation-failed",
            Self::Storage => "storage-failed",
        }
    }
}

pub struct RunnerError {
    code: RunnerErrorCode,
    detail_code: &'static str,
    has_source: bool,
    scenario_id: Option<String>,
    step_id: Option<String>,
}

impl RunnerError {
    pub const fn new(code: RunnerErrorCode, detail_code: &'static str) -> Self {
        Self {
            code,
            detail_code,
            has_source: false,
            scenario_id: None,
            step_id: None,
        }
    }

    pub fn with_source(
        code: RunnerErrorCode,
        detail_code: &'static str,
        _source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            code,
            detail_code,
            has_source: true,
            scenario_id: None,
            step_id: None,
        }
    }

    pub const fn code(&self) -> RunnerErrorCode {
        self.code
    }

    pub const fn detail_code(&self) -> &'static str {
        self.detail_code
    }

    pub fn at_step(mut self, scenario_id: &str, step_id: &str) -> Self {
        self.scenario_id = Some(scenario_id.to_owned());
        self.step_id = Some(step_id.to_owned());
        self
    }

    pub fn scenario_id(&self) -> Option<&str> {
        self.scenario_id.as_deref()
    }

    pub fn step_id(&self) -> Option<&str> {
        self.step_id.as_deref()
    }
}

impl fmt::Display for RunnerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code.as_str())
    }
}

impl fmt::Debug for RunnerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RunnerError")
            .field("code", &self.code)
            .field("detail_code", &self.detail_code)
            .field("has_source", &self.has_source)
            .field("scenario_id", &self.scenario_id)
            .field("step_id", &self.step_id)
            .finish()
    }
}

impl Error for RunnerError {}

pub type RunnerResult<T> = Result<T, RunnerError>;
