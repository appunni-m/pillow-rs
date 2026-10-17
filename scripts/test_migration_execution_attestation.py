"""Synthetic contracts for sidecar collection, without image workloads."""

from __future__ import annotations

import copy
import gzip
import hashlib
import json
import os
from pathlib import Path
import unittest
from unittest import mock

try:
    import migration_execution_attestation as attestation
    import migration_execution_identity as identity
    from test_migration_execution_identity import RuntimeIdentityTests
except ModuleNotFoundError:
    from scripts import migration_execution_attestation as attestation
    from scripts import migration_execution_identity as identity
    from scripts.test_migration_execution_identity import RuntimeIdentityTests


class ExecutionAttestationTests(unittest.TestCase):
    def setUp(self) -> None:
        # Reuse the synthetic package fixture; never import a real image API.
        self.fixture = RuntimeIdentityTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.root = self.fixture.root
        _, source_handshake = self.fixture._source()
        self.source = identity.capture_runtime_identity("source", source_handshake, "pillow", [])
        _, _, _, target_handshake = self.fixture._target()
        self.target = self.fixture._capture_target(target_handshake)
        _, _, _, simd_handshake = self.fixture._target(
            backend="simd", active=["simd"]
        )
        self.simd_target = self.fixture._capture_target(
            simd_handshake, profile="python-simd"
        )
        self.context = {"lifecycle": "warm", "boundary": None, "step_ids": [], "repeat": 1}
        self.case = {"case_id": "synthetic-case"}

    def receipt(self, environment: dict[str, str], *, side: str, context=None, case_ids=None):
        return {
            "schema": attestation.CHILD_SCHEMA,
            "status": "complete",
            "invocation_id": environment[attestation.PREFIX + "INVOCATION_ID"],
            "role": environment[attestation.PREFIX + "ROLE"],
            "side": side,
            "profile": environment[attestation.PREFIX + "PROFILE"],
            "case_ids": case_ids or [self.case["case_id"]],
            "context": copy.deepcopy(context or self.context),
            "runtime": copy.deepcopy(self.source if side == "source" else self.target),
            "error": None,
        }

    def child(
        self,
        collector,
        *,
        side="target",
        role="parity",
        context=None,
        runtime=None,
        profile=None,
    ):
        context = context or self.context
        environment = collector.child_environment(
            side=side,
            profile=profile or ("pillow" if side == "source" else "python-cpu"),
            role=role, cases=[self.case], context=context,
        )
        document = self.receipt(
            environment,
            side=side,
            context=context,
            case_ids=[self.case["case_id"]],
        )
        if runtime is not None:
            document["runtime"] = runtime
        attestation._publish(Path(environment[attestation.PREFIX + "OUTPUT"]), document)
        collector.collect(environment)
        return environment

    def result(self, kind="parity"):
        result = {
            "schema": f"migration-parity/{kind}-result@1", "status": "completed",
            "identity": {"run_id": "synthetic-run", "manifest": {"sha256": "a" * 64}, "inputs": []},
        }
        if kind == "parity":
            result["comparisons"] = [{"case_id": self.case["case_id"], "target_profile": "python-cpu"}]
            selection = {"case_profiles": result["comparisons"], "workload_profiles": []}
        else:
            result["workloads"] = [{"workload_id": "synthetic-workload", "subjects": [{"id": "pillow"}, {"id": "python-cpu"}]}]
            selection = {"case_profiles": [], "workload_profiles": [
                {"workload_id": "synthetic-workload", "case_id": self.case["case_id"], "subject_id": subject}
                for subject in ["pillow", "python-cpu"]
            ]}
        path = self.root / f"{kind}.json"
        path.write_text(json.dumps(result) + "\n", encoding="utf-8")
        return path, result, selection

    def complete(self, kind="parity"):
        collector = attestation.AttestationCollector(self.root / f"{kind}-attestation.json")
        if kind == "parity":
            self.child(collector, side="source")
            self.child(collector)
        else:
            timed = {"lifecycle": "warm", "boundary": "whole_workflow", "step_ids": ["call"], "repeat": 2}
            self.child(collector, side="source", role="benchmark_timed", context=timed)
            self.child(collector, role="benchmark_timed", context=timed)
            self.child(collector, role="benchmark_execution_proof")
        path, result, selection = self.result(kind)
        document = collector.finalize(path, result=result, selection=selection)
        return collector, path, result, document

    def test_complete_parity_binds_exact_membership_and_closed_report(self):
        _, path, result, document = self.complete()
        self.assertEqual(document["status"], "complete")
        self.assertEqual(document["report"]["canonical_sha256"], hashlib.sha256(path.read_bytes()).hexdigest())
        attestation.validate_attestation(document, result=result)
        self.assertEqual(len(document["invocations"]), 2)
        self.assertEqual(set(result), {"schema", "status", "identity", "comparisons"})

    def test_merged_backend_parity_attestation_binds_union_and_source_reruns(self):
        backend_attestations = []
        for profile, target_runtime in (
            ("python-cpu", self.target),
            ("python-simd", self.simd_target),
        ):
            backend_result = {
                "schema": "migration-parity/parity-result@1",
                "status": "completed",
                "identity": {
                    "run_id": f"run-{profile}",
                    "manifest": {"sha256": "a" * 64},
                    "inputs": [],
                },
                "comparisons": [
                    {
                        "case_id": self.case["case_id"],
                        "target_profile": profile,
                    }
                ],
            }
            report_path = self.root / f"{profile}.json"
            report_path.write_text(json.dumps(backend_result) + "\n")
            collector = attestation.AttestationCollector(
                self.root / f"{profile}-attestation.json"
            )
            self.child(collector, side="source")
            self.child(
                collector,
                profile=profile,
                runtime=target_runtime,
            )
            document = collector.finalize(
                report_path,
                result=backend_result,
                selection={
                    "case_profiles": [
                        {
                            "case_id": self.case["case_id"],
                            "target_profile": profile,
                        }
                    ],
                    "workload_profiles": [],
                },
            )
            self.assertEqual(document["status"], "complete")
            backend_attestations.append(
                (
                    document,
                    backend_result,
                    attestation.report_fingerprint(report_path),
                )
            )

        merged_result = {
            "schema": "migration-parity/parity-result@1",
            "status": "completed",
            "identity": {
                "run_id": "merged-run",
                "manifest": {"sha256": "a" * 64},
                "inputs": [],
                "command": {
                    "command_id": "parity",
                    "argv": ["make", "migration-parity-benchmark"],
                },
            },
            "comparisons": [
                {"case_id": self.case["case_id"], "target_profile": profile}
                for profile in ("python-cpu", "python-simd")
            ],
        }
        merged_path = self.root / "merged.json"
        merged_path.write_text(json.dumps(merged_result) + "\n")
        merged_attestation = attestation.merge_parity_attestations(
            self.root / "merged-attestation.json",
            merged_path,
            merged_result,
            backend_attestations,
        )
        self.assertEqual(merged_attestation["status"], "complete")
        self.assertEqual(len(merged_attestation["invocations"]), 4)
        with self.assertRaisesRegex(ValueError, "duplicate"):
            attestation.validate_attestation(merged_attestation, result=merged_result)
        attestation.validate_attestation(
            merged_attestation,
            result=merged_result,
            allow_duplicate_parity_sources=True,
        )

    def test_benchmark_requires_separate_target_timing_and_proof(self):
        _, _, result, document = self.complete("benchmark")
        self.assertEqual(document["status"], "complete")
        for role in ["benchmark_timed", "benchmark_execution_proof"]:
            changed = copy.deepcopy(document)
            changed["invocations"] = [r for r in changed["invocations"] if not (r["side"] == "target" and r["role"] == role)]
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "membership"):
                attestation.validate_attestation(changed, result=result)

    def test_current_benchmark_proof_must_match_reused_target_identity(self):
        collector, _, _, document = self.complete("benchmark")
        self.assertEqual(document["status"], "complete")
        collector.require_runtime_identities(
            {("target", "python-cpu"): self.target["stable_sha256"]},
            role="benchmark_execution_proof",
        )
        with self.assertRaisesRegex(ValueError, "differs from reused parity"):
            collector.require_runtime_identities(
                {("target", "python-cpu"): "0" * 64},
                role="benchmark_execution_proof",
            )
        with self.assertRaisesRegex(ValueError, "no benchmark_execution_proof"):
            collector.require_runtime_identities(
                {("target", "python-simd"): self.target["stable_sha256"]},
                role="benchmark_execution_proof",
            )

    def test_missing_corrupt_and_uncollected_receipts_are_not_proven(self):
        for index, mode in enumerate(["missing", "corrupt", "uncollected"]):
            collector = attestation.AttestationCollector(self.root / f"failed-{index}.json")
            self.child(collector, side="source")
            environment = collector.child_environment(side="target", profile="python-cpu", role="parity", cases=[self.case], context=self.context)
            if mode == "corrupt":
                path = Path(environment[attestation.PREFIX + "OUTPUT"])
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("{incomplete", encoding="utf-8")
            if mode != "uncollected":
                collector.collect(environment)
            path, result, selection = self.result()
            original = path.read_bytes()
            document = collector.finalize(path, result=result, selection=selection)
            with self.subTest(mode=mode):
                self.assertEqual(document["status"], "not_proven")
                self.assertTrue(document["errors"])
                self.assertEqual(path.read_bytes(), original)

    def test_duplicate_extra_and_unknown_fields_are_rejected(self):
        _, _, result, document = self.complete()
        mutations = [
            lambda d: d.update(extra=True),
            lambda d: d["invocations"].append(copy.deepcopy(d["invocations"][0])),
            lambda d: d["selection"]["case_profiles"].append({"case_id": "extra", "target_profile": "python-cpu"}),
            lambda d: d["invocations"][0]["receipt"].update(extra=True),
            lambda d: d.update(errors={}),
            lambda d: d["report"].update(canonical_bytes=True),
        ]
        for index, mutate in enumerate(mutations):
            changed = copy.deepcopy(document)
            mutate(changed)
            with self.subTest(index=index), self.assertRaises((ValueError, identity.RuntimeIdentityError)):
                attestation.validate_attestation(changed, result=result)

    def test_timed_proof_and_cold_child_identity_drift_are_rejected(self):
        for index, lifecycle in enumerate(["warm", "cold"]):
            collector = attestation.AttestationCollector(self.root / f"drift-{index}.json")
            timed = {"lifecycle": lifecycle, "boundary": "whole_workflow", "step_ids": ["call"], "repeat": 1}
            self.child(collector, side="source", role="benchmark_timed", context=timed)
            self.child(collector, role="benchmark_timed", context=timed)
            drift = copy.deepcopy(self.target)
            drift["stable"]["host"]["runner_id"] = "changed-runner"
            drift["stable_sha256"] = identity._canonical_json_sha256(drift["stable"])
            if lifecycle == "cold":
                self.child(collector, role="benchmark_timed", context=timed, runtime=drift)
                self.child(collector, role="benchmark_execution_proof", context={**self.context, "lifecycle": lifecycle})
            else:
                self.child(collector, role="benchmark_execution_proof", runtime=drift)
            path, result, selection = self.result("benchmark")
            document = collector.finalize(path, result=result, selection=selection)
            with self.subTest(lifecycle=lifecycle):
                self.assertEqual(document["status"], "not_proven")
                self.assertIn("identity changed", document["errors"][0])

    def test_report_mutation_and_run_id_mismatch_are_rejected(self):
        _, path, result, document = self.complete()
        changed = copy.deepcopy(document)
        changed["report"]["run_id"] = "other-run"
        with self.assertRaisesRegex(ValueError, "run ID"):
            attestation.validate_attestation(changed, result=result)
        path.write_text(path.read_text() + " ", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "bytes changed"):
            attestation.validate_attestation(document, result=result)

    def test_gzip_canonical_hash_and_container_hash_are_distinct(self):
        payload = b'{"schema":"synthetic"}\n'
        path = self.root / "report.JSON.GZ"
        path.write_bytes(gzip.compress(payload))
        binding = attestation.report_fingerprint(path)
        self.assertEqual(binding["canonical_sha256"], hashlib.sha256(payload).hexdigest())
        self.assertEqual(binding["container_sha256"], hashlib.sha256(path.read_bytes()).hexdigest())
        self.assertEqual(binding["canonical_bytes"], len(payload))
        path.write_bytes(path.read_bytes()[:-3])
        with self.assertRaises((EOFError, OSError)):
            attestation.report_fingerprint(path)

    def test_exclusive_publication_preserves_existing_artifacts(self):
        path = self.root / "existing.json"
        path.write_bytes(b"preserve me\n")
        with self.assertRaises(FileExistsError):
            attestation._publish(path, {"new": True})
        with self.assertRaises(FileExistsError):
            attestation.AttestationCollector(path)
        self.assertEqual(path.read_bytes(), b"preserve me\n")

    def test_publication_retains_private_permissions(self):
        path = self.root / "private.json"
        previous = os.umask(0o022)
        try:
            attestation._publish(path, {"private": True})
        finally:
            os.umask(previous)
        self.assertEqual(path.stat().st_mode & 0o777, 0o600)

    def test_planned_context_is_frozen_against_caller_mutation(self):
        collector = attestation.AttestationCollector(self.root / "context.json")
        context = copy.deepcopy(self.context)
        environment = collector.child_environment(side="target", profile="python-cpu", role="parity", cases=[self.case], context=context)
        context["repeat"] = 9
        document = self.receipt(environment, side="target")
        attestation._publish(Path(environment[attestation.PREFIX + "OUTPUT"]), document)
        collector.collect(environment)
        self.assertEqual(collector._collected[document["invocation_id"]]["errors"], [])

    def test_child_capture_is_opt_in_and_failure_preserves_results(self):
        with mock.patch.dict(os.environ, {}, clear=True), mock.patch.object(attestation, "capture_runtime_identity") as capture:
            attestation.write_child_attestation_from_environment("target", {}, [self.case], context=self.context, declared_features=[])
            capture.assert_not_called()
        collector = attestation.AttestationCollector(self.root / "failed-capture.json")
        environment = collector.child_environment(side="target", profile="python-cpu", role="parity", cases=[self.case], context=self.context)
        with mock.patch.dict(os.environ, environment), mock.patch.object(attestation, "capture_runtime_identity", side_effect=RuntimeError("unreadable core")):
            attestation.write_child_attestation_from_environment("target", {}, [self.case], context=self.context, declared_features=[])
        child = json.loads(Path(environment[attestation.PREFIX + "OUTPUT"]).read_text())
        attestation.validate_child_receipt(child)
        self.assertEqual(child["status"], "not_proven")
        self.assertEqual(child["error"], "unreadable core")


if __name__ == "__main__":
    unittest.main()
