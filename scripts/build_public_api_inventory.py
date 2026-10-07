#!/usr/bin/env python3
"""Inventory callable and data endpoints exported by pillow-rs's public PIL package.

This imports the local Python facade and its built native extension, but does
not call image operations. It joins public target paths to the selected Pillow
contract where a source path exists; unjoined paths remain explicit inventory
gaps instead of being inferred from Rust implementation symbols.
"""

from __future__ import annotations

import argparse
import ast
import csv
import inspect
import json
from pathlib import Path
import subprocess
import sys
from types import ModuleType
from typing import Any

import yaml


ROOT = Path(__file__).resolve().parents[1]
PYTHON_SOURCE = ROOT / "pillow-rs-py/python"
DEFAULT_OUTPUT = ROOT / "docs/evidence/performance-optimization-public-api.csv"
EXPECTED_PIL = ROOT / "pillow-rs-py/python/PIL/__init__.py"

FIELDS = (
    "target_revision",
    "target_dirty",
    "target_version",
    "python_source",
    "native_extension",
    "public_path",
    "entry_kind",
    "module_alias",
    "defined_module",
    "defined_file",
    "manifest_status",
    "manifest_operation",
    "manifest_source_path",
    "manifest_kind",
    "manifest_profiles",
    "benchmark_workloads",
    "workload_ids",
)

MAGIC_METHODS = {
    "__bytes__",
    "__call__",
    "__contains__",
    "__eq__",
    "__getitem__",
    "__iter__",
    "__len__",
    "__next__",
    "__repr__",
    "__setitem__",
    "__str__",
}


def git_identity() -> tuple[str, bool]:
    revision = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, check=True, text=True,
        capture_output=True,
    ).stdout.strip()
    dirty = subprocess.run(
        ["git", "diff", "--quiet", "--ignore-submodules", "--"],
        cwd=ROOT, check=False,
    ).returncode != 0
    untracked = subprocess.run(
        ["git", "ls-files", "--others", "--exclude-standard"], cwd=ROOT,
        check=True, text=True, capture_output=True,
    ).stdout.strip()
    return revision, dirty or bool(untracked)


def public_member_kind(value: Any) -> str | None:
    if isinstance(value, property):
        return "property"
    if isinstance(value, (staticmethod, classmethod)):
        return "method"
    if inspect.isclass(value):
        return None
    if callable(value):
        return "method"
    if inspect.isdatadescriptor(value):
        return "property"
    return None


def public_class_members(cls: type[Any]) -> dict[str, str]:
    members: dict[str, str] = {}
    # Include public methods inherited from pillow-rs wrapper bases, such as
    # ImageEnhance's shared enhance() method; omit language-runtime base APIs.
    for base in reversed(cls.__mro__[:-1]):
        if not str(getattr(base, "__module__", "")).startswith(("pillow_rs", "PIL")):
            continue
        for name, value in vars(base).items():
            if name in {"__init__", "__new__"}:
                continue
            if name.startswith("_") and name not in MAGIC_METHODS:
                continue
            kind = public_member_kind(value)
            if kind is not None:
                members[name] = kind
    return members


def instance_fields(cls: type[Any]) -> set[str]:
    """Find public data attributes set by a Python __init__ implementation."""
    source = inspect.getsourcefile(cls)
    if not source:
        return set()
    try:
        tree = ast.parse(Path(source).read_text(encoding="utf-8"))
    except (OSError, SyntaxError):
        return set()
    fields: set[str] = set()
    for class_node in tree.body:
        if not isinstance(class_node, ast.ClassDef) or class_node.name != cls.__name__:
            continue
        for method in class_node.body:
            if not isinstance(method, (ast.FunctionDef, ast.AsyncFunctionDef)) or method.name != "__init__":
                continue
            for node in ast.walk(method):
                targets = []
                if isinstance(node, ast.Assign):
                    targets = node.targets
                elif isinstance(node, ast.AnnAssign):
                    targets = [node.target]
                for target in targets:
                    for child in ast.walk(target):
                        if (
                            isinstance(child, ast.Attribute)
                            and isinstance(child.value, ast.Name)
                            and child.value.id == "self"
                            and not child.attr.startswith("_")
                        ):
                            fields.add(child.attr)
    return fields


def source_location(value: Any) -> tuple[str, str]:
    module_name = str(getattr(value, "__module__", ""))
    try:
        source = inspect.getsourcefile(value) or inspect.getfile(value)
    except (TypeError, OSError):
        source = ""
    return module_name, str(Path(source).resolve()) if source else ""


def selected_contract() -> tuple[dict[str, dict[str, str]], dict[str, list[str]]]:
    manifest_path = ROOT / "pillow-rs/tests/fixtures/manifest.yaml"
    manifest = yaml.safe_load(manifest_path.read_text(encoding="utf-8"))
    by_source: dict[str, dict[str, str]] = {}
    requirement_owner: dict[str, str] = {}
    for surface in manifest["surfaces"]:
        for operation in surface.get("operations", []):
            operation_id = f"{surface['id']}.{operation['id']}"
            source_path = str(operation.get("source", {}).get("path") or operation_id)
            profiles = sorted(
                {
                    str(profile)
                    for requirement in operation.get("requirements", [])
                    if requirement.get("dimension") == "performance"
                    for profile in requirement.get("target_profiles", [])
                }
            )
            by_source[source_path] = {
                "operation": operation_id,
                "kind": str(operation.get("kind", "")),
                "profiles": ";".join(profiles),
            }
            for requirement in operation.get("requirements", []):
                requirement_id = requirement.get("id")
                if isinstance(requirement_id, str):
                    requirement_owner[requirement_id] = operation_id

    specs: dict[str, dict[str, Any]] = {}
    for path in sorted((ROOT / "pillow-rs/tests/fixtures/inputs/benchmark").glob("*.json")):
        document = json.loads(path.read_text(encoding="utf-8"))
        for item in document.get("workloads", []):
            workload_id = item.get("workload_id")
            if isinstance(workload_id, str):
                specs[workload_id] = item
    workloads_by_operation: dict[str, list[str]] = {value["operation"]: [] for value in by_source.values()}
    for workload_id, spec in specs.items():
        for requirement in spec.get("covers", []):
            owner = requirement_owner.get(str(requirement))
            if owner is not None:
                workloads_by_operation.setdefault(owner, []).append(workload_id)
    for operation_id in workloads_by_operation:
        workloads_by_operation[operation_id] = sorted(set(workloads_by_operation[operation_id]))
    return by_source, workloads_by_operation


def manifest_source_for_target(target_path: str, manifest: dict[str, dict[str, str]]) -> str | None:
    """Resolve the public wrapper spelling to its Pillow contract path."""
    if target_path in manifest:
        return target_path
    prefix = "PIL.ImageDraw.Draw"
    if target_path == prefix or target_path.startswith(prefix + "."):
        source_path = "PIL.ImageDraw.ImageDraw" + target_path[len(prefix):]
        if source_path in manifest:
            return source_path
    return None


def target_path_for_manifest(source_path: str) -> str:
    prefix = "PIL.ImageDraw.ImageDraw"
    if source_path == prefix or source_path.startswith(prefix + "."):
        return "PIL.ImageDraw.Draw" + source_path[len(prefix):]
    return source_path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()

    sys.path.insert(0, str(PYTHON_SOURCE))
    import PIL  # noqa: PLC0415 - local facade import must follow sys.path setup
    import pillow_rs  # noqa: PLC0415

    pil_file = Path(PIL.__file__).resolve()
    if pil_file != EXPECTED_PIL.resolve():
        raise RuntimeError(f"expected local PIL facade at {EXPECTED_PIL}, loaded {pil_file}")
    core = pillow_rs._core
    native_extension = str(Path(core.__file__).resolve())
    revision, dirty = git_identity()
    manifest, workloads_by_operation = selected_contract()
    rows: dict[str, dict[str, Any]] = {}

    def add_entry(
        path: str,
        kind: str,
        module_alias: str,
        value: Any,
        *,
        definition_module: str | None = None,
        definition_file: str | None = None,
    ) -> None:
        if path in rows:
            existing = rows[path]
            if existing["entry_kind"] == "class" and kind != "class":
                existing["entry_kind"] = kind
            return
        source_path = manifest_source_for_target(path, manifest)
        source = manifest.get(source_path) if source_path is not None else None
        operation_id = source["operation"] if source else ""
        workloads = workloads_by_operation.get(operation_id, [])
        if definition_module is None or definition_file is None:
            detected_module, detected_file = source_location(value)
            definition_module = definition_module if definition_module is not None else detected_module
            definition_file = definition_file if definition_file is not None else detected_file
        rows[path] = {
            "target_revision": revision,
            "target_dirty": str(dirty).lower(),
            "target_version": str(getattr(PIL, "__version__", "")),
            "python_source": str(pil_file),
            "native_extension": native_extension,
            "public_path": path,
            "entry_kind": kind,
            "module_alias": module_alias,
            "defined_module": definition_module or "",
            "defined_file": definition_file or "",
            "manifest_status": "selected_manifest" if source else "outside_selected_manifest",
            "manifest_operation": operation_id,
            "manifest_source_path": source_path or "",
            "manifest_kind": source["kind"] if source else "",
            "manifest_profiles": source["profiles"] if source else "",
            "benchmark_workloads": str(len(workloads)),
            "workload_ids": ";".join(workloads),
        }

    modules = {
        name: getattr(PIL, name)
        for name in getattr(PIL, "__all__", [])
        if isinstance(getattr(PIL, name, None), ModuleType)
    }
    for alias, module in sorted(modules.items()):
        module_path = f"PIL.{alias}"
        if alias == "Image":
            names = list(getattr(module, "__all__", []))
        else:
            names = [
                name for name, value in vars(module).items()
                if not name.startswith("_")
                and (inspect.isfunction(value) or inspect.isclass(value))
                and str(getattr(value, "__module__", "")).startswith(("pillow_rs", "PIL"))
            ]
        for name in sorted(set(names)):
            value = getattr(module, name, None)
            path = f"{module_path}.{name}"
            if inspect.isfunction(value):
                add_entry(path, "function", alias, value)
            elif inspect.isclass(value):
                add_entry(path, "class", alias, value)
                for member, kind in public_class_members(value).items():
                    member_path = f"{path}.{member}"
                    add_entry(member_path, kind, alias, getattr(value, member, value))
                for field in instance_fields(value):
                    field_path = f"{path}.{field}"
                    add_entry(field_path, "property", alias, value)

    for name in getattr(PIL, "__all__", []):
        value = getattr(PIL, name, None)
        if isinstance(value, ModuleType):
            continue
        path = f"PIL.{name}"
        if inspect.isfunction(value):
            add_entry(path, "function", "PIL", value)
        elif inspect.isclass(value):
            add_entry(path, "class", "PIL", value)
            for member, kind in public_class_members(value).items():
                member_path = f"{path}.{member}"
                add_entry(member_path, kind, "PIL", getattr(value, member, value))

    # Keep contract entries exposed through a differently named target
    # wrapper (ImageDraw.Draw maps to Pillow's ImageDraw.ImageDraw).
    for source_path, source in manifest.items():
        target_path = target_path_for_manifest(source_path)
        if target_path not in rows:
            add_entry(target_path, source["kind"], "PIL", None)

    joined_manifest_sources = {
        row["manifest_source_path"]
        for row in rows.values()
        if row["manifest_status"] == "selected_manifest"
    }
    if joined_manifest_sources != set(manifest):
        missing = sorted(set(manifest) - joined_manifest_sources)
        unexpected = sorted(joined_manifest_sources - set(manifest))
        raise RuntimeError(
            "public API inventory does not join every selected manifest path "
            f"(missing={missing}, unexpected={unexpected})"
        )

    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=FIELDS, extrasaction="raise", lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows[path] for path in sorted(rows))
    print(json.dumps({
        "output": str(args.output),
        "public_entries": len(rows),
        "selected_manifest_entries": sum(row["manifest_status"] == "selected_manifest" for row in rows.values()),
        "outside_selected_manifest_entries": sum(row["manifest_status"] == "outside_selected_manifest" for row in rows.values()),
        "workload_mapped_entries": sum(bool(row["workload_ids"]) for row in rows.values()),
        "target_revision": revision,
        "target_dirty": dirty,
        "native_extension": native_extension,
    }, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
