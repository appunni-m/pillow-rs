"""Negative conformance tests for the fixed manifest/input interfaces."""

from __future__ import annotations

import copy
import json
import os
import signal
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import yaml

from scripts import run_migration_parity
from scripts import run_migration_benchmark
from scripts import run_all_backend_tests
from scripts import build_migration_parity_inputs

from scripts.validate_migration_parity_contract import (
    _manifest_operation_index,
    _requirement_index,
    _validate_benchmark_document,
    _validate_coverage_document,
    _validate_parity_document,
    validate_inputs,
    validate_manifest,
)


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "pillow-rs" / "tests" / "fixtures"
MANIFEST_PATH = FIXTURE_ROOT / "manifest.yaml"


class MigrationParityContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.manifest = yaml.safe_load(MANIFEST_PATH.read_text(encoding="utf-8"))
        cls.operations = _manifest_operation_index(cls.manifest)
        cls.requirements = _requirement_index(cls.manifest)
        cls.profiles = {item["id"] for item in cls.manifest["target_profiles"]}
        cls.commands = {item["id"] for item in cls.manifest["commands"]}
        cls.components = {item["id"] for item in cls.manifest["coverage_components"]}

    def test_active_tree_is_fully_conformant(self) -> None:
        validate_manifest(self.manifest, manifest_path=MANIFEST_PATH)
        validate_inputs(self.manifest, FIXTURE_ROOT)

    def test_gpu_adapter_timeout_is_bounded(self) -> None:
        original_backend = run_migration_parity.TARGET_BACKEND
        try:
            run_migration_parity.TARGET_BACKEND = "gpu"
            with mock.patch.dict(
                os.environ, {"MIGRATION_GPU_TIMEOUT_SECONDS": "120"}
            ):
                self.assertEqual(
                    run_migration_parity.effective_adapter_timeout(3600), 120
                )
                self.assertEqual(
                    run_migration_parity.effective_adapter_timeout(30), 30
                )
            with mock.patch.dict(
                os.environ, {"MIGRATION_GPU_TIMEOUT_SECONDS": "999"}
            ):
                self.assertEqual(
                    run_migration_parity.effective_adapter_timeout(3600), 300
                )
        finally:
            run_migration_parity.TARGET_BACKEND = original_backend

    def test_full_gpu_is_requested_by_default(self) -> None:
        self.assertTrue(run_all_backend_tests.parse_args([]).gpu_full)
        self.assertFalse(
            run_all_backend_tests.parse_args(["--no-gpu-full"]).gpu_full
        )

    def test_full_gpu_outer_timeout_is_independent_and_bounded(self) -> None:
        self.assertEqual(
            run_all_backend_tests.parity_lane_timeout(
                "gpu", requested_seconds=7200, smoke=False
            ),
            300,
        )
        self.assertEqual(
            run_all_backend_tests.parity_lane_timeout(
                "gpu", requested_seconds=30, smoke=False
            ),
            30,
        )
        self.assertEqual(
            run_all_backend_tests.parity_lane_timeout(
                "cpu", requested_seconds=7200, smoke=False
            ),
            7200,
        )

    def test_benchmark_profiles_are_backend_specific(self) -> None:
        self.assertEqual(
            run_migration_benchmark.benchmark_subjects(),
            [
                ("oracle", "pillow"),
                ("target_profile", "python-cpu"),
                ("target_profile", "python-simd"),
                ("target_profile", "python-gpu"),
            ],
        )
        self.assertTrue(
            {"python-cpu", "python-simd", "python-gpu"}.issubset(
                self.profiles
            )
        )

    def test_benchmark_inputs_measure_complete_workflows(self) -> None:
        expected_subjects = [
            {"kind": kind, "id": subject_id}
            for kind, subject_id in run_migration_benchmark.benchmark_subjects()
        ]
        for relative in self.manifest["input_index"]["benchmark"]:
            document = json.loads(
                (FIXTURE_ROOT / relative).read_text(encoding="utf-8")
            )
            for workload in document["workloads"]:
                self.assertEqual(workload["subjects"], expected_subjects)
                pipeline = (
                    build_migration_parity_inputs.BENCHMARK_PIPELINE_WORKLOADS.get(
                        workload["workload_id"]
                    )
                )
                indexed_pipeline = workload["workload_id"].startswith("pipeline-op.")
                quick_pipeline = workload["workload_id"].startswith("pipeline.quick.")
                self.assertEqual(
                    workload["measurement"]["boundary"],
                    "observed_steps"
                    if pipeline or indexed_pipeline or quick_pipeline
                    else "whole_workflow",
                )
                self.assertEqual(
                    workload["measurement"]["step_ids"],
                    pipeline["step_ids"]
                    if pipeline
                    else ["call", "materialize"]
                    if indexed_pipeline
                    else ["pipeline-primary", "pipeline-secondary", "materialize"]
                    if quick_pipeline
                    else [],
                )

    def test_benchmark_rejects_partial_backend_subjects(self) -> None:
        document = json.loads(
            (FIXTURE_ROOT / "inputs/benchmark/pil-image.json").read_text(
                encoding="utf-8"
            )
        )
        workload = copy.deepcopy(document["workloads"][0])
        workload["subjects"] = workload["subjects"][:-1]
        with self.assertRaisesRegex(
            ValueError, "Pillow/CPU/SIMD/GPU contract"
        ):
            run_migration_benchmark.validate_selected_workloads([workload])

    def test_grouped_benchmark_timing_excludes_result_encoding(self) -> None:
        document = json.loads(
            (FIXTURE_ROOT / "inputs/parity/pil-image-image.json").read_text(
                encoding="utf-8"
            )
        )
        case = next(
            item
            for item in document["cases"]
            if item["case_id"]
            == "PIL.Image.Image.transpose.benchmark.materialized-pipeline-1024"
        )
        policy = build_migration_parity_inputs.BENCHMARK_PIPELINE_WORKLOADS[
            "pil-image-image.transpose.standard"
        ]
        timings: list[int] = []
        with (
            tempfile.TemporaryDirectory() as directory,
            mock.patch.object(
                run_migration_parity,
                "call_workflow_step",
                return_value=object(),
            ),
            mock.patch.object(
                run_migration_parity.time,
                "perf_counter_ns",
                side_effect=[100, 350],
            ),
            mock.patch.object(
                run_migration_parity, "serialize_value"
            ) as serialize,
        ):
            result = run_migration_parity.run_case(
                "source",
                case,
                run_migration_parity.build_operation_index(self.manifest),
                Path(directory),
                timing_steps=set(policy["step_ids"]),
                timing_sink=timings,
                timing_boundary="observed_steps",
                serialize_observations=False,
            )
        self.assertEqual(timings, [250])
        self.assertEqual(result["observations"], [])
        serialize.assert_not_called()

    def test_gpu_benchmark_timeout_is_bounded(self) -> None:
        with mock.patch.dict(
            os.environ,
            {"MIGRATION_GPU_BENCHMARK_TIMEOUT_SECONDS": "900"},
        ):
            self.assertEqual(
                run_migration_benchmark.gpu_benchmark_timeout(7200), 900
            )
            self.assertEqual(
                run_migration_benchmark.gpu_benchmark_timeout(30), 30
            )
        with mock.patch.dict(
            os.environ,
            {"MIGRATION_GPU_BENCHMARK_TIMEOUT_SECONDS": "9999"},
        ):
            self.assertEqual(
                run_migration_benchmark.gpu_benchmark_timeout(7200), 1800
            )

    @unittest.skipIf(os.name == "nt", "POSIX process-group contract")
    def test_benchmark_timeout_kills_group_and_reaps_child(self) -> None:
        process = mock.Mock()
        process.returncode = -signal.SIGKILL
        process.communicate.side_effect = subprocess.TimeoutExpired(
            ["python", "benchmark"], 1
        )
        with (
            mock.patch.object(
                run_migration_benchmark.subprocess,
                "Popen",
                return_value=process,
            ) as popen,
            mock.patch.object(
                run_migration_benchmark,
                "reap_timed_out_process",
                return_value=("", "driver wedged"),
            ) as reap,
        ):
            with self.assertRaisesRegex(
                RuntimeError, "GPU benchmark timed out after 1s"
            ):
                run_migration_benchmark.run_process(
                    ["python", "benchmark"],
                    env={},
                    timeout=1,
                    label="GPU benchmark",
                )
        self.assertTrue(popen.call_args.kwargs["start_new_session"])
        reap.assert_called_once_with(process)

    @unittest.skipIf(os.name == "nt", "POSIX process-group contract")
    def test_outer_timeout_kills_group_and_reaps_child(self) -> None:
        process = mock.Mock()
        process.pid = 4242
        process.returncode = -signal.SIGKILL
        process.communicate.side_effect = [
            subprocess.TimeoutExpired(["make", "migration-parity-test"], 1),
            ("partial stdout", "partial stderr"),
        ]
        with (
            mock.patch.object(
                run_all_backend_tests.subprocess, "Popen", return_value=process
            ) as popen,
            mock.patch.object(run_all_backend_tests.os, "killpg") as killpg,
        ):
            returncode, stdout, stderr, timed_out = (
                run_all_backend_tests.run_command(
                    ["make", "migration-parity-test"], timeout_seconds=1
                )
            )
        self.assertEqual(returncode, 124)
        self.assertTrue(timed_out)
        self.assertEqual(stdout, "partial stdout")
        self.assertEqual(stderr, "partial stderr")
        self.assertTrue(popen.call_args.kwargs["start_new_session"])
        killpg.assert_called_once_with(4242, signal.SIGKILL)
        self.assertEqual(process.communicate.call_count, 2)

    @unittest.skipIf(os.name == "nt", "POSIX process-group contract")
    def test_adapter_timeout_kills_group_and_reaps_child(self) -> None:
        process = mock.Mock()
        process.pid = 4343
        process.returncode = -signal.SIGKILL
        process.communicate.side_effect = [
            subprocess.TimeoutExpired(["python", "--side", "target"], 1),
            ("", "driver wedged"),
        ]
        with (
            mock.patch.object(
                run_migration_parity.subprocess, "Popen", return_value=process
            ) as popen,
            mock.patch.object(run_migration_parity.os, "killpg") as killpg,
        ):
            with self.assertRaisesRegex(
                RuntimeError, "target adapter timed out after 1s"
            ):
                run_migration_parity.run_side_subprocess(
                    "target", MANIFEST_PATH, [], 1
                )
        self.assertTrue(popen.call_args.kwargs["start_new_session"])
        killpg.assert_called_once_with(4343, signal.SIGKILL)
        self.assertEqual(process.communicate.call_count, 2)

    def test_gpu_smoke_timeout_is_a_real_failure(self) -> None:
        with (
            tempfile.TemporaryDirectory() as directory,
            mock.patch.object(
                run_all_backend_tests,
                "run_command",
                return_value=(124, "", "driver wedged", True),
            ),
        ):
            lane = run_all_backend_tests.run_parity_lane(
                "gpu",
                output_dir=Path(directory),
                timeout_seconds=7200,
                smoke=True,
            )
        self.assertEqual(lane["status"], "failed")
        self.assertTrue(lane["timed_out"])
        self.assertIn("after 180s", lane["reason"])

    def test_gpu_smoke_without_adapter_is_skippable(self) -> None:
        result = {
            "comparisons": [
                {
                    "target": {
                        "observations": [
                            {
                                "error": {
                                    "message": (
                                        "GPU adapter not available: enumerated=0 adapters=[]"
                                    )
                                }
                            }
                        ]
                    }
                }
            ]
        }
        self.assertTrue(run_all_backend_tests.gpu_adapter_unavailable(result))
        with (
            tempfile.TemporaryDirectory() as directory,
            mock.patch.object(
                run_all_backend_tests,
                "run_command",
                return_value=(2, "", "adapter unavailable", False),
            ),
            mock.patch.object(
                run_all_backend_tests, "result_document", return_value=result
            ),
        ):
            lane = run_all_backend_tests.run_parity_lane(
                "gpu",
                output_dir=Path(directory),
                timeout_seconds=7200,
                smoke=True,
            )
        self.assertEqual(lane["status"], "skipped")
        self.assertIn("adapter unavailable", lane["reason"].lower())

    def test_gpu_smoke_kernel_failure_is_not_skippable(self) -> None:
        result = {
            "comparisons": [
                {
                    "target": {
                        "observations": [
                            {"error": {"message": "GPU readback map_async failed"}}
                        ]
                    }
                }
            ]
        }
        self.assertFalse(run_all_backend_tests.gpu_adapter_unavailable(result))

    def test_manifest_unknown_field_is_rejected(self) -> None:
        invalid = copy.deepcopy(self.manifest)
        invalid["extra"] = True
        with self.assertRaises(ValueError):
            validate_manifest(invalid)

    def test_parity_unknown_case_field_is_rejected(self) -> None:
        document = json.loads(
            (FIXTURE_ROOT / self.manifest["input_index"]["parity"][0]).read_text(
                encoding="utf-8"
            )
        )
        document["cases"][0]["extra"] = True
        with self.assertRaises(ValueError):
            _validate_parity_document(
                document,
                "parity.json",
                self.operations,
                self.requirements,
                self.profiles,
                FIXTURE_ROOT,
                set(),
            )

    def test_parity_missing_required_argument_is_rejected(self) -> None:
        document = json.loads(
            (FIXTURE_ROOT / "inputs/parity/pil-image.json").read_text(
                encoding="utf-8"
            )
        )
        case = copy.deepcopy(document["cases"][0])
        call = next(step for step in case["steps"] if step["step_id"] == "call")
        operation = self.operations[(call["surface"], call["operation"])]
        required = next(
            parameter["id"]
            for parameter in operation["source"]["parameters"]
            if parameter["omission"]["kind"] == "required"
            and parameter["style"] != "receiver"
        )
        call["arguments"].pop(required, None)
        document["cases"] = [case]
        with self.assertRaises(ValueError):
            _validate_parity_document(
                document,
                "parity.json",
                self.operations,
                self.requirements,
                self.profiles,
                FIXTURE_ROOT,
                set(),
            )

    def test_coverage_empty_selector_is_rejected(self) -> None:
        document = json.loads(
            (FIXTURE_ROOT / "inputs/coverage/pil-image.json").read_text(
                encoding="utf-8"
            )
        )
        document["plans"][0]["selectors"] = {
            "parity_case_ids": [],
            "command_ids": [],
        }
        with self.assertRaises(ValueError):
            _validate_coverage_document(
                document,
                "coverage.json",
                self.operations,
                self.requirements,
                self.profiles,
                self.commands,
                self.components,
                set(),
                set(),
            )

    def test_benchmark_expected_output_is_rejected(self) -> None:
        document = json.loads(
            (FIXTURE_ROOT / "inputs/benchmark/pil-image.json").read_text(
                encoding="utf-8"
            )
        )
        document["workloads"][0]["expected_output"] = {"bytes": "forbidden"}
        with self.assertRaises(ValueError):
            _validate_benchmark_document(
                document,
                "benchmark.json",
                self.operations,
                self.requirements,
                self.profiles,
                set(),
            )


if __name__ == "__main__":
    unittest.main()
