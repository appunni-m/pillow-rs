#!/usr/bin/env python3
"""Derive the canonical migration-parity inventory from deprecated authority.

The deprecated project manifest is retained migration evidence, not an active
specification. This command expands every legacy row, merges the two known
duplicate Pillow aliases, adds the public setup endpoints required to express
the retained cases as independent workflows, and emits one deterministic
selected-scope endpoint inventory.

It never imports Pillow or pillow_rs and never reads generated results.
Runtime identity and binding checks belong to separate maintained gates.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, Iterable

import yaml


WORKSPACE_ROOT = Path(__file__).resolve().parents[1]
AUTHORITY_PATH = (
    WORKSPACE_ROOT
    / "deprecated"
    / "migration-parity-v0"
    / "manifest-history"
    / "project_manifest_v0"
    / "manifest.yaml"
)

EXPECTED_AUTHORITY_VERSION = "0.2.0"
EXPECTED_PILLOW_VERSION = "12.2.0"
EXPECTED_AUTHORITY_SHA256 = (
    "082fa73b89ed6275b8d315585f0035ca7b74287a2c24ee22c78b2a057cc5ee8f"
)
EXPECTED_MODULES = (
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
)
EXPECTED_LEGACY_ROWS = 199
EXPECTED_LEGACY_UNIQUE_ENDPOINTS = 197

# ``Image.show`` is retained in the deprecated authority for accounting, but
# it is not part of the active selected-scope contract.  Calling it can open
# an external viewer or block indefinitely in headless parity/coverage runs.
EXCLUDED_ENDPOINTS = {
    "PIL.Image.Image::show": (
        "Headless parity, coverage, and benchmark workflows must not invoke "
        "the external Image.show viewer."
    ),
}


@dataclass(frozen=True, order=True)
class LegacyRef:
    module: str
    section: str
    owner: str | None
    name: str

    @property
    def id(self) -> str:
        owner = f".{self.owner}" if self.owner else ""
        return f"{self.module}{owner}.{self.section}.{self.name}"


@dataclass(frozen=True)
class Endpoint:
    surface: str
    operation: str
    kind: str
    source_path: str
    classification: str
    authority: str
    legacy_refs: tuple[LegacyRef, ...]
    correction_reason: str | None = None

    @property
    def id(self) -> str:
        return f"{self.surface}::{self.operation}"

    def to_json(self) -> dict[str, Any]:
        payload = asdict(self)
        payload["id"] = self.id
        payload["legacy_refs"] = [reference.id for reference in self.legacy_refs]
        return payload


@dataclass(frozen=True)
class EndpointSeed:
    surface: str
    operation: str
    kind: str
    source_path: str
    legacy_ref: LegacyRef


CORRECTIONS: tuple[Endpoint, ...] = (
    Endpoint(
        surface="PIL.ImageDraw",
        operation="Draw",
        kind="function",
        source_path="PIL.ImageDraw.Draw",
        classification="endpoint",
        authority="workflow-correction",
        legacy_refs=(),
        correction_reason=(
            "Public factory required to construct the receiver for every "
            "inventoried PIL.ImageDraw.ImageDraw method."
        ),
    ),
    Endpoint(
        surface="PIL.ImageDraw",
        operation="Outline",
        kind="function",
        source_path="PIL.ImageDraw.Outline",
        classification="endpoint",
        authority="workflow-correction",
        legacy_refs=(),
        correction_reason=(
            "Public factory required to express the retained ImageDraw.shape "
            "workflow without a runner-private pseudo-parameter."
        ),
    ),
    Endpoint(
        surface="PIL.ImagePalette",
        operation="ImagePalette",
        kind="type",
        source_path="PIL.ImagePalette.ImagePalette",
        classification="endpoint",
        authority="workflow-correction",
        legacy_refs=(),
        correction_reason=(
            "Public constructor required to construct the receiver for every "
            "inventoried PIL.ImagePalette.ImagePalette method."
        ),
    ),
    Endpoint(
        surface="PIL.ImageSequence.Iterator",
        operation="__iter__",
        kind="method",
        source_path="PIL.ImageSequence.Iterator.__iter__",
        classification="endpoint",
        authority="workflow-correction",
        legacy_refs=(),
        correction_reason=(
            "The public iterator protocol is required to express real "
            "ImageSequence iteration workflows; the deprecated authority "
            "listed only the constructor."
        ),
    ),
    Endpoint(
        surface="PIL.ImageSequence.Iterator",
        operation="__next__",
        kind="method",
        source_path="PIL.ImageSequence.Iterator.__next__",
        classification="endpoint",
        authority="workflow-correction",
        legacy_refs=(),
        correction_reason=(
            "The public iterator protocol is required to compare first-frame "
            "success and StopIteration; the deprecated authority listed only "
            "the constructor."
        ),
    ),
    Endpoint(
        surface="PIL.ImageFilter.Color3DLUT",
        operation="__repr__",
        kind="method",
        source_path="PIL.ImageFilter.Color3DLUT.__repr__",
        classification="endpoint",
        authority="workflow-correction",
        legacy_refs=(),
        correction_reason=(
            "The Color3DLUT representation is a public method used by the "
            "target wrapper and is required to measure its Rust formatter."
        ),
    ),
    Endpoint(
        surface="PIL.ImageFilter.Color3DLUT",
        operation="generate",
        kind="function",
        source_path="PIL.ImageFilter.Color3DLUT.generate",
        classification="endpoint",
        authority="workflow-correction",
        legacy_refs=(),
        correction_reason=(
            "The public classmethod is required to exercise Rust-owned LUT "
            "callback and table-length validation."
        ),
    ),
    Endpoint(
        surface="PIL.ImageFilter.Color3DLUT",
        operation="transform",
        kind="method",
        source_path="PIL.ImageFilter.Color3DLUT.transform",
        classification="endpoint",
        authority="workflow-correction",
        legacy_refs=(),
        correction_reason=(
            "The public transform method is required to exercise Rust-owned "
            "callback and output-channel validation."
        ),
    ),
    *tuple(
        Endpoint(
            surface=f"PIL.ImageEnhance.{class_name}",
            operation="enhance",
            kind="method",
            source_path=f"PIL.ImageEnhance.{class_name}.enhance",
            classification="endpoint",
            authority="workflow-correction",
            legacy_refs=(),
            correction_reason=(
                "The deprecated class row combined construction with the "
                "independently observable public enhance method."
            ),
        )
        for class_name in ("Brightness", "Color", "Contrast", "Sharpness")
    ),
)


def load_authority(path: Path = AUTHORITY_PATH) -> dict[str, Any]:
    authority_bytes = path.read_bytes()
    observed_sha256 = hashlib.sha256(authority_bytes).hexdigest()
    if path == AUTHORITY_PATH and observed_sha256 != EXPECTED_AUTHORITY_SHA256:
        raise ValueError(
            f"{path}: authority digest drifted: "
            f"expected={EXPECTED_AUTHORITY_SHA256}, "
            f"observed={observed_sha256}"
        )
    manifest = yaml.safe_load(authority_bytes)
    if not isinstance(manifest, dict):
        raise ValueError(f"{path}: authority must be an object")
    if manifest.get("version") != EXPECTED_AUTHORITY_VERSION:
        raise ValueError(
            f"{path}: expected version {EXPECTED_AUTHORITY_VERSION!r}, "
            f"observed {manifest.get('version')!r}"
        )
    if manifest.get("pillow_version") != EXPECTED_PILLOW_VERSION:
        raise ValueError(
            f"{path}: expected Pillow {EXPECTED_PILLOW_VERSION!r}, "
            f"observed {manifest.get('pillow_version')!r}"
        )
    modules = manifest.get("modules")
    if not isinstance(modules, dict):
        raise ValueError(f"{path}: modules must be an object")
    if tuple(modules) != EXPECTED_MODULES:
        raise ValueError(
            f"{path}: module order drifted: expected={EXPECTED_MODULES!r}, "
            f"observed={tuple(modules)!r}"
        )
    return manifest


def require_entry(
    entry: Any,
    *,
    module: str,
    section: str,
    owner: str | None,
) -> tuple[str, dict[str, Any]]:
    if isinstance(entry, str) and owner == "Stat" and section == "properties":
        return entry, {"name": entry}
    if not isinstance(entry, dict) or not isinstance(entry.get("name"), str):
        where = ".".join(filter(None, (module, owner, section)))
        raise ValueError(f"{where}: inventory row must contain a string name")
    return entry["name"], entry


def legacy_seed(
    module: str,
    section: str,
    name: str,
    owner: str | None,
) -> EndpointSeed:
    reference = LegacyRef(module, section, owner, name)

    if module == "Image":
        if section == "class_methods":
            return EndpointSeed(
                "PIL.Image", name, "function", f"PIL.Image.{name}", reference
            )
        kind = "property_get" if section == "properties" else "method"
        return EndpointSeed(
            "PIL.Image.Image",
            name,
            kind,
            f"PIL.Image.Image.{name}",
            reference,
        )
    if module == "ImageModule":
        return EndpointSeed(
            "PIL.Image", name, "function", f"PIL.Image.{name}", reference
        )
    if module == "ImageDraw":
        return EndpointSeed(
            "PIL.ImageDraw.ImageDraw",
            name,
            "method",
            f"PIL.ImageDraw.ImageDraw.{name}",
            reference,
        )
    if module == "ImageFilter":
        return EndpointSeed(
            "PIL.ImageFilter",
            name,
            "type",
            f"PIL.ImageFilter.{name}",
            reference,
        )
    if module == "ImageEnhance":
        return EndpointSeed(
            "PIL.ImageEnhance",
            name,
            "type",
            f"PIL.ImageEnhance.{name}",
            reference,
        )
    if module in {"ImageOps", "ImageChops", "ImageColor"}:
        return EndpointSeed(
            f"PIL.{module}",
            name,
            "function",
            f"PIL.{module}.{name}",
            reference,
        )
    if module == "ImagePalette":
        return EndpointSeed(
            "PIL.ImagePalette.ImagePalette",
            name,
            "method",
            f"PIL.ImagePalette.ImagePalette.{name}",
            reference,
        )
    if module == "ImageFont":
        if owner is not None:
            return EndpointSeed(
                f"PIL.ImageFont.{owner}",
                name,
                "method",
                f"PIL.ImageFont.{owner}.{name}",
                reference,
            )
        kind = {
            "properties": "constant",
            "functions": "function",
            "classes": "type",
        }[section]
        return EndpointSeed(
            "PIL.ImageFont",
            name,
            kind,
            f"PIL.ImageFont.{name}",
            reference,
        )
    if module == "ImageStat":
        if owner is not None:
            return EndpointSeed(
                f"PIL.ImageStat.{owner}",
                name,
                "property_get",
                f"PIL.ImageStat.{owner}.{name}",
                reference,
            )
        return EndpointSeed(
            "PIL.ImageStat",
            name,
            "type",
            f"PIL.ImageStat.{name}",
            reference,
        )
    if module == "ImageSequence":
        return EndpointSeed(
            "PIL.ImageSequence",
            name,
            "type",
            f"PIL.ImageSequence.{name}",
            reference,
        )
    raise ValueError(f"unmapped authority module: {module}")


def iter_legacy_seeds(manifest: dict[str, Any]) -> Iterable[EndpointSeed]:
    modules = manifest["modules"]
    for module, module_data in modules.items():
        if not isinstance(module_data, dict):
            raise ValueError(f"{module}: module entry must be an object")
        for section in ("class_methods", "methods", "properties", "functions"):
            entries = module_data.get(section, [])
            if not isinstance(entries, list):
                raise ValueError(f"{module}.{section}: expected an array")
            for entry in entries:
                name, _ = require_entry(
                    entry, module=module, section=section, owner=None
                )
                yield legacy_seed(module, section, name, None)

        classes = module_data.get("classes", [])
        if not isinstance(classes, list):
            raise ValueError(f"{module}.classes: expected an array")
        for class_entry in classes:
            class_name, class_data = require_entry(
                class_entry, module=module, section="classes", owner=None
            )
            yield legacy_seed(module, "classes", class_name, None)
            for section in (
                "class_methods",
                "methods",
                "properties",
                "functions",
            ):
                entries = class_data.get(section, [])
                if not isinstance(entries, list):
                    raise ValueError(
                        f"{module}.{class_name}.{section}: expected an array"
                    )
                for entry in entries:
                    name, _ = require_entry(
                        entry,
                        module=module,
                        section=section,
                        owner=class_name,
                    )
                    yield legacy_seed(module, section, name, class_name)


def derive_inventory(
    authority_path: Path = AUTHORITY_PATH,
    *,
    include_excluded: bool = False,
) -> tuple[list[Endpoint], int]:
    manifest = load_authority(authority_path)
    seeds = list(iter_legacy_seeds(manifest))
    if len(seeds) != EXPECTED_LEGACY_ROWS:
        raise ValueError(
            "deprecated authority row count drifted: "
            f"expected={EXPECTED_LEGACY_ROWS}, observed={len(seeds)}"
        )

    grouped: dict[tuple[str, str], list[EndpointSeed]] = {}
    for seed in seeds:
        grouped.setdefault((seed.surface, seed.operation), []).append(seed)

    if len(grouped) != EXPECTED_LEGACY_UNIQUE_ENDPOINTS:
        raise ValueError(
            "legacy unique endpoint count drifted: "
            f"expected={EXPECTED_LEGACY_UNIQUE_ENDPOINTS}, "
            f"observed={len(grouped)}"
        )

    duplicate_refs = {
        tuple(reference.legacy_ref.id for reference in group)
        for group in grouped.values()
        if len(group) > 1
    }
    expected_duplicates = {
        (
            "Image.class_methods.open",
            "ImageModule.functions.open",
        ),
        (
            "Image.class_methods.new",
            "ImageModule.functions.new",
        ),
    }
    if duplicate_refs != expected_duplicates:
        raise ValueError(
            "legacy alias set drifted: "
            f"expected={expected_duplicates!r}, observed={duplicate_refs!r}"
        )

    endpoints: list[Endpoint] = []
    for (surface, operation), group in grouped.items():
        first = group[0]
        if any(
            (seed.kind, seed.source_path) != (first.kind, first.source_path)
            for seed in group[1:]
        ):
            raise ValueError(
                f"{surface}::{operation}: merged rows disagree on binding"
            )
        endpoints.append(
            Endpoint(
                surface=surface,
                operation=operation,
                kind=first.kind,
                source_path=first.source_path,
                classification="endpoint",
                authority="deprecated-project-manifest-v0",
                legacy_refs=tuple(seed.legacy_ref for seed in group),
            )
        )

    existing_ids = {endpoint.id for endpoint in endpoints}
    for correction in CORRECTIONS:
        if correction.id in existing_ids:
            raise ValueError(f"correction duplicates legacy endpoint {correction.id}")
        endpoints.append(correction)

    endpoints.sort(key=lambda endpoint: (endpoint.surface, endpoint.operation))
    ids = [endpoint.id for endpoint in endpoints]
    if len(ids) != len(set(ids)):
        raise ValueError("canonical endpoint IDs are not unique")
    if not include_excluded:
        endpoints = [
            endpoint
            for endpoint in endpoints
            if endpoint.id not in EXCLUDED_ENDPOINTS
        ]
    return endpoints, len(seeds)


def render_json(endpoints: list[Endpoint], legacy_rows: int) -> str:
    payload = {
        "schema": "pillow-rs/migration-parity-public-inventory@1",
        "authority": AUTHORITY_PATH.relative_to(WORKSPACE_ROOT).as_posix(),
        "authority_version": EXPECTED_AUTHORITY_VERSION,
        "authority_sha256": EXPECTED_AUTHORITY_SHA256,
        "pillow_version": EXPECTED_PILLOW_VERSION,
        "legacy_rows": legacy_rows,
        "legacy_unique_endpoints": EXPECTED_LEGACY_UNIQUE_ENDPOINTS,
        "correction_endpoints": len(CORRECTIONS),
        "endpoint_count": len(endpoints),
        "endpoints": [endpoint.to_json() for endpoint in endpoints],
    }
    return json.dumps(payload, indent=2, sort_keys=False) + "\n"


def render_ids(endpoints: list[Endpoint]) -> str:
    return "".join(f"{endpoint.id}\n" for endpoint in endpoints)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--format",
        choices=("ids", "json"),
        default="ids",
        help="Output canonical IDs or the complete diagnostic inventory.",
    )
    parser.add_argument(
        "--authority",
        type=Path,
        default=AUTHORITY_PATH,
        help="Deprecated project-wide authority manifest.",
    )
    args = parser.parse_args()

    endpoints, legacy_rows = derive_inventory(args.authority.resolve())
    if args.format == "json":
        print(render_json(endpoints, legacy_rows), end="")
    else:
        print(render_ids(endpoints), end="")


if __name__ == "__main__":
    main()
