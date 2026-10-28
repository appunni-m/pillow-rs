#!/usr/bin/env python3
"""Verify deterministic regeneration of active inputs and crash quarantine."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import tempfile
from pathlib import Path

sys.dont_write_bytecode = True

from build_migration_parity_inputs import (
    CRASH_QUARANTINE_RELATIVE,
    DEFAULT_MANIFEST,
    FIXTURE_ROOT,
    build_inputs,
    indexed_generated_asset_paths,
    load_manifest,
    prepare_asset_overlay,
)
from validate_migration_parity_contract import validate_inputs


QUARANTINE_SCHEMA = "migration-parity/crash-quarantine-input@2"
QUARANTINE_CASE_KEYS = {
    "case_id",
    "surface",
    "operation",
    "covers",
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
        for key in ("covers", "assets", "steps", "observations"):
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
    with tempfile.TemporaryDirectory(prefix="migration-parity-inputs-") as directory:
        generated_root = Path(directory)
        generated_assets_root = prepare_asset_overlay(
            generated_root / "assets",
            FIXTURE_ROOT / "assets",
        )
        build_inputs(manifest, generated_root, generated_assets_root)
        generated_quarantine = generated_root / CRASH_QUARANTINE_RELATIVE
        if not quarantine.is_file():
            raise SystemExit(
                f"missing crash quarantine input: {CRASH_QUARANTINE_RELATIVE}"
            )
        if quarantine.read_bytes() != generated_quarantine.read_bytes():
            raise SystemExit(
                f"input drift in crash quarantine: {CRASH_QUARANTINE_RELATIVE} differs from generator"
            )
        validate_crash_quarantine(quarantine)
        if args.quarantine_only:
            validate_crash_quarantine(generated_quarantine)
            print("crash quarantine inputs reproduce exactly (static check only)")
            return
        for lane, lane_paths in manifest["input_index"].items():
            for relative in lane_paths:
                active = active_root / relative
                if not active.is_file():
                    raise SystemExit(f"missing indexed {lane} input: {relative}")
        generated_assets = generated_root / "assets" / "generated"
        if generated_assets.exists():
            for generated in generated_assets.rglob("*"):
                if not generated.is_file():
                    continue
                relative = generated.relative_to(generated_assets)
                active = active_root / "assets" / "generated" / relative
                if not active.is_file():
                    raise SystemExit(
                        f"missing generated parity asset: assets/generated/{relative.as_posix()}"
                    )
        validate_inputs(manifest, active_root)
        referenced_assets = indexed_generated_asset_paths(active_root, manifest)
        active_generated_assets = active_root / "assets" / "generated"
        active_asset_paths = {
            path.relative_to(active_generated_assets).as_posix()
            for path in active_generated_assets.rglob("*")
            if path.is_file()
        }
        if active_asset_paths != referenced_assets:
            raise SystemExit(
                "generated asset cache differs from indexed references: "
                f"unreferenced={sorted(active_asset_paths - referenced_assets)}, "
                f"missing={sorted(referenced_assets - active_asset_paths)}"
            )
        for relative in sorted(referenced_assets):
            asset_path = active_generated_assets / relative
            digest = hashlib.sha256(asset_path.read_bytes()).hexdigest()
            if Path(relative).stem != digest:
                raise SystemExit(
                    f"generated asset is not content-addressed: {relative}"
                )
        for lane, relative_paths in manifest["input_index"].items():
            for relative in relative_paths:
                active = active_root / relative
                generated = generated_root / relative
                if active.read_bytes() != generated.read_bytes():
                    raise SystemExit(
                        f"input drift in {lane}: {relative} differs from generator"
                    )
        if generated_assets.exists():
            generated_paths = sorted(
                path for path in generated_assets.rglob("*") if path.is_file()
            )
            for generated in generated_paths:
                relative = generated.relative_to(generated_assets)
                active = active_root / "assets" / "generated" / relative
                if active.read_bytes() != generated.read_bytes():
                    raise SystemExit(
                        f"asset drift: assets/generated/{relative.as_posix()} differs from generator"
                    )
    print("migration parity inputs and crash quarantine reproduce exactly")


if __name__ == "__main__":
    main()
