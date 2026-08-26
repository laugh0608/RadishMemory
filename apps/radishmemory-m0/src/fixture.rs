use radishmemory_core::{compute_digest, compute_exact_bytes_digest, compute_nfc_text_digest};
use serde_json::{Map, Value};

use crate::error::{RunnerError, RunnerErrorCode, RunnerResult};

pub const EMBEDDED_FIXTURE: &str = include_str!("../../../fixtures/m0/local-memory-loop.v1.json");

#[derive(Clone)]
pub struct Operation {
    pub step_id: String,
    pub name: String,
    pub input: Value,
    pub expected_status: String,
    pub assertions: Vec<String>,
}

#[derive(Clone)]
pub struct Scenario {
    pub scenario_id: String,
    pub isolation_key: String,
    pub operations: Vec<Operation>,
    pub metric_oracles: Vec<Value>,
}

pub struct FixtureSuite {
    pub contract_version: String,
    pub suite_id: String,
    pub suite_digest: String,
    pub namespace_id: String,
    pub device_id: String,
    pub scenarios: Vec<Scenario>,
    pub metric_gates: Vec<Value>,
}

impl FixtureSuite {
    pub fn parse(input: &str) -> RunnerResult<Self> {
        let mut root: Value = serde_json::from_str(input).map_err(|source| {
            RunnerError::with_source(
                RunnerErrorCode::InvalidFixture,
                "fixture-json-invalid",
                source,
            )
        })?;
        let object = root.as_object_mut().ok_or_else(invalid_fixture)?;
        require_exact(
            object,
            "fixture_contract_version",
            "radishmemory.m0-fixture/1",
        )?;
        require_exact(object, "canonical_schema_version", "radishmemory.m0/1")?;
        require_exact(object, "data_classification", "synthetic")?;
        require_exact(
            object,
            "canonical_json_profile",
            "radishmemory-canonical-json-v1",
        )?;
        require_exact(object, "fixture_id_profile", "radishmemory-fixture-id-v1")?;

        let digest_object = object
            .get("suite_digest")
            .and_then(Value::as_object)
            .ok_or_else(invalid_fixture)?;
        require_exact(digest_object, "algorithm", "sha256")?;
        require_exact(digest_object, "profile", "fixture-suite-v1")?;
        let stored_digest = string(digest_object, "value")?.to_owned();
        object.remove("suite_digest");
        let actual_digest =
            compute_digest("fixture-suite-v1", &root.to_string()).map_err(|source| {
                RunnerError::with_source(
                    RunnerErrorCode::InvalidFixture,
                    "suite-digest-input-invalid",
                    source,
                )
            })?;
        if actual_digest.value() != stored_digest {
            return Err(RunnerError::new(
                RunnerErrorCode::InvalidFixture,
                "suite-digest-mismatch",
            ));
        }

        let object = root.as_object().ok_or_else(invalid_fixture)?;
        validate_id_vectors(array(object, "id_vectors")?)
            .map_err(|error| refine_shape_error(error, "fixture-id-vector-shape-invalid"))?;
        validate_digest_vectors(array(object, "digest_vectors")?)
            .map_err(|error| refine_shape_error(error, "digest-vector-shape-invalid"))?;
        let scenarios = array(object, "scenarios")?
            .iter()
            .map(parse_scenario)
            .collect::<RunnerResult<Vec<_>>>()
            .map_err(|error| refine_shape_error(error, "scenario-shape-invalid"))?;
        if scenarios.len() != 12 {
            return Err(invalid_fixture());
        }
        let step_count = scenarios
            .iter()
            .map(|scenario| scenario.operations.len())
            .sum::<usize>();
        if step_count != 86 {
            return Err(invalid_fixture());
        }
        Ok(Self {
            contract_version: string(object, "fixture_contract_version")?.to_owned(),
            suite_id: string(object, "suite_id")?.to_owned(),
            suite_digest: stored_digest,
            namespace_id: string(object, "namespace_id")?.to_owned(),
            device_id: string(object, "device_id")?.to_owned(),
            scenarios,
            metric_gates: array(object, "metric_gates")?.to_vec(),
        })
    }
}

fn parse_scenario(value: &Value) -> RunnerResult<Scenario> {
    let object = value.as_object().ok_or_else(invalid_fixture)?;
    let operations = array(object, "operations")?
        .iter()
        .map(|operation| {
            let operation = operation.as_object().ok_or_else(invalid_fixture)?;
            let expect = operation
                .get("expect")
                .and_then(Value::as_object)
                .ok_or_else(invalid_fixture)?;
            Ok(Operation {
                step_id: string(operation, "step_id")?.to_owned(),
                name: string(operation, "op")?.to_owned(),
                input: operation
                    .get("input")
                    .cloned()
                    .ok_or_else(invalid_fixture)?,
                expected_status: string(expect, "status")?.to_owned(),
                assertions: strings(expect, "assertions")?,
            })
        })
        .collect::<RunnerResult<Vec<_>>>()?;
    Ok(Scenario {
        scenario_id: string(object, "scenario_id")?.to_owned(),
        isolation_key: string(object, "isolation_key")?.to_owned(),
        operations,
        metric_oracles: array(object, "metric_observations")?.to_vec(),
    })
}

fn validate_id_vectors(values: &[Value]) -> RunnerResult<()> {
    for value in values {
        let object = value.as_object().ok_or_else(invalid_fixture)?;
        let actual = stable_fixture_id(
            string(object, "scenario_id")?,
            string(object, "object_type")?,
            string(object, "logical_key")?,
        )?;
        if actual != string(object, "expected_id")? {
            return Err(RunnerError::new(
                RunnerErrorCode::InvalidFixture,
                "fixture-id-vector-mismatch",
            ));
        }
    }
    Ok(())
}

fn validate_digest_vectors(values: &[Value]) -> RunnerResult<()> {
    for value in values {
        let object = value.as_object().ok_or_else(invalid_fixture)?;
        let profile = string(object, "profile")?;
        let actual = match profile {
            "exact-bytes-v1" => {
                compute_exact_bytes_digest(string(object, "input_text")?.as_bytes())
            }
            "utf8-nfc-text-v1" => compute_nfc_text_digest(string(object, "input_text")?),
            "canonical-json-v1" => compute_digest(
                "canonical-json-v1",
                &object
                    .get("input_value")
                    .ok_or_else(invalid_fixture)?
                    .to_string(),
            )
            .map_err(|source| {
                RunnerError::with_source(
                    RunnerErrorCode::InvalidFixture,
                    "digest-vector-input-invalid",
                    source,
                )
            })?,
            _ => return Err(invalid_fixture()),
        };
        if actual.value() != string(object, "expected_sha256")? {
            return Err(RunnerError::new(
                RunnerErrorCode::InvalidFixture,
                "digest-vector-mismatch",
            ));
        }
    }
    Ok(())
}

pub fn stable_fixture_id(
    scenario_id: &str,
    object_type: &str,
    logical_key: &str,
) -> RunnerResult<String> {
    let object_type = match object_type {
        "SourceArtifact" => "source-artifact",
        "SourceFragment" => "source-fragment",
        "MemoryProposal" => "memory-proposal",
        "MemoryDecision" => "memory-decision",
        "MemoryRecord" => "memory-record",
        "MemoryStateEvent" => "memory-state-event",
        "ContextPack" => "context-pack",
        "DeleteRequest" => "delete-request",
        "DeletionEvidence" => "deletion-evidence",
        _ => return Err(invalid_fixture()),
    };
    Ok(format!(
        "urn:radishmemory:fixture:{}:{object_type}:{logical_key}",
        scenario_id.to_ascii_lowercase()
    ))
}

pub fn object(value: &Value) -> RunnerResult<&Map<String, Value>> {
    value.as_object().ok_or_else(invalid_fixture)
}

pub fn string<'a>(object: &'a Map<String, Value>, key: &str) -> RunnerResult<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(invalid_fixture)
}

pub fn optional_string<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    object.get(key).and_then(Value::as_str)
}

pub fn array<'a>(object: &'a Map<String, Value>, key: &str) -> RunnerResult<&'a [Value]> {
    object
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(invalid_fixture)
}

pub fn optional_array<'a>(object: &'a Map<String, Value>, key: &str) -> &'a [Value] {
    object
        .get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

pub fn strings(object: &Map<String, Value>, key: &str) -> RunnerResult<Vec<String>> {
    array(object, key)?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(invalid_fixture)
        })
        .collect()
}

pub fn optional_strings(object: &Map<String, Value>, key: &str) -> RunnerResult<Vec<String>> {
    optional_array(object, key)
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(invalid_fixture)
        })
        .collect()
}

pub fn u64_value(object: &Map<String, Value>, key: &str) -> RunnerResult<u64> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(invalid_fixture)
}

pub fn bool_value(object: &Map<String, Value>, key: &str) -> RunnerResult<bool> {
    object
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(invalid_fixture)
}

fn require_exact(object: &Map<String, Value>, key: &str, expected: &str) -> RunnerResult<()> {
    if string(object, key)? == expected {
        Ok(())
    } else {
        Err(invalid_fixture())
    }
}

pub fn invalid_fixture() -> RunnerError {
    RunnerError::new(RunnerErrorCode::InvalidFixture, "fixture-shape-invalid")
}

fn refine_shape_error(error: RunnerError, detail_code: &'static str) -> RunnerError {
    if error.detail_code() == "fixture-shape-invalid" {
        RunnerError::new(RunnerErrorCode::InvalidFixture, detail_code)
    } else {
        error
    }
}
