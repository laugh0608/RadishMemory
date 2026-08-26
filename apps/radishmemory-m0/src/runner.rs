use std::collections::{BTreeMap, BTreeSet};

use radishmemory_core::{
    ComponentStatus, DeletionOverallStatus, EgressPolicy, GovernedCanonicalObject,
    M0_SCHEMA_VERSION,
};
use radishmemory_sqlite::SQLITE_SCHEMA_VERSION;
use serde_json::{Map, Value, json};

use crate::error::{RunnerError, RunnerErrorCode, RunnerResult};
use crate::fixture::{FixtureSuite, Scenario, object, string};
use crate::operations;
use crate::state::ScenarioState;

#[derive(Debug)]
pub struct SuiteRun {
    report: Value,
    passed: bool,
}

impl SuiteRun {
    pub const fn passed(&self) -> bool {
        self.passed
    }

    pub const fn report(&self) -> &Value {
        &self.report
    }

    pub fn into_report(self) -> Value {
        self.report
    }
}

pub fn run_fixture(input: &str) -> RunnerResult<SuiteRun> {
    let suite = FixtureSuite::parse(input)?;
    let mut scenario_reports = Vec::new();
    let mut aggregate = BTreeMap::<String, MetricValue>::new();
    let mut suite_passed = true;
    for scenario in &suite.scenarios {
        let report = run_scenario(&suite, scenario)?;
        suite_passed &= report.passed;
        for (metric_id, value) in &report.metrics {
            aggregate
                .entry(metric_id.clone())
                .or_insert_with(|| value.zero())
                .add(value)?;
        }
        scenario_reports.push(report.value);
    }
    let gate_reports = suite
        .metric_gates
        .iter()
        .map(|gate| evaluate_gate(gate, &aggregate))
        .collect::<RunnerResult<Vec<_>>>()?;
    suite_passed &= gate_reports
        .iter()
        .all(|gate| gate.get("passed").and_then(Value::as_bool).unwrap_or(false));
    let aggregate_json = aggregate
        .iter()
        .map(|(key, value)| (key.clone(), value.to_json()))
        .collect::<Map<_, _>>();
    let report = json!({
        "adapter": {
            "id": "radishmemory-sqlite",
            "schema_version": SQLITE_SCHEMA_VERSION,
        },
        "canonical_schema_version": M0_SCHEMA_VERSION,
        "finished_at": "2026-08-26T00:01:26Z",
        "fixture_contract_version": suite.contract_version,
        "implementation_id": "radishmemory-m0",
        "implementation_version": env!("CARGO_PKG_VERSION"),
        "logical_clock": "fixture-step-order-v1",
        "metric_aggregates": aggregate_json,
        "metric_gates": gate_reports,
        "network_interceptor": {
            "mode": "no-network-capability-linked",
            "request_count": 0,
            "passed": true,
        },
        "passed": suite_passed,
        "scenarios": scenario_reports,
        "started_at": "2026-08-26T00:00:00Z",
        "suite_digest": {
            "algorithm": "sha256",
            "profile": "fixture-suite-v1",
            "value": suite.suite_digest,
        },
        "suite_id": suite.suite_id,
    });
    Ok(SuiteRun {
        report,
        passed: suite_passed,
    })
}

struct ScenarioReport {
    value: Value,
    metrics: BTreeMap<String, MetricValue>,
    passed: bool,
}

fn run_scenario(suite: &FixtureSuite, scenario: &Scenario) -> RunnerResult<ScenarioReport> {
    let mut state = ScenarioState::new(
        &scenario.scenario_id,
        &scenario.isolation_key,
        &suite.namespace_id,
        &suite.device_id,
    )?;
    let mut step_reports = Vec::new();
    let mut scenario_passed = true;
    for operation in &scenario.operations {
        let before = state.emitted.keys().cloned().collect::<BTreeSet<_>>();
        let outcome = operations::dispatch(&mut state, operation)
            .map_err(|error| error.at_step(&scenario.scenario_id, &operation.step_id))?;
        let assertion_results = operation
            .assertions
            .iter()
            .map(|assertion| {
                let passed = outcome.assertions.get(assertion.as_str()).copied();
                let Some(passed) = passed else {
                    return Err(RunnerError::new(
                        RunnerErrorCode::InvalidFixture,
                        "assertion-code-unsupported",
                    ));
                };
                Ok(json!({
                    "assertion_id": assertion,
                    "passed": passed,
                }))
            })
            .collect::<RunnerResult<Vec<_>>>()?;
        let step_passed = outcome.status == operation.expected_status
            && assertion_results.iter().all(|result| {
                result
                    .get("passed")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            });
        scenario_passed &= step_passed;
        let emitted = state
            .emitted
            .iter()
            .filter(|(key, _)| !before.contains(*key))
            .map(|(key, (object_id, digest))| {
                let mut value = json!({
                    "logical_key": key,
                    "object_id": object_id,
                });
                if let Some((profile, digest)) = digest {
                    value.as_object_mut().expect("JSON object").insert(
                        "digest".to_owned(),
                        json!({
                            "algorithm": "sha256",
                            "profile": profile,
                            "value": digest,
                        }),
                    );
                }
                value
            })
            .collect::<Vec<_>>();
        let mut step = json!({
            "assertions": assertion_results,
            "emitted": emitted,
            "expected_status": operation.expected_status,
            "observed_status": outcome.status,
            "operation": operation.name,
            "passed": step_passed,
            "step_id": operation.step_id,
        });
        if let Some(error_code) = outcome.error_code {
            let object = step.as_object_mut().expect("JSON object");
            object.insert("error_code".to_owned(), Value::String(error_code));
            object.insert(
                "retryable".to_owned(),
                Value::Bool(outcome.retryable.unwrap_or(false)),
            );
        }
        step_reports.push(step);
    }
    let mut metrics = BTreeMap::new();
    let mut metric_reports = Vec::new();
    for oracle in &scenario.metric_oracles {
        let oracle_object = object(oracle)?;
        let metric_id = string(oracle_object, "metric_id")?;
        let actual = actual_metric(metric_id, &state, scenario_passed)?;
        let expected = MetricValue::from_observation(oracle_object)?;
        let passed = actual == expected;
        scenario_passed &= passed;
        metrics.insert(metric_id.to_owned(), actual.clone());
        metric_reports.push(json!({
            "metric_id": metric_id,
            "passed": passed,
            "value": actual.to_json(),
        }));
    }
    Ok(ScenarioReport {
        value: json!({
            "metric_observations": metric_reports,
            "passed": scenario_passed,
            "scenario_id": scenario.scenario_id,
            "steps": step_reports,
        }),
        metrics,
        passed: scenario_passed,
    })
}

fn actual_metric(
    metric_id: &str,
    state: &ScenarioState,
    steps_passed: bool,
) -> RunnerResult<MetricValue> {
    let value = match metric_id {
        "citation_resolve_rate" => MetricValue::Ratio(
            state.metrics.citation_numerator,
            state.metrics.citation_denominator,
        ),
        "retrieval_recall_at_5" => MetricValue::Ratio(
            state.metrics.retrieval_numerator,
            state.metrics.retrieval_denominator,
        ),
        "unconfirmed_context_count" => {
            let proposal_ids = state
                .proposals
                .values()
                .map(|proposal| proposal.params().proposal_id.as_str())
                .collect::<BTreeSet<_>>();
            let count = state
                .contexts
                .values()
                .flat_map(|pack| pack.params().items.iter())
                .flat_map(|item| item.params().object_refs.iter())
                .filter(|reference| proposal_ids.contains(reference.object_id().as_str()))
                .count() as u64;
            MetricValue::Count(count)
        }
        "duplicate_reproposal_count" => {
            MetricValue::Count(state.metrics.duplicate_reproposal_count)
        }
        "silent_overwrite_count" => MetricValue::Count(state.metrics.silent_overwrite_count),
        "silent_conflict_selection_count" => {
            let selected = state
                .conflicts
                .values()
                .flatten()
                .filter_map(|key| state.proposals.get(key))
                .filter(|proposal| {
                    state.records.values().any(|record| {
                        record.params().origin_proposal_id == proposal.params().proposal_id
                            && record.params().current_state
                                == radishmemory_core::MemoryState::Confirmed
                    })
                })
                .count() as u64;
            MetricValue::Count(selected)
        }
        "policy_violation_count" => {
            let violations = state
                .sources
                .values()
                .filter(|source| source.governance().egress_policy() != EgressPolicy::LocalOnly)
                .count()
                + state
                    .records
                    .values()
                    .filter(|record| record.governance().egress_policy() != EgressPolicy::LocalOnly)
                    .count()
                + state
                    .contexts
                    .values()
                    .filter(|pack| pack.governance().egress_policy() != EgressPolicy::LocalOnly)
                    .count();
            MetricValue::Count(violations as u64)
        }
        "network_request_count" => MetricValue::Count(state.network_request_count),
        "deletion_component_coverage" => MetricValue::Ratio(
            state.metrics.deletion_numerator,
            state.metrics.deletion_denominator,
        ),
        "false_complete_deletion_count" => {
            let count = state
                .evidences
                .values()
                .filter(|evidence| {
                    evidence.params().overall_status == DeletionOverallStatus::Completed
                        && evidence
                            .params()
                            .component_results
                            .iter()
                            .any(|result| result.params().status != ComponentStatus::Succeeded)
                })
                .count() as u64;
            MetricValue::Count(count)
        }
        "model_free_loop_completion_rate" => MetricValue::Ratio(u64::from(steps_passed), 1),
        "relevant_source_set_drift_count" => {
            MetricValue::Count(state.metrics.relevant_source_set_drift_count)
        }
        _ => {
            return Err(RunnerError::new(
                RunnerErrorCode::InvalidFixture,
                "metric-code-unsupported",
            ));
        }
    };
    Ok(value)
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MetricValue {
    Count(u64),
    Ratio(u64, u64),
}

impl MetricValue {
    fn from_observation(object: &Map<String, Value>) -> RunnerResult<Self> {
        if let Some(value) = object.get("value").and_then(Value::as_u64) {
            Ok(Self::Count(value))
        } else {
            Ok(Self::Ratio(
                object
                    .get("numerator")
                    .and_then(Value::as_u64)
                    .ok_or_else(invalid_metric)?,
                object
                    .get("denominator")
                    .and_then(Value::as_u64)
                    .ok_or_else(invalid_metric)?,
            ))
        }
    }

    const fn zero(&self) -> Self {
        match self {
            Self::Count(_) => Self::Count(0),
            Self::Ratio(_, _) => Self::Ratio(0, 0),
        }
    }

    fn add(&mut self, other: &Self) -> RunnerResult<()> {
        match (self, other) {
            (Self::Count(left), Self::Count(right)) => *left += right,
            (Self::Ratio(left_n, left_d), Self::Ratio(right_n, right_d)) => {
                *left_n += right_n;
                *left_d += right_d;
            }
            _ => return Err(invalid_metric()),
        }
        Ok(())
    }

    fn to_json(&self) -> Value {
        match self {
            Self::Count(value) => json!({"value": value}),
            Self::Ratio(numerator, denominator) => {
                json!({"denominator": denominator, "numerator": numerator})
            }
        }
    }
}

fn evaluate_gate(gate: &Value, aggregate: &BTreeMap<String, MetricValue>) -> RunnerResult<Value> {
    let gate = object(gate)?;
    let metric_id = string(gate, "metric_id")?;
    if string(gate, "comparator")? != "eq" {
        return Err(invalid_metric());
    }
    let actual = aggregate.get(metric_id).ok_or_else(invalid_metric)?;
    let expected = match string(gate, "kind")? {
        "count" => MetricValue::Count(
            gate.get("threshold")
                .and_then(Value::as_u64)
                .ok_or_else(invalid_metric)?,
        ),
        "ratio" => {
            let threshold = gate
                .get("threshold")
                .and_then(Value::as_object)
                .ok_or_else(invalid_metric)?;
            MetricValue::Ratio(
                threshold
                    .get("numerator")
                    .and_then(Value::as_u64)
                    .ok_or_else(invalid_metric)?,
                threshold
                    .get("denominator")
                    .and_then(Value::as_u64)
                    .ok_or_else(invalid_metric)?,
            )
        }
        _ => return Err(invalid_metric()),
    };
    let passed = match (actual, &expected) {
        (MetricValue::Count(actual), MetricValue::Count(expected)) => actual == expected,
        (MetricValue::Ratio(actual_n, actual_d), MetricValue::Ratio(expected_n, expected_d)) => {
            *actual_d != 0
                && *expected_d != 0
                && u128::from(*actual_n) * u128::from(*expected_d)
                    == u128::from(*expected_n) * u128::from(*actual_d)
        }
        _ => false,
    };
    Ok(json!({
        "actual": actual.to_json(),
        "metric_id": metric_id,
        "passed": passed,
        "threshold": expected.to_json(),
    }))
}

fn invalid_metric() -> RunnerError {
    RunnerError::new(RunnerErrorCode::InvalidFixture, "metric-shape-invalid")
}
