"""Regression tests for composing migration benchmark workload filters."""
from __future__ import annotations

import argparse
import copy
import gzip
import io
from contextlib import ExitStack, redirect_stderr, redirect_stdout
import json
import unittest

import os
from pathlib import Path
import tempfile
from unittest.mock import patch

from run_migration_benchmark import (
    FROZEN_TARGET_PROFILES,
    PARALLEL_CPU_PROFILE,
    TARGET_BACKENDS,
    TARGET_PROFILES,
    applicable_parity_profiles,
    apply_profile_to_workloads,
    benchmark_subjects,
    frozen_workload_subjects,
    execution_result,
    parity_gate_outcome,
    profile_applies_to_case,
    parity_profile_for_subject,
    parity_timing_allowed,
    runtime_backend_for_profile,
    read_and_remove_backend_parity,
    run_timed_side,
    select_workloads,
    sha256,
    validate_selected_workloads,
    validate_reused_parity,
    write_parity_artifact,
)
import run_migration_benchmark
import run_migration_parity
from test_migration_benchmark_samples import synthetic_report
from run_migration_parity import TARGET_FEATURES, target_profile_for_backend
from validate_migration_parity_result import execution_receipt


class BenchmarkExecutionProofScopeTests(unittest.TestCase):
    @staticmethod
    def receipt(
        step_id, operation, *, terminal_complete, path="cpu", vector_blocks=0
    ):
        return {
            "step_id": step_id,
            "status": "completed",
            "terminal_complete": terminal_complete,
            "actual_backend": "cpu",
            "operation_count": 1,
            "dispatch_count": 0,
            "operation_telemetry": [
                {
                    "operation": operation,
                    "path": path,
                    "vector_block_count": vector_blocks,
                    "scalar_tail_count": 0,
                    "mode_conversion_count": 0,
                    "handoff_count": 0,
                }
            ],
        }

    def test_observed_step_uses_its_receipt_and_keeps_full_case_completion(self):
        result = execution_result(
            "target_profile",
            "python-cpu",
            [
                self.receipt(
                    "i-getcolors", "GetColors", terminal_complete=False
                ),
                self.receipt(
                    "f-getprojection", "GetProjection", terminal_complete=True
                ),
            ],
            {"boundary": "observed_steps", "step_ids": ["i-getcolors"]},
        )

        self.assertEqual(result["status"], "completed")
        self.assertTrue(result["terminal_complete"])
        self.assertEqual(result["actual_backend"], "cpu")
        self.assertEqual(result["actual_backend_counts"], {"cpu": 1})
        self.assertEqual(
            [item["operation"] for item in result["operation_telemetry"]],
            ["GetColors"],
        )
        self.assertEqual(result["selected_step_ids"], ["i-getcolors"])
        self.assertEqual(
            [item["step_id"] for item in result["operation_telemetry"]],
            ["i-getcolors"],
        )

    def test_vector_telemetry_is_bound_to_selected_timed_step(self):
        result = execution_result(
            "target_profile",
            "python-simd",
            [
                self.receipt(
                    "i-getcolors",
                    "GetColors",
                    terminal_complete=False,
                    path="scalar-gather",
                ),
                self.receipt(
                    "f-getprojection",
                    "GetProjection",
                    terminal_complete=True,
                    path="ssse3-gather",
                    vector_blocks=64,
                ),
            ],
            {"boundary": "observed_steps", "step_ids": ["i-getcolors"]},
        )

        self.assertEqual(result["selected_step_ids"], ["i-getcolors"])
        self.assertEqual(
            result["operation_telemetry"],
            [
                {
                    "operation": "GetColors",
                    "path": "scalar-gather",
                    "vector_block_count": 0,
                    "scalar_tail_count": 0,
                    "mode_conversion_count": 0,
                    "handoff_count": 0,
                    "step_id": "i-getcolors",
                }
            ],
        )

    def test_selected_vector_step_retains_its_step_identity(self):
        result = execution_result(
            "target_profile",
            "python-simd",
            [
                self.receipt(
                    "i-getcolors",
                    "GetColors",
                    terminal_complete=False,
                    path="ssse3-gather",
                    vector_blocks=64,
                ),
                self.receipt(
                    "f-getprojection",
                    "GetProjection",
                    terminal_complete=True,
                    path="cpu",
                ),
            ],
            {"boundary": "observed_steps", "step_ids": ["i-getcolors"]},
        )

        self.assertEqual(result["selected_step_ids"], ["i-getcolors"])
        self.assertEqual(len(result["operation_telemetry"]), 1)
        self.assertEqual(result["operation_telemetry"][0]["step_id"], "i-getcolors")
        self.assertEqual(result["operation_telemetry"][0]["vector_block_count"], 64)

    def test_validator_rejects_telemetry_for_an_unselected_step(self):
        result = execution_result(
            "target_profile",
            "python-simd",
            [
                self.receipt(
                    "i-getcolors",
                    "GetColors",
                    terminal_complete=True,
                    path="ssse3-gather",
                    vector_blocks=64,
                )
            ],
            {"boundary": "whole_workflow", "step_ids": []},
        )
        result["operation_telemetry"][0]["step_id"] = "unselected-step"

        with self.assertRaisesRegex(ValueError, "not in selected_step_ids"):
            execution_receipt(result, "benchmark.execution")

    def test_validator_rejects_receipts_outside_workload_step_selection(self):
        result = execution_result(
            "target_profile",
            "python-simd",
            [
                self.receipt(
                    "i-getcolors",
                    "GetColors",
                    terminal_complete=True,
                    path="ssse3-gather",
                    vector_blocks=64,
                )
            ],
            {"boundary": "observed_steps", "step_ids": ["i-getcolors"]},
        )

        with self.assertRaisesRegex(ValueError, "outside workload measurement_policy"):
            execution_receipt(
                result,
                "benchmark.execution",
                allowed_step_ids={"f-getprojection"},
            )

    def test_operation_telemetry_without_step_identity_is_rejected(self):
        selected = self.receipt(
            "i-getcolors",
            "GetColors",
            terminal_complete=True,
            path="ssse3-gather",
            vector_blocks=64,
        )
        selected.pop("step_id")

        with self.assertRaisesRegex(ValueError, "step_id: expected non-empty string"):
            execution_result(
                "target_profile",
                "python-simd",
                [selected],
                {"boundary": "whole_workflow", "step_ids": []},
            )

    def test_observed_step_without_its_own_receipt_is_not_attributed_later(self):
        result = execution_result(
            "target_profile",
            "python-cpu",
            [
                self.receipt(
                    "f-getprojection", "GetProjection", terminal_complete=True
                ),
            ],
            {"boundary": "observed_steps", "step_ids": ["i-getcolors"]},
        )

        self.assertEqual(result["status"], "not_proven")
        self.assertFalse(result["terminal_complete"])
        self.assertIsNone(result["actual_backend"])
        self.assertEqual(result["actual_backend_counts"], {})
        self.assertEqual(result["operation_telemetry"], [])
        execution_receipt(result, "benchmark.execution")

    def test_fallback_from_any_proof_step_remains_visible(self):
        later_step = self.receipt(
            "f-getprojection", "GetProjection", terminal_complete=True
        )
        later_step["fallback_reason"] = "unsupported-profile"
        result = execution_result(
            "target_profile",
            "python-cpu",
            [
                self.receipt(
                    "i-getcolors", "GetColors", terminal_complete=False
                ),
                later_step,
            ],
            {"boundary": "observed_steps", "step_ids": ["i-getcolors"]},
        )

        self.assertEqual(
            result["fallback_reason_counts"], {"unsupported-profile": 1}
        )


class BenchmarkWorkloadSelectionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.workloads = {
            workload_id: {"workload_id": workload_id}
            for workload_id in (
                "pil-imagechops.overlay.standard",
                "pipeline-matrix.expanded.overlay.1024x768",
                "pipeline-op.overlay.benchmark-materialized",
            )
        }

    def test_pipeline_and_workload_id_filters_compose(self) -> None:
        selected = select_workloads(
            self.workloads,
            pipeline=True,
            workload_ids=["pipeline-matrix.expanded.overlay.1024x768"],
            limit=None,
        )

        self.assertEqual(
            [workload["workload_id"] for workload in selected],
            ["pipeline-matrix.expanded.overlay.1024x768"],
        )

    def test_pipeline_selection_rejects_a_non_pipeline_workload(self) -> None:
        with self.assertRaisesRegex(ValueError, "not in the selected pipeline set"):
            select_workloads(
                self.workloads,
                pipeline=True,
                workload_ids=["pil-imagechops.overlay.standard"],
                limit=None,
            )

    def test_benchmark_backend_proof_does_not_add_target_lock_to_timing(self) -> None:
        with (
            patch.object(run_migration_parity, "STRICT_TARGET_BACKEND", True),
            patch.object(run_migration_parity, "BENCHMARK_ADAPTER", True),
        ):
            self.assertFalse(run_migration_parity.should_lock_target_backend("target"))
            self.assertFalse(run_migration_parity.should_lock_target_backend("source"))

        with (
            patch.object(run_migration_parity, "STRICT_TARGET_BACKEND", True),
            patch.object(run_migration_parity, "BENCHMARK_ADAPTER", False),
        ):
            self.assertTrue(run_migration_parity.should_lock_target_backend("target"))
            self.assertFalse(run_migration_parity.should_lock_target_backend("source"))

    def test_unknown_workload_ids_remain_errors(self) -> None:
        with self.assertRaisesRegex(ValueError, "unknown benchmark workload"):
            select_workloads(
                self.workloads,
                pipeline=False,
                workload_ids=["missing.workload"],
                limit=None,
            )

    def test_benchmark_identity_uses_its_start_not_reused_parity_start(self) -> None:
        benchmark_started_at = "2026-10-13T10:30:00Z"
        parity_identity = {
            "started_at": "2026-10-13T10:00:00Z",
            "oracles": [],
        }
        with patch(
            "run_migration_benchmark.benchmark_target_identities", return_value=[]
        ):
            result_identity = run_migration_benchmark.identity(
                run_migration_benchmark.DEFAULT_MANIFEST,
                [],
                [],
                parity_identity,
                30,
                started_at=benchmark_started_at,
            )

        self.assertEqual(result_identity["started_at"], benchmark_started_at)
        self.assertNotEqual(
            result_identity["started_at"], parity_identity["started_at"]
        )

    def test_material_rgb_thumbnail_times_lazy_pixel_setup(self) -> None:
        inputs = (
            Path(__file__).resolve().parents[1]
            / "pillow-rs"
            / "tests"
            / "fixtures"
            / "inputs"
            / "benchmark"
            / "pipeline-operations.json"
        )
        workloads = json.loads(inputs.read_text(encoding="utf-8"))["workloads"]
        workload = next(
            workload
            for workload in workloads
            if workload["workload_id"] == "pipeline-op.thumbnail.material-rgb-1024x768"
        )

        self.assertEqual(workload["measurement"]["boundary"], "whole_workflow")
        self.assertEqual(workload["measurement"]["step_ids"], [])

    def test_getdata_workloads_cover_banded_modes_and_sizes_with_parity(self) -> None:
        inputs = (
            Path(__file__).resolve().parents[1]
            / "pillow-rs"
            / "tests"
            / "fixtures"
            / "inputs"
            / "benchmark"
            / "pipeline-operations.json"
        )
        workloads = json.loads(inputs.read_text(encoding="utf-8"))["workloads"]
        getdata = [
            workload
            for workload in workloads
            if "PIL.Image.Image.getdata.performance.standard"
            in workload.get("covers", [])
        ]

        expected_modes = {
            "1",
            "L",
            "LA",
            "La",
            "P",
            "PA",
            "RGB",
            "RGBA",
            "CMYK",
            "YCbCr",
            "HSV",
            "LAB",
        }
        mode_sizes = {
            (workload["context"]["mode"], tuple(workload["context"]["size"]))
            for workload in getdata
        }
        self.assertEqual({mode for mode, _ in mode_sizes}, expected_modes)
        self.assertIn(("RGB", (2048, 1536)), mode_sizes)
        composed_workloads = {
            workload["workload_id"]
            for workload in getdata
            if "material-invert-rgb-band0" in workload["input"]["case_id"]
            or workload["workload_id"] == "pipeline-chain.terminal-read.rgb-band0"
        }
        self.assertEqual(
            composed_workloads,
            {
                "pipeline-chain.terminal-read.rgb-band0",
                "pipeline-chain.terminal-read.getdata.invert-rgb-band0-1024x768",
                "pipeline-chain.terminal-read.getdata.invert-rgb-band0-2048x1536",
            },
        )
        for mode in {"L", "LA", "RGB", "RGBA"}:
            self.assertIn((mode, (32, 32)), mode_sizes)
            self.assertIn((mode, (1024, 768)), mode_sizes)
        for workload in getdata:
            with self.subTest(workload=workload["workload_id"]):
                self.assertEqual(workload["input"]["kind"], "parity_case")
                if (
                    "material-invert-rgb-band0" in workload["input"]["case_id"]
                    or workload["workload_id"]
                    == "pipeline-chain.terminal-read.rgb-band0"
                ):
                    self.assertEqual(
                        workload["measurement"]["boundary"], "whole_workflow"
                    )
                    self.assertEqual(workload["measurement"]["step_ids"], [])
                else:
                    self.assertEqual(
                        workload["measurement"]["boundary"], "observed_steps"
                    )
                    self.assertEqual(
                        workload["measurement"]["step_ids"], ["band-data"]
                    )
                self.assertEqual(
                    workload["measurement"]["correctness_gate"], "parity_pass"
                )
                self.assertGreaterEqual(workload["measurement"]["warmup_iterations"], 5)
                self.assertGreaterEqual(
                    workload["measurement"]["measurement_iterations"], 20
                )
                self.assertGreaterEqual(workload["measurement"]["samples"], 5)

    def test_backend_parity_receipt_is_removed_after_loading(self) -> None:
        expected = {"status": "completed", "comparisons": [{"case_id": "case"}]}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "cpu.json"
            path.write_text(json.dumps(expected), encoding="utf-8")

            self.assertEqual(read_and_remove_backend_parity(path), expected)
            self.assertFalse(path.exists())

    def test_compressed_backend_parity_receipt_is_removed_after_loading(self) -> None:
        expected = {"status": "completed", "comparisons": [{"case_id": "case"}]}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "cpu.json.gz"
            with gzip.open(path, "wt", encoding="utf-8") as stream:
                json.dump(expected, stream)

            self.assertEqual(read_and_remove_backend_parity(path), expected)
            self.assertFalse(path.exists())

    def test_parity_writer_streams_gzip_artifacts(self) -> None:
        expected = {
            "schema": "migration-parity/parity-result@1",
            "comparisons": [{"case_id": "case", "actual": "x" * 200_000}],
        }
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "parity.json.gz"
            write_parity_artifact(path, expected)

            with gzip.open(path, "rt", encoding="utf-8") as stream:
                actual = json.load(stream)

        self.assertEqual(actual, expected)

    def test_parallel_cpu_uses_cpu_applicability_without_becoming_simd(self) -> None:
        cpu_case = {"target_profiles": ["python-cpu"]}
        simd_only_case = {"target_profiles": ["python-simd"]}

        self.assertTrue(profile_applies_to_case("python-parallel-cpu", cpu_case))
        self.assertFalse(profile_applies_to_case("python-parallel-cpu", simd_only_case))
        self.assertEqual(runtime_backend_for_profile("python-parallel-cpu"), "cpu")

    def test_parity_gate_requires_every_declared_target_profile(self) -> None:
        case = {"target_profiles": ["python-cpu", "python-simd"]}
        comparisons = {
            ("shared", "python-cpu"): {"outcome": "pass"},
            ("shared", "python-simd"): {"outcome": "pass"},
        }

        cpu_profiles = applicable_parity_profiles(
            ["python-cpu"], case, active_profiles=("python-cpu", "python-simd")
        )
        mixed_profiles = applicable_parity_profiles(
            ["python-cpu", "python-simd"],
            case,
            active_profiles=("python-cpu", "python-simd"),
        )
        self.assertEqual(cpu_profiles, ["python-cpu"])
        self.assertEqual(mixed_profiles, ["python-cpu", "python-simd"])
        self.assertEqual(
            parity_gate_outcome("shared", ["python-cpu"], cpu_profiles, comparisons),
            "pass",
        )
        self.assertEqual(
            parity_gate_outcome(
                "shared", ["python-cpu", "python-simd"], mixed_profiles, comparisons
            ),
            "pass",
        )

    def test_partial_shared_case_parity_is_unproven_but_failure_is_a_fail(self) -> None:
        cpu_only_case = {"target_profiles": ["python-cpu"]}
        mixed_subjects = ["python-cpu", "python-simd"]
        applicable = applicable_parity_profiles(
            mixed_subjects,
            cpu_only_case,
            active_profiles=("python-cpu", "python-simd"),
        )
        self.assertEqual(applicable, ["python-cpu"])

        cpu_pass = {("shared", "python-cpu"): {"outcome": "pass"}}
        self.assertEqual(
            parity_gate_outcome("shared", mixed_subjects, applicable, cpu_pass),
            "not_proven",
        )
        self.assertTrue(parity_timing_allowed("not_proven"))
        fully_applicable = applicable_parity_profiles(
            mixed_subjects,
            {"target_profiles": ["python-cpu", "python-simd"]},
            active_profiles=("python-cpu", "python-simd"),
        )
        self.assertEqual(
            parity_gate_outcome(
                "shared", mixed_subjects, fully_applicable, cpu_pass
            ),
            "not_proven",
        )
        cpu_fail = {("shared", "python-cpu"): {"outcome": "fail"}}
        self.assertEqual(
            parity_gate_outcome("shared", mixed_subjects, applicable, cpu_fail),
            "fail",
        )
        self.assertFalse(parity_timing_allowed("fail"))

    def test_unapplicable_profile_never_passes_parity(self) -> None:
        cpu_only_case = {"target_profiles": ["python-cpu"]}
        simd_only_subjects = ["python-simd"]
        applicable = applicable_parity_profiles(
            simd_only_subjects,
            cpu_only_case,
            active_profiles=("python-cpu", "python-simd"),
        )

        self.assertEqual(applicable, [])
        self.assertEqual(
            parity_gate_outcome("shared", simd_only_subjects, applicable, {}),
            "not_proven",
        )
        self.assertTrue(parity_timing_allowed("not_proven"))

    def test_parallel_parity_uses_its_result_key_and_cpu_applicability(self) -> None:
        cpu_only_case = {"target_profiles": ["python-cpu"]}
        parallel_subjects = ["python-parallel-cpu"]
        applicable = applicable_parity_profiles(
            parallel_subjects,
            cpu_only_case,
            active_profiles=("python-parallel-cpu",),
        )

        self.assertEqual(
            parity_profile_for_subject("python-parallel-cpu"), "python-cpu"
        )
        self.assertEqual(applicable, ["python-parallel-cpu"])
        self.assertEqual(
            parity_gate_outcome(
                "parallel",
                parallel_subjects,
                applicable,
                {("parallel", "python-parallel-cpu"): {"outcome": "pass"}},
            ),
            "pass",
        )
        self.assertEqual(
            parity_gate_outcome(
                "parallel",
                parallel_subjects,
                applicable,
                {("parallel", "python-cpu"): {"outcome": "pass"}},
            ),
            "not_proven",
        )

    def test_selected_profile_has_matching_backend_and_feature_identity(self) -> None:
        selected_parallel = os.environ.get("MIGRATION_TARGET_PROFILE") == "parallel-cpu"
        self.assertEqual(PARALLEL_CPU_PROFILE, selected_parallel)
        configured_backends = os.environ.get("MIGRATION_BENCHMARK_BACKENDS", "").strip()
        if configured_backends:
            expected_backends = tuple(
                item.strip().lower()
                for item in configured_backends.split(",")
                if item.strip()
            )
        elif selected_parallel:
            expected_backends = ("cpu",)
        else:
            expected_backends = ("cpu", "simd", "gpu")
        self.assertEqual(TARGET_BACKENDS, expected_backends)
        if selected_parallel:
            self.assertEqual(TARGET_BACKENDS, ("cpu",))
            self.assertEqual(TARGET_PROFILES, ("python-parallel-cpu",))
            self.assertEqual(target_profile_for_backend("cpu"), "python-parallel-cpu")
            self.assertEqual(
                TARGET_FEATURES[-2:], ["pillow-rs-py/parallel", "pillow-rs/parallel"]
            )
        else:
            expected_profiles = tuple(f"python-{backend}" for backend in expected_backends)
            self.assertEqual(TARGET_PROFILES, expected_profiles)
            for backend, profile in zip(TARGET_BACKENDS, TARGET_PROFILES, strict=True):
                self.assertEqual(target_profile_for_backend(backend), profile)

    def test_parallel_cpu_times_only_its_named_profile_after_pillow_parity(self) -> None:
        standard_subjects = [
            {"kind": "oracle", "id": "pillow"},
            {"kind": "target_profile", "id": "python-cpu"},
            {"kind": "target_profile", "id": "python-simd"},
            {"kind": "target_profile", "id": "python-gpu"},
        ]
        workload = {"workload_id": "fixture", "subjects": standard_subjects}

        profiled = apply_profile_to_workloads([workload])

        if PARALLEL_CPU_PROFILE:
            self.assertEqual(
                profiled[0]["subjects"],
                [{"kind": "target_profile", "id": "python-parallel-cpu"}],
            )
            self.assertEqual(
                benchmark_subjects(),
                [("target_profile", "python-parallel-cpu")],
            )
        else:
            expected_subjects = standard_subjects[:1] + [
                {"kind": "target_profile", "id": profile} for profile in TARGET_PROFILES
            ]
            self.assertEqual(profiled[0]["subjects"], expected_subjects)
            self.assertEqual(
                benchmark_subjects(),
                [
                    ("oracle", "pillow"),
                    *(('target_profile', profile) for profile in TARGET_PROFILES),
                ],
            )
        self.assertEqual(workload["subjects"], standard_subjects)

    def test_cpu_only_frozen_contract_is_not_expanded_to_acceleration_profiles(self) -> None:
        cpu_only_subjects = [
            {"kind": "oracle", "id": "pillow"},
            {"kind": "target_profile", "id": "python-cpu"},
        ]
        workload = self._subject_contract_workload("cpu-only", cpu_only_subjects)
        with (
            patch.object(run_migration_benchmark, "PARALLEL_CPU_PROFILE", False),
            patch.object(run_migration_benchmark, "PILLOW_SIMD_COMPARISON", False),
            patch.object(run_migration_benchmark, "BENCHMARK_SOURCE_NAME", "Pillow"),
            patch.object(
                run_migration_benchmark,
                "TARGET_BACKENDS",
                ("cpu", "simd", "gpu"),
            ),
        ):
            original = frozen_workload_subjects([workload])
            profiled = apply_profile_to_workloads([workload])
            validate_selected_workloads(profiled, original)

        self.assertEqual(profiled[0]["subjects"], cpu_only_subjects)
        self.assertEqual(workload["subjects"], cpu_only_subjects)

    def test_mixed_frozen_contracts_keep_each_workload_subject_scope(self) -> None:
        full_subjects = [
            {"kind": "oracle", "id": "pillow"},
            {"kind": "target_profile", "id": "python-cpu"},
            {"kind": "target_profile", "id": "python-simd"},
            {"kind": "target_profile", "id": "python-gpu"},
        ]
        cpu_only_subjects = full_subjects[:2]
        workloads = [
            self._subject_contract_workload("full", full_subjects),
            self._subject_contract_workload("cpu-only", cpu_only_subjects),
        ]
        with (
            patch.object(run_migration_benchmark, "PARALLEL_CPU_PROFILE", False),
            patch.object(run_migration_benchmark, "PILLOW_SIMD_COMPARISON", False),
            patch.object(run_migration_benchmark, "BENCHMARK_SOURCE_NAME", "Pillow"),
            patch.object(
                run_migration_benchmark,
                "TARGET_BACKENDS",
                ("cpu", "simd", "gpu"),
            ),
        ):
            original = frozen_workload_subjects(workloads)
            profiled = apply_profile_to_workloads(workloads)
            validate_selected_workloads(profiled, original)

        self.assertEqual(profiled[0]["subjects"], full_subjects)
        self.assertEqual(profiled[1]["subjects"], cpu_only_subjects)

    def test_cpu_profile_intersects_each_frozen_contract(self) -> None:
        full_subjects = [
            {"kind": "oracle", "id": "pillow"},
            {"kind": "target_profile", "id": "python-cpu"},
            {"kind": "target_profile", "id": "python-simd"},
            {"kind": "target_profile", "id": "python-gpu"},
        ]
        cpu_only_subjects = full_subjects[:2]
        workloads = [
            self._subject_contract_workload("full", full_subjects),
            self._subject_contract_workload("cpu-only", cpu_only_subjects),
        ]
        with (
            patch.object(run_migration_benchmark, "PARALLEL_CPU_PROFILE", False),
            patch.object(run_migration_benchmark, "PILLOW_SIMD_COMPARISON", False),
            patch.object(run_migration_benchmark, "BENCHMARK_SOURCE_NAME", "Pillow"),
            patch.object(run_migration_benchmark, "TARGET_BACKENDS", ("cpu",)),
        ):
            original = frozen_workload_subjects(workloads)
            profiled = apply_profile_to_workloads(workloads)
            validate_selected_workloads(profiled, original)

        expected = [
            {"kind": "oracle", "id": "pillow"},
            {"kind": "target_profile", "id": "python-cpu"},
        ]
        self.assertEqual(profiled[0]["subjects"], expected)
        self.assertEqual(profiled[1]["subjects"], expected)

    def test_parallel_cpu_maps_only_a_declared_cpu_subject_to_its_featured_profile(self) -> None:
        cpu_only_subjects = [
            {"kind": "oracle", "id": "pillow"},
            {"kind": "target_profile", "id": "python-cpu"},
        ]
        workload = self._subject_contract_workload("cpu-only", cpu_only_subjects)
        with (
            patch.object(run_migration_benchmark, "PARALLEL_CPU_PROFILE", True),
            patch.object(run_migration_benchmark, "TARGET_BACKENDS", ("cpu",)),
        ):
            original = frozen_workload_subjects([workload])
            profiled = apply_profile_to_workloads([workload])
            validate_selected_workloads(profiled, original)

        self.assertEqual(
            profiled[0]["subjects"],
            [{"kind": "target_profile", "id": "python-parallel-cpu"}],
        )

    def test_pillow_simd_keeps_its_source_id_for_a_cpu_only_contract(self) -> None:
        cpu_only_subjects = [
            {"kind": "oracle", "id": "pillow"},
            {"kind": "target_profile", "id": "python-cpu"},
        ]
        workload = self._subject_contract_workload("cpu-only", cpu_only_subjects)
        with (
            patch.object(run_migration_benchmark, "PARALLEL_CPU_PROFILE", False),
            patch.object(run_migration_benchmark, "PILLOW_SIMD_COMPARISON", True),
            patch.object(run_migration_benchmark, "BENCHMARK_SOURCE_NAME", "Pillow-SIMD"),
            patch.object(run_migration_benchmark, "TARGET_BACKENDS", ("cpu", "simd")),
        ):
            original = frozen_workload_subjects([workload])
            profiled = apply_profile_to_workloads([workload])
            validate_selected_workloads(profiled, original)

        self.assertEqual(
            profiled[0]["subjects"],
            [
                {"kind": "oracle", "id": "pillow-simd"},
                {"kind": "target_profile", "id": "python-cpu"},
            ],
        )

    def test_profiled_workload_cannot_drop_a_declared_requested_subject(self) -> None:
        full_subjects = [
            {"kind": "oracle", "id": "pillow"},
            {"kind": "target_profile", "id": "python-cpu"},
            {"kind": "target_profile", "id": "python-simd"},
            {"kind": "target_profile", "id": "python-gpu"},
        ]
        workload = self._subject_contract_workload("full", full_subjects)
        with (
            patch.object(run_migration_benchmark, "PARALLEL_CPU_PROFILE", False),
            patch.object(run_migration_benchmark, "PILLOW_SIMD_COMPARISON", False),
            patch.object(run_migration_benchmark, "BENCHMARK_SOURCE_NAME", "Pillow"),
            patch.object(
                run_migration_benchmark,
                "TARGET_BACKENDS",
                ("cpu", "simd", "gpu"),
            ),
        ):
            original = frozen_workload_subjects([workload])
            profiled = apply_profile_to_workloads([workload])
            profiled[0]["subjects"] = profiled[0]["subjects"][:-1]

            with self.assertRaisesRegex(
                ValueError, "do not preserve the declared subjects"
            ):
                validate_selected_workloads(profiled, original)

    def test_frozen_subject_contract_rejects_malformed_duplicate_unknown_or_missing(self) -> None:
        invalid_contracts = (
            (
                "malformed",
                [{"kind": "oracle", "id": "pillow", "extra": True}],
                "exactly kind and id",
            ),
            (
                "duplicate",
                [
                    {"kind": "oracle", "id": "pillow"},
                    {"kind": "target_profile", "id": "python-cpu"},
                    {"kind": "target_profile", "id": "python-cpu"},
                ],
                "duplicate subject",
            ),
            (
                "unknown",
                [
                    {"kind": "oracle", "id": "pillow"},
                    {"kind": "target_profile", "id": "python-neon"},
                ],
                "unknown target profile",
            ),
            (
                "missing-oracle",
                [{"kind": "target_profile", "id": "python-cpu"}],
                "exactly one Pillow oracle",
            ),
            (
                "missing-target",
                [{"kind": "oracle", "id": "pillow"}],
                "at least one target profile",
            ),
        )
        for workload_id, subjects, message in invalid_contracts:
            with self.subTest(contract=workload_id):
                workload = self._subject_contract_workload(workload_id, subjects)
                with self.assertRaisesRegex(ValueError, message):
                    frozen_workload_subjects([workload])

    @staticmethod
    def _subject_contract_workload(
        workload_id: str, subjects: list[dict[str, str]]
    ) -> dict:
        return {
            "workload_id": workload_id,
            "subjects": subjects,
            "measurement": {
                "warmup_iterations": 0,
                "measurement_iterations": 1,
                "samples": 1,
            },
        }

    def test_standard_cpu_simd_subset_keeps_pillow_and_requested_backends(self) -> None:
        standard_subjects = [
            {"kind": "oracle", "id": "pillow"},
            {"kind": "target_profile", "id": "python-cpu"},
            {"kind": "target_profile", "id": "python-simd"},
            {"kind": "target_profile", "id": "python-gpu"},
        ]
        workload = {"workload_id": "fixture", "subjects": standard_subjects}
        with patch("run_migration_benchmark.PARALLEL_CPU_PROFILE", False), patch(
            "run_migration_benchmark.PILLOW_SIMD_COMPARISON", False
        ), patch("run_migration_benchmark.TARGET_BACKENDS", ("cpu", "simd")):
            profiled = apply_profile_to_workloads([workload])
            self.assertEqual(profiled[0]["subjects"], standard_subjects[:3])
            self.assertEqual(
                benchmark_subjects(),
                [
                    ("oracle", "pillow"),
                    ("target_profile", "python-cpu"),
                    ("target_profile", "python-simd"),
                ],
            )
        self.assertEqual(workload["subjects"], standard_subjects)

    def test_pillow_simd_profile_keeps_a_distinct_source_and_cpu_simd_targets(self) -> None:
        standard_subjects = [
            {"kind": "oracle", "id": "pillow"},
            {"kind": "target_profile", "id": "python-cpu"},
            {"kind": "target_profile", "id": "python-simd"},
            {"kind": "target_profile", "id": "python-gpu"},
        ]
        workload = {"workload_id": "fixture", "subjects": standard_subjects}
        for source_name, source_id in (
            ("Pillow", "pillow"),
            ("Pillow-SIMD", "pillow-simd"),
        ):
            with (
                self.subTest(source=source_name),
                patch("run_migration_benchmark.PARALLEL_CPU_PROFILE", False),
                patch("run_migration_benchmark.PILLOW_SIMD_COMPARISON", True),
                patch("run_migration_benchmark.TARGET_BACKENDS", ("cpu", "simd")),
                patch("run_migration_benchmark.BENCHMARK_SOURCE_NAME", source_name),
            ):
                profiled = apply_profile_to_workloads([workload])
                source_subjects = [
                    {"kind": "oracle", "id": source_id},
                    *standard_subjects[1:3],
                ]
                self.assertEqual(profiled[0]["subjects"], source_subjects)
                self.assertEqual(
                    benchmark_subjects(),
                    [
                        ("oracle", source_id),
                        ("target_profile", "python-cpu"),
                        ("target_profile", "python-simd"),
                    ],
                )
        self.assertEqual(workload["subjects"], standard_subjects)

    def test_timed_source_can_be_distinct_from_the_parity_oracle(self) -> None:
        payload = {
            "identity": {
                "side": "source",
                "implementation": "Pillow-SIMD",
                "version": "12.1.1.post0",
            },
            "results": [],
            "timings_ns": {},
            "telemetry": {},
            "execution": {},
        }

        def fake_run_process(command, *, env, **_kwargs):
            self.assertEqual(command[0], "/opt/pillow-simd/bin/python")
            self.assertEqual(env["MIGRATION_ORACLE_PYTHON"], "/opt/pillow/bin/python")
            self.assertEqual(env["MIGRATION_BENCHMARK_SOURCE_PYTHON"], command[0])
            self.assertEqual(env["MIGRATION_PARITY_TIMED_SOURCE"], "1")
            return 0, json.dumps(payload), ""

        with (
            patch.dict(
                os.environ,
                {
                    "MIGRATION_ORACLE_PYTHON": "/opt/pillow/bin/python",
                    "MIGRATION_BENCHMARK_SOURCE_PYTHON": "/opt/pillow-simd/bin/python",
                },
                clear=False,
            ),
            patch.object(run_migration_benchmark, "BENCHMARK_SOURCE_NAME", "Pillow-SIMD"),
            patch.object(
                run_migration_benchmark, "BENCHMARK_SOURCE_VERSION", "12.1.1.post0"
            ),
            patch("run_migration_benchmark.run_process", side_effect=fake_run_process),
        ):
            result = run_timed_side(
                "source",
                run_migration_benchmark.DEFAULT_MANIFEST,
                [{"case_id": "fixture"}],
                1,
                30,
                backend="cpu",
                timing_boundary="observed_steps",
                timing_steps=["operation"],
            )

        self.assertEqual(result["identity"]["implementation"], "Pillow-SIMD")

    def test_parity_preflight_does_not_inherit_timed_source_selection(self) -> None:
        observed = {}
        parity_result = {
            "status": "completed",
            "identity": {"run_id": "regular-pillow-preflight"},
            "comparisons": [],
            "infrastructure_errors": [],
        }
        merged_result = {"status": "merged"}

        def fake_run_process(command, *, env, **_kwargs):
            observed.update(env)
            output_path = Path(command[command.index("--output") + 1])
            output_path.write_bytes(b"stub parity receipt")
            return 0, "{}", ""

        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "parity.json"
            with (
                patch.dict(
                    os.environ,
                    {
                        "MIGRATION_ORACLE_PYTHON": "/oracle/pillow/bin/python",
                        "MIGRATION_BENCHMARK_SOURCE_PYTHON": "/oracle/pillow-simd/bin/python",
                        "MIGRATION_PARITY_TIMED_SOURCE": "1",
                    },
                    clear=False,
                ),
                patch.object(run_migration_benchmark, "TARGET_BACKENDS", ("cpu",)),
                patch("run_migration_benchmark.run_process", side_effect=fake_run_process),
                patch(
                    "run_migration_benchmark.read_and_remove_backend_parity",
                    return_value=parity_result,
                ),
                patch(
                    "run_migration_benchmark.merge_parity_results",
                    return_value=merged_result,
                ),
                patch("run_migration_benchmark.write_parity_artifact") as write_artifact,
            ):
                result = run_migration_benchmark.run_parity(
                    run_migration_benchmark.DEFAULT_MANIFEST,
                    output,
                    30,
                    [{"case_id": "fixture", "target_profiles": ["python-cpu"]}],
                )

        self.assertIs(result, merged_result)
        self.assertEqual(observed["MIGRATION_ORACLE_PYTHON"], "/oracle/pillow/bin/python")
        self.assertEqual(
            observed["MIGRATION_BENCHMARK_SOURCE_PYTHON"],
            "/oracle/pillow-simd/bin/python",
        )
        self.assertNotIn("MIGRATION_PARITY_TIMED_SOURCE", observed)
        write_artifact.assert_called_once_with(output, merged_result)

    def test_parity_preflight_merges_backend_attestations_when_requested(self) -> None:
        observed_commands = []
        observed_output_paths = []
        parity_result = {"status": "completed", "identity": {"run_id": "backend"}}
        merged_result = {"status": "completed", "identity": {"run_id": "merged"}}

        def fake_run_process(command, *, env, **_kwargs):
            observed_commands.append(command)
            output_path = Path(command[command.index("--output") + 1])
            observed_output_paths.append(output_path)
            output_path.write_bytes(gzip.compress(b"stub parity receipt"))
            attestation_path = Path(
                command[command.index("--attestation-output") + 1]
            )
            attestation_path.write_text(json.dumps({"sidecar": "captured"}))
            producer_path = Path(
                command[command.index("--producer-attestation-output") + 1]
            )
            producer_path.write_text(json.dumps({"producer": "captured"}))
            producer_path.with_name(producer_path.name + ".bundle.json").write_text(
                json.dumps({"bundle": "captured"})
            )
            return 0, "{}", ""

        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "parity.json"
            attestation_output = Path(directory) / "parity.attestation.json"
            producer_attestation_output = Path(directory) / "parity.producer.json"
            report_binding = {
                "path": "",
                "canonical_sha256": "a" * 64,
                "canonical_bytes": 20,
                "container_sha256": "b" * 64,
                "container_bytes": 30,
            }

            def fake_fingerprint(path):
                return {**report_binding, "path": str(path.resolve())}

            def assert_report_released(*args):
                self.assertFalse(observed_output_paths[0].exists())

            with (
                patch.object(run_migration_benchmark, "TARGET_BACKENDS", ("cpu",)),
                patch("run_migration_benchmark.run_process", side_effect=fake_run_process),
                patch(
                    "run_migration_benchmark.read_parity_artifact",
                    return_value=parity_result,
                ),
                patch(
                    "run_migration_benchmark.merge_parity_results",
                    return_value=merged_result,
                ),
                patch("run_migration_benchmark.execution_report_fingerprint", side_effect=fake_fingerprint),
                patch("run_migration_benchmark.validate_attestation"),
                patch("run_migration_benchmark.write_parity_artifact") as write_artifact,
                patch(
                    "run_migration_benchmark.merge_parity_attestations",
                    side_effect=assert_report_released,
                ) as merge_attestations,
                patch(
                    "run_migration_benchmark.merge_backend_attestations"
                ) as merge_producers,
            ):
                result = run_migration_benchmark.run_parity(
                    run_migration_benchmark.DEFAULT_MANIFEST,
                    output,
                    30,
                    [{"case_id": "fixture", "target_profiles": ["python-cpu"]}],
                    attestation_output=attestation_output,
                    producer_attestation_output=producer_attestation_output,
                )

        self.assertIs(result, merged_result)
        command = observed_commands[0]
        self.assertIn("--attestation-output", command)
        self.assertIn(".attestation.json", command[command.index("--attestation-output") + 1])
        self.assertIn("--producer-attestation-output", command)
        self.assertIn(".producer.json", command[command.index("--producer-attestation-output") + 1])
        write_artifact.assert_called_once_with(output, merged_result)
        merge_attestations.assert_called_once_with(
            attestation_output,
            output,
            merged_result,
            [
                (
                    {"sidecar": "captured"},
                    parity_result,
                    {**report_binding, "path": str(observed_output_paths[0].resolve())},
                )
            ],
        )
        merge_producers.assert_called_once()
        producer_call = merge_producers.call_args.args
        self.assertEqual(producer_call[:4], (
            producer_attestation_output,
            output,
            attestation_output,
            merged_result,
        ))
        self.assertEqual(len(producer_call[4]), 1)
        self.assertTrue(producer_call[4][0][0].name.endswith(".producer.json"))
        self.assertTrue(producer_call[4][0][1].name.endswith(".attestation.json"))
        self.assertTrue(producer_call[4][0][2].name.endswith(".producer.json.bundle.json"))

    def test_reused_parity_accepts_only_attested_passing_supersets(self) -> None:
        # Keep this synthetic case valid when the environment narrows the
        # benchmark's active profiles (for example, to SIMD only).
        case = {
            "case_id": "selected",
            "target_profiles": list(FROZEN_TARGET_PROFILES),
        }
        selected_profiles = [
            profile
            for profile in TARGET_PROFILES
            if profile_applies_to_case(profile, case)
        ]
        input_record = {
            "path": "synthetic-input.json",
            "schema": "migration-parity/parity-input@1",
            "sha256": "c" * 64,
        }
        source_sha256 = "a" * 64
        target_sha256 = "b" * 64
        runtime_identity_hashes = [(('source', 'pillow'), source_sha256)] + [
            (('target', profile), target_sha256) for profile in selected_profiles
        ]
        comparison_rows = [
            {
                "case_id": case_id,
                "target_profile": profile,
                "outcome": "pass",
            }
            for profile in selected_profiles
            for case_id in ("selected", "extra-passing-guard")
        ]
        comparison_count = len(comparison_rows)
        parity = {
            "schema": "migration-parity/parity-result@1",
            "status": "completed",
            "identity": {
                "manifest": {"sha256": sha256(run_migration_benchmark.DEFAULT_MANIFEST)},
                "inputs": [input_record],
                "oracles": [
                    {
                        "name": run_migration_parity.ORACLE_NAME,
                        "version": run_migration_parity.ORACLE_VERSION,
                    }
                ],
                "targets": [
                    {
                        "target_profile": profile,
                        "target_id": run_migration_parity.TARGET_ID,
                        "revision": run_migration_parity.git_revision(),
                        "dirty": run_migration_parity.git_dirty(),
                        "backend": runtime_backend_for_profile(profile),
                        "features": TARGET_FEATURES.copy(),
                    }
                    for profile in selected_profiles
                ],
            },
            "summary": {
                "selected": comparison_count,
                "executed": comparison_count,
                "passed": comparison_count,
                "failed": 0,
                "not_run": 0,
                "infrastructure_errors": 0,
            },
            "comparisons": comparison_rows,
            "infrastructure_errors": [],
        }
        attestation = {
            "report": {
                "canonical_sha256": "d" * 64,
                "canonical_bytes": 10,
                "container_sha256": "e" * 64,
                "container_bytes": 10,
            },
            "invocations": [
                {
                    "side": side,
                    "profile": profile,
                    "role": "parity",
                    "receipt": {"runtime": {"stable_sha256": stable_sha256}},
                }
                for (side, profile), stable_sha256 in runtime_identity_hashes
            ],
        }
        report_path = Path("synthetic-parity.json")
        report_fingerprint = {
            "path": str(report_path.resolve()),
            "canonical_sha256": "d" * 64,
            "canonical_bytes": 10,
            "container_sha256": "e" * 64,
            "container_bytes": 10,
        }
        with (
            patch.object(run_migration_benchmark, "validate_attestation"),
            patch.object(
                run_migration_benchmark,
                "execution_report_fingerprint",
                return_value=report_fingerprint,
            ),
        ):
            runtime_hashes = validate_reused_parity(
                parity,
                run_migration_benchmark.DEFAULT_MANIFEST,
                [case],
                report_path=report_path,
                attestation=attestation,
                expected_inputs=[input_record],
            )
            self.assertEqual(runtime_hashes[("source", "pillow")], source_sha256)
            for profile in selected_profiles:
                self.assertEqual(runtime_hashes[("target", profile)], target_sha256)

            incomplete = copy.deepcopy(parity)
            incomplete["comparisons"] = [
                row for row in incomplete["comparisons"]
                if row["case_id"] != "selected"
            ]
            incomplete_count = len(incomplete["comparisons"])
            incomplete["summary"].update(
                selected=incomplete_count,
                executed=incomplete_count,
                passed=incomplete_count,
            )
            with self.assertRaisesRegex(ValueError, "does not cover"):
                validate_reused_parity(
                    incomplete,
                    run_migration_benchmark.DEFAULT_MANIFEST,
                    [case],
                    report_path=report_path,
                    attestation=attestation,
                    expected_inputs=[input_record],
                )

            failed_extra = copy.deepcopy(parity)
            failed_extra["comparisons"][1]["outcome"] = "fail"
            failed_extra["summary"].update(passed=1, failed=1)
            with self.assertRaisesRegex(ValueError, "complete passing"):
                validate_reused_parity(
                    failed_extra,
                    run_migration_benchmark.DEFAULT_MANIFEST,
                    [case],
                    report_path=report_path,
                    attestation=attestation,
                    expected_inputs=[input_record],
                )

            duplicate = copy.deepcopy(parity)
            duplicate["comparisons"].append(copy.deepcopy(duplicate["comparisons"][0]))
            duplicate_count = len(duplicate["comparisons"])
            duplicate["summary"].update(
                selected=duplicate_count,
                executed=duplicate_count,
                passed=duplicate_count,
            )
            with self.assertRaisesRegex(ValueError, "duplicate"):
                validate_reused_parity(
                    duplicate,
                    run_migration_benchmark.DEFAULT_MANIFEST,
                    [case],
                    report_path=report_path,
                    attestation=attestation,
                    expected_inputs=[input_record],
                )

            changed_input = dict(input_record, sha256="f" * 64)
            with self.assertRaisesRegex(ValueError, "selected input bytes"):
                validate_reused_parity(
                    parity,
                    run_migration_benchmark.DEFAULT_MANIFEST,
                    [case],
                    report_path=report_path,
                    attestation=attestation,
                    expected_inputs=[changed_input],
                )


class BenchmarkTimingGroupTests(unittest.TestCase):
    def run_shared_case(self, scenario: str, capture: bool):
        case_id = "synthetic.shared-case"
        case = {"case_id": case_id, "target_profiles": ["python-cpu"], "steps": [], "assets": []}
        base = {
            "input": {"kind": "parity_case", "case_id": case_id},
            "covers": ["synthetic.requirement"],
            "subjects": [
                {"kind": "oracle", "id": "pillow"},
                {"kind": "target_profile", "id": "python-cpu"},
            ],
            "measurement": {
                "correctness_gate": "parity_pass",
                "boundary": "observed_steps",
                "step_ids": ["call", "materialize"],
                "cache_state": "warm",
                "warmup_iterations": 1,
                "measurement_iterations": 2,
                "samples": 1,
                "metrics": ["latency", "throughput"],
            },
            "context": {
                "size": [1, 1], "mode": "RGB", "chain_length": 1,
                "operation_class": "point", "cache_state": "warm", "build_profile": "release",
            },
        }
        first, second = copy.deepcopy(base), copy.deepcopy(base)
        first["workload_id"], second["workload_id"] = "A.full", "B.call"
        second["measurement"]["step_ids"] = ["call"]
        if scenario == "different-repeats":
            second["measurement"]["measurement_iterations"] = 3
        if scenario in {"same-group", "mixed-subjects"}:
            second["measurement"]["step_ids"] = ["call", "materialize"]
        if scenario == "mixed-subjects":
            case["target_profiles"].append("python-simd")
            second["subjects"].append({"kind": "target_profile", "id": "python-simd"})
        workloads = {workload["workload_id"]: workload for workload in [first, second]}
        identity = synthetic_report([])["identity"]
        identity["oracles"][0].update(oracle_id="pillow", name="Pillow")
        if scenario == "mixed-subjects":
            simd_identity = dict(identity["targets"][0])
            simd_identity.update(target_profile="python-simd", backend="simd")
            identity["targets"].append(simd_identity)
        parity = {
            "identity": {"run_id": "synthetic-parity"},
            "comparisons": [
                {"case_id": case_id, "target_profile": profile, "outcome": "pass"}
                for profile in case["target_profiles"]
            ],
        }
        calls = []

        def timed_side(side, _manifest, cases, repeat, _timeout, *, backend,
                       timing_boundary, timing_steps, lifecycle, raw_sample_capture=None,
                       raw_sample_workloads_by_case=None, **_kwargs):
            full = "materialize" in timing_steps
            nanoseconds = (9_000_000 if side == "source" else 6_000_000) if full else (
                3_000_000 if side == "source" else 2_000_000
            )
            timings = {case_id: [nanoseconds] * repeat}
            telemetry = {case_id: [{"pipeline_ns": nanoseconds, "total_ns": nanoseconds}] * repeat}
            marker = "full" if full else "call"
            calls.append({
                "side": side, "case_ids": [case["case_id"] for case in cases],
                "profiles": [case["target_profiles"] for case in cases], "repeat": repeat,
            })
            if raw_sample_capture is not None:
                raw_sample_capture.record_timed_child(
                    group_id=f"{side}-{backend}-{marker}", side=side,
                    profile="pillow" if side == "source" else f"python-{backend}",
                    cases=cases, workload_ids_by_case=raw_sample_workloads_by_case,
                    planned_repeat=repeat, child_repeat=repeat, lifecycle=lifecycle,
                    boundary=timing_boundary, step_ids=timing_steps,
                    timings_ns=timings, telemetry=telemetry,
                    child_invocation_id=f"{side}-{backend}-{marker}",
                )
            failed = scenario == "late-failure" and not full
            return {
                "identity": {"side": side},
                "results": [{"case_id": case_id, "status": "failed" if failed else "completed",
                             "execution_errors": [{"marker": marker}] if failed else []}],
                "timings_ns": timings, "telemetry": telemetry,
                "execution": {case_id: [{"status": "not_applicable", "fallback_reason": marker}]},
            }

        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            args = argparse.Namespace(
                manifest=Path(run_migration_benchmark.__file__), output=directory / "report.json",
                parity_output=directory / "parity.json",
                raw_samples_output=directory / "raw.json" if capture else None,
                reuse_parity=None, pipeline=False, workload_id=None, limit=None, timeout=30,
            )
            with ExitStack() as stack:
                for name, value in {
                    "TARGET_BACKENDS": ("cpu", "simd") if scenario == "mixed-subjects" else ("cpu",),
                    "TARGET_PROFILES": tuple(case["target_profiles"]),
                    "PARALLEL_CPU_PROFILE": False, "PILLOW_SIMD_COMPARISON": False,
                    "BENCHMARK_SOURCE_NAME": "Pillow",
                }.items():
                    stack.enter_context(patch.object(run_migration_benchmark, name, value))
                for name, value in {
                    "load_manifest": {},
                    "load_parity_cases": ({case_id: case}, {case_id: "synthetic-cases.json"}),
                    "load_benchmarks": (workloads, [], {key: "synthetic-benchmarks.json" for key in workloads}),
                    "run_parity": parity, "identity": identity,
                }.items():
                    stack.enter_context(patch.object(run_migration_benchmark, name, return_value=value))
                stack.enter_context(patch.object(run_migration_benchmark, "run_timed_side", side_effect=timed_side))
                with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                    self.assertEqual(run_migration_benchmark.run(args), 0)
            report = json.loads(args.output.read_text())
            raw = json.loads(args.raw_samples_output.read_text()) if capture else None
            return report["workloads"], raw, calls

    def test_shared_case_preserves_boundary_samples_phases_and_execution(self):
        for capture in (False, True):
            with self.subTest(capture=capture):
                rows, raw, calls = self.run_shared_case("different-steps", capture)
                self.assertEqual(len(calls), 4)
                for row, expected, marker in zip(rows, [(9, 6), (3, 2)], ["full", "call"]):
                    self.assertEqual(row["correctness"]["outcome"], "pass")
                    for subject, median in zip(row["subjects"], expected):
                        self.assertEqual(subject["measurements"][0]["statistics"]["median"], median)
                        self.assertEqual(subject["phases"]["pipeline"]["statistics"]["median"], median)
                        if subject["kind"] == "target_profile":
                            self.assertEqual(subject["execution"]["fallback_reason_counts"], {marker: 1})
                if raw is not None:
                    self.assertEqual(len(raw["records"]), 4)
                    self.assertEqual([r["workload_ids"] for r in raw["records"]],
                                     [["A.full"], ["A.full"], ["B.call"], ["B.call"]])

    def test_shared_case_preserves_each_sampling_count(self):
        for capture in (False, True):
            with self.subTest(capture=capture):
                rows, _, calls = self.run_shared_case("different-repeats", capture)
                self.assertEqual([call["repeat"] for call in calls], [3, 3, 4, 4])
                for row, count in zip(rows, [2, 3]):
                    self.assertEqual(row["correctness"]["outcome"], "pass")
                    self.assertEqual([s["measurements"][0]["sample_count"] for s in row["subjects"]], [count, count])

    def test_later_failure_does_not_erase_earlier_success(self):
        for capture in (False, True):
            with self.subTest(capture=capture):
                rows, _, _ = self.run_shared_case("late-failure", capture)
                self.assertEqual(rows[0]["correctness"]["outcome"], "pass")
                self.assertEqual(rows[1]["correctness"]["outcome"], "not_proven")
                self.assertTrue(all(s["status"] == "completed" for s in rows[0]["subjects"]))
                self.assertTrue(all(s["status"] == "failed" for s in rows[1]["subjects"]))
                for subject in rows[0]["subjects"]:
                    self.assertEqual(subject["execution"]["errors"], [])
                self.assertEqual(rows[1]["subjects"][1]["execution"]["errors"], [{"marker": "call"}])

    def test_same_policy_aliases_share_one_child_case_and_keep_both_rows(self):
        for capture in (False, True):
            with self.subTest(capture=capture):
                rows, raw, calls = self.run_shared_case("same-group", capture)
                self.assertEqual(len(calls), 2)
                self.assertTrue(all(call["case_ids"] == ["synthetic.shared-case"] for call in calls))
                self.assertTrue(all(row["correctness"]["outcome"] == "pass" for row in rows))
                if raw is not None:
                    self.assertEqual(len(raw["records"]), 2)
                    self.assertTrue(all(r["workload_ids"] == ["A.full", "B.call"] for r in raw["records"]))

    def test_aliases_union_child_profiles_without_expanding_workload_subjects(self):
        for capture in (False, True):
            with self.subTest(capture=capture):
                rows, raw, calls = self.run_shared_case("mixed-subjects", capture)
                self.assertEqual(len(calls), 3)
                self.assertTrue(all(call["case_ids"] == ["synthetic.shared-case"] for call in calls))
                self.assertTrue(all(call["profiles"] == [["python-cpu", "python-simd"]] for call in calls))
                self.assertEqual([s["id"] for s in rows[0]["subjects"]], ["pillow", "python-cpu"])
                self.assertEqual([s["id"] for s in rows[1]["subjects"]], ["pillow", "python-cpu", "python-simd"])
                self.assertTrue(all(row["correctness"]["outcome"] == "pass" for row in rows))
                if raw is not None:
                    self.assertEqual(len(raw["records"]), 3)
                    self.assertEqual(raw["records"][-1]["workload_ids"], ["B.call"])



if __name__ == "__main__":
    unittest.main()
