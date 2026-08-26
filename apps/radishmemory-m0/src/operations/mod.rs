mod context_search;
mod deletion;
mod source_memory;

use std::collections::BTreeMap;

use serde_json::Value;

use crate::error::RunnerResult;
use crate::fixture::Operation;
use crate::state::ScenarioState;

#[derive(Clone)]
pub struct OperationOutcome {
    pub status: &'static str,
    pub assertions: BTreeMap<&'static str, bool>,
    pub error_code: Option<String>,
    pub retryable: Option<bool>,
}

impl OperationOutcome {
    pub fn succeeded(assertions: impl IntoIterator<Item = (&'static str, bool)>) -> Self {
        Self {
            status: "succeeded",
            assertions: assertions.into_iter().collect(),
            error_code: None,
            retryable: None,
        }
    }

    pub fn with_status(mut self, status: &'static str) -> Self {
        self.status = status;
        self
    }

    pub fn with_error(mut self, error_code: String, retryable: bool) -> Self {
        self.error_code = Some(error_code);
        self.retryable = Some(retryable);
        self
    }
}

pub fn dispatch(
    state: &mut ScenarioState,
    operation: &Operation,
) -> RunnerResult<OperationOutcome> {
    match operation.name.as_str() {
        "capture" => source_memory::capture(state, &operation.input),
        "segment" => source_memory::segment(state, &operation.input),
        "propose" => source_memory::propose(state, &operation.input),
        "decide" => source_memory::decide(state, &operation.input),
        "materialize_memory" => source_memory::materialize_memory(state, &operation.input),
        "attempt_duplicate_proposal" => {
            source_memory::attempt_duplicate_proposal(state, &operation.input)
        }
        "search" => context_search::search(state, &operation.step_id, &operation.input),
        "query_at" => context_search::query_at(state, &operation.step_id, &operation.input),
        "query_current" => {
            context_search::query_current(state, &operation.step_id, &operation.input)
        }
        "detect_conflict" => {
            context_search::detect_conflict(state, &operation.step_id, &operation.input)
        }
        "compile_context" => {
            context_search::compile_context(state, &operation.step_id, &operation.input)
        }
        "seed_noise" => context_search::seed_noise(state, &operation.input),
        "compare_source_set" => context_search::compare_source_set(state, &operation.input),
        "assert_environment" => assert_environment(&operation.input),
        "assert_no_network" => assert_no_network(state, &operation.input),
        "plan_delete" => deletion::plan_delete(state, &operation.input),
        "execute_deletion" => deletion::execute_deletion(state, &operation.input),
        "emit_deletion_evidence" => deletion::emit_deletion_evidence(state, &operation.input),
        _ => Err(crate::error::RunnerError::new(
            crate::error::RunnerErrorCode::UnsupportedOperation,
            "operation-name-unsupported",
        )),
    }
}

fn assert_environment(input: &Value) -> RunnerResult<OperationOutcome> {
    let input = crate::fixture::object(input)?;
    let model_absent = !crate::fixture::bool_value(input, "model_configured")?;
    let provider_absent = !crate::fixture::bool_value(input, "provider_key_configured")?;
    let network_blocked = crate::fixture::string(input, "network_mode")? == "blocked";
    Ok(OperationOutcome::succeeded([
        ("model-absent", model_absent),
        ("provider-key-absent", provider_absent),
        ("network-blocked", network_blocked),
    ]))
}

fn assert_no_network(state: &ScenarioState, input: &Value) -> RunnerResult<OperationOutcome> {
    let input = crate::fixture::object(input)?;
    let expected_requests = crate::fixture::u64_value(input, "expected_request_count")?;
    let expected_manifests = crate::fixture::u64_value(input, "expected_manifest_count")?;
    let expected_traces = crate::fixture::u64_value(input, "expected_provider_trace_count")?;
    let expected_usage = crate::fixture::u64_value(input, "expected_usage_record_count")?;
    Ok(OperationOutcome::succeeded([
        (
            "network-request-count-zero",
            state.network_request_count == expected_requests && expected_requests == 0,
        ),
        (
            "provider-artifacts-absent",
            state.provider_artifact_count == expected_manifests + expected_traces + expected_usage
                && expected_manifests + expected_traces + expected_usage == 0,
        ),
    ]))
}
