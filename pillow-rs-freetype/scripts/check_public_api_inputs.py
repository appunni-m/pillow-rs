#!/usr/bin/env python3
"""Sanity-check public API input JSON files against tests/manifest.yaml."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "tests" / "manifest.yaml"
INPUT_DIR = ROOT / "tests" / "fixtures" / "inputs" / "public-api"


def filename_for_subject(subject: str) -> str:
    return re.sub(r"[^A-Za-z0-9_.-]", "_", subject) + ".json"


def read_manifest() -> dict[str, set[str]]:
    subjects: dict[str, set[str]] = {}
    current: str | None = None
    for line in MANIFEST.read_text().splitlines():
        if line.startswith("  - id: "):
            current = line.split(": ", 1)[1].strip()
            subjects[current] = set()
        elif line.startswith("      - id: "):
            if current is None:
                raise RuntimeError("case before subject in manifest")
            subjects[current].add(line.split(": ", 1)[1].strip())
    return subjects


def check_file(subject: str, manifest_cases: set[str]) -> list[str]:
    errors: list[str] = []
    path = INPUT_DIR / filename_for_subject(subject)
    if not path.exists():
        return [f"{subject}: missing {path.relative_to(ROOT)}"]

    try:
        data = json.loads(path.read_text())
    except Exception as exc:  # noqa: BLE001 - report parse failure with path.
        return [f"{subject}: invalid json: {exc}"]

    if data.get("version") != 1:
        errors.append(f"{subject}: version must be 1")
    if data.get("subject") != subject:
        errors.append(f"{subject}: top-level subject mismatch")

    cases = data.get("cases")
    if not isinstance(cases, list) or not cases:
        errors.append(f"{subject}: cases must be a non-empty list")
        return errors

    covered: set[str] = set()
    for index, case in enumerate(cases):
        if not isinstance(case, dict):
            errors.append(f"{subject}: cases[{index}] must be object")
            continue
        if case.get("subject") != subject:
            errors.append(f"{subject}: cases[{index}] subject mismatch")
        manifest_case = case.get("case")
        covers_manifest_cases = case.get("covers_manifest_cases", [])
        if not isinstance(covers_manifest_cases, list):
            errors.append(f"{subject}: cases[{index}] covers_manifest_cases must be list")
            covers_manifest_cases = []

        if not isinstance(manifest_case, str):
            errors.append(f"{subject}: cases[{index}] missing case")
        elif manifest_case not in manifest_cases:
            if not covers_manifest_cases:
                errors.append(
                    f"{subject}: cases[{index}] unknown manifest case {manifest_case} "
                    "without covers_manifest_cases"
                )
        else:
            covered.add(manifest_case)
        for covered_case in covers_manifest_cases:
            if not isinstance(covered_case, str) or covered_case not in manifest_cases:
                errors.append(
                    f"{subject}: cases[{index}] invalid covered manifest case {covered_case}"
                )
            else:
                covered.add(covered_case)
        for key in ("case_id", "operation", "schema", "inputs", "expectation"):
            if key not in case:
                errors.append(f"{subject}: cases[{index}] missing {key}")
        expectation = case.get("expectation")
        if isinstance(expectation, dict):
            if "output_shape" not in expectation:
                errors.append(f"{subject}: cases[{index}] expectation missing output_shape")
            if "compare" not in expectation:
                errors.append(f"{subject}: cases[{index}] expectation missing compare")
        else:
            errors.append(f"{subject}: cases[{index}] expectation must be object")

    missing = sorted(manifest_cases - covered)
    if missing:
        errors.append(f"{subject}: missing manifest cases {missing}")
    return errors


def main() -> int:
    subjects = read_manifest()
    selected = sys.argv[1:]
    if selected:
        items = {subject: subjects[subject] for subject in selected}
    else:
        items = subjects

    errors: list[str] = []
    for subject, manifest_cases in items.items():
        errors.extend(check_file(subject, manifest_cases))

    if errors:
        for error in errors:
            print(error)
        return 1
    print(f"checked {len(items)} public API input files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
