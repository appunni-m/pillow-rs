"""Regression checks for reviewed migration parity case selection."""

from __future__ import annotations

import hashlib
import json
import unittest
from collections import defaultdict
from pathlib import Path
from typing import Any

import yaml

from scripts.build_migration_parity_inputs import case_signature


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "pillow-rs" / "tests" / "fixtures"


def active_cases() -> list[dict[str, Any]]:
    cases: list[dict[str, Any]] = []
    for path in sorted((FIXTURE_ROOT / "inputs" / "parity").glob("*.json")):
        cases.extend(json.loads(path.read_text())["cases"])
    return cases


def legacy_signature(document: dict[str, Any], case: dict[str, Any]) -> str:
    operation = document["operation"]
    payload = {
        "module": operation.get("module"),
        "target": operation.get("target"),
        "mode": case.get("mode"),
        "input": case.get("input"),
        "params": case.get("params"),
    }
    return hashlib.sha256(
        json.dumps(payload, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


class MigrationParityCaseReviewTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.manifest = yaml.safe_load(
            (FIXTURE_ROOT / "manifest.yaml").read_text()
        )
        cls.cases = active_cases()
        cls.by_id = {case["case_id"]: case for case in cls.cases}
        cls.requirement_to_case = {
            requirement_id: case["case_id"]
            for case in cls.cases
            for requirement_id in case["covers"]
        }

    def test_active_workflows_have_unique_behavior_signatures(self) -> None:
        signatures = [case_signature(case) for case in self.cases]
        self.assertEqual(len(signatures), len(set(signatures)))

    def test_case_ids_are_unique_and_nuanced_cases_are_retained(self) -> None:
        ids = [case["case_id"] for case in self.cases]
        self.assertEqual(len(ids), len(set(ids)))
        nuanced = [case_id for case_id in ids if ".nuanced." in case_id]
        self.assertGreaterEqual(len(nuanced), 10)
        self.assertIn(
            "PIL.ImageFont.FreeTypeFont.getbbox.nuanced.unicode-multiline",
            nuanced,
        )
        self.assertIn(
            "PIL.ImageFilter.Kernel.nuanced.three-by-three-edge",
            nuanced,
        )

    def test_edge_cases_are_not_default_labels(self) -> None:
        signatures = {case["case_id"]: case_signature(case) for case in self.cases}
        for surface in self.manifest["surfaces"]:
            for operation in surface["operations"]:
                prefix = f"{surface['id']}.{operation['id']}"
                default_id = self.requirement_to_case[f"{prefix}.behavior.default"]
                for requirement in operation["requirements"]:
                    if requirement["dimension"] not in {"boundary", "error_path"}:
                        continue
                    case_id = self.requirement_to_case[requirement["id"]]
                    self.assertNotEqual(
                        signatures[case_id],
                        signatures[default_id],
                        requirement["id"],
                    )

    def test_filter_type_cases_apply_the_public_filter_workflow(self) -> None:
        filter_cases = [
            case
            for case in self.cases
            if case["surface"] == "PIL.ImageFilter"
        ]
        self.assertTrue(filter_cases)
        for case in filter_cases:
            self.assertTrue(
                any(
                    step["surface"] == "PIL.Image.Image"
                    and step["operation"] == "filter"
                    for step in case["steps"]
                ),
                case["case_id"],
            )

    def test_coverage_selectors_do_not_repeat_cases(self) -> None:
        for path in sorted((FIXTURE_ROOT / "inputs" / "coverage").glob("*.json")):
            document = json.loads(path.read_text())
            for plan in document["plans"]:
                selected = plan["selectors"]["parity_case_ids"]
                self.assertEqual(len(selected), len(set(selected)), path.name)

    def test_legacy_duplicate_accounting_is_explicit(self) -> None:
        combined: dict[str, list[str]] = defaultdict(list)
        total = 0
        for root in (
            ROOT / "tests/deprecated/fixtures/input/jsons",
            ROOT / "tests/deprecated/fixtures_2/input/jsons",
        ):
            for path in sorted(root.glob("*.json")):
                document = json.loads(path.read_text())
                for case in document["cases"]:
                    total += 1
                    combined[legacy_signature(document, case)].append(case["id"])
        self.assertEqual(total, 1592)
        self.assertEqual(len(combined), 1432)
        self.assertEqual(
            sum(len(ids) - 1 for ids in combined.values() if len(ids) > 1),
            160,
        )

    def test_callable_case_is_explicitly_source_neutral(self) -> None:
        requirement = "PIL.Image.Image.point.parameter-combination.legacy-001"
        case = self.by_id[self.requirement_to_case[requirement]]
        argument = next(
            step["arguments"]["lut"]
            for step in case["steps"]
            if step["step_id"] == "call"
        )
        self.assertEqual(argument, {"kind": "asset", "asset_id": "lut-callable"})
        self.assertEqual(
            next(asset for asset in case["assets"] if asset["id"] == "lut-callable")["name"],
            "identity-callable",
        )


if __name__ == "__main__":
    unittest.main()
