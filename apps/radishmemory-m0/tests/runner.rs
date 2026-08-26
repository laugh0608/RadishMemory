use std::process::Command;

use radishmemory_core::compute_digest;
use radishmemory_m0::{EMBEDDED_FIXTURE, RunnerErrorCode, run_embedded_suite, run_fixture};
use radishmemory_sqlite as _;
use serde_json::{Value, json};

#[test]
fn embedded_suite_executes_all_real_steps_and_gates_without_content_leakage() {
    let run = run_embedded_suite().expect("embedded fixture must execute");
    assert!(run.passed());
    let report = run.report();
    let scenarios = report["scenarios"]
        .as_array()
        .expect("scenarios must be an array");
    assert_eq!(scenarios.len(), 12);
    assert_eq!(
        scenarios
            .iter()
            .map(|scenario| scenario["steps"].as_array().expect("steps array").len())
            .sum::<usize>(),
        86
    );
    assert!(
        report["metric_gates"]
            .as_array()
            .expect("gate array")
            .iter()
            .all(|gate| gate["passed"] == true)
    );
    let encoded = serde_json::to_string(report).expect("report must encode");
    for forbidden in [
        "Project Orchard default theme is blue.",
        "Use concise Chinese explanations.",
        "Synthetic note scheduled for deletion.",
        "radishmemory-m0-",
        ".sqlite3",
    ] {
        assert!(!encoded.contains(forbidden));
    }
}

#[test]
fn report_is_deterministic_across_isolated_runs() {
    let first = run_embedded_suite().expect("first run must execute");
    let second = run_embedded_suite().expect("second run must execute");
    assert_eq!(first.report(), second.report());
}

#[test]
fn unknown_operation_and_assertion_fail_closed_with_stable_step_context() {
    let unknown_operation = mutate_fixture(|root| {
        root["scenarios"][0]["operations"][0]["op"] = json!("unknown-operation");
    });
    let error = run_fixture(&unknown_operation).expect_err("unknown operation must fail");
    assert_eq!(error.code(), RunnerErrorCode::UnsupportedOperation);
    assert_eq!(error.detail_code(), "operation-name-unsupported");
    assert_eq!(error.scenario_id(), Some("M0-E01"));
    assert_eq!(error.step_id(), Some("m0-e01-s01"));

    let unknown_assertion = mutate_fixture(|root| {
        root["scenarios"][0]["operations"][0]["expect"]["assertions"][0] =
            json!("unknown-assertion");
    });
    let error = run_fixture(&unknown_assertion).expect_err("unknown assertion must fail");
    assert_eq!(error.code(), RunnerErrorCode::InvalidFixture);
    assert_eq!(error.detail_code(), "assertion-code-unsupported");
}

#[test]
fn binary_emits_passing_json_and_expected_failure_evidence() {
    let output = Command::new(env!("CARGO_BIN_EXE_radishmemory-m0"))
        .output()
        .expect("runner binary must start");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: Value = serde_json::from_slice(&output.stdout).expect("stdout must be JSON");
    assert_eq!(report["passed"], true);
    let failed_step = report["scenarios"]
        .as_array()
        .expect("scenario array")
        .iter()
        .find(|scenario| scenario["scenario_id"] == "M0-E10")
        .and_then(|scenario| scenario["steps"].as_array())
        .and_then(|steps| steps.iter().find(|step| step["step_id"] == "m0-e10-s04"))
        .expect("expected-failure step must exist");
    assert_eq!(failed_step["expected_status"], "failed");
    assert_eq!(failed_step["observed_status"], "failed");
    assert_eq!(failed_step["error_code"], "fixture-index-delete-failed");
    assert_eq!(failed_step["retryable"], true);
    assert_eq!(failed_step["passed"], true);
}

fn mutate_fixture(change: impl FnOnce(&mut Value)) -> String {
    let mut root: Value = serde_json::from_str(EMBEDDED_FIXTURE).expect("fixture must parse");
    change(&mut root);
    root.as_object_mut()
        .expect("fixture root")
        .remove("suite_digest");
    let digest = compute_digest("fixture-suite-v1", &root.to_string())
        .expect("mutated suite digest must compute");
    root.as_object_mut().expect("fixture root").insert(
        "suite_digest".to_owned(),
        json!({
            "algorithm": "sha256",
            "profile": "fixture-suite-v1",
            "value": digest.value(),
        }),
    );
    root.to_string()
}
