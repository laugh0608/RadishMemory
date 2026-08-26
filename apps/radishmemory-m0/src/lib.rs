mod error;
mod fixture;
mod operations;
mod runner;
mod state;

pub use error::{RunnerError, RunnerErrorCode};
pub use fixture::EMBEDDED_FIXTURE;
pub use runner::{SuiteRun, run_fixture};

pub fn run_embedded_suite() -> Result<SuiteRun, RunnerError> {
    run_fixture(EMBEDDED_FIXTURE)
}

pub fn error_report(error: &RunnerError) -> serde_json::Value {
    let mut report = serde_json::json!({
        "error_code": error.code().as_str(),
        "error_detail_code": error.detail_code(),
        "implementation_id": "radishmemory-m0",
        "passed": false,
    });
    if let Some(scenario_id) = error.scenario_id() {
        report
            .as_object_mut()
            .expect("error report is an object")
            .insert("scenario_id".to_owned(), scenario_id.into());
    }
    if let Some(step_id) = error.step_id() {
        report
            .as_object_mut()
            .expect("error report is an object")
            .insert("step_id".to_owned(), step_id.into());
    }
    report
}
