"""Conformance checks for the generated migration-parity evidence boundary."""

from __future__ import annotations

import argparse
import json
import tempfile
import unittest
from pathlib import Path

from scripts.aggregate_migration_parity import aggregate
from scripts.run_migration_coverage import coverage_case_failed, file_dimensions
from scripts.run_migration_rust_coverage import llvm_shape, merged_file_data
from scripts.validate_migration_parity_result import status_report


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "pillow-rs" / "tests" / "fixtures" / "manifest.yaml"


class MigrationParityEvidenceTests(unittest.TestCase):
    def test_llvm_shape_maps_four_dimensions(self) -> None:
        file_entry = {
            "summary": {
                "functions": {"covered": 2, "count": 3},
                "lines": {"covered": 4, "count": 5},
                "branches": {"covered": 1, "count": 2},
                "regions": {"covered": 6, "count": 8},
            },
            "segments": [
                [10, 5, 0, True, True, False],
                [11, 5, 2, True, True, False],
                [12, 5, 0, False, False, False],
            ],
            "branches": [
                [13, 1, 13, 2, 0, 0, 0, 0, 4],
                [14, 1, 14, 2, 0, 3, 0, 0, 4],
            ],
        }
        shape = llvm_shape(file_entry)
        self.assertEqual(shape["summary"]["num_functions"], 3)
        self.assertEqual(shape["summary"]["num_statements"], 5)
        self.assertEqual(shape["summary"]["num_branches"], 2)
        self.assertEqual(shape["summary"]["num_regions"], 8)
        self.assertEqual(shape["missing_lines"], [10])
        self.assertEqual(shape["missing_branches"], [13, 14])

    def test_merged_file_data_prefers_llvm_for_rust_and_python_for_py(self) -> None:
        python_files = {
            Path("/repo/pillow_rs/image.py"): {
                "summary": {"covered_lines": 1, "num_statements": 2},
                "missing_lines": [3],
            }
        }
        llvm_files = {
            Path("/repo/pillow-rs/src/image.rs"): {
                "summary": {
                    "lines": {"covered": 4, "count": 5},
                    "functions": {"covered": 0, "count": 0},
                    "branches": {"covered": 0, "count": 0},
                    "regions": {"covered": 0, "count": 0},
                },
                "segments": [[9, 5, 0, True, True, False]],
            }
        }
        py_data = merged_file_data(
            python_files, llvm_files, Path("/repo/pillow_rs/image.py")
        )
        self.assertEqual(py_data["summary"]["num_statements"], 2)
        rust_data = merged_file_data(
            python_files, llvm_files, Path("/repo/pillow-rs/src/image.rs")
        )
        self.assertEqual(rust_data["summary"]["num_statements"], 5)
        self.assertEqual(rust_data["missing_lines"], [9])

    def test_file_dimensions_normalizes_coverage_branch_pairs(self) -> None:
        data = {
            "summary": {
                "covered_branches": 1,
                "num_branches": 3,
            },
            "missing_lines": [4, 5],
            "missing_branches": [[7, 8], [9, 10]],
        }
        dimensions = {
            item["dimension"]: item
            for item in file_dimensions(Path("/repo/example.py"), data)
        }
        self.assertEqual(dimensions["branch"]["uncovered"], [7, 9])
        self.assertEqual(dimensions["line"]["uncovered"], [4, 5])

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
            self.assertEqual(len(result["operations"]), 205)
            self.assertEqual(result["evidence"], [])
            parity = next(
                item
                for item in result["completeness"]
                if item["dimension"] == "parity_outcome"
            )
            self.assertEqual(parity["numerator"], 0)
            self.assertEqual(parity["denominator"], 1914)
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
