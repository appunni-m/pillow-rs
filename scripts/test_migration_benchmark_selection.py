"""Regression tests for composing migration benchmark workload filters."""
from __future__ import annotations

import gzip
import json
import unittest

import os
from pathlib import Path
import tempfile
from unittest.mock import patch

from run_migration_benchmark import (
    PARALLEL_CPU_PROFILE,
    TARGET_BACKENDS,
    TARGET_PROFILES,
    apply_profile_to_workloads,
    benchmark_subjects,
    profile_applies_to_case,
    runtime_backend_for_profile,
    read_and_remove_backend_parity,
    run_timed_side,
    select_workloads,
    sha256,
    validate_reused_parity,
    write_parity_artifact,
)
import run_migration_benchmark
import run_migration_parity
from run_migration_parity import TARGET_FEATURES, target_profile_for_backend


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
        with patch("run_migration_benchmark.PARALLEL_CPU_PROFILE", False), patch(
            "run_migration_benchmark.PILLOW_SIMD_COMPARISON", True
        ), patch("run_migration_benchmark.TARGET_BACKENDS", ("cpu", "simd")):
            profiled = apply_profile_to_workloads([workload])
            pillow_simd_subjects = [
                {"kind": "oracle", "id": "pillow-simd"},
                *standard_subjects[1:3],
            ]
            self.assertEqual(
                profiled[0]["subjects"],
                pillow_simd_subjects,
            )
            self.assertEqual(
                benchmark_subjects(),
                [
                    ("oracle", "pillow-simd"),
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

    def test_reused_parity_receipt_must_cover_every_selected_profile(self) -> None:
        case = {
            "case_id": "fixture",
            # Inputs declare capability applicability: Parallel CPU reuses
            # the CPU declaration while keeping its own selected profile ID.
            "target_profiles": ["python-cpu", "python-simd", "python-gpu"],
        }
        selected_profiles = [
            profile
            for profile in TARGET_PROFILES
            if profile_applies_to_case(profile, case)
        ]
        parity = {
            "schema": "migration-parity/parity-result@1",
            "status": "completed",
            "identity": {
                "manifest": {"sha256": sha256(run_migration_benchmark.DEFAULT_MANIFEST)},
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
            "summary": {"failed": 0, "not_run": 0},
            "comparisons": [
                {
                    "case_id": "fixture",
                    "target_profile": profile,
                    "outcome": "pass",
                }
                for profile in selected_profiles
            ],
            "infrastructure_errors": [],
        }
        validate_reused_parity(
            parity, run_migration_benchmark.DEFAULT_MANIFEST, [case]
        )
        incomplete = dict(parity)
        incomplete["comparisons"] = parity["comparisons"][:-1]
        with self.assertRaisesRegex(ValueError, "does not cover"):
            validate_reused_parity(
                incomplete, run_migration_benchmark.DEFAULT_MANIFEST, [case]
            )


if __name__ == "__main__":
    unittest.main()
