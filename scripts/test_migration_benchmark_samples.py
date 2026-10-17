"""Focused contracts for optional raw benchmark-sample retention."""
from __future__ import annotations

import hashlib
import importlib
import io
import json
from contextlib import ExitStack, redirect_stdout
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch


SCRIPTS = Path(__file__).resolve().parent
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

benchmark = importlib.import_module("run_migration_benchmark")
sample_module = importlib.import_module("migration_benchmark_samples")
RawSampleCaptureError = sample_module.RawSampleCaptureError
RawSampleCollector = sample_module.RawSampleCollector
validate_result = importlib.import_module("validate_migration_parity_result")


def policy(*, warmup: int = 1, iterations: int = 2, samples: int = 2, cache: str = "warm") -> dict[str, object]:
    return {
        "boundary": "observed_steps",
        "step_ids": ["operation"],
        "cache_state": cache,
        "warmup_iterations": warmup,
        "measurement_iterations": iterations,
        "samples": samples,
        "metrics": ["latency", "throughput"],
    }


def phases(count: int) -> list[dict[str, int]]:
    return [
        {"setup_ns": index + 1, "pipeline_ns": index + 2, "total_ns": index + 3}
        for index in range(count)
    ]


def synthetic_report(
    specs: list[tuple[str, str, str, str, list[int]]],
    *,
    measurement_policy: dict[str, object] | None = None,
) -> dict[str, object]:
    selected_policy = dict(measurement_policy or policy())
    workloads = []
    for workload_id, case_id, subject_kind, subject_id, durations in specs:
        subject = benchmark.subject_result(
            subject_kind,
            subject_id,
            durations,
            phases(len(durations)),
            [],
            selected_policy,
        )
        workloads.append(
            {
                "workload_id": workload_id,
                "requirements": ["synthetic.requirement"],
                "measurement_policy": dict(selected_policy),
                "context": {
                    "size": [2, 2],
                    "mode": "RGB",
                    "chain_length": 1,
                    "operation_class": "point",
                    "cache_state": selected_policy["cache_state"],
                    "build_profile": "release",
                },
                "correctness": {
                    "gate": "successful_execution",
                    "outcome": "pass",
                    "evidence_id": "synthetic-proof",
                },
                "subjects": [subject],
                "budgets": [],
            }
        )
    return {
        "schema": "migration-parity/benchmark-result@1",
        "identity": {
            "run_id": "raw-sample-test-run",
            "started_at": "2026-10-15T00:00:00Z",
            "finished_at": "2026-10-15T00:00:01Z",
            "manifest": {
                "path": "fixtures/manifest.synthetic.yaml",
                "schema": "migration-parity/manifest@2",
                "sha256": "a" * 64,
            },
            "inputs": [
                {
                    "path": "fixtures/benchmark.synthetic.json",
                    "schema": "migration-parity/benchmark-input@1",
                    "sha256": "b" * 64,
                }
            ],
            "assets": [],
            "oracles": [
                {
                    "oracle_id": "pillow-simd",
                    "name": "Pillow-SIMD",
                    "version": "synthetic",
                    "runtime": "python",
                }
            ],
            "targets": [
                {
                    "target_profile": "python-cpu",
                    "target_id": "pillow-rs",
                    "revision": "synthetic-revision",
                    "dirty": False,
                    "runtime": "python",
                    "backend": "cpu",
                    "features": [],
                }
            ],
            "command": {
                "command_id": "synthetic-benchmark",
                "argv": ["python", "run_migration_benchmark.py"],
                "cwd": "/synthetic",
                "timeout_seconds": 30,
            },
        },
        "status": "completed",
        "environment": {
            "machine_id": "synthetic-machine",
            "os": "synthetic-os",
            "architecture": "synthetic-architecture",
            "cpu": "synthetic-cpu",
            "memory_bytes": 0,
            "power_mode": "unknown",
            "toolchain": "python-synthetic",
        },
        "summary": {
            "workloads_selected": len(workloads),
            "workloads_measured": len(workloads),
            "workloads_not_run": 0,
            "budgets_passed": 0,
            "budgets_failed": 0,
            "budgets_not_proven": 0,
        },
        "workloads": workloads,
        "suites": [],
        "infrastructure_errors": [],
    }


def record_case(
    collector: RawSampleCollector,
    *,
    group_id: str,
    workload_id: str,
    case_id: str,
    side: str,
    profile: str,
    durations: list[int],
    selected_policy: dict[str, object] | None = None,
    cases: list[dict[str, object]] | None = None,
    planned_repeat: int | None = None,
    child_repeat: int | None = None,
) -> None:
    current_policy = dict(selected_policy or policy())
    repeat = int(current_policy["warmup_iterations"]) + int(
        current_policy["measurement_iterations"]
    ) * int(current_policy["samples"])
    collector.record_timed_child(
        group_id=group_id,
        side=side,
        profile=profile,
        cases=cases or [{"case_id": case_id, "steps": [{"op": "synthetic"}]}],
        workload_ids_by_case={case_id: [workload_id]},
        planned_repeat=repeat if planned_repeat is None else planned_repeat,
        child_repeat=len(durations) if child_repeat is None else child_repeat,
        lifecycle=str(current_policy["cache_state"]),
        boundary=str(current_policy["boundary"]),
        step_ids=list(current_policy["step_ids"]),
        timings_ns={case_id: durations},
        telemetry={case_id: phases(len(durations))},
        child_invocation_id=f"invocation-{group_id}",
    )


class RawSampleCollectorTests(unittest.TestCase):
    def test_publish_retains_order_labels_metric_refs_and_sidecar_sha(self) -> None:
        durations = [100, 200, 0, 400, 500]
        specs = [
            ("work-source", "case-source", "oracle", "pillow-simd", durations),
            ("work-target", "case-target", "target_profile", "python-cpu", durations),
        ]
        report = synthetic_report(specs)
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            report_path = directory / "benchmark.json"
            sample_path = directory / "samples.json"
            collector = RawSampleCollector(sample_path)
            record_case(
                collector,
                group_id="source-group",
                workload_id="work-source",
                case_id="case-source",
                side="source",
                profile="pillow-simd",
                durations=durations,
            )
            record_case(
                collector,
                group_id="target-group",
                workload_id="work-target",
                case_id="case-target",
                side="target",
                profile="python-cpu",
                durations=durations,
            )

            collector.publish(
                report=report,
                report_path=report_path,
                workload_case_ids={
                    "work-source": "case-source",
                    "work-target": "case-target",
                },
            )

            document_bytes = sample_path.read_bytes()
            document = json.loads(document_bytes)
            self.assertEqual(document["schema"], sample_module.SCHEMA)
            self.assertEqual(document["report"]["path"], str(report_path.resolve()))
            self.assertEqual(document["report"]["run_id"], "raw-sample-test-run")
            self.assertEqual(document["report"]["schema"], report["schema"])
            self.assertEqual(document["report"]["manifest"], report["identity"]["manifest"])
            self.assertEqual(document["report"]["inputs"], report["identity"]["inputs"])
            self.assertEqual(
                [(record["case_id"], record["subject_kind"], record["profile"]) for record in document["records"]],
                [
                    ("case-source", "oracle", "pillow-simd"),
                    ("case-target", "target_profile", "python-cpu"),
                ],
            )

            source_ref = report["workloads"][0]["subjects"][0]["measurements"][0]["raw_samples_ref"]
            target_ref = report["workloads"][1]["subjects"][0]["measurements"][0]["raw_samples_ref"]
            self.assertEqual(source_ref, target_ref)
            self.assertTrue(
                all(
                    measurement["raw_samples_ref"] == source_ref
                    for workload in report["workloads"]
                    for subject in workload["subjects"]
                    for measurement in subject["measurements"]
                )
            )
            relative_path, referenced_sha = source_ref.split("#sha256=", 1)
            self.assertEqual(relative_path, "samples.json")
            self.assertEqual(referenced_sha, hashlib.sha256(document_bytes).hexdigest())

            retained = document["records"][0]
            self.assertEqual(retained["timings_ns"], durations)
            self.assertEqual(retained["phase_telemetry_ns"], phases(len(durations)))
            self.assertEqual(
                [label["role"] for label in retained["sample_labels"]],
                ["warmup", "measurement", "measurement", "measurement", "measurement"],
            )
            self.assertEqual(
                [label["ordinal"] for label in retained["sample_labels"]],
                [0, 1, 2, 3, 4],
            )
            self.assertEqual(
                [(label["measurement_iteration"], label["sample_in_iteration"]) for label in retained["sample_labels"]],
                [(None, None), (1, 1), (1, 2), (2, 1), (2, 2)],
            )
            self.assertEqual(
                retained["throughput_operations_per_second"],
                [10_000_000.0, 5_000_000.0, None, 2_500_000.0, 2_000_000.0],
            )
            source_measurements = report["workloads"][0]["subjects"][0]["measurements"]
            self.assertEqual(
                [(item["metric"], item["sample_count"]) for item in source_measurements],
                [("latency", 4), ("throughput", 3)],
            )
            validate_result.benchmark(report)

    def test_cold_timed_children_preserve_process_order(self) -> None:
        case = {"case_id": "case-cold", "steps": [{"op": "synthetic"}]}
        cold_policy = policy(warmup=0, iterations=3, samples=1, cache="cold")
        durations_by_child = iter((101, 202, 303))
        events: list[str] = []

        class EventCollector(RawSampleCollector):
            def record_timed_child(self, **kwargs) -> None:
                events.append("capture")
                super().record_timed_child(**kwargs)

        def fake_process(command, *, input_text, **_kwargs):
            self.assertIn("--timings", command)
            child_case = json.loads(input_text)[0]
            duration = next(durations_by_child)
            events.extend((f"start-{duration}", f"exit-{duration}"))
            payload = {
                "identity": {
                    "side": "source",
                    "implementation": benchmark.BENCHMARK_SOURCE_NAME,
                    "version": benchmark.BENCHMARK_SOURCE_VERSION,
                },
                "results": [{"case_id": child_case["case_id"], "status": "completed"}],
                "timings_ns": {child_case["case_id"]: [duration]},
                "telemetry": {child_case["case_id"]: [{"total_ns": duration}]},
                "execution": {child_case["case_id"]: [{"proof": "timed"}]},
            }
            return 0, json.dumps(payload), ""

        with tempfile.TemporaryDirectory() as temporary:
            sample_path = Path(temporary) / "cold-samples.json"
            collector = EventCollector(sample_path)
            with (
                patch.object(benchmark, "benchmark_adapter_environment", return_value={}),
                patch.object(benchmark, "benchmark_adapter_python", return_value="/fake/python"),
                patch.object(benchmark, "run_process", side_effect=fake_process),
            ):
                timed = benchmark.run_timed_side(
                    "source",
                    Path("manifest.yaml"),
                    [case],
                    3,
                    30,
                    backend="cpu",
                    timing_boundary="observed_steps",
                    timing_steps=["operation"],
                    lifecycle="cold",
                    raw_sample_capture=collector,
                    raw_sample_workloads_by_case={"case-cold": ["work-cold"]},
                )
            self.assertEqual(timed["timings_ns"]["case-cold"], [101, 202, 303])
            self.assertEqual(
                events,
                [
                    "start-101",
                    "exit-101",
                    "capture",
                    "start-202",
                    "exit-202",
                    "capture",
                    "start-303",
                    "exit-303",
                    "capture",
                ],
            )

            report = synthetic_report(
                [("work-cold", "case-cold", "oracle", benchmark.benchmark_source_subject_id(), [101, 202, 303])],
                measurement_policy=cold_policy,
            )
            collector.publish(
                report=report,
                report_path=Path(temporary) / "benchmark.json",
                workload_case_ids={"work-cold": "case-cold"},
            )
            document = json.loads(sample_path.read_text(encoding="utf-8"))
            self.assertEqual(
                [(row["child_sequence"], row["timings_ns"]) for row in document["records"]],
                [(0, [101]), (1, [202]), (2, [303])],
            )

    def test_missing_or_incomplete_timing_and_phase_arrays_fail(self) -> None:
        invalid_arrays = [
            ({}, {"case-a": [{}, {}]}),
            ({"case-a": [1, 2]}, {}),
            ({"case-a": [1]}, {"case-a": [{}]}),
            ({"case-a": [1, 2]}, {"case-a": [{}]}),
        ]
        for timings_ns, telemetry in invalid_arrays:
            with self.subTest(timings_ns=timings_ns, telemetry=telemetry):
                with tempfile.TemporaryDirectory() as temporary:
                    collector = RawSampleCollector(Path(temporary) / "samples.json")
                    with self.assertRaises(RawSampleCaptureError):
                        collector.record_timed_child(
                            group_id="group",
                            side="source",
                            profile="pillow-simd",
                            cases=[{"case_id": "case-a"}],
                            workload_ids_by_case={"case-a": ["work-a"]},
                            planned_repeat=2,
                            child_repeat=2,
                            lifecycle="warm",
                            boundary="observed_steps",
                            step_ids=["operation"],
                            timings_ns=timings_ns,
                            telemetry=telemetry,
                            child_invocation_id="child",
                        )

    def test_existing_sidecar_is_never_overwritten(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            sample_path = Path(temporary) / "samples.json"
            sample_path.write_text("preserve-me", encoding="utf-8")
            with self.assertRaises(FileExistsError):
                RawSampleCollector(sample_path)
            self.assertEqual(sample_path.read_text(encoding="utf-8"), "preserve-me")

            raced_path = Path(temporary) / "raced-samples.json"
            collector = RawSampleCollector(raced_path)
            report = synthetic_report(
                [("work-a", "case-a", "target_profile", "python-cpu", [100, 200, 300, 400, 500])]
            )
            record_case(
                collector,
                group_id="group",
                workload_id="work-a",
                case_id="case-a",
                side="target",
                profile="python-cpu",
                durations=[100, 200, 300, 400, 500],
            )
            raced_path.write_text("sentinel", encoding="utf-8")
            with self.assertRaises(FileExistsError):
                collector.publish(
                    report=report,
                    report_path=Path(temporary) / "benchmark.json",
                    workload_case_ids={"work-a": "case-a"},
                )
            self.assertEqual(raced_path.read_text(encoding="utf-8"), "sentinel")

    def test_failed_publish_leaves_report_measurement_refs_null(self) -> None:
        durations = [100, 200, 300, 400, 500]
        report = synthetic_report(
            [("work-a", "case-a", "target_profile", "python-cpu", durations)]
        )
        with tempfile.TemporaryDirectory() as temporary:
            collector = RawSampleCollector(Path(temporary) / "samples.json")
            record_case(
                collector,
                group_id="group",
                workload_id="work-a",
                case_id="case-a",
                side="target",
                profile="python-cpu",
                durations=durations,
            )
            with patch.object(
                sample_module,
                "_publish_exclusive",
                side_effect=OSError("synthetic write failure"),
            ):
                with self.assertRaisesRegex(OSError, "synthetic write failure"):
                    collector.publish(
                        report=report,
                        report_path=Path(temporary) / "benchmark.json",
                        workload_case_ids={"work-a": "case-a"},
                    )
            self.assertTrue(
                all(
                    measurement["raw_samples_ref"] is None
                    for workload in report["workloads"]
                    for subject in workload["subjects"]
                    for measurement in subject["measurements"]
                )
            )

    def test_fsync_failure_preserves_artifact_and_null_report_refs(self) -> None:
        durations = [100, 200, 300, 400, 500]
        report = synthetic_report(
            [("work-a", "case-a", "target_profile", "python-cpu", durations)]
        )
        with tempfile.TemporaryDirectory() as temporary:
            sample_path = Path(temporary) / "samples.json"
            collector = RawSampleCollector(sample_path)
            record_case(
                collector,
                group_id="group",
                workload_id="work-a",
                case_id="case-a",
                side="target",
                profile="python-cpu",
                durations=durations,
            )
            with patch.object(
                sample_module.os,
                "fsync",
                side_effect=OSError("synthetic fsync failure"),
            ):
                with self.assertRaisesRegex(OSError, "synthetic fsync failure"):
                    collector.publish(
                        report=report,
                        report_path=Path(temporary) / "benchmark.json",
                        workload_case_ids={"work-a": "case-a"},
                    )
            self.assertTrue(sample_path.is_file())
            self.assertTrue(sample_path.read_bytes())
            self.assertTrue(
                all(
                    measurement["raw_samples_ref"] is None
                    for workload in report["workloads"]
                    for subject in workload["subjects"]
                    for measurement in subject["measurements"]
                )
            )

    def test_sidecar_does_not_copy_case_assets_or_steps(self) -> None:
        marker = "synthetic-image-pixel-payload"
        case = {
            "case_id": "case-a",
            "assets": [{"asset_id": "pixels", "data": marker}],
            "steps": [{"op": "synthetic", "pixels": marker}],
        }
        durations = [100, 200, 300, 400, 500]
        report = synthetic_report(
            [("work-a", "case-a", "target_profile", "python-cpu", durations)]
        )
        with tempfile.TemporaryDirectory() as temporary:
            sample_path = Path(temporary) / "samples.json"
            collector = RawSampleCollector(sample_path)
            record_case(
                collector,
                group_id="group",
                workload_id="work-a",
                case_id="case-a",
                side="target",
                profile="python-cpu",
                durations=durations,
                cases=[case],
            )
            collector.publish(
                report=report,
                report_path=Path(temporary) / "benchmark.json",
                workload_case_ids={"work-a": "case-a"},
            )
            serialized = sample_path.read_text(encoding="utf-8")
            self.assertNotIn(marker, serialized)
            document = json.loads(serialized)
            self.assertEqual(set(document["records"][0]), {
                "group_id",
                "child_sequence",
                "child_invocation_id",
                "side",
                "subject_kind",
                "profile",
                "case_id",
                "workload_ids",
                "boundary",
                "lifecycle",
                "step_ids",
                "planned_repeat",
                "child_repeat",
                "timings_ns",
                "phase_telemetry_ns",
                "sample_labels",
                "capture_errors",
                "throughput_operations_per_second",
            })


class TimedSideSampleCaptureTests(unittest.TestCase):
    def setUp(self) -> None:
        self.case = {"case_id": "case-a", "steps": [{"op": "synthetic"}]}

    def fake_target_process(self, events: list[str], *, first_durations: list[int]):
        calls: list[dict[str, object]] = []

        def run_process(command, *, input_text, env, **_kwargs):
            timed = "--timings" in command
            events.append("timed-child-start" if timed else "proof-child-start")
            execution_path = env.get("MIGRATION_PARITY_EXECUTION_OUTPUT")
            if execution_path is not None:
                Path(execution_path).write_text("{}", encoding="utf-8")
            case_rows = json.loads(input_text)
            case_ids = [case["case_id"] for case in case_rows]
            durations = first_durations if timed else []
            timing_rows = {case_id: list(durations) for case_id in case_ids}
            telemetry_rows = {
                case_id: [{"total_ns": value} for value in durations]
                for case_id in case_ids
            }
            payload = {
                "identity": {
                    "side": "target",
                    "implementation": "pillow-rs",
                    "backend": "cpu",
                    "backend_state": {"requested": "cpu", "active": ["cpu"]},
                },
                "results": [
                    {"case_id": case_id, "status": "completed"} for case_id in case_ids
                ],
                "timings_ns": timing_rows if timed else {},
                "telemetry": telemetry_rows if timed else {},
                "execution": {
                    case_id: [{"proof": "timed" if timed else "untimed"}]
                    for case_id in case_ids
                },
            }
            calls.append({"timed": timed, "payload": payload})
            events.append("timed-child-exit" if timed else "proof-child-exit")
            return 0, json.dumps(payload), ""

        return calls, run_process

    def test_no_capture_preserves_target_timing_and_proof_calls(self) -> None:
        events: list[str] = []
        calls, fake_process = self.fake_target_process(events, first_durations=[11, 12])
        with (
            patch.object(benchmark, "benchmark_adapter_environment", return_value={}),
            patch.object(benchmark, "benchmark_adapter_python", return_value="/fake/python"),
            patch.object(benchmark, "run_process", side_effect=fake_process),
        ):
            result = benchmark.run_timed_side(
                "target",
                Path("manifest.yaml"),
                [self.case],
                2,
                30,
                backend="cpu",
                timing_boundary="observed_steps",
                timing_steps=["operation"],
            )
        self.assertEqual([call["timed"] for call in calls], [True, False])
        self.assertEqual(result["timings_ns"]["case-a"], [11, 12])
        self.assertEqual(result["execution"]["case-a"], [{"proof": "untimed"}])

    def test_disabled_capture_does_not_resolve_sample_profile(self) -> None:
        events: list[str] = []
        calls = []

        def fake_process(command, *, input_text, **_kwargs):
            events.append("child-start")
            case_id = json.loads(input_text)[0]["case_id"]
            payload = {
                "identity": {
                    "side": "source",
                    "implementation": benchmark.BENCHMARK_SOURCE_NAME,
                    "version": benchmark.BENCHMARK_SOURCE_VERSION,
                },
                "results": [{"case_id": case_id, "status": "completed"}],
                "timings_ns": {case_id: [{"total_ns": 9}]},
                "telemetry": {case_id: [{"total_ns": 9}]},
                "execution": {case_id: [{"proof": "timed"}]},
            }
            calls.append(command)
            events.append("child-exit")
            return 0, json.dumps(payload), ""

        with (
            patch.object(benchmark, "benchmark_adapter_environment", return_value={}),
            patch.object(benchmark, "benchmark_adapter_python", return_value="/fake/python"),
            patch.object(benchmark, "run_process", side_effect=fake_process),
            patch.object(
                benchmark,
                "benchmark_source_subject_id",
                side_effect=AssertionError("sample-only profile lookup"),
            ) as profile_lookup,
        ):
            benchmark.run_timed_side(
                "source",
                Path("manifest.yaml"),
                [self.case],
                1,
                30,
                backend="cpu",
                timing_boundary="observed_steps",
                timing_steps=["operation"],
            )
        profile_lookup.assert_not_called()
        self.assertEqual(len(calls), 1)

    def test_capture_occurs_after_timed_exit_and_excludes_proof_child(self) -> None:
        events: list[str] = []

        class EventCollector(RawSampleCollector):
            def record_timed_child(self, **kwargs) -> None:
                events.append("capture")
                super().record_timed_child(**kwargs)

        calls, fake_process = self.fake_target_process(events, first_durations=[11, 12])
        with tempfile.TemporaryDirectory() as temporary:
            collector = EventCollector(Path(temporary) / "samples.json")
            with (
                patch.object(benchmark, "benchmark_adapter_environment", return_value={}),
                patch.object(benchmark, "benchmark_adapter_python", return_value="/fake/python"),
                patch.object(benchmark, "run_process", side_effect=fake_process),
            ):
                benchmark.run_timed_side(
                    "target",
                    Path("manifest.yaml"),
                    [self.case],
                    2,
                    30,
                    backend="cpu",
                    timing_boundary="observed_steps",
                    timing_steps=["operation"],
                    raw_sample_capture=collector,
                    raw_sample_workloads_by_case={"case-a": ["work-a"]},
                )
            self.assertEqual(
                events,
                ["timed-child-start", "timed-child-exit", "capture", "proof-child-start", "proof-child-exit"],
            )
            self.assertEqual([call["timed"] for call in calls], [True, False])
            self.assertEqual(len(collector._records), 1)
            self.assertEqual(collector._records[0]["timings_ns"], [11, 12])


class BenchmarkRunSampleIntegrationTests(unittest.TestCase):
    def test_run_publishes_sample_refs_for_completed_source_and_target(self) -> None:
        workload_id = "workflow.synthetic-samples"
        case_id = f"benchmark-workflow.{workload_id}"
        workload = {
            "workload_id": workload_id,
            "input": {
                "kind": "workflow",
                "assets": [],
                "steps": [{"surface": "synthetic", "operation": "call"}],
                "observations": [],
            },
            "covers": ["synthetic.requirement"],
            "subjects": [
                {"kind": "oracle", "id": "pillow"},
                {"kind": "target_profile", "id": "python-cpu"},
            ],
            "measurement": {
                "correctness_gate": "successful_execution",
                "boundary": "observed_steps",
                "step_ids": ["call"],
                "cache_state": "warm",
                "warmup_iterations": 1,
                "measurement_iterations": 2,
                "samples": 2,
                "metrics": ["latency", "throughput"],
            },
            "context": {
                "size": [1, 1],
                "mode": "RGB",
                "chain_length": 1,
                "operation_class": "point",
                "cache_state": "warm",
                "build_profile": "release",
            },
        }
        synthetic_identity = {
            "run_id": "synthetic-run-integration",
            "started_at": "2026-10-15T00:00:00Z",
            "finished_at": "2026-10-15T00:00:01Z",
            "manifest": {
                "path": "synthetic/manifest.yaml",
                "schema": "migration-parity/manifest@2",
                "sha256": "a" * 64,
            },
            "inputs": [
                {
                    "path": "synthetic/benchmark.json",
                    "schema": "migration-parity/benchmark-input@1",
                    "sha256": "b" * 64,
                }
            ],
            "assets": [],
            "oracles": [
                {
                    "oracle_id": "pillow",
                    "name": "Pillow",
                    "version": "synthetic",
                    "runtime": "python",
                }
            ],
            "targets": [
                {
                    "target_profile": "python-cpu",
                    "target_id": "pillow-rs",
                    "revision": "synthetic-revision",
                    "dirty": False,
                    "runtime": "python",
                    "backend": "cpu",
                    "features": [],
                }
            ],
            "command": {
                "command_id": "synthetic-benchmark",
                "argv": ["python", "run_migration_benchmark.py"],
                "cwd": "/synthetic",
                "timeout_seconds": 30,
            },
        }

        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            args = type("Args", (), {})()
            args.manifest = Path(__file__).resolve()
            args.output = directory / "benchmark-result.json"
            args.parity_output = directory / "parity-result.json"
            args.raw_samples_output = directory / "raw-samples.json"
            args.reuse_parity = None
            args.pipeline = False
            args.workload_id = None
            args.limit = None
            args.timeout = 30
            calls: list[dict[str, object]] = []

            def fake_timed_side(
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
                raw_sample_capture=None,
                raw_sample_workloads_by_case=None,
            ):
                self.assertIsNotNone(raw_sample_capture)
                self.assertEqual(
                    raw_sample_workloads_by_case,
                    {case_id: [workload_id]},
                )
                profile = (
                    benchmark.benchmark_source_subject_id()
                    if side == "source"
                    else benchmark.target_profile_for_backend(backend)
                )
                durations = [100, 200, 300, 400, 500]
                telemetry = phases(len(durations))
                group_id = f"{side}-{backend}"
                raw_sample_capture.record_timed_child(
                    group_id=group_id,
                    side=side,
                    profile=profile,
                    cases=cases,
                    workload_ids_by_case=raw_sample_workloads_by_case,
                    planned_repeat=repeat,
                    child_repeat=repeat,
                    lifecycle=lifecycle,
                    boundary=timing_boundary,
                    step_ids=timing_steps,
                    timings_ns={case_id: durations},
                    telemetry={case_id: telemetry},
                    child_invocation_id=f"{side}-invocation",
                )
                calls.append(
                    {
                        "side": side,
                        "profile": profile,
                        "capture": raw_sample_capture,
                    }
                )
                return {
                    "identity": {"side": side},
                    "results": [{"case_id": case_id, "status": "completed"}],
                    "timings_ns": {case_id: durations},
                    "telemetry": {case_id: telemetry},
                    "execution": {case_id: []},
                }

            with ExitStack() as stack:
                stack.enter_context(patch.object(benchmark, "TARGET_BACKENDS", ("cpu",)))
                stack.enter_context(patch.object(benchmark, "TARGET_PROFILES", ("python-cpu",)))
                stack.enter_context(patch.object(benchmark, "PARALLEL_CPU_PROFILE", False))
                stack.enter_context(patch.object(benchmark, "BENCHMARK_SOURCE_NAME", "Pillow"))
                stack.enter_context(patch.object(benchmark, "BENCHMARK_SOURCE_VERSION", "synthetic"))
                stack.enter_context(patch.object(benchmark, "load_manifest", return_value={}))
                stack.enter_context(patch.object(benchmark, "load_parity_cases", return_value=({}, {})))
                stack.enter_context(
                    patch.object(
                        benchmark,
                        "load_benchmarks",
                        return_value=({workload_id: workload}, [], {workload_id: "synthetic.json"}),
                    )
                )
                stack.enter_context(patch.object(benchmark, "sha256", return_value="c" * 64))
                stack.enter_context(
                    patch.object(
                        benchmark,
                        "execution_identity",
                        return_value={"oracles": [], "targets": []},
                    )
                )
                stack.enter_context(patch.object(benchmark, "write_parity_artifact"))
                stack.enter_context(patch.object(benchmark, "identity", return_value=synthetic_identity))
                stack.enter_context(patch.object(benchmark, "run_timed_side", side_effect=fake_timed_side))
                stack.enter_context(
                    patch.dict(benchmark.os.environ, {"MIGRATION_BENCHMARK_RUNNER_ID": "native-test-runner"})
                )
                with redirect_stdout(io.StringIO()):
                    result_code = benchmark.run(args)

            self.assertEqual(result_code, 0)
            self.assertEqual([call["side"] for call in calls], ["source", "target"])
            self.assertIs(calls[0]["capture"], calls[1]["capture"])
            report = json.loads(args.output.read_text(encoding="utf-8"))
            validate_result.benchmark(report)
            self.assertEqual(report["environment"]["runner_id"], "native-test-runner")
            workload_result = report["workloads"][0]
            self.assertEqual(workload_result["correctness"]["outcome"], "pass")
            self.assertEqual(
                [subject["status"] for subject in workload_result["subjects"]],
                ["completed", "completed"],
            )
            references = [
                measurement["raw_samples_ref"]
                for subject in workload_result["subjects"]
                for measurement in subject["measurements"]
            ]
            self.assertTrue(references)
            self.assertTrue(all(reference == references[0] for reference in references))
            relative_path, digest = references[0].split("#sha256=", 1)
            self.assertEqual(relative_path, "raw-samples.json")
            sample_bytes = (directory / relative_path).read_bytes()
            self.assertEqual(digest, hashlib.sha256(sample_bytes).hexdigest())
            self.assertEqual(
                json.loads(sample_bytes)["report"]["run_id"],
                report["identity"]["run_id"],
            )


if __name__ == "__main__":
    unittest.main()
