#!/usr/bin/env python3
"""Regression tests for explicit pipeline terminal-completeness receipts."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from scripts.run_all_backend_tests import pipeline_execution_evidence
from scripts.run_migration_benchmark import execution_result
from scripts.run_migration_parity import write_pipeline_execution_evidence
from scripts.validate_migration_parity_result import execution_receipt


RESOURCE_FIELDS = (
    "upload_bytes",
    "readback_bytes",
    "auxiliary_bytes",
    "parameter_bytes",
    "retained_cache_bytes",
    "full_frame_copy_count",
    "mode_conversion_count",
    "host_buffer_count",
    "host_buffer_bytes",
    "peak_live_host_bytes",
    "fused_operation_count",
)


def benchmark_record(*, terminal_complete: bool) -> dict[str, object]:
    return {
        "status": "completed",
        "terminal_complete": terminal_complete,
        "actual_backend": "gpu",
        "operation_count": 1,
        "dispatch_count": 1,
        "resize_coeff_cache_hits": 0,
        "resize_coeff_cache_misses": 0,
        "route_ns": 1,
        "validation_ns": 1,
        "backend_ns": 1,
        "resource": {field: 0 for field in RESOURCE_FIELDS},
    }


def aggregate_policy() -> dict[str, int]:
    return {"warmup_iterations": 0, "measurement_iterations": 1, "samples": 1}


def execution_receipt_value(
    *, status: str = "partial", terminal_complete: bool | None = False
) -> dict[str, object]:
    value: dict[str, object] = {
        "status": status,
        "requested_backend": "gpu",
        "actual_backend": "gpu" if status == "completed" else None,
        "actual_backend_counts": {"gpu": 1} if status == "completed" else {},
        "fallback_reason_counts": {},
        "operation_count": {
            "sample_count": 1 if status == "completed" else 0,
            "min": 1 if status == "completed" else None,
            "median": 1 if status == "completed" else None,
            "mean": 1 if status == "completed" else None,
            "max": 1 if status == "completed" else None,
            "total": 1 if status == "completed" else None,
        },
        "dispatch_count": {
            "sample_count": 1 if status == "completed" else 0,
            "min": 1 if status == "completed" else None,
            "median": 1 if status == "completed" else None,
            "mean": 1 if status == "completed" else None,
            "max": 1 if status == "completed" else None,
            "total": 1 if status == "completed" else None,
        },
        "resize_coeff_cache_hits": {
            "sample_count": 1 if status == "completed" else 0,
            "min": 0 if status == "completed" else None,
            "median": 0 if status == "completed" else None,
            "mean": 0 if status == "completed" else None,
            "max": 0 if status == "completed" else None,
            "total": 0 if status == "completed" else None,
        },
        "resize_coeff_cache_misses": {
            "sample_count": 1 if status == "completed" else 0,
            "min": 0 if status == "completed" else None,
            "median": 0 if status == "completed" else None,
            "mean": 0 if status == "completed" else None,
            "max": 0 if status == "completed" else None,
            "total": 0 if status == "completed" else None,
        },
        "phase_timings_ns": {
            phase: {
                "sample_count": 1 if status == "completed" else 0,
                "min": 1 if status == "completed" else None,
                "median": 1 if status == "completed" else None,
                "mean": 1 if status == "completed" else None,
                "max": 1 if status == "completed" else None,
                "total": 1 if status == "completed" else None,
            }
            for phase in ("route_ns", "validation_ns", "backend_ns")
        },
        "resource": {
            "sample_count": 1 if status == "completed" else 0,
            **{
                field: {
                    "sample_count": 1 if status == "completed" else 0,
                    "min": 0 if status == "completed" else None,
                    "median": 0 if status == "completed" else None,
                    "mean": 0 if status == "completed" else None,
                    "max": 0 if status == "completed" else None,
                    "total": 0 if status == "completed" else None,
                }
                for field in RESOURCE_FIELDS
            },
        },
        "sample_count": 1 if status == "completed" else 0,
        "cached_sample_count": 0,
    }
    if terminal_complete is not None:
        value["terminal_complete"] = terminal_complete
    return value


class ReceiptStateTests(unittest.TestCase):
    def test_drained_prefix_with_exact_error_is_partial_not_completed(self) -> None:
        result = execution_result(
            "target_profile",
            "python-gpu",
            [benchmark_record(terminal_complete=False)],
            aggregate_policy(),
            errors=[
                {
                    "step_id": "later-step",
                    "error": {"kind": "runtime_error"},
                }
            ],
        )
        self.assertEqual(result["status"], "partial")
        self.assertFalse(result["terminal_complete"])
        self.assertEqual(result["actual_backend_counts"], {})
        self.assertEqual(result["errors"][0]["step_id"], "later-step")

    def test_successful_terminal_receipt_is_completed(self) -> None:
        result = execution_result(
            "target_profile",
            "python-gpu",
            [benchmark_record(terminal_complete=True)],
            aggregate_policy(),
        )
        self.assertEqual(result["status"], "completed")
        self.assertTrue(result["terminal_complete"])
        self.assertEqual(result["actual_backend_counts"], {"gpu": 1})

    def test_legacy_receipt_without_bit_remains_accepted(self) -> None:
        legacy = execution_receipt_value(terminal_complete=None)
        execution_receipt(legacy, "legacy.execution")

    def test_impossible_aggregate_states_are_rejected(self) -> None:
        completed_without_bit = execution_receipt_value(
            status="completed", terminal_complete=False
        )
        with self.assertRaisesRegex(ValueError, "requires terminal_complete=true"):
            execution_receipt(completed_without_bit, "invalid.execution")

        completed_with_error = execution_receipt_value(
            status="completed", terminal_complete=True
        )
        completed_with_error["errors"] = [
            {
                "step_id": "later-step",
                "error": {
                    "class": "RuntimeError",
                    "kind": "runtime_error",
                    "message": "failed",
                    "stage": "call",
                    "code": None,
                },
            }
        ]
        with self.assertRaisesRegex(ValueError, "cannot carry execution errors"):
            execution_receipt(completed_with_error, "invalid.execution")

    def test_sidecar_keeps_prefix_and_exact_errors_out_of_terminal_counts(self) -> None:
        cases = [{"case_id": "case-prefix"}]
        execution = {
            "case-prefix": [
                {
                    "status": "completed",
                    "terminal_complete": False,
                    "actual_backend": "gpu",
                },
                {"status": "partial", "terminal_complete": False},
            ]
        }
        exact_error = {
            "step_id": "later-step",
            "error": {
                "class": "RuntimeError",
                "kind": "runtime_error",
                "message": "failed",
                "stage": "call",
                "code": None,
            },
        }
        result = {
            "case_id": "case-prefix",
            "status": "not_run",
            "observations": [],
            "execution_errors": [exact_error],
        }
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "execution.json"
            write_pipeline_execution_evidence(
                path,
                cases,
                {"side": "target", "backend": "gpu"},
                execution,
                results={"case-prefix": result},
            )
            document = json.loads(path.read_text(encoding="utf-8"))
            self.assertEqual(document["summary"]["completed_receipts"], 1)
            self.assertEqual(document["summary"]["terminal_complete_receipts"], 0)
            self.assertEqual(document["summary"]["terminal_incomplete_cases"], 1)
            self.assertEqual(document["summary"]["actual_backend_counts"], {})
            self.assertEqual(document["errors"]["case-prefix"], [exact_error])
            digest = hashlib.sha256(b"case-prefix\n").hexdigest()
            evidence = pipeline_execution_evidence(
                path,
                expected_scope={
                    "kind": "public-parity-corpus",
                    "selected": 1,
                    "case_ids_sha256": digest,
                },
                expected_backend="gpu",
            )
            self.assertEqual(evidence["status"], "measured")


if __name__ == "__main__":
    unittest.main()
