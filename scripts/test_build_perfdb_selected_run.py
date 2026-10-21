"""Selected-run packaging contracts; do not execute image workloads."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import shutil
import tempfile
import unittest

import yaml

from scripts import build_perfdb_selected_run as selected
from scripts import migration_build_witness as build_witness
from scripts import migration_execution_attestation as execution
from scripts import migration_execution_identity as runtime_identity
from scripts import migration_producer_identity as producer_identity


class SelectedRunPackageTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        from scripts.test_migration_execution_attestation import ExecutionAttestationTests
        from scripts.test_migration_producer_identity import ProducerIdentityTests

        self.execution_fixture = ExecutionAttestationTests(
            methodName="test_complete_parity_binds_exact_membership_and_closed_report"
        )
        self.execution_fixture.setUp()
        self.addCleanup(self.execution_fixture.doCleanups)
        self.execution_fixture.case = {"case_id": "PIL.Image.test_operation.mode.l"}
        self.execution_fixture.target["stable"]["host"]["runner_id"] = "test-runner"
        self.execution_fixture.target["stable_sha256"] = runtime_identity._canonical_json_sha256(
            self.execution_fixture.target["stable"]
        )
        self.producer_fixture = ProducerIdentityTests()
        self.producer_fixture.setUp()
        self.addCleanup(self.producer_fixture.doCleanups)
        self.fixture_root = self.root / "fixtures"
        (self.fixture_root / "inputs" / "benchmark").mkdir(parents=True)
        (self.fixture_root / "inputs" / "parity").mkdir(parents=True)
        self.manifest_path = self.root / "manifest.yaml"
        self.manifest = {
            "schema": selected.MANIFEST_SCHEMA,
            "target_profiles": [{"id": "python-cpu", "backend": "cpu"}],
        }
        self.manifest_path.write_text(
            yaml.safe_dump(self.manifest, sort_keys=True), encoding="utf-8"
        )

        self.workload_id = "pil-image.test-operation.materialized"
        self.case_id = "PIL.Image.test_operation.mode.l"
        self.benchmark_input_path = self.fixture_root / "inputs/benchmark/input.json"
        self.benchmark_full_input_path = self.fixture_root / "inputs/benchmark/full.json"
        self.parity_input_path = self.fixture_root / "inputs/parity/input.json"
        self.parity_full_input_path = self.fixture_root / "inputs/parity/full.json"
        self.benchmark_input = {
            "schema": selected.BENCHMARK_INPUT_SCHEMA,
            "workloads": [
                {
                    "workload_id": self.workload_id,
                    "input": {"kind": "parity_case", "case_id": self.case_id},
                }
            ],
        }
        self.parity_input = {
            "schema": selected.PARITY_INPUT_SCHEMA,
            "cases": [{"case_id": self.case_id}],
        }
        self.benchmark_full_input = {
            "schema": selected.BENCHMARK_INPUT_SCHEMA,
            "workloads": [
                {
                    "workload_id": "pil-image.other-operation.materialized",
                    "input": {
                        "kind": "parity_case",
                        "case_id": "PIL.Image.other_operation.mode.l",
                    },
                }
            ],
        }
        self.parity_full_input = {
            "schema": selected.PARITY_INPUT_SCHEMA,
            "cases": [{"case_id": "PIL.Image.other_operation.mode.l"}],
        }
        self.write_json(self.benchmark_input_path, self.benchmark_input)
        self.write_json(self.benchmark_full_input_path, self.benchmark_full_input)
        self.write_json(self.parity_input_path, self.parity_input)
        self.write_json(self.parity_full_input_path, self.parity_full_input)
        self.benchmark_input_reference = self.input_reference(
            self.benchmark_input_path,
            "inputs/benchmark/input.json",
            selected.BENCHMARK_INPUT_SCHEMA,
        )
        self.parity_input_reference = self.input_reference(
            self.parity_input_path,
            "inputs/parity/input.json",
            selected.PARITY_INPUT_SCHEMA,
        )
        self.benchmark_full_input_reference = self.input_reference(
            self.benchmark_full_input_path,
            "inputs/benchmark/full.json",
            selected.BENCHMARK_INPUT_SCHEMA,
        )
        self.parity_full_input_reference = self.input_reference(
            self.parity_full_input_path,
            "inputs/parity/full.json",
            selected.PARITY_INPUT_SCHEMA,
        )
        self.manifest_reference = {
            "path": "pillow-rs/tests/fixtures/manifest.yaml",
            "schema": selected.MANIFEST_SCHEMA,
            "sha256": hashlib.sha256(self.manifest_path.read_bytes()).hexdigest(),
        }

        self.benchmark_report_path = self.root / "benchmark.json"
        self.parity_report_path = self.root / "parity.json"
        self.benchmark_report = {
            "schema": selected.BENCHMARK_SCHEMA,
            "status": "completed",
            "identity": {
                "run_id": "benchmark-run",
                "manifest": self.manifest_reference,
                "inputs": [
                    self.benchmark_input_reference,
                    self.benchmark_full_input_reference,
                    self.parity_input_reference,
                    self.parity_full_input_reference,
                ],
                "targets": [self.target_identity()],
            },
            "environment": {"runner_id": "test-runner"},
            "workloads": [
                {
                    "workload_id": self.workload_id,
                    "correctness": {
                        "outcome": "pass",
                        "evidence_id": "parity-run",
                    },
                    "subjects": [
                        {
                            "kind": "target_profile",
                            "id": "python-cpu",
                            "status": "completed",
                            "execution": {"actual_backend": "cpu"},
                        }
                    ],
                }
            ],
        }
        self.parity_report = {
            "schema": selected.PARITY_SCHEMA,
            "status": "completed",
            "identity": {
                "run_id": "parity-run",
                "manifest": self.manifest_reference,
                "inputs": [
                    self.parity_input_reference,
                    self.parity_full_input_reference,
                ],
                "targets": [self.target_identity()],
            },
            "summary": {"failed": 0},
            "infrastructure_errors": [],
            "comparisons": [
                {
                    "case_id": self.case_id,
                    "target_profile": "python-cpu",
                    "outcome": "pass",
                }
            ],
        }
        self.write_json(self.benchmark_report_path, self.benchmark_report)
        self.write_json(self.parity_report_path, self.parity_report)

        self.benchmark_attestation_path = self.root / "benchmark.attestation.json"
        self.parity_attestation_path = self.root / "parity.attestation.json"
        self.write_execution(self.benchmark_attestation_path, self.benchmark_report_path, self.benchmark_report)
        self.write_execution(self.parity_attestation_path, self.parity_report_path, self.parity_report)
        self.benchmark_bundle_path = self.root / "benchmark.producer.json.bundle.json"
        self.parity_bundle_path = self.root / "parity.producer.json.bundle.json"
        self.write_json(
            self.benchmark_bundle_path,
            {"schema": selected.PRODUCER_BUNDLE_SCHEMA},
        )
        self.write_json(
            self.parity_bundle_path,
            {"schema": selected.PRODUCER_BUNDLE_SCHEMA},
        )
        self.benchmark_producer_path = self.root / "benchmark.producer.json"
        self.parity_producer_path = self.root / "parity.producer.json"
        self.write_producer(
            self.benchmark_producer_path,
            self.benchmark_bundle_path,
            self.benchmark_attestation_path,
            self.benchmark_report,
        )
        self.write_producer(
            self.parity_producer_path,
            self.parity_bundle_path,
            self.parity_attestation_path,
            self.parity_report,
        )
        self.source_build_path = self.root / "source-build-witness.json"
        self.write_json(
            self.source_build_path,
            {
                "schema": selected.MATERIALIZED_BUILD_WITNESS_SCHEMA,
                "status": "complete",
                "workspace": {"git_head": "test-revision", "dirty": False},
                "build": {"features": {"pillow-rs-py": [], "pillow-rs": []}},
                "errors": [],
                "materialization": {
                    "schema": "pillow-rs/maturin-materialization@1",
                    "status": "complete",
                    "module_artifact": self.materialized_module(),
                },
            },
        )

    def target_identity(self) -> dict:
        return {
            "target_profile": "python-cpu",
            "target_id": "pillow-rs-python",
            "backend": "cpu",
            "revision": "test-revision",
            "runtime": "3.12.10",
            "dirty": False,
            "features": ["pillow-rs-py/default", "pillow-rs/default"],
        }

    def materialized_module(self) -> dict:
        return dict(
            self.execution_fixture.target["stable"]["target_execution"][
                "selected_core"
            ]["file"]
        )

    def direct_cargo_build_witness(self) -> dict:
        source_files = [
            ("Cargo.toml", b"workspace manifest"),
            ("Cargo.lock", b"workspace lock"),
            ("pillow-rs/Cargo.toml", b"core manifest"),
            ("pillow-rs/src/lib.rs", b"core source"),
            ("pillow-rs-py/Cargo.toml", b"binding manifest"),
            ("pillow-rs-py/pyproject.toml", b"maturin config"),
            ("pillow-rs-py/src/lib.rs", b"binding source"),
        ]
        files = [
            {
                "path": path,
                "size": len(content),
                "sha256": hashlib.sha256(content).hexdigest(),
            }
            for path, content in source_files
        ]
        snapshot = {"sha256": build_witness._canonical_sha256(files), "files": files}
        module = self.materialized_module()
        return {
            "schema": selected.BUILD_WITNESS_SCHEMA,
            "status": "complete",
            "workspace": {
                "root": str(self.root),
                "git_head": "test-revision",
                "dirty": False,
            },
            "source": {"before": snapshot, "after": snapshot},
            "build": {"features": {"pillow-rs-py": ["default"], "pillow-rs": ["default"]}},
            "artifact": {
                "package_id": f"path+file://{self.root}/pillow-rs-py#pillow-rs-py@0.1.0",
                "target": "_core",
                "crate_types": ["cdylib"],
                "path": module["path"],
                "size": module["size"],
                "sha256": module["sha256"],
            },
            "errors": [],
        }

    @staticmethod
    def write_json(path: Path, document: dict) -> None:
        path.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")

    @staticmethod
    def input_reference(path: Path, relative: str, schema: str) -> dict:
        return {
            "path": relative,
            "schema": schema,
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        }

    def write_execution(self, output: Path, report_path: Path, report: dict) -> None:
        collector = execution.AttestationCollector(output)
        self.execution_fixture.case = {"case_id": self.case_id}
        if report["schema"] == selected.PARITY_SCHEMA:
            self.execution_fixture.child(collector, side="source", role="parity")
            self.execution_fixture.child(
                collector,
                role="parity",
                profile="python-cpu",
                runtime=self.execution_fixture.target,
            )
            selection = {
                "case_profiles": [
                    {"case_id": self.case_id, "target_profile": "python-cpu"}
                ],
                "workload_profiles": [],
            }
        else:
            timed = {
                "lifecycle": "warm",
                "boundary": "whole_workflow",
                "step_ids": ["call"],
                "repeat": 2,
            }
            self.execution_fixture.child(
                collector,
                role="benchmark_timed",
                profile="python-cpu",
                context=timed,
                runtime=self.execution_fixture.target,
            )
            self.execution_fixture.child(
                collector,
                role="benchmark_execution_proof",
                profile="python-cpu",
                runtime=self.execution_fixture.target,
            )
            selection = {
                "case_profiles": [],
                "workload_profiles": [
                    {
                        "workload_id": self.workload_id,
                        "case_id": self.case_id,
                        "subject_id": "python-cpu",
                    }
                ],
            }
        collector.finalize(report_path, result=report, selection=selection)

    def write_producer(
        self, output: Path, bundle_path: Path, execution_path: Path, report: dict
    ) -> None:
        bundle = self.producer_fixture._bundle()
        self.write_json(bundle_path, bundle)
        parent_modules = {
            "__main__": self.producer_fixture._module(
                "__main__", self.producer_fixture.parent
            )
        }
        parent_identity = producer_identity.capture_producer_identity(
            bundle, role="parent", modules=parent_modules
        )
        child_identity = producer_identity.capture_producer_identity(
            bundle,
            role="child",
            modules=self.producer_fixture._child_modules(bundle),
        )
        execution_document = selected.read_json(execution_path)
        bundle_digest = hashlib.sha256(bundle_path.read_bytes()).hexdigest()
        report_path = (
            self.benchmark_report_path
            if report is self.benchmark_report
            else self.parity_report_path
        )
        self.write_json(
            output,
            {
                "schema": selected.PRODUCER_SCHEMA,
                "status": "complete",
                "report": {
                    "canonical_sha256": selected.report_fingerprint(report_path)[
                        "canonical_sha256"
                    ],
                    "schema": report["schema"],
                    "run_id": report["identity"]["run_id"],
                },
                "execution_attestation_sha256": hashlib.sha256(
                    execution_path.read_bytes()
                ).hexdigest(),
                "producer_bundle_sha256": bundle_digest,
                "parent_start": parent_identity,
                "parent_end": parent_identity,
                "observed_invocation": {
                    "runner_argv": ["/venv/bin/python", "scripts/run_migration_benchmark.py"],
                    "effective_make_context": None,
                },
                "invocations": [
                    {
                        "invocation_id": invocation["invocation_id"],
                        "receipt": {
                            "schema": "migration-parity/child-producer-attestation@1",
                            "status": "complete",
                            "invocation_id": invocation["invocation_id"],
                            "producer_bundle_sha256": bundle_digest,
                            "identity": child_identity,
                            "error": None,
                        },
                        "errors": [],
                    }
                    for invocation in execution_document["invocations"]
                ],
                "errors": [],
            },
        )

    def build(self, output: Path | None = None) -> Path:
        return selected.build_selected_run(
            benchmark_report_path=self.benchmark_report_path,
            benchmark_attestation_path=self.benchmark_attestation_path,
            benchmark_producer_path=self.benchmark_producer_path,
            benchmark_bundle_path=self.benchmark_bundle_path,
            parity_report_path=self.parity_report_path,
            parity_attestation_path=self.parity_attestation_path,
            parity_producer_path=self.parity_producer_path,
            parity_bundle_path=self.parity_bundle_path,
            manifest_path=self.manifest_path,
            source_build_path=self.source_build_path,
            workload_id=self.workload_id,
            target_profile="python-cpu",
            baseline_name="test-operation-arm64",
            target_policy="tracking",
            max_regression_percent=0,
            output_dir=output or self.root / "selected-run",
            fixture_root=self.fixture_root,
        )

    def test_packages_full_input_closure_and_deduplicates_shared_documents(self) -> None:
        selection_path = self.build()
        selection = json.loads(selection_path.read_text(encoding="utf-8"))
        self.assertEqual(selection["workload_id"], self.workload_id)
        self.assertEqual(selection["case_id"], self.case_id)
        self.assertEqual(selection["target_profile"], "python-cpu")
        self.assertEqual(selection["schema"], "pillow-perfdb/selected-run@2")
        self.assertEqual(
            selection["benchmark"]["input"],
            selection["benchmark"]["identity_inputs"][0]["file"],
        )
        self.assertEqual(
            selection["parity"]["input"],
            selection["parity"]["identity_inputs"][0]["file"],
        )
        package = selection_path.parent
        self.assertEqual(
            (package / selection["benchmark"]["input"]).read_bytes(),
            self.benchmark_input_path.read_bytes(),
        )
        self.assertEqual(
            (package / selection["parity"]["input"]).read_bytes(),
            self.parity_input_path.read_bytes(),
        )
        benchmark_references = self.benchmark_report["identity"]["inputs"]
        parity_references = self.parity_report["identity"]["inputs"]
        self.assertEqual(
            [
                {key: item[key] for key in ("path", "schema", "sha256")}
                for item in selection["benchmark"]["identity_inputs"]
            ],
            benchmark_references,
        )
        self.assertEqual(
            [
                {key: item[key] for key in ("path", "schema", "sha256")}
                for item in selection["parity"]["identity_inputs"]
            ],
            parity_references,
        )
        packaged_files = {
            item["file"]
            for section in ("benchmark", "parity")
            for item in selection[section]["identity_inputs"]
        }
        self.assertEqual(len(packaged_files), 4)
        self.assertEqual(len(list((package / "inputs").iterdir())), 4)
        for section in ("benchmark", "parity"):
            for item in selection[section]["identity_inputs"]:
                source = self.fixture_root / item["path"]
                self.assertEqual((package / item["file"]).read_bytes(), source.read_bytes())
        self.assertEqual(
            (package / "manifest.yaml").read_bytes(), self.manifest_path.read_bytes()
        )
        self.assertEqual(
            json.loads((package / "source-build-witness.json").read_text())["status"],
            "complete",
        )
        self.assertFalse((package / "benchmark.json").exists())
        self.assertFalse((package / "parity.json").exists())

    def test_accepts_rehosted_reports_when_attested_bytes_are_identical(self) -> None:
        for attestation_path, producer_path in (
            (self.benchmark_attestation_path, self.benchmark_producer_path),
            (self.parity_attestation_path, self.parity_producer_path),
        ):
            attestation = selected.read_json(attestation_path)
            attestation["report"]["path"] = (
                "/home/runner/work/pillow-rs/evidence/selected-report.json"
            )
            self.write_json(attestation_path, attestation)
            producer = selected.read_json(producer_path)
            producer["execution_attestation_sha256"] = hashlib.sha256(
                attestation_path.read_bytes()
            ).hexdigest()
            self.write_json(producer_path, producer)

        selection_path = self.build()

        self.assertTrue(selection_path.is_file())

    def test_rehosts_producer_paths_only_for_strict_local_validation(self) -> None:
        bundle = self.producer_fixture._bundle()
        parent_identity = producer_identity.capture_producer_identity(
            bundle,
            role="parent",
            modules={
                "__main__": self.producer_fixture._module(
                    "__main__", self.producer_fixture.parent
                )
            },
        )
        child_identity = producer_identity.capture_producer_identity(
            bundle,
            role="child",
            modules=self.producer_fixture._child_modules(bundle),
        )
        local_root = Path(bundle["root"])
        remote_root = "/remote/runner/scripts"

        def remote_path(value: str) -> str:
            path = Path(value)
            try:
                relative = path.relative_to(local_root)
            except ValueError:
                return value
            return str(Path(remote_root) / relative)

        remote_bundle = json.loads(json.dumps(bundle))
        remote_bundle["root"] = remote_root
        for record in remote_bundle["files"]:
            record["resolved_path"] = remote_path(record["resolved_path"])
        for record in remote_bundle["entrypoints"].values():
            record["path"] = remote_path(record["path"])

        def remote_identity(identity: dict) -> dict:
            updated = json.loads(json.dumps(identity))
            updated["entrypoint"]["path"] = remote_path(
                updated["entrypoint"]["path"]
            )
            for record in updated["loaded_modules"]:
                if record["origin"] is not None:
                    record["origin"] = remote_path(record["origin"])
                record["path"] = remote_path(record["path"])
            updated["files_sha256"] = producer_identity._canonical_json_sha256(
                remote_bundle["files"]
            )
            return updated

        remote_producer = {
            "parent_start": remote_identity(parent_identity),
            "parent_end": remote_identity(parent_identity),
            "invocations": [
                {"receipt": {"identity": remote_identity(child_identity)}}
            ],
        }
        original_root = remote_bundle["root"]
        mapped_bundle, mapped_producer = selected.rehost_producer_evidence_for_validation(
            remote_bundle, remote_producer, {remote_root: str(local_root)}
        )

        producer_identity.validate_producer_bundle(mapped_bundle)
        producer_identity.validate_producer_identity(
            mapped_producer["parent_start"], mapped_bundle
        )
        producer_identity.validate_producer_identity(
            mapped_producer["invocations"][0]["receipt"]["identity"],
            mapped_bundle,
        )
        self.assertEqual(remote_bundle["root"], original_root)
        self.assertEqual(remote_producer["parent_start"]["entrypoint"]["path"],
                         remote_identity(parent_identity)["entrypoint"]["path"])

        changed_root = self.root / "changed-producer-snapshot"
        shutil.copytree(local_root, changed_root)
        (changed_root / "producer_helper.py").write_text("VALUE = 2\n", encoding="utf-8")
        changed_bundle, _ = selected.rehost_producer_evidence_for_validation(
            remote_bundle, remote_producer, {remote_root: str(changed_root.resolve())}
        )
        with self.assertRaisesRegex(
            producer_identity.ProducerIdentityError,
            "differs from the current source file",
        ):
            producer_identity.validate_producer_bundle(changed_bundle)

    def test_rehosted_report_still_requires_exact_attested_bytes(self) -> None:
        attestation = selected.read_json(self.benchmark_attestation_path)
        attestation["report"]["path"] = (
            "/home/runner/work/pillow-rs/evidence/selected-report.json"
        )
        self.write_json(self.benchmark_attestation_path, attestation)
        producer = selected.read_json(self.benchmark_producer_path)
        producer["execution_attestation_sha256"] = hashlib.sha256(
            self.benchmark_attestation_path.read_bytes()
        ).hexdigest()
        self.write_json(self.benchmark_producer_path, producer)
        with self.benchmark_report_path.open("a", encoding="utf-8") as stream:
            stream.write(" ")

        output = self.root / "rejected-rehosted-report-bytes"
        with self.assertRaisesRegex(ValueError, "does not bind its report bytes"):
            self.build(output)
        self.assertFalse(output.exists())

    def test_rejects_a_selected_profile_that_did_not_execute(self) -> None:
        self.benchmark_report["workloads"][0]["subjects"][0]["execution"][
            "actual_backend"
        ] = "simd"
        self.write_json(self.benchmark_report_path, self.benchmark_report)
        output = self.root / "rejected"
        with self.assertRaisesRegex(ValueError, "executed 'simd'"):
            self.build(output)
        self.assertFalse(output.exists())

    def test_rejects_failed_parity_without_creating_a_package(self) -> None:
        self.parity_report["comparisons"][0]["outcome"] = "fail"
        self.parity_report["summary"]["failed"] = 1
        self.write_json(self.parity_report_path, self.parity_report)
        output = self.root / "rejected-parity"
        with self.assertRaisesRegex(ValueError, "report contains failed"):
            self.build(output)
        self.assertFalse(output.exists())

    def test_rejects_benchmark_linked_to_a_different_parity_run(self) -> None:
        self.benchmark_report["workloads"][0]["correctness"]["evidence_id"] = "other-run"
        self.write_json(self.benchmark_report_path, self.benchmark_report)
        output = self.root / "rejected-wrong-parity-run"
        with self.assertRaisesRegex(ValueError, "does not reference the supplied parity run"):
            self.build(output)
        self.assertFalse(output.exists())

    def test_rejects_a_mismatched_loaded_core_hash(self) -> None:
        witness = selected.read_json(self.source_build_path)
        witness["materialization"]["module_artifact"]["sha256"] = "0" * 64
        self.write_json(self.source_build_path, witness)
        output = self.root / "rejected-wrong-core"
        with self.assertRaisesRegex(ValueError, "loaded target core hash differs"):
            self.build(output)
        self.assertFalse(output.exists())

    def test_rejects_incomplete_producer_invocation_membership(self) -> None:
        producer = selected.read_json(self.benchmark_producer_path)
        producer["invocations"].pop()
        self.write_json(self.benchmark_producer_path, producer)
        output = self.root / "rejected-producer-membership"
        with self.assertRaisesRegex(ValueError, "producer invocation membership differs"):
            self.build(output)
        self.assertFalse(output.exists())

    def test_accepts_only_complete_maturin_materialization_witnesses(self) -> None:
        self.write_json(
            self.source_build_path,
            {
                "schema": selected.MATERIALIZED_BUILD_WITNESS_SCHEMA,
                "status": "complete",
                "workspace": {"git_head": "test-revision", "dirty": False},
                "build": {"features": {"pillow-rs-py": [], "pillow-rs": []}},
                "errors": [],
                "materialization": {
                    "schema": "pillow-rs/maturin-materialization@1",
                    "status": "complete",
                    "module_artifact": self.materialized_module(),
                },
            },
        )
        self.build()

        incomplete_path = self.root / "incomplete-witness.json"
        self.write_json(
            incomplete_path,
            {
                "schema": selected.MATERIALIZED_BUILD_WITNESS_SCHEMA,
                "status": "complete",
                "workspace": {"git_head": "test-revision", "dirty": False},
                "errors": [],
                "materialization": {
                    "schema": "pillow-rs/maturin-materialization@1",
                    "status": "not_proven",
                },
            },
        )
        self.source_build_path = incomplete_path
        output = self.root / "rejected-materialization"
        with self.assertRaisesRegex(ValueError, "no complete Maturin module artifact"):
            self.build(output)
        self.assertFalse(output.exists())

    def test_accepts_direct_cargo_witness_when_loaded_core_matches_exactly(self) -> None:
        self.write_json(self.source_build_path, self.direct_cargo_build_witness())

        selection = json.loads(self.build().read_text(encoding="utf-8"))
        self.assertEqual(selection["source_build"], "source-build-witness.json")

    def test_rejects_direct_cargo_witness_when_loaded_core_hash_differs(self) -> None:
        witness = self.direct_cargo_build_witness()
        witness["artifact"]["sha256"] = "0" * 64
        self.write_json(self.source_build_path, witness)
        output = self.root / "rejected-direct-cargo-core"
        with self.assertRaisesRegex(ValueError, "loaded target core hash differs"):
            self.build(output)
        self.assertFalse(output.exists())

if __name__ == "__main__":
    unittest.main()
