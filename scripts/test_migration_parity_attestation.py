from __future__ import annotations

import argparse
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

from scripts import run_migration_parity as parity
from scripts.migration_execution_attestation import PREFIX


class RecordingAttestationCollector:
    def __init__(self, *, events: list[str] | None = None):
        self.events = events
        self.planned: list[dict[str, object]] = []
        self.collected: list[dict[str, str]] = []
        self.finalized: list[dict[str, object]] = []

    def child_environment(
        self,
        *,
        side: str,
        profile: str,
        role: str,
        cases: list[dict[str, object]],
        context: dict[str, object],
    ) -> dict[str, str]:
        invocation = f"invocation-{len(self.planned)}"
        planned = {
            "side": side,
            "profile": profile,
            "role": role,
            "case_ids": sorted(str(case["case_id"]) for case in cases),
            "context": context,
        }
        self.planned.append(planned)
        return {
            PREFIX + "OUTPUT": f"/attestation/children/{invocation}.json",
            PREFIX + "INVOCATION_ID": invocation,
            PREFIX + "ROLE": role,
            PREFIX + "PROFILE": profile,
        }

    def collect(self, environment: dict[str, str]) -> None:
        self.collected.append(dict(environment))
        if self.events is not None:
            self.events.append("collect")

    def finalize(
        self,
        report_path: Path,
        *,
        result: dict[str, object],
        selection: dict[str, object],
    ) -> None:
        self.finalized.append(
            {"report_path": report_path, "result": result, "selection": selection}
        )


class MigrationParityAttestationTests(unittest.TestCase):
    def test_transport_collects_after_nonzero_exit_without_changing_raw_result(self):
        events: list[str] = []
        collector = RecordingAttestationCollector(events=events)
        process = Mock()
        process.returncode = 9

        def communicate(*, input: str, timeout: int):
            events.append("process-returned")
            return "adapter failed", "failure detail"

        process.communicate.side_effect = communicate
        case = {"case_id": "case-a"}

        def broken_collect(_environment: dict[str, str]) -> None:
            events.append("collect")
            raise ValueError("receipt collection failure")

        with patch.object(collector, "collect", side_effect=broken_collect) as collect:
            with patch(
                "scripts.run_migration_parity.subprocess.Popen", return_value=process
            ) as popen:
                with self.assertRaisesRegex(RuntimeError, "exited 9"):
                    parity._run_side_subprocess_batch(
                        "source",
                        Path("manifest.yaml"),
                        [case],
                        10,
                        attestation=collector,
                    )

        self.assertEqual(events, ["process-returned", "collect"])
        collect.assert_called_once()
        child_environment = popen.call_args.kwargs["env"]
        self.assertEqual(
            child_environment[PREFIX + "INVOCATION_ID"], "invocation-0"
        )

    def test_transport_collects_after_malformed_envelope(self):
        events: list[str] = []
        collector = RecordingAttestationCollector(events=events)
        process = Mock()
        process.returncode = 0

        def communicate(*, input: str, timeout: int):
            events.append("process-returned")
            return "not-json", ""

        process.communicate.side_effect = communicate

        with patch(
            "scripts.run_migration_parity.subprocess.Popen", return_value=process
        ):
            with self.assertRaisesRegex(RuntimeError, "malformed JSON"):
                parity._run_side_subprocess_batch(
                    "source",
                    Path("manifest.yaml"),
                    [{"case_id": "case-a"}],
                    10,
                    attestation=collector,
                )

        self.assertEqual(events, ["process-returned", "collect"])

    def test_stateful_batches_get_distinct_attestation_receipts_and_keep_sidecar_overrides(self):
        class AdapterProcess:
            returncode = 0

            def communicate(self, *, input: str, timeout: int):
                cases = json.loads(input)
                envelope = {
                    "identity": {"side": "source", "version": parity.ORACLE_VERSION},
                    "results": [
                        {"case_id": case["case_id"], "status": "completed"}
                        for case in cases
                    ],
                }
                return json.dumps(envelope), ""

        cases = [
            {"case_id": "ordinary", "steps": []},
            {
                "case_id": "noise",
                "steps": [{"surface": "PIL.Image", "operation": "effect_noise"}],
            },
            {
                "case_id": "spread",
                "steps": [
                    {"surface": "PIL.Image.Image", "operation": "effect_spread"}
                ],
            },
        ]
        collector = RecordingAttestationCollector()

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            environment = {
                "MIGRATION_PARITY_EXECUTION_OUTPUT": str(root / "execution.json"),
                "MIGRATION_GPU_WGSL_COVERAGE_OUTPUT": str(root / "shader.json"),
            }
            with patch.dict(parity.os.environ, environment, clear=False):
                with patch(
                    "scripts.run_migration_parity.timed_source_identity",
                    return_value=("Pillow", "12.2.0"),
                ), patch(
                    "scripts.run_migration_parity.subprocess.Popen",
                    side_effect=lambda *args, **kwargs: AdapterProcess(),
                ) as popen, patch(
                    "scripts.run_migration_parity.write_pipeline_execution_evidence"
                ), patch("scripts.run_migration_parity.write_gpu_shader_coverage"):
                    identity, results = parity.run_side_subprocess(
                        "source",
                        Path("manifest.yaml"),
                        cases,
                        10,
                        attestation=collector,
                    )

        self.assertEqual(identity["side"], "source")
        self.assertEqual(set(results), {"ordinary", "noise", "spread"})
        self.assertEqual(len(popen.call_args_list), 3)
        self.assertEqual(len(collector.planned), 3)
        self.assertEqual(len(collector.collected), 3)
        invocation_ids = {
            call.kwargs["env"][PREFIX + "INVOCATION_ID"]
            for call in popen.call_args_list
        }
        self.assertEqual(len(invocation_ids), 3)
        for call in popen.call_args_list:
            child_environment = call.kwargs["env"]
            self.assertTrue(child_environment["MIGRATION_PARITY_EXECUTION_OUTPUT"])
            self.assertTrue(child_environment["MIGRATION_GPU_WGSL_COVERAGE_OUTPUT"])
            self.assertTrue(child_environment[PREFIX + "OUTPUT"].endswith(".json"))
        self.assertEqual(
            [planned["case_ids"] for planned in collector.planned],
            [["ordinary"], ["noise"], ["spread"]],
        )
        self.assertEqual(
            [planned["profile"] for planned in collector.planned],
            ["pillow", "pillow", "pillow"],
        )

    def test_source_attestation_profiles_are_canonical_and_reject_unknown(self):
        self.assertEqual(parity.parity_source_profile("Pillow"), "pillow")
        self.assertEqual(parity.parity_source_profile("Pillow-SIMD"), "pillow-simd")
        with self.assertRaisesRegex(ValueError, "unsupported parity source"):
            parity.parity_source_profile("Other Pillow")

    def test_batched_orchestrator_finalizes_streamed_comparison_selection(self):
        class FinalizingCollector(RecordingAttestationCollector):
            def finalize(self, report_path, *, result, selection):
                self.report_document = json.loads(report_path.read_text(encoding="utf-8"))
                super().finalize(report_path, result=result, selection=selection)

        cases = [
            {"case_id": "case-a", "covers": []},
            {"case_id": "case-b", "covers": []},
        ]
        collector = FinalizingCollector()
        side_calls: list[tuple[str, list[str], object]] = []
        handshakes = {
            "source": {"side": "source", "version": parity.ORACLE_VERSION},
            "target": {"side": "target", "version": parity.ORACLE_VERSION},
        }

        def run_side(side, manifest_path, batch, timeout, *, attestation=None):
            side_calls.append(
                (side, [case["case_id"] for case in batch], attestation)
            )
            return handshakes[side], {
                case["case_id"]: {
                    "case_id": case["case_id"],
                    "status": "completed",
                    "observations": [],
                }
                for case in batch
            }

        identity = {
            "run_id": "run-1",
            "manifest": {"sha256": "manifest-hash"},
            "inputs": [{"path": "input.json", "sha256": "input-hash"}],
        }
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "parity.json"
            args = argparse.Namespace(output=output, timeout=10)
            with patch.dict(parity.os.environ, {"MIGRATION_PARITY_SERIAL": "1"}):
                with patch("scripts.run_migration_parity.build_identity", return_value=identity), patch(
                    "scripts.run_migration_parity.git_dirty", return_value=False
                ), patch(
                    "scripts.run_migration_parity.now_rfc3339", side_effect=["start", "finish"]
                ), patch(
                    "scripts.run_migration_parity.run_side_subprocess", side_effect=run_side
                ), patch(
                    "scripts.run_migration_parity.compare_case", return_value=("pass", [])
                ), patch(
                    "scripts.run_migration_parity.target_profile_for_backend",
                    return_value="python-cpu",
                ), patch("scripts.run_migration_parity.write_summary_sidecar"):
                    status = parity.run_orchestrator_batched(
                        args,
                        Path("manifest.yaml"),
                        cases,
                        {"case-a": "input.json", "case-b": "input.json"},
                        {},
                        10,
                        2,
                        attestation=collector,
                    )

        self.assertEqual(status, 0)
        self.assertEqual(len(side_calls), 2)
        self.assertTrue(all(call[2] is collector for call in side_calls))
        self.assertEqual(
            [row["case_id"] for row in collector.finalized[0]["result"]["comparisons"]],
            ["case-a", "case-b"],
        )
        self.assertEqual(
            collector.finalized[0]["selection"],
            {
                "case_profiles": [
                    {"case_id": "case-a", "target_profile": "python-cpu"},
                    {"case_id": "case-b", "target_profile": "python-cpu"},
                ],
                "workload_profiles": [],
            },
        )
        self.assertEqual(collector.report_document["status"], "completed")
        self.assertEqual(len(collector.report_document["comparisons"]), 2)

    def test_orchestrator_attestation_optout_preserves_legacy_call_signatures(self):
        case = {"case_id": "case-a", "covers": []}
        identity = {
            "run_id": "run-1",
            "manifest": {"sha256": "manifest-hash"},
            "inputs": [{"path": "input.json", "sha256": "input-hash"}],
        }
        side_calls: list[tuple[str, dict[str, object]]] = []

        def run_side(side, manifest_path, cases, timeout, **kwargs):
            side_calls.append((side, kwargs))
            return {"version": parity.ORACLE_VERSION}, {
                case["case_id"]: {
                    "case_id": case["case_id"],
                    "status": "completed",
                    "observations": [],
                }
            }

        with tempfile.TemporaryDirectory() as temporary:
            args = argparse.Namespace(
                manifest=Path("manifest.yaml"),
                output=Path(temporary) / "parity.json",
                case_id=None,
                surface=None,
                limit=None,
                timeout=10,
            )
            with patch.dict(parity.os.environ, {"MIGRATION_PARITY_SERIAL": "1"}):
                with patch("scripts.run_migration_parity.load_manifest", return_value={}), patch(
                    "scripts.run_migration_parity.load_cases",
                    return_value=([case], {"case-a": "input.json"}),
                ), patch("scripts.run_migration_parity.build_operation_index", return_value={}), patch(
                    "scripts.run_migration_parity.effective_adapter_timeout", return_value=10
                ), patch("scripts.run_migration_parity.parity_batch_size", return_value=1), patch(
                    "scripts.run_migration_parity.git_dirty", return_value=False
                ), patch("scripts.run_migration_parity.build_identity", return_value=identity), patch(
                    "scripts.run_migration_parity.run_side_subprocess", side_effect=run_side
                ), patch("scripts.run_migration_parity.compare_case", return_value=("pass", [])), patch(
                    "scripts.run_migration_parity.now_rfc3339", side_effect=["start", "finish"]
                ), patch("scripts.run_migration_parity.write_summary_sidecar"), patch(
                    "builtins.print"
                ):
                    result = parity.run_orchestrator(args)

        self.assertEqual(result, 0)
        self.assertEqual([side for side, _kwargs in side_calls], ["source", "target"])
        self.assertTrue(all(not kwargs for _side, kwargs in side_calls))

    def test_child_writes_attestation_after_receipts_and_before_stdout(self):
        events: list[str] = []

        class RecordingOutput:
            text = ""

            def write(self, value: str) -> int:
                events.append("stdout")
                self.text += value
                return len(value)

        class TargetCore:
            def set_pipeline_telemetry(self, enabled: bool) -> None:
                pass

            def take_pipeline_telemetry(self):
                return None

            def set_gpu_shader_coverage(self, enabled: bool) -> None:
                pass

            def take_gpu_shader_coverage(self):
                return []

        case = {"case_id": "case-a", "steps": []}
        handshake = {"side": "target", "version": "target-version"}
        context_seen: list[dict[str, object]] = []
        output = RecordingOutput()
        args = argparse.Namespace(
            side="target",
            manifest=Path("manifest.yaml"),
            lifecycle="resident",
            repeat=3,
            timing_step=["observe-b", "observe-a"],
            timing_boundary="whole_workflow",
            timings=True,
        )

        def run_resident(*_args, **kwargs):
            events.append("workflow")
            kwargs["timing_sink"].extend([10, 11, 12])
            return {"case_id": "case-a", "status": "completed", "observations": []}

        def child_attestation(side, actual_handshake, actual_cases, *, context, declared_features):
            events.append("child-attestation")
            context_seen.append(
                {
                    "side": side,
                    "handshake": actual_handshake,
                    "case_ids": [item["case_id"] for item in actual_cases],
                    "context": context,
                    "declared_features": declared_features,
                }
            )

        environment = {
            "MIGRATION_PARITY_EXECUTION_OUTPUT": "/tmp/execution.json",
            "MIGRATION_GPU_WGSL_COVERAGE_OUTPUT": "/tmp/shader.json",
            PREFIX + "OUTPUT": "/tmp/child-attestation.json",
            PREFIX + "INVOCATION_ID": "invocation-a",
            PREFIX + "ROLE": "parity",
            PREFIX + "PROFILE": "python-cpu",
        }
        with patch.dict(parity.os.environ, environment, clear=False), patch(
            "scripts.run_migration_parity.load_manifest", return_value={}
        ), patch("scripts.run_migration_parity.build_operation_index", return_value={}), patch(
            "scripts.run_migration_parity.side_identity", return_value=handshake
        ), patch("scripts.run_migration_parity.run_resident_case", side_effect=run_resident), patch(
            "scripts.run_migration_parity.write_gpu_shader_coverage",
            side_effect=lambda *_args, **_kwargs: events.append("shader-receipt"),
        ), patch(
            "scripts.run_migration_parity.write_pipeline_execution_evidence",
            side_effect=lambda *_args, **_kwargs: events.append("pipeline-receipt"),
        ), patch(
            "scripts.run_migration_parity.write_child_attestation_from_environment",
            side_effect=child_attestation,
        ), patch(
            "scripts.run_migration_parity.importlib.import_module",
            return_value=TargetCore(),
        ), patch.object(
            parity.sys, "stdin", io.StringIO(json.dumps([case]))
        ), patch.object(parity.sys, "stdout", output):
            status = parity.run_side(args)

        self.assertEqual(status, 0)
        self.assertLess(events.index("workflow"), events.index("shader-receipt"))
        self.assertLess(events.index("shader-receipt"), events.index("pipeline-receipt"))
        self.assertLess(events.index("pipeline-receipt"), events.index("child-attestation"))
        self.assertLess(events.index("child-attestation"), events.index("stdout"))
        self.assertEqual(
            context_seen[0]["context"],
            {
                "lifecycle": "resident",
                "boundary": "whole_workflow",
                "step_ids": ["observe-a", "observe-b"],
                "repeat": 3,
            },
        )
        self.assertEqual(context_seen[0]["declared_features"], parity.TARGET_FEATURES)

    def test_child_without_attestation_environment_emits_no_receipt(self):
        case = {"case_id": "case-a", "steps": []}
        handshake = {"side": "source", "version": parity.ORACLE_VERSION}
        args = argparse.Namespace(
            side="source",
            manifest=Path("manifest.yaml"),
            lifecycle="warm",
            repeat=1,
            timing_step=[],
            timing_boundary="observed_steps",
            timings=False,
        )
        output = io.StringIO()
        environment = {
            "MIGRATION_PARITY_EXECUTION_OUTPUT": "",
            "MIGRATION_GPU_WGSL_COVERAGE_OUTPUT": "",
            PREFIX + "OUTPUT": "",
        }

        with patch.dict(parity.os.environ, environment, clear=False), patch(
            "scripts.run_migration_parity.load_manifest", return_value={}
        ), patch("scripts.run_migration_parity.build_operation_index", return_value={}), patch(
            "scripts.run_migration_parity.side_identity", return_value=handshake
        ), patch(
            "scripts.run_migration_parity.run_case",
            return_value={"case_id": "case-a", "status": "completed", "observations": []},
        ), patch.object(parity.sys, "stdin", io.StringIO(json.dumps([case]))), patch.object(
            parity.sys, "stdout", output
        ):
            status = parity.run_side(args)

        self.assertEqual(status, 0)
        envelope = json.loads(output.getvalue())
        self.assertEqual(set(envelope), {"identity", "results"})
        self.assertEqual(envelope["identity"], handshake)
        self.assertEqual(envelope["results"][0]["case_id"], "case-a")


if __name__ == "__main__":
    unittest.main()
