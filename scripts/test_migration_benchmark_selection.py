"""Regression tests for composing migration benchmark workload filters."""
from __future__ import annotations

import gzip
import json
import unittest

import os
from pathlib import Path
import tempfile

from run_migration_benchmark import (
    PARALLEL_CPU_PROFILE,
    TARGET_BACKENDS,
    TARGET_PROFILES,
    apply_profile_to_workloads,
    benchmark_subjects,
    profile_applies_to_case,
    runtime_backend_for_profile,
    read_and_remove_backend_parity,
    select_workloads,
    write_parity_artifact,
)
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

    def test_unknown_workload_ids_remain_errors(self) -> None:
        with self.assertRaisesRegex(ValueError, "unknown benchmark workload"):
            select_workloads(
                self.workloads,
                pipeline=False,
                workload_ids=["missing.workload"],
                limit=None,
            )

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
        if selected_parallel:
            self.assertEqual(TARGET_BACKENDS, ("cpu",))
            self.assertEqual(TARGET_PROFILES, ("python-parallel-cpu",))
            self.assertEqual(target_profile_for_backend("cpu"), "python-parallel-cpu")
            self.assertEqual(
                TARGET_FEATURES[-2:], ["pillow-rs-py/parallel", "pillow-rs/parallel"]
            )
        else:
            self.assertEqual(TARGET_BACKENDS, ("cpu", "simd", "gpu"))
            self.assertEqual(TARGET_PROFILES, ("python-cpu", "python-simd", "python-gpu"))

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
            self.assertEqual(profiled[0]["subjects"], standard_subjects)
            self.assertEqual(
                benchmark_subjects(),
                [
                    ("oracle", "pillow"),
                    ("target_profile", "python-cpu"),
                    ("target_profile", "python-simd"),
                    ("target_profile", "python-gpu"),
                ],
            )
        self.assertEqual(workload["subjects"], standard_subjects)


if __name__ == "__main__":
    unittest.main()
