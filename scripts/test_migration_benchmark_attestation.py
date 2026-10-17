"""Focused contracts for optional migration benchmark attestation."""
from __future__ import annotations

import argparse
import hashlib
import importlib
import json
from contextlib import ExitStack
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch


SCRIPTS = Path(__file__).resolve().parent
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))
benchmark = importlib.import_module("run_migration_benchmark")
attestation_module = importlib.import_module("migration_execution_attestation")


class RecordingCollector:
    """Record orchestration calls without depending on receipt implementation."""

    def __init__(self) -> None:
        self.requests: list[dict[str, object]] = []
        self.collected: list[dict[str, str]] = []
        self.events: list[tuple[str, str]] = []
        self.execution_paths: dict[str, Path | None] = {}
        self.receipt_alive_when_collected: list[bool] = []

    def child_environment(
        self,
        *,
        side: str,
        profile: str,
        role: str,
        cases: list[dict[str, object]],
        context: dict[str, object],
    ) -> dict[str, str]:
        token = f"child-{len(self.requests)}"
        self.events.append((token, "environment"))
        self.requests.append(
            {
                "side": side,
                "profile": profile,
                "role": role,
                "cases": cases,
                "context": context,
            }
        )
        return {"MIGRATION_TEST_ATTESTATION_TOKEN": token}

    def collect(self, overrides: dict[str, str]) -> None:
        token = overrides["MIGRATION_TEST_ATTESTATION_TOKEN"]
        self.events.append((token, "collect"))
        self.collected.append(overrides)
        path = self.execution_paths[token]
        self.receipt_alive_when_collected.append(
            path is None or path.is_file()
        )


class MigrationBenchmarkAttestationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.cases = [
            {"case_id": "case-a", "steps": [{"op": "invert"}]},
            {"case_id": "case-b", "steps": [{"op": "transpose"}]},
        ]

    def fake_process(self, collector: RecordingCollector | None = None):
        calls: list[dict[str, object]] = []

        def run_process(command, *, input_text, env, **_kwargs):
            side = command[command.index("--side") + 1]
            backend = "cpu"
            case_rows = json.loads(input_text)
            case_ids = [case["case_id"] for case in case_rows]
            timed = "--timings" in command
            token = env.get("MIGRATION_TEST_ATTESTATION_TOKEN")
            execution_path = env.get("MIGRATION_PARITY_EXECUTION_OUTPUT")
            if token is not None:
                self.assertIsNotNone(collector)
                collector.events.append((token, "process"))
                collector.execution_paths[token] = (
                    Path(execution_path) if execution_path is not None else None
                )
            if execution_path is not None:
                Path(execution_path).write_text("{}", encoding="utf-8")

            if side == "source":
                identity = {
                    "side": "source",
                    "implementation": benchmark.BENCHMARK_SOURCE_NAME,
                    "version": benchmark.BENCHMARK_SOURCE_VERSION,
                }
            else:
                identity = {
                    "side": "target",
                    "implementation": "pillow-rs",
                    "backend": backend,
                    "backend_state": {"requested": backend, "active": [backend]},
                }

            role = env.get("MIGRATION_TEST_ATTESTATION_TOKEN", "unattested")
            timing_records = {
                case_id: (
                    [{"sample": len(calls), "token": role}] if timed else []
                )
                for case_id in case_ids
            }
            execution_records = {
                case_id: [{"proof": role}] for case_id in case_ids
            }
            payload = {
                "identity": identity,
                "results": [
                    {"case_id": case_id, "status": "completed"}
                    for case_id in case_ids
                ],
                "timings_ns": timing_records if timed else {},
                "telemetry": timing_records if timed else {},
                "execution": execution_records,
            }
            calls.append(
                {
                    "side": side,
                    "cases": case_rows,
                    "timed": timed,
                    "token": token,
                    "execution_path": execution_path,
                    "payload": payload,
                }
            )
            return 0, json.dumps(payload), ""

        return calls, run_process

    def run_side(
        self,
        side: str,
        cases: list[dict[str, object]],
        *,
        repeat: int = 1,
        lifecycle: str = "warm",
        attestation: RecordingCollector | None = None,
        timing_steps: list[str] | None = None,
    ) -> tuple[dict[str, object], list[dict[str, object]]]:
        calls, fake_process = self.fake_process(attestation)
        with (
            patch.object(
                benchmark, "benchmark_adapter_environment", return_value={}
            ),
            patch.object(
                benchmark,
                "benchmark_adapter_python",
                return_value="/fake/python",
            ),
            patch.object(benchmark, "run_process", side_effect=fake_process),
        ):
            result = benchmark.run_timed_side(
                side,
                Path("manifest.yaml"),
                cases,
                repeat,
                30,
                backend="cpu",
                timing_boundary="observed_steps",
                timing_steps=(
                    ["operation"] if timing_steps is None else timing_steps
                ),
                lifecycle=lifecycle,
                **({"attestation": attestation} if attestation is not None else {}),
            )
        return result, calls

    def run_synthetic_benchmark(
        self, directory: Path, *, fail_finalization: bool = False
    ) -> tuple[int, Path, Path]:
        workload_id = "workflow.synthetic-attestation"
        workload = {
            "workload_id": workload_id,
            "input": {
                "kind": "workflow",
                "assets": [],
                "steps": [{"surface": "PIL.Image", "operation": "invert"}],
                "observations": [],
            },
            "covers": [],
            "subjects": [
                {"kind": "oracle", "id": "pillow"},
                {"kind": "target_profile", "id": "python-cpu"},
            ],
            "measurement": {
                "correctness_gate": "successful_execution",
                "boundary": "observed_steps",
                "step_ids": ["call"],
                "cache_state": "warm",
                "warmup_iterations": 0,
                "measurement_iterations": 1,
                "samples": 1,
                "metrics": ["latency"],
            },
            "context": {"cache_state": "warm"},
        }
        manifest = Path(__file__).resolve()
        report_path = directory / "benchmark-result.json"
        attestation_path = directory / "benchmark-attestation.json"
        args = argparse.Namespace(
            manifest=manifest,
            output=report_path,
            parity_output=directory / "parity-result.json",
            attestation_output=attestation_path,
            reuse_parity=None,
            pipeline=False,
            workload_id=None,
            limit=None,
            timeout=30,
        )
        manifest_identity = {
            "path": "synthetic/manifest.yaml",
            "schema": "migration-parity/manifest@2",
            "sha256": "a" * 64,
        }

        def run_timed_side(
            side,
            _manifest,
            cases,
            repeat,
            _timeout,
            *,
            backend,
            timing_boundary,
            timing_steps,
            lifecycle,
            attestation=None,
        ):
            profile = (
                benchmark.benchmark_source_subject_id()
                if side == "source"
                else benchmark.target_profile_for_backend(backend)
            )
            case_ids = [case["case_id"] for case in cases]
            child_roles = ["benchmark_timed"]
            if side == "target":
                child_roles.append("benchmark_execution_proof")
            for role in child_roles:
                include_timings = role == "benchmark_timed"
                context = {
                    "lifecycle": lifecycle,
                    "boundary": timing_boundary if include_timings else None,
                    "step_ids": (
                        (sorted(set(timing_steps)) or ["call"])
                        if include_timings
                        else []
                    ),
                    "repeat": repeat if include_timings else 1,
                }
                overrides = attestation.child_environment(
                    side=side,
                    profile=profile,
                    role=role,
                    cases=cases,
                    context=context,
                )
                invocation_id = overrides[
                    attestation_module.PREFIX + "INVOCATION_ID"
                ]
                stable_sha256 = hashlib.sha256(
                    f"{side}:{profile}".encode("utf-8")
                ).hexdigest()
                receipt = {
                    "schema": attestation_module.CHILD_SCHEMA,
                    "status": "complete",
                    "invocation_id": invocation_id,
                    "role": role,
                    "side": side,
                    "profile": profile,
                    "case_ids": sorted(case_ids),
                    "context": context,
                    "runtime": {
                        "stable": {"side": side, "profile": profile},
                        "stable_sha256": stable_sha256,
                    },
                    "error": None,
                }
                receipt_path = Path(overrides[attestation_module.PREFIX + "OUTPUT"])
                receipt_path.parent.mkdir(parents=True, exist_ok=True)
                receipt_path.write_text(json.dumps(receipt), encoding="utf-8")
                attestation.collect(overrides)

            phase = {
                "setup_ns": 1,
                "pipeline_ns": 1,
                "terminal_ns": 1,
                "total_ns": 3,
            }
            return {
                "identity": {"side": side},
                "results": [
                    {"case_id": case_id, "status": "completed"}
                    for case_id in case_ids
                ],
                "timings_ns": {
                    case_id: [1_000_000] * repeat for case_id in case_ids
                },
                "telemetry": {
                    case_id: [phase] * repeat for case_id in case_ids
                },
                "execution": {case_id: [] for case_id in case_ids},
            }

        synthetic_identity = {
            "run_id": "synthetic-benchmark-run",
            "manifest": manifest_identity,
            "inputs": [],
            "assets": [],
            "oracles": [],
            "targets": [],
            "command": {},
        }
        with ExitStack() as stack:
            stack.enter_context(patch.object(benchmark, "TARGET_BACKENDS", ("cpu",)))
            stack.enter_context(patch.object(benchmark, "TARGET_PROFILES", ("python-cpu",)))
            stack.enter_context(patch.object(benchmark, "PARALLEL_CPU_PROFILE", False))
            stack.enter_context(patch.object(benchmark, "BENCHMARK_SOURCE_NAME", "Pillow"))
            stack.enter_context(patch.object(benchmark, "BENCHMARK_SOURCE_VERSION", "synthetic"))
            stack.enter_context(patch.object(benchmark, "load_manifest", return_value={}))
            stack.enter_context(
                patch.object(benchmark, "load_parity_cases", return_value=({}, {}))
            )
            stack.enter_context(
                patch.object(
                    benchmark,
                    "load_benchmarks",
                    return_value=({workload_id: workload}, [], {workload_id: "synthetic.json"}),
                )
            )
            stack.enter_context(patch.object(benchmark, "sha256", return_value="a" * 64))
            stack.enter_context(
                patch.object(
                    benchmark,
                    "execution_identity",
                    return_value={"oracles": [], "targets": []},
                )
            )
            stack.enter_context(patch.object(benchmark, "write_parity_artifact"))
            stack.enter_context(
                patch.object(benchmark, "identity", return_value=synthetic_identity)
            )
            stack.enter_context(
                patch.object(benchmark, "run_timed_side", side_effect=run_timed_side)
            )
            stack.enter_context(
                patch.object(
                    attestation_module,
                    "validate_runtime_identity",
                    return_value=None,
                )
            )
            if fail_finalization:
                stack.enter_context(
                    patch.object(
                        attestation_module.AttestationCollector,
                        "finalize",
                        side_effect=OSError("synthetic sidecar write failure"),
                    )
                )
            return_code = benchmark.run(args)
        return return_code, report_path, attestation_path

    def test_warm_target_attests_timed_child_and_separate_execution_proof(self) -> None:
        collector = RecordingCollector()
        result, calls = self.run_side(
            "target", self.cases[:1], repeat=4, attestation=collector
        )

        self.assertEqual([call["timed"] for call in calls], [True, False])
        self.assertEqual(
            [request["role"] for request in collector.requests],
            ["benchmark_timed", "benchmark_execution_proof"],
        )
        self.assertEqual(
            [request["profile"] for request in collector.requests],
            [benchmark.target_profile_for_backend("cpu")] * 2,
        )
        self.assertEqual(
            [request["context"] for request in collector.requests],
            [
                {
                    "lifecycle": "warm",
                    "boundary": "observed_steps",
                    "step_ids": ["operation"],
                    "repeat": 4,
                },
                {
                    "lifecycle": "warm",
                    "boundary": None,
                    "step_ids": [],
                    "repeat": 1,
                },
            ],
        )
        self.assertEqual(
            [event[1] for event in collector.events],
            ["environment", "process", "collect"] * 2,
        )
        self.assertEqual(collector.receipt_alive_when_collected, [True, True])
        self.assertEqual(
            set(result),
            {"identity", "results", "timings_ns", "telemetry", "execution"},
        )
        self.assertEqual(result["timings_ns"], calls[0]["payload"]["timings_ns"])
        self.assertEqual(result["execution"], calls[1]["payload"]["execution"])

    def test_cold_target_attests_each_timing_and_proof_process(self) -> None:
        collector = RecordingCollector()
        result, calls = self.run_side(
            "target", self.cases, repeat=2, lifecycle="cold", attestation=collector
        )

        self.assertEqual(len(calls), 6)
        self.assertEqual(
            [call["timed"] for call in calls],
            [True, True, True, True, False, False],
        )
        self.assertEqual(
            [request["role"] for request in collector.requests],
            ["benchmark_timed"] * 4 + ["benchmark_execution_proof"] * 2,
        )
        self.assertEqual(
            [request["cases"][0]["case_id"] for request in collector.requests],
            ["case-a", "case-a", "case-b", "case-b", "case-a", "case-b"],
        )
        self.assertTrue(
            all(
                request["context"]["lifecycle"] == "cold"
                for request in collector.requests
            )
        )
        self.assertTrue(
            all(request["context"]["repeat"] == 1 for request in collector.requests)
        )
        self.assertEqual(
            [request["context"]["boundary"] for request in collector.requests],
            ["observed_steps"] * 4 + [None] * 2,
        )
        self.assertEqual(
            [request["context"]["step_ids"] for request in collector.requests],
            [["operation"]] * 4 + [[]] * 2,
        )
        self.assertEqual(collector.receipt_alive_when_collected, [True] * 6)
        self.assertEqual(len(result["results"]), 4)
        self.assertEqual(len(result["timings_ns"]["case-a"]), 2)
        self.assertEqual(len(result["timings_ns"]["case-b"]), 2)
        self.assertEqual(
            result["execution"],
            {
                "case-a": calls[4]["payload"]["execution"]["case-a"],
                "case-b": calls[5]["payload"]["execution"]["case-b"],
            },
        )

    def test_source_attests_timing_child_without_execution_proof(self) -> None:
        collector = RecordingCollector()
        result, calls = self.run_side(
            "source", self.cases[:1], attestation=collector, timing_steps=[]
        )

        self.assertEqual(len(calls), 1)
        self.assertTrue(calls[0]["timed"])
        self.assertEqual(len(collector.requests), 1)
        self.assertEqual(collector.requests[0]["side"], "source")
        self.assertEqual(
            collector.requests[0]["profile"], benchmark.benchmark_source_subject_id()
        )
        self.assertEqual(collector.requests[0]["role"], "benchmark_timed")
        self.assertEqual(
            collector.requests[0]["context"],
            {
                "lifecycle": "warm",
                "boundary": "observed_steps",
                "step_ids": ["call"],
                "repeat": 1,
            },
        )
        self.assertIsNone(calls[0]["execution_path"])
        self.assertEqual(collector.receipt_alive_when_collected, [True])
        self.assertEqual(
            set(result),
            {"identity", "results", "timings_ns", "telemetry", "execution"},
        )

    def test_collector_failure_preserves_timed_success_and_child_failure(self) -> None:
        class FailingCollector(RecordingCollector):
            def collect(self, overrides: dict[str, str]) -> None:
                super().collect(overrides)
                raise OSError("synthetic collector failure")

        for child_returncode in (0, 7):
            with self.subTest(child_returncode=child_returncode):
                collector = FailingCollector()
                calls, fake_process = self.fake_process(collector)

                def run_process(command, *, input_text, env, **kwargs):
                    returncode, stdout, stderr = fake_process(
                        command, input_text=input_text, env=env, **kwargs
                    )
                    if child_returncode:
                        return child_returncode, stdout, "synthetic child failure"
                    return returncode, stdout, stderr

                with (
                    patch.object(
                        benchmark,
                        "benchmark_adapter_environment",
                        return_value={},
                    ),
                    patch.object(
                        benchmark,
                        "benchmark_adapter_python",
                        return_value="/fake/python",
                    ),
                    patch.object(
                        benchmark, "run_process", side_effect=run_process
                    ),
                ):
                    if child_returncode == 0:
                        result = benchmark.run_timed_side(
                            "source",
                            Path("manifest.yaml"),
                            self.cases[:1],
                            1,
                            30,
                            backend="cpu",
                            timing_boundary="observed_steps",
                            timing_steps=["operation"],
                            attestation=collector,
                        )
                        self.assertEqual(
                            result["timings_ns"]["case-a"][0]["sample"], 0
                        )
                    else:
                        with self.assertRaisesRegex(
                            RuntimeError,
                            "source benchmark adapter failed: synthetic child failure",
                        ):
                            benchmark.run_timed_side(
                                "source",
                                Path("manifest.yaml"),
                                self.cases[:1],
                                1,
                                30,
                                backend="cpu",
                                timing_boundary="observed_steps",
                                timing_steps=["operation"],
                                attestation=collector,
                            )
                self.assertEqual(len(calls), 1)
                self.assertEqual(len(collector.collected), 1)

    def test_absent_collector_keeps_five_field_envelope(self) -> None:
        result, calls = self.run_side("target", self.cases[:1])

        self.assertEqual(len(calls), 2)
        self.assertEqual(
            set(result), {"identity", "results", "timings_ns", "telemetry", "execution"}
        )
        self.assertTrue(all(call["token"] is None for call in calls))
        self.assertTrue(all(call["execution_path"] is None for call in calls if call["timed"]))

    def test_nonzero_child_is_collected_before_existing_failure(self) -> None:
        collector = RecordingCollector()

        def failed_process(_command, *, env, **_kwargs):
            token = env["MIGRATION_TEST_ATTESTATION_TOKEN"]
            collector.events.append((token, "process"))
            collector.execution_paths[token] = None
            return 7, "", "synthetic child failure"

        with (
            patch.object(
                benchmark, "benchmark_adapter_environment", return_value={}
            ),
            patch.object(
                benchmark,
                "benchmark_adapter_python",
                return_value="/fake/python",
            ),
            patch.object(benchmark, "run_process", side_effect=failed_process),
        ):
            with self.assertRaisesRegex(
                RuntimeError, "target benchmark adapter failed: synthetic child failure"
            ):
                benchmark.run_timed_side(
                    "target",
                    Path("manifest.yaml"),
                    self.cases[:1],
                    1,
                    30,
                    backend="cpu",
                    timing_boundary="observed_steps",
                    timing_steps=["operation"],
                    attestation=collector,
                )

        self.assertEqual(
            [event[1] for event in collector.events],
            ["environment", "process", "collect"],
        )
        self.assertEqual(len(collector.collected), 1)

    def test_synthetic_run_finalizes_exact_workload_profile_records(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            return_code, report_path, attestation_path = self.run_synthetic_benchmark(
                Path(temporary)
            )

            self.assertEqual(return_code, 0)
            report = json.loads(report_path.read_text(encoding="utf-8"))
            document = json.loads(attestation_path.read_text(encoding="utf-8"))
            self.assertEqual(report["schema"], "migration-parity/benchmark-result@1")
            self.assertEqual(document["status"], "complete")
            self.assertEqual(document["errors"], [])
            self.assertEqual(
                document["selection"],
                {
                    "case_profiles": [],
                    "workload_profiles": [
                        {
                            "workload_id": "workflow.synthetic-attestation",
                            "case_id": (
                                "benchmark-workflow.workflow.synthetic-attestation"
                            ),
                            "subject_id": "pillow",
                        },
                        {
                            "workload_id": "workflow.synthetic-attestation",
                            "case_id": (
                                "benchmark-workflow.workflow.synthetic-attestation"
                            ),
                            "subject_id": "python-cpu",
                        },
                    ],
                },
            )

    def test_finalization_failure_keeps_successful_raw_run_exit(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            return_code, report_path, attestation_path = self.run_synthetic_benchmark(
                Path(temporary), fail_finalization=True
            )

            self.assertEqual(return_code, 0)
            self.assertTrue(report_path.is_file())
            self.assertEqual(
                json.loads(report_path.read_text(encoding="utf-8"))["status"],
                "completed",
            )
            self.assertFalse(attestation_path.exists())


if __name__ == "__main__":
    unittest.main()
