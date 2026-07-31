"""Conformance checks for the generated migration-parity evidence boundary."""

from __future__ import annotations

import argparse
import json
import tempfile
import unittest
from pathlib import Path

from scripts.aggregate_migration_parity import aggregate
from scripts.run_migration_coverage import coverage_case_failed
from scripts.validate_migration_parity_result import status_report


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "pillow-rs" / "tests" / "fixtures" / "manifest.yaml"


class MigrationParityEvidenceTests(unittest.TestCase):
    def test_expected_public_error_does_not_fail_coverage_case(self) -> None:
        observations = [
            {"step_id": "call", "status": "error"},
            {"step_id": "dependent", "status": "not_run"},
        ]
        self.assertFalse(coverage_case_failed(observations))

    def test_unblocked_not_run_fails_coverage_case(self) -> None:
        observations = [{"step_id": "call", "status": "not_run"}]
        self.assertTrue(coverage_case_failed(observations))

    def test_empty_join_is_complete_schema_and_not_proven(self) -> None:
        with tempfile.TemporaryDirectory(prefix="migration-status-test-") as directory:
            output = Path(directory) / "status.json"
            result = aggregate(
                argparse.Namespace(
                    manifest=MANIFEST,
                    parity=Path(directory) / "parity.json",
                    coverage=Path(directory) / "coverage.json",
                    benchmark=Path(directory) / "benchmark.json",
                    output=output,
                )
            )
            status_report(result)
            self.assertEqual(result["schema"], "migration-parity/status-report@1")
            self.assertEqual(len(result["operations"]), 204)
            self.assertEqual(result["evidence"], [])
            parity = next(
                item
                for item in result["completeness"]
                if item["dimension"] == "parity_outcome"
            )
            self.assertEqual(parity["numerator"], 0)
            self.assertEqual(parity["denominator"], 1181)
            self.assertTrue(all(item["parity"]["outcome"] == "not_proven" for item in result["operations"]))

    def test_unknown_status_field_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory(prefix="migration-status-test-") as directory:
            output = Path(directory) / "status.json"
            result = aggregate(
                argparse.Namespace(
                    manifest=MANIFEST,
                    parity=Path(directory) / "parity.json",
                    coverage=Path(directory) / "coverage.json",
                    benchmark=Path(directory) / "benchmark.json",
                    output=output,
                )
            )
            result["extra"] = True
            with self.assertRaises(ValueError):
                status_report(result)

    def test_written_status_is_json_and_matches_in_memory_result(self) -> None:
        with tempfile.TemporaryDirectory(prefix="migration-status-test-") as directory:
            output = Path(directory) / "status.json"
            result = aggregate(
                argparse.Namespace(
                    manifest=MANIFEST,
                    parity=Path(directory) / "parity.json",
                    coverage=Path(directory) / "coverage.json",
                    benchmark=Path(directory) / "benchmark.json",
                    output=output,
                )
            )
            self.assertEqual(json.loads(output.read_text(encoding="utf-8")), result)


if __name__ == "__main__":
    unittest.main()
