#!/usr/bin/env python3
"""Review active parity workflows against deprecated case corpora.

The report is a selection ledger, not an evidence result.  It records exact
workflow deduplication, retained nuanced stimuli, and the old-suite duplicate
counts so a later migration cannot silently turn every legacy row into a new
active case.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

import yaml

from build_migration_parity_inputs import case_signature


WORKSPACE_ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = WORKSPACE_ROOT / "pillow-rs" / "tests" / "fixtures"
LEGACY_ROOTS = (
    WORKSPACE_ROOT
    / "deprecated"
    / "migration-parity-v0"
    / "fixtures"
    / "python"
    / "suite0"
    / "input"
    / "jsons",
    WORKSPACE_ROOT
    / "deprecated"
    / "migration-parity-v0"
    / "fixtures"
    / "python"
    / "suite1"
    / "input"
    / "jsons",
)
DEFAULT_OUTPUT = WORKSPACE_ROOT / "docs" / "migration-parity-case-review.md"


def legacy_signature(document: dict[str, Any], case: dict[str, Any]) -> str:
    operation = document["operation"]
    value = {
        "module": operation.get("module"),
        "target": operation.get("target"),
        "mode": case.get("mode"),
        "input": case.get("input"),
        "params": case.get("params"),
    }
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


def load_active_cases() -> list[dict[str, Any]]:
    cases: list[dict[str, Any]] = []
    for path in sorted((FIXTURE_ROOT / "inputs" / "parity").glob("*.json")):
        document = json.loads(path.read_text(encoding="utf-8"))
        cases.extend(document["cases"])
    return cases


def legacy_counts() -> tuple[list[tuple[str, int, int, int]], int, int]:
    rows: list[tuple[str, int, int, int]] = []
    all_signatures: dict[str, list[str]] = defaultdict(list)
    total = 0
    for root in LEGACY_ROOTS:
        signatures: dict[str, list[str]] = defaultdict(list)
        for path in sorted(root.glob("*.json")):
            document = json.loads(path.read_text(encoding="utf-8"))
            for case in document["cases"]:
                signature = legacy_signature(document, case)
                signatures[signature].append(case["id"])
                all_signatures[signature].append(case["id"])
                total += 1
        cases = sum(len(ids) for ids in signatures.values())
        duplicate_groups = sum(len(ids) > 1 for ids in signatures.values())
        duplicate_rows = sum(len(ids) - 1 for ids in signatures.values())
        rows.append((root.parent.parent.name, cases, len(signatures), duplicate_rows))
    combined_unique = len(all_signatures)
    combined_duplicate_rows = sum(
        len(ids) - 1 for ids in all_signatures.values() if len(ids) > 1
    )
    return rows, combined_unique, combined_duplicate_rows


def build_report() -> str:
    manifest = yaml.safe_load((FIXTURE_ROOT / "manifest.yaml").read_text())
    active_cases = load_active_cases()
    signatures: dict[str, list[str]] = defaultdict(list)
    for case in active_cases:
        signatures[case_signature(case)].append(case["case_id"])
    duplicate_groups = [ids for ids in signatures.values() if len(ids) > 1]
    nuanced = sorted(
        case["case_id"] for case in active_cases if ".nuanced." in case["case_id"]
    )
    cases_by_surface = Counter(case["surface"] for case in active_cases)
    legacy, combined_unique, combined_duplicate_rows = legacy_counts()
    operation_count = sum(
        len(surface["operations"]) for surface in manifest["surfaces"]
    )
    requirement_count = sum(
        len(operation["requirements"])
        for surface in manifest["surfaces"]
        for operation in surface["operations"]
    )

    lines = [
        "# Migration parity case review",
        "",
        "This is a deterministic selection ledger for input definitions. It is",
        "not a parity, coverage, or benchmark result and contains no expected",
        "outputs.",
        "",
        "## Selection outcome",
        "",
        f"- Manifest operations: {operation_count}",
        f"- Manifest requirements: {requirement_count}",
        f"- Active parity workflows: {len(active_cases)}",
        f"- Unique active workflow signatures: {len(signatures)}",
        f"- Active exact-duplicate groups: {len(duplicate_groups)}",
        f"- Deliberate nuanced workflows: {len(nuanced)}",
        "",
        "The generator merges only exact behavior-bearing duplicates. Case IDs",
        "and `covers` membership are labels and therefore do not create a second",
        "execution. Setup order, omitted versus explicit defaults, asset identity,",
        "arguments, and observations remain part of the signature.",
        "",
        "### Active cases by public surface",
        "",
        "| surface | active workflows |",
        "| --- | ---: |",
    ]
    for surface in manifest["surfaces"]:
        surface_id = surface["id"]
        lines.append(f"| `{surface_id}` | {cases_by_surface[surface_id]} |")
    lines.extend(
        [
            "",
            "## Deprecated corpus accounting",
            "",
            "| corpus | rows | unique stimuli | duplicate rows removed |",
            "| --- | ---: | ---: | ---: |",
        ]
    )
    for name, cases, unique, duplicate_rows in legacy:
        lines.append(f"| {name} | {cases} | {unique} | {duplicate_rows} |")
    lines.extend(
        [
            f"| combined | {sum(item[1] for item in legacy)} | {combined_unique} | {combined_duplicate_rows} |",
            "",
            "The old corpora are migration evidence only. Their duplicate rows",
            "are not copied into the active lane by name.",
            "",
            "## Nuanced workflows",
            "",
        ]
    )
    lines.extend(f"- `{case_id}`" for case_id in nuanced)
    lines.extend(
        [
            "",
            "These cases cover high-risk behavior families that a broad default",
            "matrix does not distinguish: Unicode/combining/multiline font text,",
            "anchored drawing, non-integer image geometry, valid color syntax,",
            "fractional centering, and a real three-by-three filter kernel.",
            "",
            "## Review rules",
            "",
            "1. Every active case calls manifest operations through public workflow",
            "   steps; no fixture-only dispatcher IDs are accepted.",
            "2. Exact workflow duplicates are merged while all requirement IDs remain",
            "   in `covers` and coverage/benchmark selectors use the canonical case.",
            "3. Edge/error requirements must change a stimulus or intentionally share",
            "   a public no-op baseline; they may not be labels on the default call.",
            "4. Non-JSON public values (for example `Image.point` callables) remain",
            "   an explicit contract/auditor blocker until the fixed value interface",
            "   defines their source-neutral representation.",
            "5. Additional nuanced cases are allowed to reuse a requirement, but they",
            "   never replace its canonical mapping or add expected output data.",
            "",
        ]
    )
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(build_report(), encoding="utf-8")
    print(args.output)


if __name__ == "__main__":
    main()
