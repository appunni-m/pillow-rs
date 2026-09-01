#!/usr/bin/env python3
"""Regression tests for explicit pipeline terminal-completeness receipts."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from scripts.run_all_backend_tests import (
    backend_coverage_report,
    pipeline_execution_evidence,
)
from scripts.run_migration_benchmark import execution_result
from scripts.run_migration_parity import (
    DEFAULT_MANIFEST,
    classify_pipeline_case,
    load_cases,
    load_manifest,
    run_case,
    write_pipeline_execution_evidence,
)
from scripts.validate_migration_parity_result import (
    all_backends as validate_all_backends,
    execution_receipt,
)


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


OPENED_EAGER_NO_RECEIPT_CASE_IDS = {
    "PIL.Image.Image.apply_transparency.nuanced.png-p-single-index-transparency",
    "PIL.Image.Image.apply_transparency.nuanced.png-p-transparency-putpalette",
    "PIL.Image.Image.apply_transparency.nuanced.png-p-transparency-table-load",
    "PIL.Image.Image.apply_transparency.nuanced.png-p-transparency-table-putpalette",
    "PIL.Image.Image.convert.nuanced.opened-p-auto",
    "PIL.Image.Image.convert.nuanced.opened-p-transparency",
    "PIL.Image.Image.convert.nuanced.opened-p-transparency-auto",
    "PIL.Image.Image.convert.nuanced.opened-p-transparency-table",
    "PIL.Image.Image.convert.nuanced.opened-p-transparency-table-to-la",
    "PIL.Image.Image.convert.nuanced.opened-p-transparency-table-to-rgb",
    "PIL.Image.Image.convert.nuanced.opened-p-transparency-to-l",
    "PIL.Image.Image.convert.nuanced.opened-p-transparency-to-la",
    "PIL.Image.Image.convert.nuanced.opened-p-transparency-to-rgb",
    "PIL.Image.Image.putpixel.nuanced.l16-png-putpixel",
    "PIL.Image.Image.putpixel.nuanced.l16-png-singleton-tuple",
    "PIL.ImageOps.exif_transpose.nuanced.jpeg-invalid-byte-order",
    "PIL.ImageOps.exif_transpose.nuanced.jpeg-invalid-magic",
    "PIL.ImageOps.exif_transpose.nuanced.jpeg-invalid-offset",
    "PIL.ImageOps.exif_transpose.nuanced.jpeg-invalid-orientation",
    "PIL.ImageOps.exif_transpose.nuanced.jpeg-no-orientation",
    "PIL.ImageOps.exif_transpose.nuanced.jpeg-short-exif-payload",
    "PIL.ImageOps.exif_transpose.nuanced.jpeg-short-tiff",
    "PIL.ImageOps.exif_transpose.nuanced.jpeg-truncated-entry",
    "PIL.ImageOps.exif_transpose.nuanced.tiff-no-orientation",
}


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


def coverage_lane(
    lane_id: str,
    backend: str,
    *,
    actual_backend_counts: dict[str, int],
    selected: int | None = None,
    terminal_complete_receipts: int = 1,
    terminal_incomplete_cases: int = 0,
    not_recorded_cases: int = 0,
    pipeline_applicable_cases: int | None = None,
    pipeline_complete_cases: int | None = None,
    pipeline_missing_receipt_cases: int | None = None,
    pipeline_partial_receipt_cases: int | None = None,
    pipeline_not_applicable_cases: int = 0,
    pipeline_indeterminate_cases: int = 0,
    fallback_reason_counts: dict[str, int] | None = None,
) -> dict[str, object]:
    if pipeline_complete_cases is None:
        pipeline_complete_cases = terminal_complete_receipts
    if pipeline_partial_receipt_cases is None:
        pipeline_partial_receipt_cases = terminal_incomplete_cases
    if pipeline_missing_receipt_cases is None:
        pipeline_missing_receipt_cases = not_recorded_cases
    if pipeline_applicable_cases is None:
        pipeline_applicable_cases = (
            pipeline_complete_cases
            + pipeline_missing_receipt_cases
            + pipeline_partial_receipt_cases
        )
    if selected is None:
        selected = max(
            1,
            not_recorded_cases
            + int(
                bool(
                    terminal_complete_receipts
                    or terminal_incomplete_cases
                    or pipeline_not_applicable_cases
                    or pipeline_indeterminate_cases
                )
            ),
        )
    return {
        "lane_id": lane_id,
        "status": "passed",
        "scope": {"selected": selected},
        "execution_evidence": {
            "status": "measured",
            "summary": {
                "selected": selected,
                "receipt_cases": selected - not_recorded_cases,
                "not_recorded_cases": not_recorded_cases,
                "completed_receipts": 1,
                "terminal_complete_receipts": terminal_complete_receipts,
                "terminal_incomplete_cases": terminal_incomplete_cases,
                "pipeline_applicable_cases": pipeline_applicable_cases,
                "pipeline_complete_cases": pipeline_complete_cases,
                "pipeline_missing_receipt_cases": pipeline_missing_receipt_cases,
                "pipeline_partial_receipt_cases": pipeline_partial_receipt_cases,
                "pipeline_not_applicable_cases": pipeline_not_applicable_cases,
                "pipeline_indeterminate_cases": pipeline_indeterminate_cases,
                "actual_backend_counts": actual_backend_counts,
                "fallback_reason_counts": fallback_reason_counts or {},
            },
        },
        "backend": backend,
    }


def complete_execution_summary(
    backend: str, *, actual_backend: str | None = None
) -> dict[str, object]:
    actual = actual_backend or backend
    return {
        "selected": 1,
        "receipt_cases": 1,
        "not_recorded_cases": 0,
        "completed_receipts": 1,
        "terminal_complete_receipts": 1,
        "terminal_incomplete_cases": 0,
        "pipeline_applicable_cases": 1,
        "pipeline_complete_cases": 1,
        "pipeline_missing_receipt_cases": 0,
        "pipeline_partial_receipt_cases": 0,
        "pipeline_not_applicable_cases": 0,
        "pipeline_indeterminate_cases": 0,
        "actual_backend_counts": {actual: 1},
        "fallback_reason_counts": {},
    }


def all_backends_lane(
    lane_id: str,
    *,
    kind: str = "python-py3-parity",
    backend: str | None = None,
    execution_summary: dict[str, object] | None = None,
) -> dict[str, object]:
    scope = {
        "kind": "public-parity-corpus",
        "selected": 1,
        "case_ids_sha256": hashlib.sha256(b"case\n").hexdigest(),
        "filter": None,
        "executed": 1,
        "pending": 0,
    }
    lane: dict[str, object] = {
        "lane_id": lane_id,
        "kind": kind,
        "backend": backend,
        "command": ["make", "migration-parity-test"],
        "status": "passed",
        "returncode": 0,
        "timed_out": False,
        "scope": scope,
    }
    if execution_summary is not None:
        lane["execution_evidence"] = {
            "status": "measured",
            "reason": "",
            "artifact": "execution.json",
            "summary": execution_summary,
        }
    if lane_id == "browser-wasm-parity":
        lane["capabilities"] = {
            "webgpu": {
                "api": "available",
                "adapter": "available",
                "device": "available",
                "shader_dispatch": "available",
                "reason": "test capability receipt",
            }
        }
    return lane


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


def classification_case(
    case_id: str,
    operation: str,
    *,
    mode: str = "RGB",
) -> dict[str, object]:
    """Build the smallest workflow that exercises one receipt partition."""

    operation_arguments: dict[str, object] = {}
    if operation == "resize":
        operation_arguments = {
            "size": {"kind": "literal", "value": [1, 1]},
        }
    elif operation == "convert":
        # RGB->L is the ordinary lazy byte converter; use it for the generic
        # maybe-path fixture instead of convert(mode=None), which is a proven
        # eager copy in the Rust source contract.
        operation_arguments = {
            "mode": {"kind": "literal", "value": "L"},
        }
    operation_surface = "PIL.Image.Image"
    operation_receiver: dict[str, object] | None = {
        "kind": "binding",
        "step_id": "new",
    }
    if operation == "scale":
        operation_surface = "PIL.ImageOps"
        operation_receiver = None
        operation_arguments = {
            "image": {"kind": "binding", "step_id": "new"},
            "factor": {"kind": "literal", "value": 1.0},
        }
    return {
        "case_id": case_id,
        "steps": [
            {
                "step_id": "new",
                "surface": "PIL.Image",
                "operation": "new",
                "arguments": {
                    "mode": {"kind": "literal", "value": mode},
                    "size": {"kind": "literal", "value": [1, 1]},
                },
            },
            {
                "step_id": "call",
                "surface": operation_surface,
                "operation": operation,
                "receiver": operation_receiver,
                "arguments": operation_arguments,
            },
            {
                "step_id": "observe",
                "surface": "PIL.Image.Image",
                "operation": "tobytes",
                "receiver": {"kind": "binding", "step_id": "call"},
                "arguments": {},
            },
        ],
        "observations": ["observe"],
    }


def eager_mode_filter_case() -> dict[str, object]:
    """Build a workflow for the eager ModeFilter implementation path."""

    return {
        "case_id": "mode-filter-eager",
        "steps": [
            {
                "step_id": "new",
                "surface": "PIL.Image",
                "operation": "new",
                "arguments": {
                    "mode": {"kind": "literal", "value": "L"},
                    "size": {"kind": "literal", "value": [1, 1]},
                },
            },
            {
                "step_id": "filter-type",
                "surface": "PIL.ImageFilter",
                "operation": "ModeFilter",
                "arguments": {},
            },
            {
                "step_id": "filter",
                "surface": "PIL.Image.Image",
                "operation": "filter",
                "receiver": {"kind": "binding", "step_id": "new"},
                "arguments": {
                    "filter": {"kind": "binding", "step_id": "filter-type"}
                },
            },
        ],
        "observations": ["filter"],
    }


def filter_constructor_error_case(operation: str) -> dict[str, object]:
    """Build a workflow where a filter descriptor precedes call validation."""

    return {
        "case_id": f"filter-constructor-{operation}-error",
        "steps": [
            {
                "step_id": "new",
                "surface": "PIL.Image",
                "operation": "new",
                "arguments": {
                    "mode": {"kind": "literal", "value": "P"},
                    "size": {"kind": "literal", "value": [1, 1]},
                },
            },
            {
                "step_id": "filter-type",
                "surface": "PIL.ImageFilter",
                "operation": operation,
                "arguments": {},
            },
            {
                "step_id": "call",
                "surface": "PIL.Image.Image",
                "operation": "filter",
                "receiver": {"kind": "binding", "step_id": "new"},
                "arguments": {
                    "filter": {"kind": "binding", "step_id": "filter-type"}
                },
            },
        ],
        "observations": ["call"],
    }


def crop_discard_case() -> dict[str, object]:
    """Build a workflow whose degenerate crop discards a queued PutPixel."""

    return {
        "case_id": "crop-discard",
        "steps": [
            {
                "step_id": "new",
                "surface": "PIL.Image",
                "operation": "new",
                "arguments": {
                    "mode": {"kind": "literal", "value": "L"},
                    "size": {"kind": "literal", "value": [1, 1]},
                },
            },
            {
                "step_id": "pixel",
                "surface": "PIL.Image.Image",
                "operation": "putpixel",
                "receiver": {"kind": "binding", "step_id": "new"},
                "arguments": {},
            },
            {
                "step_id": "crop",
                "surface": "PIL.Image.Image",
                "operation": "crop",
                "receiver": {"kind": "binding", "step_id": "new"},
                "arguments": {
                    "box": {"kind": "literal", "value": [2.0, 0.0, 2.0, 1.0]}
                },
            },
            {
                "step_id": "bytes",
                "surface": "PIL.Image.Image",
                "operation": "tobytes",
                "receiver": {"kind": "binding", "step_id": "crop"},
                "arguments": {},
            },
        ],
        "observations": ["crop", "bytes"],
    }


def error_at_deferred_call_case(
    *, earlier_deferred: bool = False, materialized: bool = False
) -> dict[str, object]:
    """Build a workflow whose public call fails before it can queue work."""

    steps: list[dict[str, object]] = [
        {
            "step_id": "new",
            "surface": "PIL.Image",
            "operation": "new",
            "arguments": {
                "mode": {"kind": "literal", "value": "RGB"},
                "size": {"kind": "literal", "value": [1, 1]},
            },
        }
    ]
    if earlier_deferred:
        steps.append(
            {
                "step_id": "resize",
                "surface": "PIL.Image.Image",
                "operation": "resize",
                "receiver": {"kind": "binding", "step_id": "new"},
                "arguments": {"size": {"kind": "literal", "value": [1, 1]}},
            }
        )
        if materialized:
            steps.append(
                {
                    "step_id": "before-error",
                    "surface": "PIL.Image.Image",
                    "operation": "tobytes",
                    "receiver": {"kind": "binding", "step_id": "resize"},
                    "arguments": {},
                }
            )
        receiver = "resize"
    else:
        receiver = "new"
    steps.extend(
        [
            {
                "step_id": "call",
                "surface": "PIL.Image.Image",
                "operation": "resize",
                "receiver": {"kind": "binding", "step_id": receiver},
                "arguments": {
                    "size": {"kind": "literal", "value": [0, 1]},
                },
            }
        ]
    )
    return {
        "case_id": "error-at-deferred-call",
        "steps": steps,
        "observations": ["before-error", "call"]
        if materialized
        else ["call"],
    }


def eager_exif_transpose_case(*, in_place: bool = False) -> dict[str, object]:
    """Build a workflow whose new image has no EXIF orientation to apply."""

    arguments: dict[str, object] = {
        "image": {"kind": "binding", "step_id": "new"},
    }
    if in_place:
        arguments["in_place"] = {"kind": "literal", "value": True}
    return {
        "case_id": "exif-transpose-no-orientation",
        "steps": [
            {
                "step_id": "new",
                "surface": "PIL.Image",
                "operation": "new",
                "arguments": {
                    "mode": {"kind": "literal", "value": "RGB"},
                    "size": {"kind": "literal", "value": [1, 1]},
                },
            },
            {
                "step_id": "call",
                "surface": "PIL.ImageOps",
                "operation": "exif_transpose",
                "arguments": arguments,
            },
        ],
        "observations": ["call"],
    }


class ReceiptStateTests(unittest.TestCase):
    def test_backend_coverage_requires_terminal_and_requested_backend_receipts(self) -> None:
        lanes = [
            coverage_lane(
                "parity-cpu",
                "cpu",
                actual_backend_counts={"cpu": 1},
                not_recorded_cases=1,
            ),
            coverage_lane(
                "parity-simd",
                "simd",
                actual_backend_counts={"cpu": 1},
                terminal_incomplete_cases=1,
            ),
            coverage_lane(
                "parity-gpu",
                "gpu",
                actual_backend_counts={"gpu": 1},
                fallback_reason_counts={"exact host semantic control": 1},
            ),
        ]
        report = backend_coverage_report(lanes)
        self.assertEqual(report["status"], "not_proven")
        by_lane = {item["lane_id"]: item for item in report["target_lanes"]}
        self.assertEqual(by_lane["parity-cpu"]["status"], "not_proven")
        self.assertEqual(by_lane["parity-simd"]["status"], "not_proven")
        self.assertEqual(by_lane["parity-gpu"]["status"], "not_proven")

    def test_backend_coverage_can_be_proven_only_with_exact_receipts(self) -> None:
        report = backend_coverage_report(
            [
                coverage_lane(
                    "parity-cpu", "cpu", actual_backend_counts={"cpu": 1}
                ),
                coverage_lane(
                    "parity-simd", "simd", actual_backend_counts={"simd": 1}
                ),
                coverage_lane(
                    "parity-gpu", "gpu", actual_backend_counts={"gpu": 1}
                ),
            ]
        )
        self.assertEqual(report["status"], "proven")

    def test_benchmark_execution_keeps_prefix_fallbacks_out_of_no_fallback_claim(self) -> None:
        first = benchmark_record(terminal_complete=True)
        first["fallback_reason"] = "exact host semantic control"
        result = execution_result(
            "target_profile",
            "python-gpu",
            [first, benchmark_record(terminal_complete=True)],
            {"warmup_iterations": 0, "measurement_iterations": 1, "samples": 2},
        )
        self.assertEqual(result["status"], "completed")
        self.assertEqual(
            result["fallback_reason_counts"],
            {"exact host semantic control": 1},
        )

    def test_all_backends_rejects_plain_pass_with_backend_gaps(self) -> None:
        lanes = [
            all_backends_lane(
                "parity-cpu",
                backend="cpu",
                execution_summary=complete_execution_summary("cpu"),
            ),
            all_backends_lane(
                "parity-simd",
                backend="simd",
                execution_summary=complete_execution_summary("simd"),
            ),
            all_backends_lane("parity-gpu-smoke", backend="gpu"),
            all_backends_lane(
                "parity-gpu",
                backend="gpu",
                execution_summary=complete_execution_summary(
                    "gpu", actual_backend="cpu"
                ),
            ),
            all_backends_lane(
                "js-wasm-parity",
                kind="javascript-wasm-parity",
            ),
            all_backends_lane(
                "browser-wasm-parity",
                kind="browser-wasm-parity",
            ),
        ]
        backend_coverage = backend_coverage_report(lanes)
        result = {
            "schema": "migration-parity/all-backends-test-result@3",
            "status": "passed",
            "started_at": "2026-01-01T00:00:00Z",
            "finished_at": "2026-01-01T00:00:01Z",
            "revision": "test-revision",
            "input_scope": {
                "kind": "public-parity-corpus",
                "selected": 1,
                "case_ids_sha256": hashlib.sha256(b"case\n").hexdigest(),
                "filter": None,
            },
            "gpu_gate": {
                "case_id": "case",
                "status": "passed",
                "timeout_seconds": 180,
            },
            "gpu_full_requested": True,
            "backend_coverage": backend_coverage,
            "lanes": lanes,
        }
        with self.assertRaisesRegex(ValueError, "status does not match"):
            validate_all_backends(result)
        result["status"] = "passed_with_backend_gaps"
        validate_all_backends(result)

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

    def test_pipeline_case_classification_preserves_receipt_gaps(self) -> None:
        deferred = classification_case("deferred", "resize")
        eager = classification_case("eager", "putdata")
        maybe = classification_case("maybe", "convert")
        eager_default = classification_case("eager-default", "convert")
        eager_default["steps"][1]["arguments"] = {}
        eager_scalar = classification_case("eager-scalar", "convert", mode="F")
        eager_matrix = classification_case("eager-matrix", "convert")
        eager_matrix["steps"][1]["arguments"] = {
            "mode": {"kind": "literal", "value": "L"},
            "matrix": {"kind": "literal", "value": [1, 0, 0, 0]},
        }
        eager_palette_default = classification_case(
            "eager-palette-default", "convert", mode="P"
        )
        eager_palette_default["steps"][1]["arguments"] = {}
        eager_transparency = classification_case("eager-transparency", "apply_transparency")
        eager_thumbnail = classification_case("eager-thumbnail", "thumbnail")
        eager_thumbnail["steps"][1]["arguments"] = {
            "size": {"kind": "literal", "value": [2, 2]},
        }
        eager_scale = classification_case("eager-scale", "scale")
        deferred_scale = classification_case("deferred-scale", "scale")
        deferred_scale["steps"][1]["arguments"]["factor"] = {
            "kind": "literal",
            "value": 2.0,
        }
        eager_paste = classification_case("eager-paste", "paste")
        eager_paste["steps"][1]["arguments"] = {
            "im": {"kind": "literal", "value": 0},
            "box": {"kind": "literal", "value": [0, 0, 0, 1]},
        }
        eager_pixel = classification_case("eager-pixel", "putpixel", mode="I;16")
        partial = classification_case("partial", "resize")

        self.assertEqual(
            classify_pipeline_case(deferred, []),
            {
                "status": "missing_receipt",
                "reason": "deferred image pipeline reached an observed boundary without a receipt",
            },
        )
        self.assertEqual(
            classify_pipeline_case(eager, []),
            {
                "status": "not_applicable",
                "reason": "workflow contains no deferred image-pipeline operation",
            },
        )
        self.assertEqual(
            classify_pipeline_case(deferred_scale, []),
            {
                "status": "missing_receipt",
                "reason": "deferred image pipeline reached an observed boundary without a receipt",
            },
        )
        self.assertEqual(
            classify_pipeline_case(maybe, []),
            {
                "status": "indeterminate",
                "reason": "workflow may use an eager or deferred path; no receipt was recorded",
            },
        )
        self.assertEqual(
            classify_pipeline_case(eager_default, []),
            {
                "status": "not_applicable",
                "reason": "workflow contains no deferred image-pipeline operation",
            },
        )
        self.assertEqual(
            classify_pipeline_case(eager_scalar, []),
            {
                "status": "not_applicable",
                "reason": "workflow contains no deferred image-pipeline operation",
            },
        )
        self.assertEqual(
            classify_pipeline_case(eager_matrix, []),
            {
                "status": "not_applicable",
                "reason": "workflow contains no deferred image-pipeline operation",
            },
        )
        self.assertEqual(
            classify_pipeline_case(eager_palette_default, []),
            {
                "status": "not_applicable",
                "reason": "workflow contains no deferred image-pipeline operation",
            },
        )
        for eager_case in (
            eager_transparency,
            eager_thumbnail,
            eager_scale,
            eager_paste,
            eager_pixel,
            eager_exif_transpose_case(),
            eager_exif_transpose_case(in_place=True),
        ):
            with self.subTest(case_id=eager_case["case_id"]):
                self.assertEqual(
                    classify_pipeline_case(eager_case, []),
                    {
                        "status": "not_applicable",
                        "reason": "workflow contains no deferred image-pipeline operation",
                    },
                )
        self.assertEqual(
            classify_pipeline_case(
                partial,
                [{"status": "partial", "terminal_complete": False}],
            ),
            {
                "status": "partial_receipt",
                "reason": "receipt recorded without a terminal-complete boundary",
            },
        )
        self.assertEqual(
            classify_pipeline_case(
                deferred,
                [{"status": "completed", "terminal_complete": True}],
            )["status"],
            "complete",
        )

    def test_pipeline_case_classification_proves_opened_eager_fixture_paths(self) -> None:
        """Header/EXIF proofs cover the fixed opened no-receipt cohort only."""

        manifest = load_manifest(DEFAULT_MANIFEST)
        cases, _ = load_cases(
            manifest,
            case_ids=OPENED_EAGER_NO_RECEIPT_CASE_IDS,
            surface=None,
        )
        self.assertEqual(
            {case["case_id"] for case in cases},
            OPENED_EAGER_NO_RECEIPT_CASE_IDS,
        )
        for case in cases:
            with self.subTest(case_id=case["case_id"]):
                result = {
                    "status": "completed",
                    "observations": [
                        {"step_id": step_id, "status": "completed"}
                        for step_id in case["observations"]
                    ],
                }
                self.assertEqual(
                    classify_pipeline_case(case, [], result=result),
                    {
                        "status": "not_applicable",
                        "reason": "workflow contains no deferred image-pipeline operation",
                    },
                )

    def test_pipeline_case_classification_keeps_unknown_opened_source_conservative(
        self,
    ) -> None:
        case = classification_case("opened-unknown", "convert")
        case["steps"][0] = {
            "step_id": "open",
            "surface": "PIL.Image",
            "operation": "open",
            "receiver": None,
            "arguments": {
                "fp": {"kind": "asset", "asset_id": "not-provided"},
            },
        }
        case["steps"][1]["receiver"]["step_id"] = "open"
        case["assets"] = []
        self.assertEqual(
            classify_pipeline_case(case, []),
            {
                "status": "indeterminate",
                "reason": "workflow may use an eager or deferred path; no receipt was recorded",
            },
        )

    def test_pipeline_case_classification_recognizes_eager_mode_filter(self) -> None:
        self.assertEqual(
            classify_pipeline_case(eager_mode_filter_case(), []),
            {
                "status": "not_applicable",
                "reason": "workflow contains no deferred image-pipeline operation",
            },
        )

    def test_pipeline_case_classification_does_not_count_filter_constructor_as_deferred(
        self,
    ) -> None:
        for operation in ("BLUR", "BoxBlur", "Kernel", "RankFilter"):
            with self.subTest(operation=operation):
                self.assertEqual(
                    classify_pipeline_case(
                        filter_constructor_error_case(operation),
                        [],
                        result={
                            "status": "completed",
                            "observations": [
                                {
                                    "step_id": "call",
                                    "status": "error",
                                    "error": {"kind": "invalid_argument"},
                                }
                            ],
                        },
                    ),
                    {
                        "status": "not_applicable",
                        "reason": "workflow ended in a public error before pipeline materialization",
                    },
                )

    def test_pipeline_case_classification_accepts_error_at_first_deferred_call(self) -> None:
        self.assertEqual(
            classify_pipeline_case(
                error_at_deferred_call_case(),
                [],
                result={
                    "status": "completed",
                    "observations": [
                        {
                            "step_id": "call",
                            "status": "error",
                            "error": {"kind": "invalid_argument"},
                        },
                        {
                            "step_id": "observe-result",
                            "status": "not_run",
                            "reason": "dependency step call failed",
                        },
                    ],
                },
            ),
            {
                "status": "not_applicable",
                "reason": "workflow ended in a public error before pipeline materialization",
            },
        )

    def test_pipeline_case_classification_uses_unobserved_execution_error(self) -> None:
        """Setup failures are classified from the internal step error record."""

        case = classification_case("hidden-error", "resize")
        result = {
            "status": "completed",
            "observations": [
                {
                    "step_id": "observe",
                    "status": "not_run",
                    "reason": "dependency step call failed",
                }
            ],
            "execution_errors": [
                {
                    "step_id": "call",
                    "error": {"kind": "invalid_argument"},
                }
            ],
        }
        self.assertEqual(
            classify_pipeline_case(case, [], result=result),
            {
                "status": "not_applicable",
                "reason": "workflow ended in a public error before pipeline materialization",
            },
        )

    def test_pipeline_case_classification_accepts_queued_work_before_unreached_error(
        self,
    ) -> None:
        self.assertEqual(
            classify_pipeline_case(
                error_at_deferred_call_case(earlier_deferred=True),
                [],
                result={
                    "status": "completed",
                    "observations": [
                        {
                            "step_id": "call",
                            "status": "error",
                            "error": {"kind": "invalid_argument"},
                        }
                    ],
                },
            ),
            {
                "status": "not_applicable",
                "reason": "workflow ended in a public error before pipeline materialization",
            },
        )

    def test_pipeline_case_classification_keeps_materialized_prefix_indeterminate(
        self,
    ) -> None:
        self.assertEqual(
            classify_pipeline_case(
                error_at_deferred_call_case(
                    earlier_deferred=True, materialized=True
                ),
                [],
                result={
                    "status": "completed",
                    "observations": [
                        {"step_id": "before-error", "status": "ok"},
                        {
                            "step_id": "call",
                            "status": "error",
                            "error": {"kind": "invalid_argument"},
                        },
                    ],
                },
            ),
            {
                "status": "indeterminate",
                "reason": "workflow errored after or during a potentially deferred operation",
            },
        )

    def test_pipeline_case_classification_keeps_partial_receipt_authoritative_on_error(
        self,
    ) -> None:
        self.assertEqual(
            classify_pipeline_case(
                error_at_deferred_call_case(earlier_deferred=True),
                [{"status": "partial", "terminal_complete": False}],
                result={
                    "status": "completed",
                    "observations": [
                        {
                            "step_id": "call",
                            "status": "error",
                            "error": {"kind": "invalid_argument"},
                        }
                    ],
                },
            ),
            {
                "status": "partial_receipt",
                "reason": "receipt recorded without a terminal-complete boundary",
            },
        )

    def test_pipeline_case_classification_keeps_blocked_dependency_conservative(self) -> None:
        self.assertEqual(
            classify_pipeline_case(
                error_at_deferred_call_case(),
                [],
                result={
                    "status": "completed",
                    "observations": [
                        {
                            "step_id": "call",
                            "status": "not_run",
                            "reason": "dependency step setup failed",
                        }
                    ],
                },
            ),
            {
                "status": "indeterminate",
                "reason": "workflow errored after or during a potentially deferred operation",
            },
        )

    def test_pipeline_case_classification_ignores_queued_pixel_discarded_by_crop(
        self,
    ) -> None:
        self.assertEqual(
            classify_pipeline_case(crop_discard_case(), []),
            {
                "status": "not_applicable",
                "reason": "workflow contains no deferred image-pipeline operation",
            },
        )

    def test_successful_final_observation_keeps_prior_receipt_terminal(self) -> None:
        class Telemetry:
            def __init__(self) -> None:
                self.samples: list[dict[str, object] | None] = [
                    {"actual_backend": "cpu"},
                    None,
                    None,
                ]

            def take_pipeline_telemetry(self) -> dict[str, object] | None:
                return self.samples.pop(0)

        case = {
            "case_id": "final-observation-receipt",
            "assets": [],
            "steps": [
                {
                    "step_id": "new",
                    "surface": "PIL.Image",
                    "operation": "new",
                    "arguments": {},
                },
                {
                    "step_id": "bytes",
                    "surface": "PIL.Image.Image",
                    "operation": "tobytes",
                    "arguments": {},
                },
            ],
            "observations": ["bytes"],
        }
        sink: list[dict[str, object]] = []
        with tempfile.TemporaryDirectory() as directory:
            with (
                patch(
                    "scripts.run_migration_parity.operation_definition",
                    return_value={"source": {"result": {"shape": "scalar"}}},
                ),
                patch(
                    "scripts.run_migration_parity.call_workflow_step",
                    side_effect=[object(), object()],
                ),
                patch(
                    "scripts.run_migration_parity.serialize_value",
                    return_value=7,
                ),
            ):
                result = run_case(
                    "target",
                    case,
                    {},
                    Path(directory),
                    pipeline_execution_api=Telemetry(),
                    pipeline_execution_sink=sink,
                )
        self.assertEqual(result["status"], "completed")
        self.assertEqual(result["observations"], [{"step_id": "bytes", "status": "ok", "value": 7}])
        self.assertEqual(len(sink), 1)
        self.assertTrue(sink[0]["terminal_complete"])

    def test_unobserved_final_step_clears_prior_receipt_candidate(self) -> None:
        class Telemetry:
            def __init__(self) -> None:
                self.samples: list[dict[str, object] | None] = [
                    None,
                    {"actual_backend": "cpu"},
                    None,
                    None,
                ]

            def take_pipeline_telemetry(self) -> dict[str, object] | None:
                return self.samples.pop(0)

        case = {
            "case_id": "unobserved-final-step-receipt",
            "assets": [],
            "steps": [
                {
                    "step_id": "new",
                    "surface": "PIL.Image",
                    "operation": "new",
                    "arguments": {},
                },
                {
                    "step_id": "resize",
                    "surface": "PIL.Image.Image",
                    "operation": "resize",
                    "arguments": {},
                },
                {
                    "step_id": "unobserved",
                    "surface": "PIL.Image.Image",
                    "operation": "getbands",
                    "arguments": {},
                },
            ],
            "observations": ["resize"],
        }
        sink: list[dict[str, object]] = []
        with tempfile.TemporaryDirectory() as directory:
            with (
                patch(
                    "scripts.run_migration_parity.operation_definition",
                    return_value={"source": {"result": {"shape": "scalar"}}},
                ),
                patch(
                    "scripts.run_migration_parity.call_workflow_step",
                    side_effect=[object(), object(), object()],
                ),
                patch(
                    "scripts.run_migration_parity.serialize_value",
                    return_value=7,
                ),
            ):
                result = run_case(
                    "target",
                    case,
                    {},
                    Path(directory),
                    pipeline_execution_api=Telemetry(),
                    pipeline_execution_sink=sink,
                )
        self.assertEqual(result["status"], "completed")
        self.assertEqual(len(sink), 1)
        self.assertFalse(sink[0]["terminal_complete"])

    def test_sidecar_rejects_dropped_pipeline_case_classification(self) -> None:
        cases = [classification_case("deferred", "resize")]
        execution = {"deferred": []}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "execution.json"
            write_pipeline_execution_evidence(
                path,
                cases,
                {"side": "target", "backend": "gpu"},
                execution,
            )
            document = json.loads(path.read_text(encoding="utf-8"))
            document["pipeline_case_status"] = {}
            path.write_text(json.dumps(document), encoding="utf-8")
            evidence = pipeline_execution_evidence(
                path,
                expected_scope={
                    "kind": "public-parity-corpus",
                    "selected": 1,
                    "case_ids_sha256": hashlib.sha256(b"deferred\n").hexdigest(),
                },
                expected_backend="gpu",
            )
            self.assertEqual(evidence["status"], "not_measured")
            self.assertIn("case IDs do not match", evidence["reason"])

    def test_sidecar_keeps_prefix_and_exact_errors_out_of_terminal_counts(self) -> None:
        cases = [{"case_id": "case-prefix"}]
        execution = {
            "case-prefix": [
                {
                    "status": "completed",
                    "terminal_complete": False,
                    "actual_backend": "gpu",
                    "fallback_reason": "exact host semantic control",
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
            self.assertEqual(
                document["summary"]["fallback_reason_counts"],
                {"exact host semantic control": 1},
            )
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
