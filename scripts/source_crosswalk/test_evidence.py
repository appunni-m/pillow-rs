"""Contracts for the declared fixture-to-operation evidence index."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from scripts.source_crosswalk.evidence import build_declared_evidence_index
from scripts.source_crosswalk.builder import crosswalk_source_identity


class DeclaredEvidenceIndexTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.fixtures = self.root / "pillow-rs/tests/fixtures"
        self.parity_path = self.fixtures / "inputs/parity/cases.json"
        self.benchmark_path = self.fixtures / "inputs/benchmark/workloads.json"
        self.parity_path.parent.mkdir(parents=True)
        self.benchmark_path.parent.mkdir(parents=True)
        self.manifest = {
            "input_index": {
                "parity": ["inputs/parity/cases.json"],
                "benchmark": ["inputs/benchmark/workloads.json"],
            },
            "surfaces": [
                {
                    "id": "PIL.Image",
                    "operations": [
                        {"id": "new", "requirements": [{"id": "PIL.Image.new.behavior.default"}]},
                        {"id": "copy", "requirements": [{"id": "PIL.Image.copy.performance.standard"}]},
                        {"id": "toqimage", "requirements": []},
                    ],
                }
            ],
        }
        self.operation_paths = {
            "PIL.Image.new": "pillow-rs/src/ops/image.rs",
            "PIL.Image.copy": "pillow-rs/src/ops/image.rs",
            "PIL.Image.toqimage": "pillow-rs-py/python/PIL/Image.py",
        }

    def _write_inputs(self, *, include_copy_workload: bool = True) -> None:
        parity_cases = {
            "schema": "migration-parity/parity-input@1",
            "cases": [
                {
                    "case_id": "PIL.Image.copy.behavior.default",
                    "surface": "PIL.Image",
                    "operation": "copy",
                    "covers": [
                        "PIL.Image.copy.behavior.default",
                        "PIL.Image.copy.performance.standard",
                    ],
                    "target_profiles": ["python-cpu"],
                    "steps": [
                        {"step_id": "construct", "surface": "PIL.Image", "operation": "new"},
                        {"step_id": "call", "surface": "PIL.Image", "operation": "copy"},
                    ],
                }
            ],
        }
        workloads = {
            "schema": "migration-parity/benchmark-input@1",
            "workloads": [
                {
                    "workload_id": "copy.from-parity-case",
                    "covers": ["PIL.Image.copy.performance.standard"],
                    "subjects": [
                        {"kind": "oracle", "id": "pillow"},
                        {"kind": "target_profile", "id": "python-cpu"},
                        {"kind": "target_profile", "id": "python-simd"},
                    ],
                    "input": {"kind": "parity_case", "case_id": "PIL.Image.copy.behavior.default"},
                    "measurement": {"boundary": "whole_workflow"},
                }
            ],
        }
        if include_copy_workload:
            workloads["workloads"].append(
                {
                    "workload_id": "copy.workflow",
                    "covers": [],
                    "subjects": [{"kind": "target_profile", "id": "python-cpu"}],
                    "input": {
                        "kind": "workflow",
                        "steps": [
                            {"step_id": "construct", "surface": "PIL.Image", "operation": "new"},
                            {"step_id": "call", "surface": "PIL.Image", "operation": "copy"},
                        ],
                    },
                    "measurement": {"boundary": "observed_steps"},
                }
            )
        self.parity_path.write_text(json.dumps(parity_cases), encoding="utf-8")
        self.benchmark_path.write_text(json.dumps(workloads), encoding="utf-8")

    def test_maps_case_targets_workflow_steps_benchmark_profiles_and_boundaries(self) -> None:
        self._write_inputs()
        report = build_declared_evidence_index(self.root, self.manifest, self.operation_paths)
        operations = {item["operation_id"]: item for item in report["operations"]}

        copy = operations["PIL.Image.copy"]
        self.assertEqual(copy["parity_case_count"], 1)
        self.assertEqual(copy["benchmark_workload_count"], 2)
        parity_case = copy["parity_cases"][0]
        self.assertEqual(parity_case["case_id"], "PIL.Image.copy.behavior.default")
        self.assertIn("case_target", parity_case["relations"])
        self.assertIn("workflow_step", parity_case["relations"])
        self.assertEqual(parity_case["step_ids"], ["call"])

        construct = operations["PIL.Image.new"]
        self.assertEqual(construct["parity_cases"][0]["relations"], ["workflow_step"])
        self.assertEqual(construct["benchmark_workload_count"], 2)
        parity_workload = next(
            item for item in copy["benchmark_workloads"]
            if item["workload_id"] == "copy.from-parity-case"
        )
        self.assertEqual(parity_workload["target_profiles"], ["python-cpu", "python-simd"])
        self.assertEqual(parity_workload["measurement_boundaries"], ["whole_workflow"])
        self.assertEqual(parity_workload["parity_case_ids"], ["PIL.Image.copy.behavior.default"])

        self.assertEqual(report["summary"]["operations_with_parity_cases"], 2)
        self.assertEqual(report["summary"]["operations_without_parity_cases"], 1)
        self.assertEqual(report["summary"]["operations_without_benchmark_workloads"], 1)
        missing = report["review_queues"]["public_operation_without_benchmark_workload"]
        self.assertEqual([item["operation_id"] for item in missing], ["PIL.Image.toqimage"])
        self.assertTrue(all(item["review_only"] for item in missing))
        self.assertFalse(report["scope"]["parity_execution_proven"])
        self.assertFalse(report["scope"]["benchmark_execution_proven"])

    def test_unmapped_operation_and_missing_case_references_are_review_only(self) -> None:
        self._write_inputs(include_copy_workload=False)
        parity = json.loads(self.parity_path.read_text(encoding="utf-8"))
        parity["cases"][0]["steps"].append(
            {"step_id": "unknown", "surface": "PIL.Image", "operation": "unknown"}
        )
        self.parity_path.write_text(json.dumps(parity), encoding="utf-8")
        workloads = json.loads(self.benchmark_path.read_text(encoding="utf-8"))
        workloads["workloads"].append(
            {
                "workload_id": "missing.case",
                "covers": [],
                "subjects": [],
                "input": {"kind": "parity_case", "case_id": "PIL.Image.missing.case"},
                "measurement": {"boundary": "whole_workflow"},
            }
        )
        self.benchmark_path.write_text(json.dumps(workloads), encoding="utf-8")

        report = build_declared_evidence_index(self.root, self.manifest, self.operation_paths)
        unmapped = report["review_queues"]["unmapped_fixture_operation_reference"]
        self.assertEqual(len(unmapped), 1)
        self.assertEqual(unmapped[0]["public_api_path"], "PIL.Image.unknown")
        self.assertTrue(unmapped[0]["review_only"])
        unresolved = report["review_queues"]["unresolved_benchmark_parity_case_reference"]
        self.assertEqual([item["workload_id"] for item in unresolved], ["missing.case"])
        self.assertTrue(unresolved[0]["review_only"])

    def test_unmapped_parity_cases_and_workloads_get_review_rows(self) -> None:
        self._write_inputs(include_copy_workload=False)
        parity = json.loads(self.parity_path.read_text(encoding="utf-8"))
        parity["cases"].append(
            {
                "case_id": "orphan.case",
                "surface": "PIL.Image",
                "operation": "missing",
                "covers": [],
                "target_profiles": ["python-cpu"],
                "steps": [],
            }
        )
        self.parity_path.write_text(json.dumps(parity), encoding="utf-8")
        workloads = json.loads(self.benchmark_path.read_text(encoding="utf-8"))
        workloads["workloads"].append(
            {
                "workload_id": "orphan.workload",
                "covers": [],
                "subjects": [{"kind": "target_profile", "id": "python-cpu"}],
                "input": {
                    "kind": "workflow",
                    "steps": [{"step_id": "unknown", "surface": "PIL.Image", "operation": "missing"}],
                },
                "measurement": {"boundary": "whole_workflow"},
            }
        )
        self.benchmark_path.write_text(json.dumps(workloads), encoding="utf-8")

        report = build_declared_evidence_index(self.root, self.manifest, self.operation_paths)
        parity_review = report["review_queues"]["parity_case_without_public_operation"]
        workload_review = report["review_queues"]["benchmark_workload_without_public_operation"]
        self.assertEqual([item["case_id"] for item in parity_review], ["orphan.case"])
        self.assertEqual([item["workload_id"] for item in workload_review], ["orphan.workload"])
        self.assertTrue(all(item["review_only"] for item in parity_review + workload_review))

    def test_duplicate_ids_are_rejected_even_within_one_input_file(self) -> None:
        self._write_inputs()
        parity = json.loads(self.parity_path.read_text(encoding="utf-8"))
        parity["cases"].append(parity["cases"][0])
        self.parity_path.write_text(json.dumps(parity), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "duplicate parity case ID"):
            build_declared_evidence_index(self.root, self.manifest, self.operation_paths)

        self._write_inputs()
        workloads = json.loads(self.benchmark_path.read_text(encoding="utf-8"))
        workloads["workloads"].append(workloads["workloads"][0])
        self.benchmark_path.write_text(json.dumps(workloads), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "duplicate benchmark workload ID"):
            build_declared_evidence_index(self.root, self.manifest, self.operation_paths)

    def test_indexed_input_bytes_participate_in_crosswalk_source_identity(self) -> None:
        manifest_path = self.fixtures / "manifest.yaml"
        manifest_path.write_text(
            "input_index:\n  parity:\n    - inputs/parity/cases.json\n",
            encoding="utf-8",
        )
        self.parity_path.write_text('{"cases": []}\n', encoding="utf-8")
        indexed_input = "pillow-rs/tests/fixtures/inputs/parity/cases.json"
        first = crosswalk_source_identity(self.root, (indexed_input,))
        self.parity_path.write_text('{"cases": [{"case_id": "changed"}]}\n', encoding="utf-8")
        second = crosswalk_source_identity(self.root, (indexed_input,))
        self.assertEqual(first["crosswalk_input_file_count"], 2)
        self.assertNotEqual(first["crosswalk_input_sha256"], second["crosswalk_input_sha256"])

    def test_rejects_input_index_paths_outside_fixture_owner(self) -> None:
        self.manifest["input_index"]["parity"] = ["../outside.json"]
        with self.assertRaisesRegex(ValueError, "escapes its owner"):
            build_declared_evidence_index(self.root, self.manifest, self.operation_paths)


if __name__ == "__main__":
    unittest.main()
