"""Negative conformance tests for the fixed manifest/input interfaces."""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

import yaml

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
