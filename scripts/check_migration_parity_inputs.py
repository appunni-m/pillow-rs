#!/usr/bin/env python3
"""Verify deterministic regeneration of active inputs and crash quarantine."""

from __future__ import annotations

import argparse
import json
import tempfile
from pathlib import Path

from build_migration_parity_inputs import (
    CRASH_QUARANTINE_RELATIVE,
    DEFAULT_MANIFEST,
    FIXTURE_ROOT,
    build_inputs,
    load_manifest,
)
from validate_migration_parity_contract import validate_inputs


QUARANTINE_SCHEMA = "migration-parity/crash-quarantine-input@1"
QUARANTINE_CASE_KEYS = {
    "case_id",
    "surface",
    "operation",
    "covers",
    "target_profiles",
    "assets",
    "steps",
    "observations",
}
OUTPUT_KEYS = {"expected_output", "oracle_output", "target_output", "expected_result"}


def _reject_output_fields(value: object, path: str) -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            if key in OUTPUT_KEYS:
                raise SystemExit(f"{path}.{key}: expected outputs are forbidden")
            _reject_output_fields(child, f"{path}.{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _reject_output_fields(child, f"{path}[{index}]")


def validate_crash_quarantine(path: Path) -> None:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise SystemExit(f"invalid crash quarantine JSON: {exc}") from exc
    expected_keys = {"schema", "status", "active", "execution", "reason", "cases"}
    if not isinstance(document, dict):
        raise SystemExit(f"{path}: expected a JSON object")
    if set(document) != expected_keys:
        raise SystemExit(
            f"{path}: expected keys {sorted(expected_keys)}, got {sorted(document)}"
        )
    if document["schema"] != QUARANTINE_SCHEMA:
        raise SystemExit(f"{path}: unexpected quarantine schema")
    if document["status"] != "quarantined" or document["active"] is not False:
        raise SystemExit(f"{path}: quarantine must be explicitly inactive")
    if document["execution"] != "manual":
        raise SystemExit(f"{path}: crash cases must be manual-only")
    if not isinstance(document["reason"], str) or not document["reason"]:
        raise SystemExit(f"{path}: quarantine reason is required")
    cases = document["cases"]
    if not isinstance(cases, list) or not cases:
        raise SystemExit(f"{path}: quarantine must contain at least one case")
    case_ids: set[str] = set()
    for index, case in enumerate(cases):
        case_path = f"{path}.cases[{index}]"
        if not isinstance(case, dict) or set(case) != QUARANTINE_CASE_KEYS:
            actual = sorted(case) if isinstance(case, dict) else type(case).__name__
            raise SystemExit(
                f"{case_path}: expected keys {sorted(QUARANTINE_CASE_KEYS)}, got {actual}"
            )
        case_id = case["case_id"]
        if not isinstance(case_id, str) or not case_id:
            raise SystemExit(f"{case_path}.case_id: expected non-empty string")
        if case_id in case_ids:
            raise SystemExit(f"{case_path}.case_id: duplicate case ID")
        case_ids.add(case_id)
        for key in ("surface", "operation"):
            if not isinstance(case[key], str) or not case[key]:
                raise SystemExit(f"{case_path}.{key}: expected non-empty string")
        for key in ("covers", "target_profiles", "assets", "steps", "observations"):
            if not isinstance(case[key], list) or not case[key]:
                raise SystemExit(f"{case_path}.{key}: expected non-empty array")
    _reject_output_fields(document, str(path))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--quarantine-only",
        action="store_true",
        help="check only the isolated crash corpus without validating active inputs",
    )
    args = parser.parse_args()
    manifest = load_manifest(DEFAULT_MANIFEST)
    active_root = FIXTURE_ROOT
    quarantine = active_root / CRASH_QUARANTINE_RELATIVE
    validate_crash_quarantine(quarantine)
    with tempfile.TemporaryDirectory(prefix="migration-parity-inputs-") as directory:
        generated_root = Path(directory)
        build_inputs(manifest, generated_root, FIXTURE_ROOT / "assets")
        generated_quarantine = generated_root / CRASH_QUARANTINE_RELATIVE
        if quarantine.read_bytes() != generated_quarantine.read_bytes():
            raise SystemExit(
                f"input drift in crash quarantine: {CRASH_QUARANTINE_RELATIVE} differs from generator"
            )
        if args.quarantine_only:
            validate_crash_quarantine(generated_quarantine)
            print("crash quarantine inputs reproduce exactly (static check only)")
            return
        validate_inputs(manifest, active_root)
        for lane, relative_paths in manifest["input_index"].items():
            for relative in relative_paths:
                active = active_root / relative
                generated = generated_root / relative
                if active.read_bytes() != generated.read_bytes():
                    raise SystemExit(
                        f"input drift in {lane}: {relative} differs from generator"
                    )
    print("migration parity inputs and crash quarantine reproduce exactly")


if __name__ == "__main__":
    main()
