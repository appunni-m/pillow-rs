"""Focused producer companion contracts; no image API or image workload."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import sys
import unittest
from unittest import mock

from scripts import migration_execution_attestation as execution
from scripts import migration_producer_attestation as companion
from scripts import migration_producer_identity as identity
from scripts.test_migration_producer_identity import ProducerIdentityTests


class ProducerAttestationTests(unittest.TestCase):
    def setUp(self):
        self.fixture = ProducerIdentityTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.root = self.fixture.base
        self.bundle = self.fixture._bundle()
        self.parent = identity.capture_producer_identity(self.bundle, role="parent", modules={
            "__main__": self.fixture._module("__main__", self.fixture.parent),
        })
        self.child = identity.capture_producer_identity(self.bundle, role="child", modules=self.fixture._child_modules(self.bundle))
        self.capture_bundle = mock.patch.object(companion, "capture_producer_bundle", return_value=self.bundle)
        self.capture_identity = mock.patch.object(companion, "capture_producer_identity", side_effect=lambda _bundle, *, role: copy.deepcopy(self.parent if role == "parent" else self.child))
        self.capture_bundle.start()
        self.capture_identity.start()
        # Earlier runner guards can add scripts/ to sys.path. Match the actual
        # lazy-import aliases so mocks cover the producer object being called.
        aliases = mock.patch.dict(sys.modules, {
            "migration_producer_attestation": companion,
            "migration_execution_attestation": execution,
            "migration_producer_identity": identity,
        })
        aliases.start()
        self.addCleanup(aliases.stop)
        self.addCleanup(self.capture_bundle.stop)
        self.addCleanup(self.capture_identity.stop)

    def collector(self, name="producer.json"):
        return companion.ProducerCollector(self.root / name)

    def child_receipt(self, collector, invocation="one"):
        environment = collector.child_environment(invocation)
        with mock.patch.dict(os.environ, environment, clear=False):
            companion.write_child_producer_from_environment()
        collector.collect(invocation)
        return environment

    def finalized(self, collector, invocation_ids=None):
        report = self.root / "report.json"
        report.write_bytes(b'{"schema":"synthetic"}\n')
        receipt = {
            "schema": execution.SCHEMA, "status": "complete",
            "report": {"canonical_sha256": hashlib.sha256(report.read_bytes()).hexdigest(),
                "schema": "migration-parity/parity-result@1", "run_id": "synthetic-run"},
            "invocations": [{"invocation_id": invocation} for invocation in (invocation_ids or ["one"])],
        }
        path = self.root / "execution.json"
        execution._publish(path, receipt)
        original = (report.read_bytes(), path.read_bytes())
        result = collector.finalize(path, receipt)
        self.assertEqual(original, (report.read_bytes(), path.read_bytes()))
        return result, path

    def test_companion_binds_closed_bytes_parent_and_every_child(self):
        collector = self.collector()
        self.child_receipt(collector)
        result, path = self.finalized(collector)
        self.assertEqual(result["status"], "complete")
        self.assertEqual(result["execution_attestation_sha256"], hashlib.sha256(path.read_bytes()).hexdigest())
        self.assertEqual(result["producer_bundle_sha256"], hashlib.sha256(collector.bundle_path.read_bytes()).hexdigest())
        self.assertEqual(result["parent_start"]["role"], "parent")
        self.assertEqual(result["parent_end"]["role"], "parent")
        self.assertTrue(result["observed_invocation"]["runner_argv"])
        self.assertIn("effective_make_context", result["observed_invocation"])
        self.assertEqual(result["invocations"][0]["receipt"]["identity"]["role"], "child")

    def test_backend_merge_binds_the_merged_report_execution_and_children(self):
        backend_artifacts = []
        invocation_ids = ["cpu-invocation", "simd-invocation"]
        for index, invocation_id in enumerate(invocation_ids):
            report_path = self.root / f"backend-{index}.json"
            backend_report = {
                "schema": "migration-parity/parity-result@1",
                "status": "completed",
                "identity": {"run_id": f"backend-run-{index}"},
            }
            report_path.write_text(json.dumps(backend_report), encoding="utf-8")
            backend_execution_path = self.root / f"backend-{index}.execution.json"
            backend_execution = {
                "schema": execution.SCHEMA,
                "status": "complete",
                "report": {
                    "canonical_sha256": hashlib.sha256(report_path.read_bytes()).hexdigest(),
                    "schema": backend_report["schema"],
                    "run_id": backend_report["identity"]["run_id"],
                },
                "invocations": [{"invocation_id": invocation_id}],
            }
            execution._publish(backend_execution_path, backend_execution)

            collector = self.collector(f"backend-{index}.producer.json")
            self.child_receipt(collector, invocation_id)
            producer_path = collector.output
            producer = collector.finalize(backend_execution_path, backend_execution)
            self.assertEqual(producer["status"], "complete")
            backend_artifacts.append(
                (
                    producer_path,
                    backend_execution_path,
                    collector.bundle_path,
                )
            )

        merged_report_path = self.root / "merged.json"
        merged_report = {
            "schema": "migration-parity/parity-result@1",
            "status": "completed",
            "identity": {"run_id": "merged-run"},
        }
        merged_report_path.write_text(json.dumps(merged_report), encoding="utf-8")
        merged_execution_path = self.root / "merged.execution.json"
        merged_execution = {
            "schema": execution.SCHEMA,
            "status": "complete",
            "report": {
                **execution.report_fingerprint(merged_report_path),
                "schema": merged_report["schema"],
                "run_id": merged_report["identity"]["run_id"],
            },
            "invocations": [{"invocation_id": item} for item in invocation_ids],
            "errors": [],
        }
        execution._publish(merged_execution_path, merged_execution)
        output = self.root / "merged.producer.json"

        result = companion.merge_backend_attestations(
            output,
            merged_report_path,
            merged_execution_path,
            merged_report,
            backend_artifacts,
        )

        self.assertEqual(result["status"], "complete")
        self.assertEqual(
            [item["invocation_id"] for item in result["invocations"]], invocation_ids
        )
        self.assertEqual(
            result["execution_attestation_sha256"],
            hashlib.sha256(merged_execution_path.read_bytes()).hexdigest(),
        )
        bundle_path = output.with_name(output.name + ".bundle.json")
        self.assertEqual(
            result["producer_bundle_sha256"],
            hashlib.sha256(bundle_path.read_bytes()).hexdigest(),
        )

    def test_backend_merge_rejects_bundle_changes_before_publication(self):
        report_path = self.root / "backend.json"
        report = {
            "schema": "migration-parity/parity-result@1",
            "status": "completed",
            "identity": {"run_id": "backend-run"},
        }
        report_path.write_text(json.dumps(report), encoding="utf-8")
        backend_execution_path = self.root / "backend.execution.json"
        backend_execution = {
            "schema": execution.SCHEMA,
            "status": "complete",
            "report": {
                "canonical_sha256": hashlib.sha256(report_path.read_bytes()).hexdigest(),
                "schema": report["schema"],
                "run_id": report["identity"]["run_id"],
            },
            "invocations": [{"invocation_id": "one"}],
        }
        execution._publish(backend_execution_path, backend_execution)
        collector = self.collector("backend.producer.json")
        self.child_receipt(collector)
        producer_path = collector.output
        collector.finalize(backend_execution_path, backend_execution)
        collector.bundle_path.write_bytes(collector.bundle_path.read_bytes() + b" ")

        merged_report_path = self.root / "merged.json"
        merged_report_path.write_text(json.dumps(report), encoding="utf-8")
        merged_execution_path = self.root / "merged.execution.json"
        merged_execution = {
            "schema": execution.SCHEMA,
            "status": "complete",
            "report": {
                **execution.report_fingerprint(merged_report_path),
                "schema": report["schema"],
                "run_id": "merged-run",
            },
            "invocations": [{"invocation_id": "one"}],
            "errors": [],
        }
        execution._publish(merged_execution_path, merged_execution)
        output = self.root / "merged.producer.json"

        with self.assertRaisesRegex(ValueError, "bundle hash differs"):
            companion.merge_backend_attestations(
                output,
                merged_report_path,
                merged_execution_path,
                {**report, "identity": {"run_id": "merged-run"}},
                [(producer_path, backend_execution_path, collector.bundle_path)],
            )
        self.assertFalse(output.exists())

    def test_missing_corrupt_uncollected_and_mismatched_children_are_unproven(self):
        for index, mode in enumerate(["missing", "corrupt", "uncollected", "wrong-invocation", "wrong-bundle"]):
            collector = self.collector(f"producer-{index}.json")
            environment = collector.child_environment("one")
            path = Path(environment[companion.PREFIX + "OUTPUT"])
            if mode in {"wrong-invocation", "wrong-bundle"}:
                receipt = {"schema": companion.CHILD_SCHEMA, "status": "complete",
                    "invocation_id": "other" if mode == "wrong-invocation" else "one",
                    "producer_bundle_sha256": "0" * 64 if mode == "wrong-bundle" else collector.bundle_sha,
                    "identity": self.child, "error": None}
                execution._publish(path, receipt)
            elif mode == "corrupt":
                path.parent.mkdir(parents=True)
                path.write_text("{broken", encoding="utf-8")
            if mode != "uncollected":
                collector.collect("one")
            # Each subcase has independent report/receipt filenames.
            with mock.patch.object(execution, "_publish", wraps=execution._publish):
                report = self.root / f"report-{index}.json"
                execution._publish(report, {"report": {"canonical_sha256": "a" * 64,
                    "schema": "migration-parity/parity-result@1", "run_id": "run"},
                    "invocations": [{"invocation_id": "one"}]})
                result = collector.finalize(report, json.loads(report.read_bytes()))
            with self.subTest(mode=mode):
                self.assertEqual(result["status"], "not_proven")
                self.assertTrue(result["errors"])
        collector = self.collector("duplicate.json")
        self.child_receipt(collector)
        with self.assertRaisesRegex(ValueError, "more than once"):
            collector.collect("one")

    def test_mutated_bundle_and_parent_loaded_module_disappearance_are_unproven(self):
        for index, mode in enumerate(["bundle", "module"]):
            collector = self.collector(f"mutated-{index}.json")
            self.child_receipt(collector)
            if mode == "bundle":
                collector.bundle_path.write_bytes(collector.bundle_path.read_bytes() + b" ")
            else:
                # Parent may gain imports, but an existing loaded identity cannot vanish.
                collector.parent_start["loaded_modules"].append({"module": "lost"})
            path = self.root / f"mutation-execution-{index}.json"
            receipt = {"report": {"canonical_sha256": "a" * 64,
                "schema": "migration-parity/parity-result@1", "run_id": "run"},
                "invocations": [{"invocation_id": "one"}]}
            execution._publish(path, receipt)
            result = collector.finalize(path, receipt)
            with self.subTest(mode=mode):
                self.assertEqual(result["status"], "not_proven")
                self.assertTrue(result["errors"])

    def test_existing_producer_artifacts_are_preserved(self):
        for name in ["existing.json", "existing-bundle.json"]:
            path = self.root / name
            preserved = path if name == "existing.json" else path.with_name(path.name + ".bundle.json")
            preserved.write_bytes(b"preserve\n")
            with self.assertRaises(FileExistsError):
                companion.ProducerCollector(path)
            self.assertEqual(preserved.read_bytes(), b"preserve\n")

    def test_optional_execution_collector_forwards_same_invocation_and_closes_first(self):
        producer = mock.Mock()
        producer.child_environment.return_value = {companion.PREFIX + "OUTPUT": "/optional.json"}
        with mock.patch.object(companion, "ProducerCollector", return_value=producer):
            collector = execution.AttestationCollector(self.root / "runtime.json", producer_output=self.root / "optional.json")
        environment = collector.child_environment(side="target", profile="python-cpu", role="parity",
            cases=[{"case_id": "case"}], context={"lifecycle": "warm", "boundary": None, "step_ids": [], "repeat": 1})
        invocation = environment[execution.PREFIX + "INVOCATION_ID"]
        producer.child_environment.assert_called_once_with(invocation)
        collector.collect(environment)  # Missing runtime receipt remains an independent failure.
        producer.collect.assert_called_once_with(invocation)
        path = self.root / "raw-result.json"
        path.write_bytes(b"{}\n")
        result = {"schema": "migration-parity/parity-result@1", "status": "completed",
            "identity": {"run_id": "run", "manifest": {}, "inputs": []}, "comparisons": []}
        def finalized(receipt_path, document):
            self.assertEqual(json.loads(receipt_path.read_bytes()), document)
            self.assertNotIn("producer", document)
            self.assertEqual(path.read_bytes(), b"{}\n")
        producer.finalize.side_effect = finalized
        collector.finalize(path, result=result, selection={"case_profiles": [], "workload_profiles": []})
        producer.finalize.assert_called_once()

    def test_child_capture_failure_is_diagnostic_and_base_publication_precedes_companion(self):
        events = []
        environment = {execution.PREFIX + "OUTPUT": str(self.root / "child-runtime.json"),
            execution.PREFIX + "INVOCATION_ID": "one", execution.PREFIX + "ROLE": "parity",
            execution.PREFIX + "PROFILE": "python-cpu", companion.PREFIX + "OUTPUT": "/optional.json"}
        with mock.patch.dict(os.environ, environment, clear=False), \
            mock.patch.object(execution, "capture_runtime_identity", side_effect=ValueError("capture failed")), \
            mock.patch.object(execution, "_publish", side_effect=lambda *_: events.append("runtime-closed")), \
            mock.patch.object(companion, "write_child_producer_from_environment", side_effect=lambda: events.append("producer-after-public-calls")):
            execution.write_child_attestation_from_environment("target", {}, [{"case_id": "case"}],
                context={"lifecycle": "warm", "boundary": None, "step_ids": [], "repeat": 1}, declared_features=[])
        self.assertEqual(events, ["runtime-closed", "producer-after-public-calls"])

    def test_producer_cli_option_requires_execution_option_before_loading_workloads(self):
        from scripts import run_migration_benchmark, run_migration_parity
        args = argparse.Namespace(attestation_output=None, producer_attestation_output=Path("producer.json"))
        for run in [run_migration_benchmark.run, run_migration_parity.run_orchestrator]:
            with self.subTest(run=run.__name__), self.assertRaisesRegex(ValueError, "requires --attestation-output"):
                run(args)

        parity_args = argparse.Namespace(
            attestation_output=None,
            producer_attestation_output=None,
            parity_producer_attestation_output=Path("parity.producer.json"),
        )
        with self.assertRaisesRegex(ValueError, "requires --parity-attestation-output"):
            run_migration_benchmark.run(parity_args)


if __name__ == "__main__":
    unittest.main()
