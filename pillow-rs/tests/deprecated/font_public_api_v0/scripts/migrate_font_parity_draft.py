#!/usr/bin/env python3
"""Deprecated rejected Font-only manifest migrator retained as evidence.

The project-wide v0 manifest supplies the complete Pillow surface denominator.
The Font v0 corpus supplies the first executable input-only surface. Both remain
checked in as migration evidence. This command never reads or copies a stored
oracle result.
"""

from __future__ import annotations

import argparse
import json
import shutil
from collections import defaultdict
from pathlib import Path
from typing import Any

import yaml

WORKSPACE_ROOT = Path(__file__).resolve().parents[1]
CRATE_ROOT = WORKSPACE_ROOT / "pillow-rs"
DEPRECATED_ROOT = CRATE_ROOT / "tests" / "deprecated" / "font_public_api_v0"
DEPRECATED_PROJECT_MANIFEST = (
    CRATE_ROOT
    / "tests"
    / "deprecated"
    / "project_manifest_v0"
    / "manifest.yaml"
)
DEFAULT_OUTPUT_ROOT = CRATE_ROOT / "tests" / "fixtures"
SURFACE = "font"

EXPECTED_PROJECT_SURFACES = [
    "Image",
    "ImageModule",
    "ImageDraw",
    "ImageFilter",
    "ImageEnhance",
    "ImageOps",
    "ImageChops",
    "ImageColor",
    "ImagePalette",
    "ImageFont",
    "ImageStat",
    "ImageSequence",
]

TARGET_PATHS = {
    "Image": "pillow_rs::Image",
    "ImageModule": "pillow_rs root module functions",
    "ImageDraw": "pillow_rs::draw",
    "ImageFilter": "pillow_rs::ops::filter",
    "ImageEnhance": "pillow_rs::ops::enhance",
    "ImageOps": "pillow_rs::ops::imageops",
    "ImageChops": "pillow_rs::ops::chops",
    "ImageColor": "pillow_rs::color",
    "ImagePalette": "pillow_rs palette public surface",
    "ImageFont": "pillow_rs::imagefont_*",
    "ImageStat": "pillow_rs::ops::analysis",
    "ImageSequence": "pillow_rs image sequence public surface",
}

LEGACY_KINDS = {
    "class_methods": "class-method",
    "methods": "method",
    "properties": "property",
    "functions": "function",
    "classes": "class",
}

BEHAVIORAL_PUBLIC_NAMES = [
    "FreeTypeFont",
    "ImageFont",
    "Layout",
    "TransposedFont",
    "load",
    "load_default",
    "load_default_imagefont",
    "load_path",
    "truetype",
]

NON_ENDPOINT_PUBLIC_NAMES = [
    "Any",
    "Axis",
    "BinaryIO",
    "BytesIO",
    "DeferredError",
    "IO",
    "Image",
    "IntEnum",
    "MAX_STRING_LENGTH",
    "ModuleType",
    "StrOrBytesPath",
    "TYPE_CHECKING",
    "TypedDict",
    "annotations",
    "base64",
    "cast",
    "core",
    "is_path",
    "os",
    "sys",
    "warnings",
]


def normalize_operation(operation: str) -> str:
    return operation.removeprefix("font.")


def standard_case_id(case_id: str, operation: str) -> str:
    expected_prefix = f"{SURFACE}.{operation}."
    if case_id.startswith(expected_prefix):
        return case_id

    parts = case_id.split(".")
    if len(parts) < 3 or parts[0] != SURFACE:
        raise ValueError(f"cannot migrate nonstandard Font case id: {case_id}")
    path = parts[2:]
    if len(path) > 1 and path[0] == operation:
        path = path[1:]
    if not path:
        raise ValueError(f"case id has no independent path: {case_id}")
    return f"{expected_prefix}{'.'.join(path)}"


def migrate_asset(case_id: str, descriptor: dict[str, Any]) -> dict[str, Any]:
    kind = descriptor.get("kind")
    if kind in {"load_default", "pilfont_default"}:
        return {"kind": "builtin", "name": kind}
    if kind not in {"ref", "pilfont_ref"}:
        raise ValueError(f"{case_id}: unsupported v0 asset kind {kind!r}")

    legacy_id = descriptor.get("id")
    if not isinstance(legacy_id, str):
        raise ValueError(f"{case_id}: v0 reference asset must have a string id")
    legacy_path = Path(legacy_id)
    if legacy_path.is_absolute() or ".." in legacy_path.parts:
        raise ValueError(f"{case_id}: unsafe v0 asset id {legacy_id!r}")
    try:
        relative = legacy_path.relative_to("input")
    except ValueError as error:
        raise ValueError(
            f"{case_id}: v0 asset id must be beneath input/: {legacy_id!r}"
        ) from error

    source = DEPRECATED_ROOT / legacy_path
    migrated: dict[str, Any] = {
        "kind": "ref" if source.is_file() else "missing_ref",
        "path": (Path(SURFACE) / relative).as_posix(),
    }
    if kind == "pilfont_ref":
        migrated["format"] = "pilfont"
    return migrated


def migrate_case(case: dict[str, Any]) -> dict[str, Any]:
    operation = normalize_operation(case["operation"])
    case_id = standard_case_id(case["case_id"], operation)
    legacy_inputs = case["inputs"]
    assets = {
        name: migrate_asset(case_id, descriptor)
        for name, descriptor in sorted(legacy_inputs["assets"].items())
    }
    inputs: dict[str, Any] = {
        "assets": assets,
        "params": legacy_inputs["params"],
    }
    if "environment" in legacy_inputs:
        inputs["environment"] = legacy_inputs["environment"]
    return {
        "case_id": case_id,
        "operation": operation,
        "inputs": inputs,
    }


def canonical_parameter_value(value: Any) -> str:
    if value == "<default>":
        return value
    return json.dumps(value, separators=(",", ":"), sort_keys=True)


def required_parameter_values(
    operation: str,
    cases: list[dict[str, Any]],
    covered_parameters: dict[str, list[str]],
) -> dict[str, list[str]]:
    values: dict[str, list[str]] = {}
    for parameter in covered_parameters.get(operation, []):
        observed = {
            canonical_parameter_value(
                case["inputs"]["params"].get(parameter, "<default>")
            )
            for case in cases
        }
        values[parameter] = sorted(observed)
    return values


def operation_kind(operation: str) -> str:
    if "." in operation:
        return "method"
    if operation in {
        "load",
        "load_default",
        "load_default_imagefont",
        "load_path",
        "truetype",
    }:
        return "function"
    return "public-helper"


def legacy_rows(module: str, data: dict[str, Any]) -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    seen: set[str] = set()
    for section, entries in data.items():
        if section not in LEGACY_KINDS:
            raise ValueError(f"{module}: unknown project-manifest section {section!r}")
        if not isinstance(entries, list):
            raise ValueError(f"{module}.{section}: expected a list")
        for entry in entries:
            if not isinstance(entry, dict) or not isinstance(entry.get("name"), str):
                raise ValueError(f"{module}.{section}: public row must have a name")
            name = entry["name"]
            if name in seen:
                raise ValueError(f"{module}: duplicate public name {name!r}")
            seen.add(name)
            legacy_status = entry.get("status", "unclassified")
            if legacy_status not in {"implemented", "ignored", "unclassified"}:
                raise ValueError(
                    f"{module}.{name}: unknown legacy status {legacy_status!r}"
                )
            rows.append(
                {
                    "name": name,
                    "kind": LEGACY_KINDS[section],
                    "legacy_status": legacy_status,
                }
            )
    if not rows:
        raise ValueError(f"{module}: project-manifest surface is empty")
    return rows


def load_project_inventory() -> tuple[dict[str, Any], dict[str, list[dict[str, str]]]]:
    manifest = yaml.safe_load(
        DEPRECATED_PROJECT_MANIFEST.read_text(encoding="utf-8")
    )
    if manifest.get("version") != "0.2.0":
        raise ValueError("deprecated project manifest must use version 0.2.0")
    if manifest.get("pillow_version") != "12.2.0":
        raise ValueError("deprecated project manifest must target Pillow 12.2.0")
    modules = manifest.get("modules")
    if not isinstance(modules, dict):
        raise ValueError("deprecated project manifest must contain modules")
    if list(modules) != EXPECTED_PROJECT_SURFACES:
        raise ValueError(
            "deprecated project-manifest surface order drifted: "
            f"expected={EXPECTED_PROJECT_SURFACES}, observed={list(modules)}"
        )
    return manifest, {
        module: legacy_rows(module, data) for module, data in modules.items()
    }


def load_v0() -> tuple[dict[str, Any], dict[str, list[dict[str, Any]]]]:
    manifest_path = DEPRECATED_ROOT / "font_manifest.yaml"
    manifest = yaml.safe_load(manifest_path.read_text(encoding="utf-8"))
    input_root = DEPRECATED_ROOT / manifest["input_dir"]
    grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    seen_ids: set[str] = set()

    for filename in manifest["input_files"]:
        document = json.loads((input_root / filename).read_text(encoding="utf-8"))
        for legacy_case in document["cases"]:
            case = migrate_case(legacy_case)
            case_id = case["case_id"]
            if case_id in seen_ids:
                raise ValueError(f"migrated case id is not unique: {case_id}")
            seen_ids.add(case_id)
            grouped[case["operation"]].append(case)

    expected_operations = {
        normalize_operation(operation)
        for operation in manifest["required_operations"]
    } | {
        normalize_operation(operation)
        for operation in manifest["negative_operations"]
    }
    if set(grouped) != expected_operations:
        missing = sorted(expected_operations - set(grouped))
        extra = sorted(set(grouped) - expected_operations)
        raise ValueError(
            f"v0 operation accounting drifted: missing={missing}, extra={extra}"
        )
    return manifest, dict(grouped)


def pending_surface(module: str, rows: list[dict[str, str]]) -> dict[str, Any]:
    reason = (
        "Input-only cases, live Pillow execution, and the public target adapter "
        "have not yet been migrated from the deprecated project inventory."
    )
    blocker = "docs/project-parity-test-process-standard.md#migration-process"
    public_names = [row["name"] for row in rows]
    return {
        "id": module,
        "source_path": f"PIL.{module}",
        "target_path": TARGET_PATHS[module],
        "input_root": None,
        "asset_root": "assets",
        "status": "pending",
        "reason": reason,
        "blocker": blocker,
        "exclusions": [],
        "inventory": {
            "source": "deprecated-project-manifest-v0",
            "public_names": public_names,
        },
        "public_names": {
            "active": [],
            "pending": public_names,
            "non_endpoint": [],
        },
        "operations": [
            {
                "id": row["name"],
                "kind": row["kind"],
                "status": "pending",
                "legacy_status": row["legacy_status"],
                "input": None,
                "output_shape": None,
                "required_parameter_values": {},
                "branches": [],
                "coverage_regions": [],
                "case_count": 0,
                "reason": reason,
                "blocker": blocker,
            }
            for row in rows
        ],
    }


def write_active_corpus(output_root: Path) -> None:
    project_manifest, project_rows = load_project_inventory()
    deprecated_manifest, grouped = load_v0()
    inputs_root = output_root / "inputs" / SURFACE
    assets_root = output_root / "assets" / SURFACE

    if inputs_root.exists():
        shutil.rmtree(inputs_root)
    if assets_root.exists():
        shutil.rmtree(assets_root)
    inputs_root.mkdir(parents=True)
    shutil.copytree(DEPRECATED_ROOT / "input", assets_root)

    covered_parameters = {
        normalize_operation(operation): coverage.get("covered", [])
        for operation, coverage in deprecated_manifest.get(
            "public_method_parameters", {}
        ).items()
    }
    unsupported = {
        normalize_operation(operation)
        for operation in deprecated_manifest["negative_operations"]
    }
    operations = []

    for operation, cases in sorted(grouped.items()):
        filename = f"{SURFACE}.{operation}.json"
        document = {
            "version": 1,
            "surface": SURFACE,
            "operation": operation,
            "cases": cases,
        }
        (inputs_root / filename).write_text(
            json.dumps(document, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        operations.append(
            {
                "id": operation,
                "kind": operation_kind(operation),
                "status": "unsupported" if operation in unsupported else "active",
                "legacy_status": None,
                "input": filename,
                "output_shape": "object",
                "required_parameter_values": required_parameter_values(
                    operation, cases, covered_parameters
                ),
                "branches": ["success", "public-error"],
                "coverage_regions": ["pillow_rs::imagefont_* public root API"],
                "case_count": len(cases),
                "reason": None,
                "blocker": None,
            }
        )

    font_inventory_names = [row["name"] for row in project_rows["ImageFont"]]
    font_classified_names = set(BEHAVIORAL_PUBLIC_NAMES) | set(
        NON_ENDPOINT_PUBLIC_NAMES
    )
    missing_font_names = sorted(set(font_inventory_names) - font_classified_names)
    if missing_font_names:
        raise ValueError(
            "active Font classification is missing deprecated project names: "
            f"{missing_font_names}"
        )

    font_surface = {
        "id": SURFACE,
        "source_path": "PIL.ImageFont",
        "target_path": TARGET_PATHS["ImageFont"],
        "input_root": f"inputs/{SURFACE}",
        "asset_root": "assets",
        "status": "active",
        "reason": None,
        "blocker": None,
        "exclusions": deprecated_manifest["out_of_scope"],
        "inventory": {
            "source": "deprecated-project-manifest-v0",
            "public_names": font_inventory_names,
        },
        "public_names": {
            "active": BEHAVIORAL_PUBLIC_NAMES,
            "pending": [],
            "non_endpoint": NON_ENDPOINT_PUBLIC_NAMES,
        },
        "operations": operations,
    }
    surfaces = [
        font_surface
        if module == "ImageFont"
        else pending_surface(module, project_rows[module])
        for module in EXPECTED_PROJECT_SURFACES
    ]
    inventory_public_name_count = sum(len(rows) for rows in project_rows.values())
    pending_operation_count = sum(
        len(rows)
        for module, rows in project_rows.items()
        if module != "ImageFont"
    )

    manifest = {
        "version": 1,
        "source": {
            "name": "Pillow",
            "version": "12.2.0",
            "runtime": ".oracle-venv/bin/python",
            "contract": "PIL.ImageFont public behavior",
            "identity": {
                "module": "PIL.ImageFont",
                "native_core": "PIL._imagingft",
                "freetype_version": "2.14.3",
            },
        },
        "target": {
            "name": "pillow-rs",
            "version": "current-checkout",
            "runtime": "Rust integration test calling pillow_rs root public API",
            "contract": "Result-style public Font behavior",
        },
        "policy": {
            "input_only": True,
            "live_oracle": True,
            "result_comparison": True,
            "coverage_required_for_claims": True,
        },
        "migration": {
            "source": "tests/deprecated/font_public_api_v0",
            "source_status": "deprecated",
            "case_count": sum(len(cases) for cases in grouped.values()),
        },
        "accounting": {
            "inventory_source": (
                "tests/deprecated/project_manifest_v0/manifest.yaml"
            ),
            "inventory_version": project_manifest["version"],
            "surface_total": len(project_rows),
            "surface_accounted": len(surfaces),
            "surface_accounting_percent": 100,
            "public_name_total": inventory_public_name_count,
            "public_name_accounted": inventory_public_name_count,
            "public_name_accounting_percent": 100,
            "active_surface_count": 1,
            "pending_surface_count": len(surfaces) - 1,
            "active_operation_count": sum(
                operation["status"] == "active" for operation in operations
            ),
            "unsupported_operation_count": sum(
                operation["status"] == "unsupported" for operation in operations
            ),
            "pending_operation_count": pending_operation_count,
        },
        "surfaces": surfaces,
        "evidence": {
            "parity_command": "make migration-parity-test",
            "coverage_command": "make coverage-font-rust-with-freetype",
            "coverage_artifact": "coverage/font-rust-with-freetype",
        },
    }
    (output_root / "manifest.yaml").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--output-root",
        type=Path,
        default=DEFAULT_OUTPUT_ROOT,
        help="Canonical tests/fixtures root or a temporary verification root",
    )
    args = parser.parse_args()
    write_active_corpus(args.output_root.resolve())


if __name__ == "__main__":
    main()
