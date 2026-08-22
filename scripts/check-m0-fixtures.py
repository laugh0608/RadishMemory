#!/usr/bin/env python3
"""Validate the dependency-free M0 fixture contract and metric oracle."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import unicodedata
from decimal import Decimal
from fractions import Fraction
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_FIXTURE = REPO_ROOT / "fixtures/m0/local-memory-loop.v1.json"

FIXTURE_CONTRACT_VERSION = "radishmemory.m0-fixture/1"
CANONICAL_SCHEMA_VERSION = "radishmemory.m0/1"
CANONICAL_JSON_PROFILE = "radishmemory-canonical-json-v1"
FIXTURE_ID_PROFILE = "radishmemory-fixture-id-v1"

EXPECTED_SCENARIOS = tuple(f"M0-E{number:02d}" for number in range(1, 13))
EXPECTED_OPERATIONS = {
    "M0-E01": ("capture", "segment"),
    "M0-E02": ("capture", "segment", "search", "compile_context"),
    "M0-E03": ("capture", "segment", "propose", "search", "compile_context"),
    "M0-E04": (
        "capture",
        "segment",
        "propose",
        "decide",
        "materialize_memory",
        "search",
        "compile_context",
    ),
    "M0-E05": (
        "capture",
        "segment",
        "propose",
        "decide",
        "attempt_duplicate_proposal",
    ),
    "M0-E06": (
        "capture",
        "segment",
        "propose",
        "decide",
        "materialize_memory",
        "capture",
        "segment",
        "propose",
        "decide",
        "materialize_memory",
        "query_at",
        "query_current",
    ),
    "M0-E07": (
        "capture",
        "segment",
        "propose",
        "capture",
        "segment",
        "propose",
        "detect_conflict",
        "compile_context",
    ),
    "M0-E08": (
        "capture",
        "segment",
        "search",
        "compile_context",
        "assert_no_network",
    ),
    "M0-E09": (
        "capture",
        "segment",
        "propose",
        "decide",
        "materialize_memory",
        "plan_delete",
        "execute_deletion",
        "emit_deletion_evidence",
        "search",
    ),
    "M0-E10": (
        "capture",
        "segment",
        "plan_delete",
        "execute_deletion",
        "emit_deletion_evidence",
    ),
    "M0-E11": (
        "assert_environment",
        "capture",
        "segment",
        "propose",
        "decide",
        "materialize_memory",
        "search",
        "compile_context",
        "capture",
        "segment",
        "propose",
        "decide",
        "materialize_memory",
        "query_current",
        "plan_delete",
        "execute_deletion",
        "emit_deletion_evidence",
        "assert_no_network",
    ),
    "M0-E12": (
        "capture",
        "segment",
        "search",
        "seed_noise",
        "search",
        "compare_source_set",
    ),
}

OBJECT_TYPE_SLUGS = {
    "SourceArtifact": "source-artifact",
    "SourceFragment": "source-fragment",
    "MemoryProposal": "memory-proposal",
    "MemoryDecision": "memory-decision",
    "MemoryRecord": "memory-record",
    "MemoryStateEvent": "memory-state-event",
    "ContextPack": "context-pack",
    "DeleteRequest": "delete-request",
    "DeletionEvidence": "deletion-evidence",
}

EXPECTED_METRIC_GATES: dict[str, tuple[str, int | Fraction]] = {
    "citation_resolve_rate": ("ratio", Fraction(1, 1)),
    "retrieval_recall_at_5": ("ratio", Fraction(1, 1)),
    "unconfirmed_context_count": ("count", 0),
    "duplicate_reproposal_count": ("count", 0),
    "silent_overwrite_count": ("count", 0),
    "silent_conflict_selection_count": ("count", 0),
    "policy_violation_count": ("count", 0),
    "network_request_count": ("count", 0),
    "deletion_component_coverage": ("ratio", Fraction(1, 1)),
    "false_complete_deletion_count": ("count", 0),
    "model_free_loop_completion_rate": ("ratio", Fraction(1, 1)),
    "relevant_source_set_drift_count": ("count", 0),
}

EXPECTED_COMPONENT_TYPES = {
    "source_body",
    "source_metadata",
    "source_fragment",
    "memory_proposal",
    "memory_decision",
    "memory_record",
    "memory_state_event",
    "full_text_index",
    "context_cache",
    "minimal_audit",
}

TOP_LEVEL_FIELDS = {
    "fixture_contract_version",
    "canonical_schema_version",
    "suite_id",
    "data_classification",
    "namespace_id",
    "device_id",
    "canonical_json_profile",
    "fixture_id_profile",
    "governance_profiles",
    "deletion_profiles",
    "id_vectors",
    "digest_vectors",
    "metric_gates",
    "scenarios",
    "suite_digest",
}

CANONICAL_INTEGER = re.compile(r"-?(?:0|[1-9][0-9]*)$")
CANONICAL_FRACTION = re.compile(r"-?(?:0|[1-9][0-9]*)\.[0-9]*[1-9]$")
LOGICAL_KEY = re.compile(r"[a-z0-9]+(?:-[a-z0-9]+)*$")
ASSERTION_CODE = re.compile(r"[a-z0-9]+(?:-[a-z0-9]+)*$")
TIMESTAMP = re.compile(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$")


class FixtureError(ValueError):
    """Raised for fixture contract violations."""


def parse_integer(text: str) -> int:
    if not CANONICAL_INTEGER.fullmatch(text) or text == "-0":
        raise FixtureError(f"non-canonical JSON integer: {text}")
    return int(text)


def parse_fraction(text: str) -> Decimal:
    if not CANONICAL_FRACTION.fullmatch(text) or text.startswith("-0."):
        raise FixtureError(f"non-canonical JSON fraction: {text}")
    return Decimal(text)


def reject_constant(text: str) -> None:
    raise FixtureError(f"non-finite JSON number is forbidden: {text}")


def unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise FixtureError(f"duplicate JSON object key: {key}")
        result[key] = value
    return result


def load_fixture(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(
            path.read_text(encoding="utf-8"),
            parse_int=parse_integer,
            parse_float=parse_fraction,
            parse_constant=reject_constant,
            object_pairs_hook=unique_object,
        )
    except (OSError, UnicodeDecodeError, json.JSONDecodeError, FixtureError) as exc:
        raise FixtureError(f"unable to load fixture: {exc}") from exc
    if not isinstance(value, dict):
        raise FixtureError("fixture root must be a JSON object")
    return value


def canonical_json(value: Any) -> str:
    if value is None:
        return "null"
    if value is True:
        return "true"
    if value is False:
        return "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, Decimal):
        text = format(value, "f")
        if "." in text:
            text = text.rstrip("0").rstrip(".")
        return "0" if text in {"-0", ""} else text
    if isinstance(value, str):
        return json.dumps(value, ensure_ascii=False, separators=(",", ":"))
    if isinstance(value, list):
        return "[" + ",".join(canonical_json(item) for item in value) + "]"
    if isinstance(value, dict):
        members = (
            canonical_json(key) + ":" + canonical_json(value[key])
            for key in sorted(value)
        )
        return "{" + ",".join(members) + "}"
    raise FixtureError(f"unsupported canonical JSON value: {type(value).__name__}")


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def suite_digest(fixture: dict[str, Any]) -> str:
    content = dict(fixture)
    content.pop("suite_digest", None)
    return sha256_hex(canonical_json(content).encode("utf-8"))


def fixture_id(scenario_id: str, object_type: str, logical_key: str) -> str:
    slug = OBJECT_TYPE_SLUGS.get(object_type)
    if slug is None:
        raise FixtureError(f"unknown fixture object type: {object_type}")
    if scenario_id not in EXPECTED_SCENARIOS:
        raise FixtureError(f"unknown fixture scenario ID: {scenario_id}")
    if not LOGICAL_KEY.fullmatch(logical_key):
        raise FixtureError(f"invalid fixture logical key: {logical_key}")
    return f"urn:radishmemory:fixture:{scenario_id.lower()}:{slug}:{logical_key}"


def require(condition: bool, message: str, errors: list[str]) -> None:
    if not condition:
        errors.append(message)


def check_no_null_or_empty(value: Any, path: str, errors: list[str]) -> None:
    if value is None:
        errors.append(f"null is forbidden: {path}")
    elif isinstance(value, str) and not value:
        errors.append(f"empty string is forbidden: {path}")
    elif isinstance(value, list):
        for index, item in enumerate(value):
            check_no_null_or_empty(item, f"{path}[{index}]", errors)
    elif isinstance(value, dict):
        for key, item in value.items():
            check_no_null_or_empty(item, f"{path}.{key}", errors)


def check_timestamp_fields(value: Any, path: str, errors: list[str]) -> None:
    if isinstance(value, list):
        for index, item in enumerate(value):
            check_timestamp_fields(item, f"{path}[{index}]", errors)
    elif isinstance(value, dict):
        for key, item in value.items():
            item_path = f"{path}.{key}"
            if key == "as_of" or key.endswith("_at"):
                require(
                    isinstance(item, str) and TIMESTAMP.fullmatch(item) is not None,
                    f"non-canonical timestamp: {item_path}",
                    errors,
                )
            check_timestamp_fields(item, item_path, errors)


def check_top_level(fixture: dict[str, Any], errors: list[str]) -> None:
    require(
        set(fixture) == TOP_LEVEL_FIELDS,
        "fixture top-level fields do not match the frozen contract",
        errors,
    )
    expected_values = {
        "fixture_contract_version": FIXTURE_CONTRACT_VERSION,
        "canonical_schema_version": CANONICAL_SCHEMA_VERSION,
        "data_classification": "synthetic",
        "canonical_json_profile": CANONICAL_JSON_PROFILE,
        "fixture_id_profile": FIXTURE_ID_PROFILE,
    }
    for field, expected in expected_values.items():
        require(fixture.get(field) == expected, f"invalid {field}: expected {expected}", errors)
    for field in ("suite_id", "namespace_id", "device_id"):
        require(isinstance(fixture.get(field), str), f"{field} must be a string", errors)


def check_governance(fixture: dict[str, Any], errors: list[str]) -> None:
    profiles = fixture.get("governance_profiles")
    require(isinstance(profiles, dict), "governance_profiles must be an object", errors)
    if not isinstance(profiles, dict):
        return
    profile = profiles.get("m0-local-personal")
    expected = {
        "sensitivity": "personal",
        "egress_policy": "local_only",
        "retention": {"mode": "until_deleted"},
        "deletion_state": "active",
        "policy_basis": "policy:m0:local-only",
    }
    require(profile == expected, "m0-local-personal governance profile drifted", errors)


def check_deletion_profiles(fixture: dict[str, Any], errors: list[str]) -> None:
    profiles = fixture.get("deletion_profiles")
    require(isinstance(profiles, dict), "deletion_profiles must be an object", errors)
    if not isinstance(profiles, dict):
        return
    targets = profiles.get("m0-local-purge")
    require(isinstance(targets, list), "m0-local-purge must be a list", errors)
    if not isinstance(targets, list):
        return
    require(len(targets) == 10, "m0-local-purge must contain 10 components", errors)
    keys: list[str] = []
    types: set[str] = set()
    for index, target in enumerate(targets):
        if not isinstance(target, dict):
            errors.append(f"deletion target {index} must be an object")
            continue
        keys.append(target.get("component_key", ""))
        types.add(target.get("component_type", ""))
        require(
            target.get("required_action") in {"delete", "redact", "retain_minimal"},
            f"deletion target {index} has invalid required_action",
            errors,
        )
    require(len(keys) == len(set(keys)), "deletion component keys must be unique", errors)
    require(types == EXPECTED_COMPONENT_TYPES, "deletion component type set drifted", errors)


def check_id_vectors(fixture: dict[str, Any], errors: list[str]) -> None:
    vectors = fixture.get("id_vectors")
    require(isinstance(vectors, list), "id_vectors must be a list", errors)
    if not isinstance(vectors, list):
        return
    require(len(vectors) == len(OBJECT_TYPE_SLUGS), "id_vectors must cover nine objects", errors)
    covered: set[str] = set()
    ids: set[str] = set()
    for index, vector in enumerate(vectors):
        if not isinstance(vector, dict):
            errors.append(f"id vector {index} must be an object")
            continue
        object_type = vector.get("object_type")
        covered.add(object_type)
        try:
            expected = fixture_id(
                vector.get("scenario_id", ""),
                object_type,
                vector.get("logical_key", ""),
            )
        except FixtureError as exc:
            errors.append(str(exc))
            continue
        actual = vector.get("expected_id")
        require(actual == expected, f"id vector {index} does not match fixture ID profile", errors)
        require(actual not in ids, f"duplicate fixture ID vector: {actual}", errors)
        ids.add(actual)
    require(covered == set(OBJECT_TYPE_SLUGS), "id vector object coverage drifted", errors)


def check_digest_vectors(fixture: dict[str, Any], errors: list[str]) -> None:
    vectors = fixture.get("digest_vectors")
    require(isinstance(vectors, list), "digest_vectors must be a list", errors)
    if not isinstance(vectors, list):
        return
    profiles: set[str] = set()
    for index, vector in enumerate(vectors):
        if not isinstance(vector, dict):
            errors.append(f"digest vector {index} must be an object")
            continue
        profile = vector.get("profile")
        profiles.add(profile)
        if profile == "exact-bytes-v1":
            payload = vector.get("input_text", "").encode("utf-8")
        elif profile == "utf8-nfc-text-v1":
            payload = unicodedata.normalize("NFC", vector.get("input_text", "")).encode(
                "utf-8"
            )
        elif profile == "canonical-json-v1":
            payload = canonical_json(vector.get("input_value")).encode("utf-8")
        else:
            errors.append(f"digest vector {index} has unknown profile: {profile}")
            continue
        require(
            vector.get("expected_sha256") == sha256_hex(payload),
            f"digest vector {index} does not match {profile}",
            errors,
        )
    require(
        profiles == {"exact-bytes-v1", "utf8-nfc-text-v1", "canonical-json-v1"},
        "digest vector profile coverage drifted",
        errors,
    )


def check_metric_gates(fixture: dict[str, Any], errors: list[str]) -> dict[str, str]:
    gates = fixture.get("metric_gates")
    require(isinstance(gates, list), "metric_gates must be a list", errors)
    if not isinstance(gates, list):
        return {}
    kinds: dict[str, str] = {}
    for index, gate in enumerate(gates):
        if not isinstance(gate, dict):
            errors.append(f"metric gate {index} must be an object")
            continue
        metric_id = gate.get("metric_id")
        expected = EXPECTED_METRIC_GATES.get(metric_id)
        require(expected is not None, f"unknown metric gate: {metric_id}", errors)
        if expected is None:
            continue
        kind, threshold = expected
        kinds[metric_id] = kind
        require(gate.get("kind") == kind, f"metric {metric_id} kind drifted", errors)
        require(gate.get("comparator") == "eq", f"metric {metric_id} must use eq", errors)
        if kind == "count":
            actual_threshold: int | Fraction = gate.get("threshold")
        else:
            value = gate.get("threshold")
            if not isinstance(value, dict) or not value.get("denominator"):
                errors.append(f"metric {metric_id} has invalid ratio threshold")
                continue
            actual_threshold = Fraction(value.get("numerator"), value.get("denominator"))
        require(actual_threshold == threshold, f"metric {metric_id} threshold drifted", errors)
    require(set(kinds) == set(EXPECTED_METRIC_GATES), "metric gate set drifted", errors)
    return kinds


def check_operation(
    scenario_id: str,
    index: int,
    operation: Any,
    prior_step_ids: set[str],
    captures: dict[str, str],
    fixture: dict[str, Any],
    errors: list[str],
) -> None:
    if not isinstance(operation, dict):
        errors.append(f"{scenario_id} operation {index} must be an object")
        return
    expected_step_id = f"{scenario_id.lower()}-s{index + 1:02d}"
    require(
        operation.get("step_id") == expected_step_id,
        f"{scenario_id} operation {index} has unstable step_id",
        errors,
    )
    inputs = operation.get("input")
    expect = operation.get("expect")
    require(isinstance(inputs, dict), f"{expected_step_id} input must be an object", errors)
    require(isinstance(expect, dict), f"{expected_step_id} expect must be an object", errors)
    if not isinstance(inputs, dict) or not isinstance(expect, dict):
        return
    require(
        expect.get("status") in {"succeeded", "rejected", "pending", "failed"},
        f"{expected_step_id} has invalid expected status",
        errors,
    )
    assertions = expect.get("assertions")
    require(
        isinstance(assertions, list) and bool(assertions),
        f"{expected_step_id} must define assertions",
        errors,
    )
    if isinstance(assertions, list):
        require(
            len(assertions) == len(set(assertions)),
            f"{expected_step_id} assertions must be unique",
            errors,
        )
        for assertion in assertions:
            require(
                isinstance(assertion, str) and ASSERTION_CODE.fullmatch(assertion) is not None,
                f"{expected_step_id} has invalid assertion code: {assertion}",
                errors,
            )

    for key, value in inputs.items():
        if key.endswith("_step_id"):
            require(
                value in prior_step_ids,
                f"{expected_step_id} references a non-prior step: {value}",
                errors,
            )

    op = operation.get("op")
    if op == "capture":
        logical_key = inputs.get("logical_key")
        require(
            isinstance(logical_key, str) and LOGICAL_KEY.fullmatch(logical_key) is not None,
            f"{expected_step_id} has invalid source logical_key",
            errors,
        )
        content = inputs.get("content")
        require(isinstance(content, str) and bool(content), f"{expected_step_id} needs content", errors)
        require(
            inputs.get("governance_profile") in fixture.get("governance_profiles", {}),
            f"{expected_step_id} references unknown governance profile",
            errors,
        )
        expected_media = {"text": "text/plain", "markdown": "text/markdown"}.get(
            inputs.get("source_kind")
        )
        require(
            inputs.get("media_type") == expected_media,
            f"{expected_step_id} source kind and media type do not match",
            errors,
        )
        for field in ("observed_at", "captured_at"):
            require(
                isinstance(inputs.get(field), str) and TIMESTAMP.fullmatch(inputs[field]) is not None,
                f"{expected_step_id} has non-canonical {field}",
                errors,
            )
        if isinstance(logical_key, str) and isinstance(content, str):
            require(logical_key not in captures, f"{expected_step_id} repeats a source key", errors)
            captures[logical_key] = content
    elif op == "segment":
        source_key = inputs.get("source_key")
        require(source_key in captures, f"{expected_step_id} references unknown source", errors)
        require(
            inputs.get("segmenter_profile") == "m0-lines-v1",
            f"{expected_step_id} uses an unknown segmenter profile",
            errors,
        )
        fragments = inputs.get("expected_fragments", [])
        if isinstance(fragments, list) and source_key in captures:
            source_bytes = captures[source_key].encode("utf-8")
            for fragment in fragments:
                if not isinstance(fragment, dict):
                    errors.append(f"{expected_step_id} fragment expectation must be an object")
                    continue
                start = fragment.get("byte_start")
                end = fragment.get("byte_end")
                if not isinstance(start, int) or not isinstance(end, int):
                    errors.append(f"{expected_step_id} fragment range must use integers")
                    continue
                try:
                    actual = source_bytes[start:end].decode("utf-8")
                except UnicodeDecodeError:
                    errors.append(f"{expected_step_id} fragment range splits UTF-8")
                    continue
                require(
                    0 <= start < end <= len(source_bytes),
                    f"{expected_step_id} fragment range is out of bounds",
                    errors,
                )
                require(
                    actual == fragment.get("content"),
                    f"{expected_step_id} fragment content does not match range",
                    errors,
                )
    elif op == "search":
        require(inputs.get("top_k") == 5, f"{expected_step_id} must use top_k 5", errors)
        require(
            isinstance(inputs.get("as_of"), str) and TIMESTAMP.fullmatch(inputs["as_of"]) is not None,
            f"{expected_step_id} must materialize canonical as_of",
            errors,
        )
    elif op == "compile_context":
        budget = inputs.get("budget")
        require(isinstance(budget, dict), f"{expected_step_id} needs a budget", errors)
        if isinstance(budget, dict):
            require(budget.get("unit") == "utf8_bytes", f"{expected_step_id} budget unit drifted", errors)
            require(
                isinstance(budget.get("limit"), int) and budget["limit"] > 0,
                f"{expected_step_id} budget limit must be positive",
                errors,
            )
    elif op == "propose":
        require(
            inputs.get("operation") in {"create", "supersede"},
            f"{expected_step_id} proposal operation is invalid",
            errors,
        )
        for field in ("confidence", "importance"):
            value = inputs.get(field)
            require(
                isinstance(value, (int, Decimal)) and not isinstance(value, bool) and 0 <= value <= 1,
                f"{expected_step_id} {field} must be within [0, 1]",
                errors,
            )
        require(
            inputs.get("governance_profile") in fixture.get("governance_profiles", {}),
            f"{expected_step_id} proposal references unknown governance profile",
            errors,
        )
        if inputs.get("operation") == "supersede":
            require(
                isinstance(inputs.get("target_memory_keys"), list)
                and bool(inputs["target_memory_keys"]),
                f"{expected_step_id} supersede proposal needs targets",
                errors,
            )
    elif op == "plan_delete":
        require(
            inputs.get("component_profile") in fixture.get("deletion_profiles", {}),
            f"{expected_step_id} references unknown deletion profile",
            errors,
        )
    elif op == "seed_noise":
        require(inputs.get("seed") == 6048, f"{expected_step_id} noise seed drifted", errors)
        require(inputs.get("count") == 1000, f"{expected_step_id} noise count drifted", errors)
    elif op == "assert_no_network":
        for field in (
            "expected_request_count",
            "expected_manifest_count",
            "expected_provider_trace_count",
            "expected_usage_record_count",
        ):
            require(inputs.get(field) == 0, f"{expected_step_id} {field} must be zero", errors)


def check_scenarios(
    fixture: dict[str, Any], metric_kinds: dict[str, str], errors: list[str]
) -> dict[str, int | tuple[int, int]]:
    scenarios = fixture.get("scenarios")
    require(isinstance(scenarios, list), "scenarios must be a list", errors)
    if not isinstance(scenarios, list):
        return {}
    actual_ids = tuple(
        scenario.get("scenario_id") if isinstance(scenario, dict) else None
        for scenario in scenarios
    )
    require(actual_ids == EXPECTED_SCENARIOS, "scenario order or coverage drifted", errors)
    isolation_keys: set[str] = set()
    all_step_ids: set[str] = set()
    aggregates: dict[str, int | tuple[int, int]] = {}

    for scenario in scenarios:
        if not isinstance(scenario, dict):
            continue
        scenario_id = scenario.get("scenario_id")
        isolation_key = scenario.get("isolation_key")
        require(
            isinstance(isolation_key, str) and LOGICAL_KEY.fullmatch(isolation_key) is not None,
            f"{scenario_id} has invalid isolation_key",
            errors,
        )
        require(isolation_key not in isolation_keys, f"duplicate isolation_key: {isolation_key}", errors)
        isolation_keys.add(isolation_key)
        operations = scenario.get("operations")
        require(isinstance(operations, list), f"{scenario_id} operations must be a list", errors)
        if not isinstance(operations, list):
            continue
        actual_ops = tuple(
            operation.get("op") if isinstance(operation, dict) else None
            for operation in operations
        )
        require(
            actual_ops == EXPECTED_OPERATIONS.get(scenario_id),
            f"{scenario_id} operation sequence drifted",
            errors,
        )
        captures: dict[str, str] = {}
        prior_step_ids: set[str] = set()
        for index, operation in enumerate(operations):
            check_operation(
                scenario_id,
                index,
                operation,
                prior_step_ids,
                captures,
                fixture,
                errors,
            )
            if isinstance(operation, dict) and isinstance(operation.get("step_id"), str):
                step_id = operation["step_id"]
                require(step_id not in all_step_ids, f"duplicate step_id: {step_id}", errors)
                all_step_ids.add(step_id)
                prior_step_ids.add(step_id)

        observations = scenario.get("metric_observations")
        require(
            isinstance(observations, list),
            f"{scenario_id} metric_observations must be a list",
            errors,
        )
        if not isinstance(observations, list):
            continue
        seen_metrics: set[str] = set()
        for observation in observations:
            if not isinstance(observation, dict):
                errors.append(f"{scenario_id} metric observation must be an object")
                continue
            metric_id = observation.get("metric_id")
            kind = metric_kinds.get(metric_id)
            require(kind is not None, f"{scenario_id} observes unknown metric: {metric_id}", errors)
            require(metric_id not in seen_metrics, f"{scenario_id} repeats metric: {metric_id}", errors)
            seen_metrics.add(metric_id)
            if kind == "count":
                value = observation.get("value")
                require(
                    isinstance(value, int) and not isinstance(value, bool) and value >= 0,
                    f"{scenario_id} count metric {metric_id} is invalid",
                    errors,
                )
                if isinstance(value, int):
                    aggregates[metric_id] = int(aggregates.get(metric_id, 0)) + value
            elif kind == "ratio":
                numerator = observation.get("numerator")
                denominator = observation.get("denominator")
                require(
                    isinstance(numerator, int)
                    and not isinstance(numerator, bool)
                    and numerator >= 0,
                    f"{scenario_id} ratio numerator {metric_id} is invalid",
                    errors,
                )
                require(
                    isinstance(denominator, int)
                    and not isinstance(denominator, bool)
                    and denominator > 0,
                    f"{scenario_id} ratio denominator {metric_id} is invalid",
                    errors,
                )
                if isinstance(numerator, int) and isinstance(denominator, int):
                    old_num, old_den = aggregates.get(metric_id, (0, 0))
                    aggregates[metric_id] = (old_num + numerator, old_den + denominator)
    return aggregates


def check_metric_aggregates(
    aggregates: dict[str, int | tuple[int, int]], errors: list[str]
) -> None:
    require(
        set(aggregates) == set(EXPECTED_METRIC_GATES),
        "metric observation coverage drifted",
        errors,
    )
    for metric_id, (kind, threshold) in EXPECTED_METRIC_GATES.items():
        aggregate = aggregates.get(metric_id)
        if kind == "count":
            actual: int | Fraction | None = aggregate if isinstance(aggregate, int) else None
        elif isinstance(aggregate, tuple) and aggregate[1] > 0:
            actual = Fraction(aggregate[0], aggregate[1])
        else:
            actual = None
        require(actual == threshold, f"metric oracle does not satisfy gate: {metric_id}", errors)


def check_declared_suite_digest(fixture: dict[str, Any], errors: list[str]) -> None:
    digest = fixture.get("suite_digest")
    require(isinstance(digest, dict), "suite_digest must be an object", errors)
    if not isinstance(digest, dict):
        return
    require(digest.get("algorithm") == "sha256", "suite digest algorithm drifted", errors)
    require(digest.get("profile") == "fixture-suite-v1", "suite digest profile drifted", errors)
    require(digest.get("value") == suite_digest(fixture), "suite digest does not match fixture", errors)


def validate_fixture(fixture: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    check_no_null_or_empty(fixture, "$", errors)
    check_timestamp_fields(fixture, "$", errors)
    check_top_level(fixture, errors)
    check_governance(fixture, errors)
    check_deletion_profiles(fixture, errors)
    check_id_vectors(fixture, errors)
    check_digest_vectors(fixture, errors)
    metric_kinds = check_metric_gates(fixture, errors)
    aggregates = check_scenarios(fixture, metric_kinds, errors)
    check_metric_aggregates(aggregates, errors)
    check_declared_suite_digest(fixture, errors)
    return errors


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("fixture", nargs="?", type=Path, default=DEFAULT_FIXTURE)
    parser.add_argument(
        "--print-suite-digest",
        action="store_true",
        help="print the computed fixture-suite-v1 digest without validating it",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        fixture = load_fixture(args.fixture)
    except FixtureError as exc:
        print(f"M0 fixture contract failed: {exc}", file=sys.stderr)
        return 1
    if args.print_suite_digest:
        print(suite_digest(fixture))
        return 0
    errors = validate_fixture(fixture)
    if errors:
        print("M0 fixture contract failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1
    scenario_count = len(fixture["scenarios"])
    operation_count = sum(len(scenario["operations"]) for scenario in fixture["scenarios"])
    metric_count = len(fixture["metric_gates"])
    print(
        "M0 fixture contract passed "
        f"({scenario_count} scenarios, {operation_count} operations, "
        f"{metric_count} metric gates)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
