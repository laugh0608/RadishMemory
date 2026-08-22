from __future__ import annotations

import copy
import importlib.util
import tempfile
import unittest
from decimal import Decimal
from pathlib import Path


SCRIPT_PATH = Path(__file__).resolve().parents[1] / "check-m0-fixtures.py"
SPEC = importlib.util.spec_from_file_location("check_m0_fixtures", SCRIPT_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("unable to load check-m0-fixtures.py")
CHECK_M0 = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK_M0)


class M0FixtureContractChecks(unittest.TestCase):
    def fixture(self) -> dict[str, object]:
        return CHECK_M0.load_fixture(CHECK_M0.DEFAULT_FIXTURE)

    def test_repository_fixture_passes(self) -> None:
        self.assertEqual([], CHECK_M0.validate_fixture(self.fixture()))

    def test_canonical_json_has_stable_key_and_number_encoding(self) -> None:
        value = {"z": Decimal("0.82"), "a": [2, 1]}

        self.assertEqual('{"a":[2,1],"z":0.82}', CHECK_M0.canonical_json(value))

    def test_operation_sequence_drift_is_rejected(self) -> None:
        fixture = copy.deepcopy(self.fixture())
        fixture["scenarios"][0]["operations"].reverse()

        errors = CHECK_M0.validate_fixture(fixture)

        self.assertIn("M0-E01 operation sequence drifted", errors)

    def test_false_complete_metric_oracle_is_rejected(self) -> None:
        fixture = copy.deepcopy(self.fixture())
        scenario = next(
            item for item in fixture["scenarios"] if item["scenario_id"] == "M0-E10"
        )
        observation = next(
            item
            for item in scenario["metric_observations"]
            if item["metric_id"] == "false_complete_deletion_count"
        )
        observation["value"] = 1

        errors = CHECK_M0.validate_fixture(fixture)

        self.assertIn(
            "metric oracle does not satisfy gate: false_complete_deletion_count",
            errors,
        )

    def test_fixture_id_vector_drift_is_rejected(self) -> None:
        fixture = copy.deepcopy(self.fixture())
        fixture["id_vectors"][0]["expected_id"] += "-changed"

        errors = CHECK_M0.validate_fixture(fixture)

        self.assertIn("id vector 0 does not match fixture ID profile", errors)

    def test_stale_suite_digest_is_rejected(self) -> None:
        fixture = copy.deepcopy(self.fixture())
        fixture["suite_digest"]["value"] = "0" * 64

        errors = CHECK_M0.validate_fixture(fixture)

        self.assertIn("suite digest does not match fixture", errors)

    def test_non_canonical_operation_timestamp_is_rejected(self) -> None:
        fixture = copy.deepcopy(self.fixture())
        fixture["scenarios"][0]["operations"][0]["input"]["observed_at"] = (
            "2026-01-10T09:00:00+00:00"
        )

        errors = CHECK_M0.validate_fixture(fixture)

        self.assertIn(
            "non-canonical timestamp: $.scenarios[0].operations[0].input.observed_at",
            errors,
        )

    def test_duplicate_json_object_key_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "duplicate.json"
            path.write_text('{"a":1,"a":2}\n', encoding="utf-8")

            with self.assertRaisesRegex(CHECK_M0.FixtureError, "duplicate JSON object key"):
                CHECK_M0.load_fixture(path)


if __name__ == "__main__":
    unittest.main()
