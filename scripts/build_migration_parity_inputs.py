#!/usr/bin/env python3
"""Build fixed parity, coverage, and benchmark input documents.

Every manifest requirement receives a deterministic input mapping. Parity
cases contain public workflows only and never contain expected source or target
outputs. Coverage plans select those cases, and benchmark workloads reuse
correctness-gated parity cases.

This is the initial project-wide definition generator. Runtime execution may
classify semantic defects in individual stimuli, but static completeness never
hides an endpoint or requirement.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import random
import re
import struct
import zlib
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import yaml

try:
    from validate_migration_parity_contract import validate_manifest as validate_fixed_manifest
except ModuleNotFoundError:  # imported as ``scripts.build_migration_parity_inputs`` in tests
    from scripts.validate_migration_parity_contract import validate_manifest as validate_fixed_manifest


WORKSPACE_ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = WORKSPACE_ROOT / "pillow-rs" / "tests" / "fixtures"
DEFAULT_MANIFEST = FIXTURE_ROOT / "manifest.yaml"
DEFAULT_OUTPUT_ROOT = FIXTURE_ROOT
TARGET_PROFILE = "python-cpu"
CRASH_QUARANTINE_RELATIVE = (
    "inputs/quarantine/pil-imagefont-freetypefont.json"
)
CRASH_QUARANTINE_REASON = (
    "Pillow 12.2.0 source execution exits with SIGSEGV (-11) while "
    "collecting this malformed variable-font name result; retain the "
    "input for isolated crash analysis, but do not execute it in active "
    "parity or coverage lanes."
)
CRASH_QUARANTINE_SPECS: tuple[dict[str, Any], ...] = (
    {
        "surface": "PIL.ImageFont.FreeTypeFont",
        "operation": "get_variation_names",
        "requirement_suffix": "behavior.default",
        "name": "missing-subfamily-name",
        "font": "font/fonts/variable-name-missing-subfamily.ttf",
    },
)


def slug(value: str) -> str:
    return re.sub(r"[^A-Za-z0-9]+", "-", value).strip("-").lower()


def literal(value: Any) -> dict[str, Any]:
    return {"kind": "literal", "value": value}


def bytes_literal(value: list[int]) -> dict[str, Any]:
    return {"kind": "bytes", "value": value}


def text_repeat_literal(text: str, repeat: int) -> dict[str, Any]:
    """Keep large text boundary inputs compact and source-neutral."""

    if repeat < 1:
        raise ValueError("text repeat must be positive")
    return literal({"protocol": "text-repeat", "text": text, "repeat": repeat})


def outline_literal(*, curve: bool = False, empty: bool = False) -> dict[str, Any]:
    """Build a real public ``ImageDraw.Outline`` input for shape parity."""

    if empty:
        commands: list[dict[str, Any]] = []
    elif curve:
        commands = [
            {"name": "move", "args": [1, 1]},
            {"name": "curve", "args": [4, 8, 8, 8, 12, 1]},
            {"name": "line", "args": [12, 10]},
            {"name": "close", "args": []},
        ]
    else:
        commands = [
            {"name": "move", "args": [2, 2]},
            {"name": "line", "args": [12, 2]},
            {"name": "line", "args": [12, 10]},
            {"name": "line", "args": [2, 10]},
            {"name": "close", "args": []},
        ]
    return literal({"protocol": "outline", "commands": commands})


def indexed_png_with_palette_alpha() -> bytes:
    """Return a tiny indexed PNG with a multi-entry tRNS table."""

    def chunk(kind: bytes, payload: bytes) -> bytes:
        checksum = zlib.crc32(kind + payload) & 0xFFFFFFFF
        return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", checksum)

    header = struct.pack(">IIBBBBB", 2, 1, 8, 3, 0, 0, 0)
    palette = bytes([10, 20, 30, 40, 50, 60])
    transparency = bytes([0, 128])
    scanline = bytes([0, 0, 1])
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"PLTE", palette)
        + chunk(b"tRNS", transparency)
        + chunk(b"IDAT", zlib.compress(scanline))
        + chunk(b"IEND", b"")
    )


def indexed_png_with_duplicate_transparent_indices() -> bytes:
    """Return an indexed PNG whose alpha table has two zero entries."""

    def chunk(kind: bytes, payload: bytes) -> bytes:
        checksum = zlib.crc32(kind + payload) & 0xFFFFFFFF
        return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", checksum)

    header = struct.pack(">IIBBBBB", 2, 1, 8, 3, 0, 0, 0)
    palette = bytes([10, 20, 30, 40, 50, 60])
    transparency = bytes([0, 0])
    scanline = bytes([0, 0, 1])
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"PLTE", palette)
        + chunk(b"tRNS", transparency)
        + chunk(b"IDAT", zlib.compress(scanline))
        + chunk(b"IEND", b"")
    )


def indexed_png_with_full_palette_index_alpha() -> bytes:
    """Return a full indexed palette with one transparent table entry."""

    def chunk(kind: bytes, payload: bytes) -> bytes:
        checksum = zlib.crc32(kind + payload) & 0xFFFFFFFF
        return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", checksum)

    header = struct.pack(">IIBBBBB", 1, 1, 8, 3, 0, 0, 0)
    palette = bytes(component for index in range(256) for component in (index,) * 3)
    scanline = bytes([0, 0])
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"PLTE", palette)
        + chunk(b"tRNS", b"\x00")
        + chunk(b"IDAT", zlib.compress(scanline))
        + chunk(b"IEND", b"")
    )


def png_header_without_image_data() -> bytes:
    """Return a decodable PNG header whose deferred pixel load fails."""

    def chunk(kind: bytes, payload: bytes) -> bytes:
        checksum = zlib.crc32(kind + payload) & 0xFFFFFFFF
        return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", checksum)

    header = struct.pack(">IIBBBBB", 2, 1, 8, 2, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IEND", b"")


def little_endian_l16_tiff() -> bytes:
    """Return a minimal valid unsigned-16-bit grayscale TIFF stimulus."""

    entries = [
        (256, 4, 1, struct.pack("<I", 2)),
        (257, 4, 1, struct.pack("<I", 2)),
        (258, 3, 1, struct.pack("<H", 16) + b"\x00\x00"),
        (259, 3, 1, struct.pack("<H", 1) + b"\x00\x00"),
        (262, 3, 1, struct.pack("<H", 1) + b"\x00\x00"),
        (273, 4, 1, struct.pack("<I", 134)),
        (277, 3, 1, struct.pack("<H", 1) + b"\x00\x00"),
        (278, 4, 1, struct.pack("<I", 2)),
        (279, 4, 1, struct.pack("<I", 8)),
        (339, 3, 1, struct.pack("<H", 1) + b"\x00\x00"),
    ]
    data = bytearray(b"II*\x00" + struct.pack("<I", 8))
    data += struct.pack("<H", len(entries))
    for tag, kind, count, value in entries:
        data += struct.pack("<HHI", tag, kind, count) + value
    data += struct.pack("<I", 0)
    data += struct.pack("<4H", 0, 32768, 65535, 16384)
    return bytes(data)


def jpeg_with_exif_variant(base: bytes, variant: str) -> bytes:
    """Add one deterministic EXIF APP1 variant to a valid JPEG stimulus.

    These remain encoded-image workflows: the source and target both open the
    same JPEG bytes through the public ``Image.open`` endpoint.  The variants
    exercise the public EXIF paths without calling the Rust parser directly.
    """

    if len(base) < 2 or base[:2] != b"\xff\xd8":
        raise ValueError("EXIF variant base must be a JPEG")

    if variant == "no-eoi":
        # Keep the JPEG frame header intact, then stop immediately after the
        # SOS header. Image.open() accepts this header-only stream for
        # getexif(), exercising the scanner's natural end-of-input path.
        sos = base.find(b"\xff\xda")
        if sos < 0 or sos + 4 > len(base):
            raise ValueError("EXIF variant base must contain a JPEG SOS")
        scan_start = sos + 2 + struct.unpack(">H", base[sos + 2 : sos + 4])[0]
        if scan_start > len(base):
            raise ValueError("JPEG SOS extends beyond EXIF variant base")
        return base[:scan_start]

    if variant == "empty-app1":
        segment = b"\xff\xe1\x00\x02"
        return base[:2] + segment + base[2:]
    if variant == "short-app1-length":
        segment = b"\xff\xe1\x00\x01"
        return base[:2] + segment + base[2:]

    if variant in {
        "le-orientation2",
        "le-orientation4",
        "le-orientation5",
        "le-orientation7",
        "le-orientation8",
        "standalone-soi",
        "standalone-rst0",
        "eoi-before-app1",
    }:
        orientation = {
            "le-orientation2": 2,
            "le-orientation4": 4,
            "le-orientation5": 5,
            "le-orientation7": 7,
            "le-orientation8": 8,
        }.get(variant, 2)
        tiff = (
            b"II\x2a\x00"
            + struct.pack("<I", 8)
            + struct.pack("<H", 1)
            + struct.pack("<HHI", 0x0112, 3, 1)
            + struct.pack("<H", orientation)
            + b"\x00\x00"
        )
    elif variant == "be-orientation3":
        tiff = (
            b"MM\x00\x2a"
            + struct.pack(">I", 8)
            + struct.pack(">H", 1)
            + struct.pack(">HHI", 0x0112, 3, 1)
            + struct.pack(">H", 3)
            + b"\x00\x00"
        )
    elif variant == "be-non-orientation-before-orientation":
        entries = [
            (0x0100, 3, 1, struct.pack(">H", 2) + b"\x00\x00"),
            (0x0112, 3, 1, struct.pack(">H", 3) + b"\x00\x00"),
        ]
        tiff = bytearray(b"MM\x00\x2a" + struct.pack(">I", 8))
        tiff += struct.pack(">H", len(entries))
        for tag, kind, count, value in entries:
            tiff += struct.pack(">HHI", tag, kind, count) + value
        tiff += struct.pack(">I", 0)
        tiff = bytes(tiff)
    elif variant == "no-orientation":
        tiff = b"II\x2a\x00" + struct.pack("<I", 8) + struct.pack("<H", 0)
    elif variant == "invalid-magic":
        tiff = b"II\x2b\x00" + struct.pack("<I", 8) + struct.pack("<H", 0)
    elif variant == "invalid-byte-order":
        tiff = b"ZZ\x2a\x00" + struct.pack("<I", 8) + struct.pack("<H", 0)
    elif variant == "invalid-offset":
        # A retained Exif payload with a valid TIFF header but an IFD0 offset
        # beyond the payload. Pillow accepts the JPEG and treats orientation
        # as absent; this reaches the public ImageOps.exif_transpose parser.
        tiff = b"II\x2a\x00" + struct.pack("<I", 0x1000) + struct.pack("<H", 0)
    elif variant == "truncated-entry":
        # The IFD advertises one entry but the payload ends before its 12-byte
        # record is available. Keep the container valid so this remains an
        # encoded-image oracle input rather than a direct parser probe.
        tiff = b"II\x2a\x00" + struct.pack("<I", 8) + struct.pack("<H", 1)
    elif variant == "non-orientation-before-orientation":
        entries = [
            (0x0100, 3, 1, struct.pack("<H", 2) + b"\x00\x00"),
            (0x0112, 3, 1, struct.pack("<H", 6) + b"\x00\x00"),
        ]
        tiff = bytearray(b"II\x2a\x00" + struct.pack("<I", 8))
        tiff += struct.pack("<H", len(entries))
        for tag, kind, count, value in entries:
            tiff += struct.pack("<HHI", tag, kind, count) + value
        tiff += struct.pack("<I", 0)
        tiff = bytes(tiff)
    elif variant == "invalid-orientation":
        tiff = (
            b"II\x2a\x00"
            + struct.pack("<I", 8)
            + struct.pack("<H", 1)
            + struct.pack("<HHI", 0x0112, 3, 1)
            + struct.pack("<H", 9)
            + b"\x00\x00"
        )
    elif variant == "short-exif-payload":
        # Keep the JPEG and APP1 framing valid while exposing the public
        # extractor's shortest retained Exif payload to the Rust parser.
        tiff = b"X"
    elif variant == "short-tiff":
        tiff = b"II\x2a\x00"
    elif variant == "no-exif-prefix":
        tiff = b"XXXX\x00\x00\x00\x00"
    else:
        raise ValueError(f"unknown EXIF variant: {variant}")

    payload = tiff if variant == "no-exif-prefix" else b"Exif\x00\x00" + tiff
    segment = b"\xff\xe1" + struct.pack(">H", len(payload) + 2) + payload
    if variant == "standalone-soi":
        return base[:2] + b"\xff\xd8" + segment + base[2:]
    if variant == "standalone-rst0":
        return base[:2] + b"\xff\xd0" + segment + base[2:]
    if variant == "eoi-before-app1":
        # Keep the JPEG frame header intact, then make EOI the first byte after
        # SOS. Image.open() only inspects the header for getexif(), so this
        # exercises the scanner's post-SOS EOI stop without materializing the
        # entropy payload or allowing a later APP1 to be considered.
        sos = base.find(b"\xff\xda")
        if sos < 0 or sos + 4 > len(base):
            raise ValueError("EXIF variant base must contain a JPEG SOS")
        scan_start = sos + 2 + struct.unpack(">H", base[sos + 2 : sos + 4])[0]
        if scan_start > len(base):
            raise ValueError("JPEG SOS extends beyond EXIF variant base")
        return base[:scan_start] + b"\xff\xd9" + segment
    return base[:2] + segment + base[2:]


def binding(step_id: str) -> dict[str, str]:
    return {"kind": "binding", "step_id": step_id}


def bindings(step_ids: list[str]) -> dict[str, Any]:
    """Bind a public sequence argument to earlier workflow results."""

    return {"kind": "bindings", "step_ids": step_ids}


def asset_value(asset_id: str) -> dict[str, str]:
    return {"kind": "asset", "asset_id": asset_id}


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def operation_key(surface: str, operation: str) -> tuple[str, str]:
    return surface, operation


def operation_prefix(surface: str, operation: str) -> str:
    return f"{surface}.{operation}"


def case_signature(case: dict[str, Any]) -> str:
    """Return the behavior-bearing identity of a parity workflow.

    Case IDs and requirement membership are labels, not stimuli.  They must
    not prevent exact duplicate workflows from being merged.  Every other
    field remains part of the signature so that omission/default semantics,
    asset identity, setup order, and observations stay distinct.
    """

    return json.dumps(
        {
            key: case[key]
            for key in (
                "surface",
                "operation",
                "target_profiles",
                "assets",
                "steps",
                "observations",
            )
        },
        sort_keys=True,
        separators=(",", ":"),
    )


def requirement_priority(requirement_id: str, prefix: str) -> tuple[int, int, str]:
    """Choose a human-readable stable label for a merged workflow."""

    suffix = requirement_id.removeprefix(prefix + ".")
    if suffix == "behavior.default":
        rank = 0
    elif suffix.startswith("parameter-combination."):
        rank = 1
    elif suffix.startswith("edge."):
        rank = 2
    elif suffix.startswith("mode.") or suffix.startswith("format."):
        rank = 3
    elif suffix.startswith("parameter."):
        rank = 4
    elif suffix.startswith("performance."):
        rank = 6
    else:
        rank = 5
    return rank, len(suffix), suffix


def merge_duplicate_cases(
    candidates: list[dict[str, Any]],
) -> tuple[list[dict[str, Any]], dict[str, str], int]:
    """Merge exact workflow duplicates while retaining every requirement map."""

    grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for candidate in candidates:
        grouped[case_signature(candidate)].append(candidate)

    merged: list[dict[str, Any]] = []
    requirement_to_case: dict[str, str] = {}
    for group in grouped.values():
        first = group[0]
        prefix = operation_prefix(first["surface"], first["operation"])
        requirement_ids = [
            requirement_id
            for candidate in group
            for requirement_id in candidate["covers"]
        ]
        unique_requirement_ids = list(dict.fromkeys(requirement_ids))
        canonical_requirement = min(
            unique_requirement_ids,
            key=lambda item: requirement_priority(item, prefix),
        )
        canonical_case = {
            **first,
            "case_id": canonical_requirement,
            "covers": unique_requirement_ids,
        }
        merged.append(canonical_case)
        for requirement_id in unique_requirement_ids:
            requirement_to_case[requirement_id] = canonical_case["case_id"]

    return merged, requirement_to_case, len(candidates) - len(merged)


def load_manifest(path: Path) -> dict[str, Any]:
    manifest = yaml.safe_load(path.read_text(encoding="utf-8"))
    # Input generation must be able to create the files newly indexed by a
    # regenerated manifest. The active-tree validator checks those files
    # after generation; validating their existence here would make adding a
    # public surface require hand-created placeholder inputs.
    return validate_fixed_manifest(manifest)


def operation_index(
    manifest: dict[str, Any],
) -> dict[tuple[str, str], dict[str, Any]]:
    index: dict[tuple[str, str], dict[str, Any]] = {}
    for surface in manifest["surfaces"]:
        for operation in surface["operations"]:
            key = operation_key(surface["id"], operation["id"])
            if key in index:
                raise ValueError(f"duplicate manifest operation: {key}")
            index[key] = operation
    return index


def parameter_index(operation: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {
        parameter["id"]: parameter
        for parameter in operation["source"]["parameters"]
    }


def requirement_focus(
    requirement: dict[str, Any],
    parameters: dict[str, dict[str, Any]],
) -> str | None:
    if requirement["dimension"] != "parameter":
        return None
    marker = ".parameter."
    suffix = requirement["id"].split(marker, 1)[1]
    for parameter_id in parameters:
        if slug(parameter_id) == suffix:
            return parameter_id
    raise ValueError(
        f"{requirement['id']}: parameter requirement has no parameter"
    )


def requirement_mode(requirement: dict[str, Any]) -> str | None:
    if requirement["dimension"] != "mode":
        return None
    value = requirement["id"].rsplit(".", 1)[1]
    return {
        "1": "1",
        "cmyk": "CMYK",
        "f": "F",
        "hsv": "HSV",
        "i": "I",
        "l": "L",
        "la": "LA",
        "p": "P",
        "pa": "PA",
        "rgb": "RGB",
        "rgba": "RGBA",
        "ycbcr": "YCbCr",
    }.get(value, value.upper())


def requirement_format(requirement: dict[str, Any]) -> str | None:
    if requirement["dimension"] != "format":
        return None
    return requirement["id"].rsplit(".", 1)[1].upper()


def requirement_edge(requirement: dict[str, Any]) -> str | None:
    if requirement["dimension"] not in {"boundary", "error_path"}:
        return None
    marker = ".edge."
    return requirement["id"].split(marker, 1)[1] if marker in requirement["id"] else None


def requirement_variant(requirement: dict[str, Any]) -> dict[str, Any]:
    if requirement["dimension"] != "parameter_combination":
        return {}
    marker = "Deprecated-authority parameter combination: "
    description = requirement["description"]
    if not description.startswith(marker):
        raise ValueError(f"{requirement['id']}: missing variant encoding")
    value = json.loads(description.removeprefix(marker))
    if not isinstance(value, dict):
        raise ValueError(f"{requirement['id']}: variant must be an object")
    return value


inspect_missing = object()


@dataclass
class WorkflowBuilder:
    operations: dict[tuple[str, str], dict[str, Any]]
    primary_surface: str
    primary_operation: str
    requirement: dict[str, Any]
    assets_root: Path
    assets: list[dict[str, Any]] = field(default_factory=list)
    steps: list[dict[str, Any]] = field(default_factory=list)
    _asset_ids: set[str] = field(default_factory=set)
    _step_counter: int = 0
    _image_steps: dict[str, str] = field(default_factory=dict)
    _font_step: str | None = None
    scenario_values: dict[str, dict[str, Any]] = field(default_factory=dict)
    scenario_mode: str | None = None
    scenario_draw_mode: str | None = None
    scenario_edge: str | None = None
    scenario_pixel: Any | None = None
    scenario_font: str | None = None
    scenario_font_size: float | None = None
    scenario_transposed_orientation: Any | None = None
    scenario_bitmap_mode: str | None = None
    scenario_bitmap_color: Any | None = None
    scenario_size: list[int] | None = None
    scenario_im_mode: str | None = None
    scenario_mask_mode: str | None = None
    scenario_asset: str | None = None
    scenario_inline_image: str | None = None
    scenario_inline_mask_image: str | None = None
    scenario_exif_variant: str | None = None
    scenario_noise_seed: int | None = None
    scenario_chain: str | None = None
    scenario_observe_result: str | None = None
    scenario_observe_receiver: bool = False
    scenario_observe_stat_properties: bool = False
    scenario_outline_curve: bool = False
    scenario_outline_empty: bool = False

    @property
    def mode(self) -> str:
        return self.scenario_mode or requirement_mode(self.requirement) or "RGB"

    @property
    def image_format(self) -> str:
        return requirement_format(self.requirement) or "PNG"

    @property
    def edge(self) -> str | None:
        return self.scenario_edge or requirement_edge(self.requirement)

    def next_step_id(self, prefix: str) -> str:
        self._step_counter += 1
        return f"{prefix}-{self._step_counter}"

    def add_asset(self, asset: dict[str, Any]) -> str:
        asset_id = asset["id"]
        if asset_id not in self._asset_ids:
            self.assets.append(asset)
            self._asset_ids.add(asset_id)
        return asset_id

    def builtin(self, asset_id: str, name: str) -> dict[str, str]:
        self.add_asset({"id": asset_id, "kind": "builtin", "name": name})
        return asset_value(asset_id)

    def missing(self, asset_id: str, path: str) -> dict[str, str]:
        self.add_asset({"id": asset_id, "kind": "missing", "path": path})
        return asset_value(asset_id)

    def inline_bytes(
        self,
        asset_id: str,
        data: bytes,
        media_type: str,
    ) -> dict[str, str]:
        self.add_asset(
            {
                "id": asset_id,
                "kind": "inline",
                "encoding": "base64",
                "data": base64.b64encode(data).decode("ascii"),
                "sha256": hashlib.sha256(data).hexdigest(),
                "media_type": media_type,
            }
        )
        return asset_value(asset_id)

    def ref(
        self,
        asset_id: str,
        path: str,
        media_type: str,
    ) -> dict[str, str]:
        full_path = self.assets_root / path
        if not full_path.is_file():
            raise ValueError(f"missing active stimulus asset: {path}")
        self.add_asset(
            {
                "id": asset_id,
                "kind": "ref",
                "path": path,
                "sha256": sha256(full_path),
                "media_type": media_type,
            }
        )
        return asset_value(asset_id)

    def add_step(
        self,
        surface: str,
        operation: str,
        *,
        receiver: dict[str, Any] | None,
        arguments: dict[str, dict[str, Any]],
        step_id: str | None = None,
    ) -> str:
        key = operation_key(surface, operation)
        if key not in self.operations:
            raise ValueError(f"workflow references unknown operation {key}")
        actual_id = step_id or self.next_step_id(slug(operation))
        self.steps.append(
            {
                "step_id": actual_id,
                "surface": surface,
                "operation": operation,
                "receiver": receiver,
                "arguments": arguments,
            }
        )
        return actual_id

    def ensure_image(self, mode: str | None = None, label: str = "image") -> str:
        requested_mode = mode or self.mode
        if self.edge == "single-band-image":
            requested_mode = "L"
        if self.edge == "webp-alpha-mismatch" and label == "image":
            requested_mode = "RGBA"
        if self.edge == "mode-mismatch" and label not in {"image", "mask"}:
            requested_mode = "L" if requested_mode != "L" else "RGB"
        if self.edge == "composite-mode-mismatch":
            if label == "image1":
                requested_mode = "L"
            elif label == "mask":
                requested_mode = "L"
        if self.edge == "blend-second-palette" and label == "im2":
            requested_mode = "P"
        cache_key = f"{label}:{requested_mode}"
        if cache_key in self._image_steps:
            return self._image_steps[cache_key]
        if self.scenario_exif_variant is not None:
            base_path = self.assets_root / "image/rgb-small.jpg"
            if not base_path.is_file():
                raise ValueError("missing EXIF variant base asset: image/rgb-small.jpg")
            data = jpeg_with_exif_variant(
                base_path.read_bytes(), self.scenario_exif_variant
            )
            data_descriptor = self.inline_bytes(
                f"{label}-exif-{slug(self.scenario_exif_variant)}",
                data,
                "image/jpeg",
            )
            step_id = self.add_step(
                "PIL.Image",
                "open",
                receiver=None,
                arguments={"fp": data_descriptor},
                step_id=self.next_step_id(f"setup-{label}"),
            )
            self._image_steps[cache_key] = step_id
            return step_id
        if label == "mask" and self.scenario_inline_mask_image is not None:
            if self.scenario_inline_mask_image == "png-no-idat":
                data_descriptor = self.inline_bytes(
                    f"{label}-png-no-idat",
                    png_header_without_image_data(),
                    "image/png",
                )
                step_id = self.add_step(
                    "PIL.Image",
                    "open",
                    receiver=None,
                    arguments={"fp": data_descriptor},
                    step_id=self.next_step_id(f"setup-{label}"),
                )
            else:
                raise ValueError(
                    "unknown inline mask image stimulus: "
                    f"{self.scenario_inline_mask_image}"
                )
            self._image_steps[cache_key] = step_id
            return step_id
        if self.scenario_inline_image is not None:
            if self.scenario_inline_image == "l16-tiff":
                data_descriptor = self.inline_bytes(
                    f"{label}-l16-tiff",
                    little_endian_l16_tiff(),
                    "image/tiff",
                )
                step_id = self.add_step(
                    "PIL.Image",
                    "open",
                    receiver=None,
                    arguments={"fp": data_descriptor},
                    step_id=self.next_step_id(f"setup-{label}"),
                )
            elif self.scenario_inline_image in {
                "i16-frombytes",
                "i16n-frombytes",
                "i16l-frombytes",
                "i16b-frombytes",
            }:
                mode = {
                    "i16-frombytes": "I;16",
                    "i16n-frombytes": "I;16N",
                    "i16l-frombytes": "I;16L",
                    "i16b-frombytes": "I;16B",
                }[self.scenario_inline_image]
                data = (
                    bytes([0, 0, 1, 0, 2, 0, 3, 0])
                    if mode != "I;16B"
                    else bytes([0, 0, 0, 1, 0, 2, 0, 3])
                )
                asset_id = (
                    f"{label}-i16n-data"
                    if mode == "I;16N"
                    else f"{label}-{self.scenario_inline_image}-data"
                )
                data_descriptor = self.inline_bytes(
                    asset_id,
                    data,
                    "application/octet-stream",
                )
                step_id = self.add_step(
                    "PIL.Image",
                    "frombytes",
                    receiver=None,
                    arguments={
                        "mode": literal(mode),
                        "size": literal([2, 2]),
                        "data": data_descriptor,
                    },
                    step_id=self.next_step_id(f"setup-{label}"),
                )
            elif self.scenario_inline_image == "rgba-frombytes":
                data_descriptor = self.inline_bytes(
                    f"{label}-rgba-data",
                    bytes([16, 32, 64, 128]),
                    "application/octet-stream",
                )
                step_id = self.add_step(
                    "PIL.Image",
                    "frombytes",
                    receiver=None,
                    arguments={
                        "mode": literal("RGBa"),
                        "size": literal([1, 1]),
                        "data": data_descriptor,
                    },
                    step_id=self.next_step_id(f"setup-{label}"),
                )
            elif self.scenario_inline_image == "png-no-idat":
                data_descriptor = self.inline_bytes(
                    f"{label}-png-no-idat",
                    png_header_without_image_data(),
                    "image/png",
                )
                step_id = self.add_step(
                    "PIL.Image",
                    "open",
                    receiver=None,
                    arguments={"fp": data_descriptor},
                    step_id=self.next_step_id(f"setup-{label}"),
                )
            else:
                raise ValueError(
                    f"unknown inline image stimulus: {self.scenario_inline_image}"
                )
            self._image_steps[cache_key] = step_id
            return step_id
        if self.edge in {"mode-filter-pattern", "mode-filter-no-majority"} and label == "image":
            if requested_mode != "L":
                raise ValueError("mode-filter pattern edges require L mode")
            if self.edge == "mode-filter-pattern":
                # A public frombytes workflow with a 3x3 all-100 center makes
                # the mode filter select a nonzero value instead of retaining
                # the initial histogram bucket (zero). Uniform Image.new
                # inputs do not reach that branch.
                data = bytes(
                    [
                        0, 0, 0, 0, 0,
                        0, 100, 100, 100, 0,
                        0, 100, 100, 100, 0,
                        0, 100, 100, 100, 0,
                        0, 0, 0, 0, 0,
                    ]
                )
                size = [5, 5]
            else:
                # Every value in this 3x3 window is distinct, so Pillow keeps
                # the original pixel because no mode occurs more than twice.
                data = bytes(range(9))
                size = [3, 3]
            data_desc = self.inline_bytes(
                f"{label}-{self.edge}",
                data,
                "application/octet-stream",
            )
            step_id = self.add_step(
                "PIL.Image",
                "frombytes",
                receiver=None,
                arguments={
                    "mode": literal("L"),
                    "size": literal(size),
                    "data": data_desc,
                },
                step_id=self.next_step_id(f"setup-{label}"),
            )
            self._image_steps[cache_key] = step_id
            return step_id
        if self.scenario_asset is not None:
            # Stimulus workflows that open an encoded container (for example
            # the JPEG-with-EXIF `ImageOps.exif_transpose` cases) build the
            # primary image from a committed asset instead of `Image.new`.
            asset_media_type = {
                ".png": "image/png",
                ".gif": "image/gif",
                ".jpg": "image/jpeg",
                ".jpeg": "image/jpeg",
            }.get(Path(self.scenario_asset).suffix.lower(), "image/jpeg")
            fp_descriptor = self.ref(
                f"{label}-asset",
                self.scenario_asset,
                asset_media_type,
            )
            step_id = self.add_step(
                "PIL.Image",
                "open",
                receiver=None,
                arguments={"fp": fp_descriptor},
                step_id=self.next_step_id(f"setup-{label}"),
            )
            self._image_steps[cache_key] = step_id
            return step_id
        if self.edge == "zero-size-frombytes" and label == "image":
            data_desc = self.inline_bytes(
                f"{label}-zero-size-data",
                b"",
                "application/octet-stream",
            )
            step_id = self.add_step(
                "PIL.Image",
                "frombytes",
                receiver=None,
                arguments={
                    "mode": literal(requested_mode),
                    "size": literal([0, 0]),
                    "data": data_desc,
                },
                step_id=self.next_step_id(f"setup-{label}"),
            )
            self._image_steps[cache_key] = step_id
            return step_id
        if self.edge == "stat-median" and label == "image":
            if requested_mode not in {"I", "F"}:
                raise ValueError("stat-median edge requires I or F mode")
            # The public wrapper accepts integer scalar values for putpixel;
            # Pillow coerces the same values to float samples in F mode.
            values = [0, 1, 2, 3]
            step_id = self.add_step(
                "PIL.Image",
                "new",
                receiver=None,
                arguments={
                    "mode": literal(requested_mode),
                    "size": literal([4, 1]),
                    "color": literal(0),
                },
                step_id=self.next_step_id(f"setup-{label}"),
            )
            for index, value in enumerate(values):
                self.add_step(
                    "PIL.Image.Image",
                    "putpixel",
                    receiver=binding(step_id),
                    arguments={
                        "xy": literal([index, 0]),
                        "value": literal(value),
                    },
                    step_id=f"setup-stat-median-{index}",
                )
            self._image_steps[cache_key] = step_id
            return step_id
        if self.edge == "raw-p-no-palette" and label == "image":
            if requested_mode != "P":
                raise ValueError("raw-p-no-palette edge requires P mode")
            size = self.scenario_size or [16, 16]
            data = bytes(index % 4 for index in range(size[0] * size[1]))
            data_desc = self.inline_bytes(
                "raw-p-no-palette-data",
                data,
                "application/octet-stream",
            )
            step_id = self.add_step(
                "PIL.Image",
                "frombytes",
                receiver=None,
                arguments={
                    "mode": literal("P"),
                    "size": literal(size),
                    "data": data_desc,
                },
                step_id=self.next_step_id(f"setup-{label}"),
            )
            self._image_steps[cache_key] = step_id
            return step_id
        if self.edge == "palette-pipeline" and label == "image2":
            if requested_mode != "P":
                raise ValueError("palette-pipeline edge requires P mode")
            image_step = self.add_step(
                "PIL.Image",
                "new",
                receiver=None,
                arguments={
                    "mode": literal("P"),
                    "size": literal(self.scenario_size or [16, 16]),
                    "color": literal(0),
                },
                step_id=self.next_step_id("setup-image2"),
            )
            self.add_step(
                "PIL.Image.Image",
                "putpalette",
                receiver=binding(image_step),
                arguments={
                    "data": literal([10, 20, 30, 40, 50, 60]),
                    "rawmode": literal("RGB"),
                },
                step_id="setup-image2-palette",
            )
            step_id = self.add_step(
                "PIL.ImageOps",
                "flip",
                receiver=None,
                arguments={"image": binding(image_step)},
                step_id="setup-image2-pipeline",
            )
            self._image_steps[cache_key] = step_id
            return step_id
        if self.edge == "quantize-hash-rebuild":
            # Use one distinct RGB triplet per pixel so the public quantizer
            # crosses QuantHash's 65,536-entry rebuild threshold without
            # relying on probabilistic random input. Pixel one is chosen to
            # collide with pixel zero in PIL's masked hash at both scale 0 and
            # scale 1, exercising linear probing and reinsert collision paths
            # without increasing the fixture size.
            size = self.scenario_size or [257, 257]
            n_pixels = size[0] * size[1]
            data = bytes(
                channel
                for pixel in range(n_pixels)
                for channel in (
                    0 if pixel == 1 else pixel & 0xFF,
                    0 if pixel == 1 else (pixel >> 8) & 0xFF,
                    4 if pixel == 1 else (pixel >> 16) & 0xFF,
                )
            )
            data_desc = self.inline_bytes(
                f"{label}-quantize-hash-rebuild",
                data,
                "application/octet-stream",
            )
            step_id = self.add_step(
                "PIL.Image",
                "frombytes",
                receiver=None,
                arguments={
                    "mode": literal(requested_mode),
                    "size": literal(size),
                    "data": data_desc,
                },
                step_id=self.next_step_id(f"setup-{label}"),
            )
            self._image_steps[cache_key] = step_id
            return step_id
        if self.edge == "quantize-hash-recursive-rebuild":
            if requested_mode != "RGB":
                raise ValueError("quantize-hash-recursive-rebuild edge requires RGB mode")
            # Use 65,537 distinct colors whose channels are all even. After
            # the first scale increase, the right-shifted colors remain
            # distinct, forcing QuantHash's recursive rebuild without a large
            # fixture or probabilistic collision pattern.
            size = self.scenario_size or [65537, 1]
            n_pixels = size[0] * size[1]
            data = bytes(
                channel
                for pixel in range(n_pixels)
                for channel in (
                    (pixel & 0x7F) * 2,
                    ((pixel >> 7) & 0x7F) * 2,
                    ((pixel >> 14) & 0x7F) * 2,
                )
            )
            data_desc = self.inline_bytes(
                f"{label}-quantize-hash-recursive-rebuild",
                data,
                "application/octet-stream",
            )
            step_id = self.add_step(
                "PIL.Image",
                "frombytes",
                receiver=None,
                arguments={
                    "mode": literal(requested_mode),
                    "size": literal(size),
                    "data": data_desc,
                },
                step_id=self.next_step_id(f"setup-{label}"),
            )
            self._image_steps[cache_key] = step_id
            return step_id
        if self.edge == "quantize-repeated-colors" and label == "image":
            if requested_mode != "RGB":
                raise ValueError("quantize-repeated-colors edge requires RGB mode")
            size = self.scenario_size or [8, 8]
            colors = ((0, 0, 0), (255, 0, 0), (0, 255, 0), (0, 0, 255))
            n_pixels = size[0] * size[1]
            data = bytes(
                channel
                for pixel in range(n_pixels)
                for channel in colors[pixel % len(colors)]
            )
            data_desc = self.inline_bytes(
                f"{label}-quantize-repeated-colors",
                data,
                "application/octet-stream",
            )
            step_id = self.add_step(
                "PIL.Image",
                "frombytes",
                receiver=None,
                arguments={
                    "mode": literal("RGB"),
                    "size": literal(size),
                    "data": data_desc,
                },
                step_id=self.next_step_id(f"setup-{label}"),
            )
            self._image_steps[cache_key] = step_id
            return step_id
        if self.edge == "noise-fill":
            # Deterministic diverse images (used by quantize MAXCOVERAGE and
            # median-cut cases) are built through the public frombytes
            # endpoint with an inline base64 payload so both the oracle and
            # the target decode the exact same pixels.
            size = self.scenario_size or [16, 16]
            rng = random.Random(self.scenario_noise_seed or 0)
            n_pixels = size[0] * size[1]
            if requested_mode == "RGB":
                data = bytes(rng.randrange(256) for _ in range(n_pixels * 3))
            elif requested_mode == "RGBA":
                data = bytes(rng.randrange(256) for _ in range(n_pixels * 4))
            elif requested_mode == "L":
                data = bytes(rng.randrange(256) for _ in range(n_pixels))
            else:
                raise ValueError(f"noise-fill edge unsupported for mode {requested_mode}")
            data_desc = self.inline_bytes(
                f"{label}-noise",
                data,
                "application/octet-stream",
            )
            step_id = self.add_step(
                "PIL.Image",
                "frombytes",
                receiver=None,
                arguments={
                    "mode": literal(requested_mode),
                    "size": literal(size),
                    "data": data_desc,
                },
                step_id=self.next_step_id(f"setup-{label}"),
            )
            self._image_steps[cache_key] = step_id
            return step_id
        size = self.scenario_size or [16, 16]
        if self.edge == "zero-width":
            size = [0, 16]
        elif self.edge == "zero-height":
            size = [16, 0]
        elif self.edge == "zero-size":
            size = [0, 0]
        elif (
            self.edge == "source-larger-than-dest"
            and label not in {"image", "mask"}
        ):
            size = [32, 32]
        elif (
            self.edge == "source-smaller-than-dest"
            and label not in {"image", "mask"}
        ):
            size = [8, 8]
        elif self.edge == "second-smaller-than-first" and label == "im2":
            size = [8, 8]
        elif self.edge == "mask-size-mismatch" and label in {"mask", "alpha"}:
            size = [8, 8]
        elif self.edge == "valid-frombytes":
            size = [8, 1] if requested_mode == "1" else [1, 1]
        step_id = self.add_step(
            "PIL.Image",
            "new",
            receiver=None,
            arguments={
                "mode": literal(requested_mode),
                "size": literal(size),
                "color": literal(
                    self.scenario_pixel
                    if self.edge == "uniform-fill" and label == "image"
                    else self.scenario_bitmap_color
                    if label == "bitmap" and self.scenario_bitmap_color is not None
                    else 0
                ),
            },
            step_id=self.next_step_id(f"setup-{label}"),
        )
        self._image_steps[cache_key] = step_id
        if self.edge == "effect-spread-p-rgba" and label == "image":
            # EffectSpread returns a new indexed image while retaining the
            # source palette. Attach that palette through the public API so
            # the active parity case observes the same metadata-bearing P
            # pipeline as Pillow.
            self.add_step(
                "PIL.Image.Image",
                "putpalette",
                receiver=binding(step_id),
                arguments={
                    "data": literal(
                        [10, 20, 30, 5, 40, 50, 60, 128, 70, 80, 90, 255]
                    ),
                    "rawmode": literal("RGBA"),
                },
                step_id=self.next_step_id("setup-effect-spread-palette"),
            )
        if self.edge == "too-many-colors" and label == "image":
            pixel_value: Any
            if requested_mode in {"L", "P", "1"}:
                pixel_value = 1
            elif requested_mode in {"LA"}:
                pixel_value = [1, 255]
            elif requested_mode in {"RGBA"}:
                pixel_value = [1, 2, 3, 255]
            else:
                pixel_value = [1, 2, 3]
            self.add_step(
                "PIL.Image.Image",
                "putpixel",
                receiver=binding(step_id),
                arguments={
                    "xy": literal([0, 0]),
                    "value": literal(pixel_value),
                },
                step_id=self.next_step_id("setup-varied-pixel"),
            )
        elif self.edge == "nonzero-pixel" and label in {"image", "image1"}:
            if self.scenario_pixel is None:
                raise ValueError("nonzero-pixel edge requires a scenario pixel")
            self.add_step(
                "PIL.Image.Image",
                "putpixel",
                receiver=binding(step_id),
                arguments={
                    "xy": literal([2, 3]),
                    "value": literal(self.scenario_pixel),
                },
                step_id=self.next_step_id("setup-varied-pixel"),
            )
        elif self.edge == "mask-nonzero-pixel" and label == "mask":
            # Keep the primary image at its default value while selecting one
            # pixel through the public L/1 mask. This complements the
            # all-zero mask case and reaches both branches of every masked
            # histogram loop without a direct native probe.
            self.add_step(
                "PIL.Image.Image",
                "putpixel",
                receiver=binding(step_id),
                arguments={
                    "xy": literal([2, 3]),
                    "value": literal(255),
                },
                step_id=self.next_step_id("setup-mask-pixel"),
            )
        return step_id

    def ensure_font(self) -> str:
        if self._font_step is not None:
            return self._font_step
        font = self.ref(
            "font",
            self.scenario_font or "font/fonts/DejaVuSans.ttf",
            "font/ttf",
        )
        size = (
            self.scenario_font_size
            if self.scenario_font_size is not None
            else 20
        )
        self._font_step = self.add_step(
            "PIL.ImageFont",
            "truetype",
            receiver=None,
            arguments={"font": font, "size": literal(size)},
            step_id=self.next_step_id("setup-font"),
        )
        return self._font_step

    def outline_value(self) -> dict[str, Any]:
        return outline_literal(
            curve=self.scenario_outline_curve,
            empty=self.scenario_outline_empty,
        )

    def receiver_for(self, surface: str) -> dict[str, str] | None:
        if surface == "PIL.Image.Image":
            return binding(self.ensure_image())
        if surface == "PIL.ImageDraw.ImageDraw":
            image_step = self.ensure_image()
            draw_arguments: dict[str, dict[str, Any]] = {"im": binding(image_step)}
            if self.scenario_draw_mode is not None:
                draw_arguments["mode"] = literal(self.scenario_draw_mode)
            draw_step = self.add_step(
                "PIL.ImageDraw",
                "Draw",
                receiver=None,
                arguments=draw_arguments,
                step_id=self.next_step_id("setup-draw"),
            )
            return binding(draw_step)
        if surface.startswith("PIL.ImageEnhance."):
            class_name = surface.rsplit(".", 1)[1]
            image_step = self.ensure_image()
            enhance_step = self.add_step(
                "PIL.ImageEnhance",
                class_name,
                receiver=None,
                arguments={"image": binding(image_step)},
                step_id=self.next_step_id(f"setup-{slug(class_name)}"),
            )
            return binding(enhance_step)
        if surface == "PIL.ImageFont.FreeTypeFont":
            return binding(self.ensure_font())
        if surface == "PIL.ImageFont.ImageFont":
            font_step = self.add_step(
                "PIL.ImageFont",
                "ImageFont",
                receiver=None,
                arguments={},
                step_id=self.next_step_id("setup-imagefont"),
            )
            return binding(font_step)
        if surface == "PIL.ImageFont.TransposedFont":
            font_step = self.ensure_font()
            arguments: dict[str, dict[str, Any]] = {
                "font": binding(font_step)
            }
            if self.scenario_transposed_orientation is not None:
                arguments["orientation"] = literal(
                    self.scenario_transposed_orientation
                )
            transposed = self.add_step(
                "PIL.ImageFont",
                "TransposedFont",
                receiver=None,
                arguments=arguments,
                step_id=self.next_step_id("setup-transposed-font"),
            )
            return binding(transposed)
        if surface == "PIL.ImagePalette.ImagePalette":
            arguments: dict[str, dict[str, Any]] = {}
            for parameter_id in ("mode", "palette"):
                descriptor = self.scenario_values.get(parameter_id)
                if descriptor is not None:
                    arguments[parameter_id] = descriptor
            palette = self.add_step(
                "PIL.ImagePalette",
                "ImagePalette",
                receiver=None,
                arguments=arguments,
                step_id=self.next_step_id("setup-palette"),
            )
            return binding(palette)
        if surface == "PIL.ImageFilter.Color3DLUT":
            return binding(self.ensure_filter_instance("Color3DLUT"))
        if surface == "PIL.ImageSequence.Iterator":
            iterator = self.add_step(
                "PIL.ImageSequence",
                "Iterator",
                receiver=None,
                arguments={"im": binding(self.ensure_image())},
                step_id=self.next_step_id("setup-iterator"),
            )
            return binding(iterator)
        if surface == "PIL.ImageStat.Stat":
            image_step = self.ensure_image()
            stat = self.add_step(
                "PIL.ImageStat",
                "Stat",
                receiver=None,
                arguments={"image_or_list": binding(image_step)},
                step_id=self.next_step_id("setup-stat"),
            )
            return binding(stat)
        return None

    def ensure_filter_instance(self, filter_name: str) -> str:
        """Construct a public ImageFilter object for an Image.filter call."""

        key = operation_key("PIL.ImageFilter", filter_name)
        operation = self.operations.get(key)
        if operation is None:
            raise ValueError(f"unknown ImageFilter operation: {filter_name}")
        if filter_name == "Color3DLUT":
            # A class-method workflow such as Color3DLUT.__repr__ has no
            # constructor parameters in its receiver operation. Build the
            # receiver through the public constructor with a valid minimal
            # 2x2x2 RGB table instead of replaying the method signature.
            arguments = {
                "size": literal(2),
                "table": literal([0.0] * 24),
            }
        else:
            arguments = self.required_arguments(operation)
        return self.add_step(
            "PIL.ImageFilter",
            filter_name,
            receiver=None,
            arguments=arguments,
            step_id=self.next_step_id(f"setup-filter-{slug(filter_name)}"),
        )

    def required_arguments(
        self, operation: dict[str, Any]
    ) -> dict[str, dict[str, Any]]:
        arguments: dict[str, dict[str, Any]] = {}
        for parameter in operation["source"]["parameters"]:
            if parameter["style"] == "receiver":
                continue
            if parameter["omission"]["kind"] == "required":
                arguments[parameter["id"]] = self.descriptor_for(parameter)
        return arguments

    def concrete_variant(self, parameter_id: str, value: Any) -> Any:
        if not isinstance(value, str):
            return value
        symbolic = {
            "default": None,
            "int": 1,
            "float": 1.0,
            "hex_string": "#204080",
            "rgb_tuple": [32, 64, 128],
            "color_tuple": [32, 64, 128],
            "rgba_tuple": [32, 64, 128, 255],
            "la_tuple": [64, 255],
            "tuple": [1, 2, 3],
            "4tuple": [1, 0, 0, 1],
            "12tuple": [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0],
            "list": [0, 1, 2, 3],
            "sequence": [0, 1, 2, 3],
            "box": [0, 0, 8, 8],
            "line": [[1, 1], [14, 14]],
            "polygon": [[2, 2], [14, 2], [8, 14]],
            "numpy": {
                "protocol": "array-interface",
                "shape": [2, 2, 3],
                "typestr": "|u1",
                "data_base64": "AAAAAAAAAAAAAAAA",
            },
            "path": "path",
            "bytes": "bytes",
            "deformer": {
                "protocol": "getmesh",
                "mesh": [
                    [[0, 0, 16, 16], [0, 0, 0, 16, 16, 16, 16, 0]]
                ],
            },
        }
        return symbolic.get(value, value)

    def edge_override(
        self,
        parameter_id: str,
        value_types: set[str],
    ) -> Any | None:
        edge = self.edge or ""
        name = parameter_id.lower()
        if not edge:
            return None
        if "mode" in name and "invalid" in edge:
            return "NOT_A_MODE"
        if name == "matrix" and edge == "invalid-matrix-size":
            return [1, 2, 3]
        if name == "channel" and edge == "invalid-channel":
            return "Z"
        if name == "band" and edge == "invalid-band":
            return 99
        if name == "bands" and edge == "mode-band-mismatch":
            return ["L"]
        if name == "bands" and edge == "invalid-mode":
            return []
        if name == "format" and edge.startswith("webp-"):
            return "WEBP"
        if name == "format" and edge == "unsupported-format":
            return "NOT_A_FORMAT"
        if edge == "too-many-colors" and name == "maxcolors":
            return 1
        if edge in {"out-of-bounds", "negative-coords", "y-out-of-bounds"} and name == "xy":
            if edge == "out-of-bounds":
                return [16, 16]
            if edge == "y-out-of-bounds":
                return [0, 16]
            return [-1, -1]
        if edge == "out-of-bounds" and name == "box":
            return [0, 0, 32, 32]
        if edge == "negative-coords" and name == "box":
            return [-2, -2, 8, 8]
        if edge == "zero-size-crop" and name == "box":
            return [0, 0, 0, 0]
        if edge == "full-image-crop" and name == "box":
            return [0, 0, 16, 16]
        if edge in {"upscale", "larger-than-image"} and name == "size":
            return [32, 32]
        if edge in {"downscale"} and name == "size":
            return [4, 4]
        if edge in {"same-size-noop", "angle-zero"} and name == "size":
            return [16, 16]
        if edge == "angle-zero" and name == "angle":
            return 0
        if edge == "angle-negative" and name == "angle":
            return -90
        if edge == "angle-360" and name == "angle":
            return 360
        if edge == "zero-size" and name == "size":
            return [0, 0]
        if edge == "zero-dimension" and name == "size":
            return [0, 0]
        if name in {"size"} and edge in {"zero-width", "zero-height"}:
            return [0, 16] if edge == "zero-width" else [16, 0]
        if ("text" in name or "data" in name) and "empty" in edge:
            return ""
        if (
            "negative" in edge
            and name
            not in {
                "image",
                "im",
                "im1",
                "im2",
                "image1",
                "image2",
                "mask",
                "bitmap",
                "palette",
            }
            and (
            "integer" in value_types or "number" in value_types
            )
        ):
            return -1
        if "unsupported" in edge and (
            "string" in value_types or "enum" in value_types
        ):
            return "UNSUPPORTED"
        return None

    def descriptor_for(
        self,
        parameter: dict[str, Any],
        *,
        variant_value: Any = inspect_missing,
    ) -> dict[str, Any]:
        parameter_id = parameter["id"]
        value_types = set(parameter["value_types"])
        name = parameter_id.lower()

        if (
            parameter_id == "callback"
            and self.primary_surface == "PIL.ImageFilter.Color3DLUT"
        ):
            callback_descriptor = self.scenario_values.get("callback")
            callback_name = (
                callback_descriptor.get("value")
                if callback_descriptor is not None
                and callback_descriptor.get("kind") == "literal"
                else None
            )
            if callback_name is None:
                callback_name = (
                    "color3dlut-generate-identity"
                    if self.primary_operation == "generate"
                    else "color3dlut-transform-identity"
                )
            if callback_name not in {
                "color3dlut-generate-identity",
                "color3dlut-transform-identity",
                "color3dlut-transform-rgba",
                "color3dlut-short-result",
            }:
                raise ValueError(f"unsupported Color3DLUT callback: {callback_name}")
            return self.builtin(
                f"{slug(parameter_id)}-{slug(callback_name)}",
                callback_name,
            )

        if (
            parameter_id == "size"
            and self.primary_surface == "PIL.ImageFilter.Color3DLUT"
            and self.primary_operation in {"generate", "transform"}
            and parameter_id not in self.scenario_values
        ):
            return literal(2)

        for scenario_key, descriptor in self.scenario_values.items():
            if scenario_key == parameter_id:
                if (
                    descriptor.get("kind") == "bytes"
                    and isinstance(descriptor.get("value"), list)
                ):
                    return self.inline_bytes(
                        f"{slug(parameter_id)}-inline",
                        bytes(descriptor["value"]),
                        "application/octet-stream",
                    )
                if (
                    parameter_id == "source_palette"
                    and descriptor.get("kind") == "literal"
                    and isinstance(descriptor.get("value"), list)
                ):
                    return self.inline_bytes(
                        f"{slug(parameter_id)}-inline",
                        bytes(descriptor["value"]),
                        "application/octet-stream",
                    )
                if (
                    self.primary_surface == "PIL.Image"
                    and self.primary_operation == "eval"
                    and parameter_id == "args"
                    and descriptor.get("kind") == "literal"
                    and descriptor.get("value")
                    == ["clamp-shift-callable"]
                ):
                    # Exercise Pillow's CLIP8 saturation with a callable whose
                    # outputs leave the [0, 255] LUT range.
                    return self.builtin("args-callable", "clamp-shift-callable")
                if (
                    self.primary_surface == "PIL.Image.Image"
                    and self.primary_operation == "point"
                    and parameter_id == "lut"
                    and descriptor.get("kind") == "literal"
                    and descriptor.get("value") == ["clamp-shift-callable"]
                ):
                    return self.builtin("lut-callable", "clamp-shift-callable")
                return descriptor

        if (
            name == "shape"
            and self.primary_surface.startswith("PIL.ImageDraw")
        ):
            return self.outline_value()

        if self.edge == "valid-frombytes":
            if parameter_id == "size":
                return literal([8, 1] if self.mode == "1" else [1, 1])
            if parameter_id == "data":
                data_by_mode = {
                    "1": b"\x80",
                    "L": b"\x7f",
                    "P": b"\x07",
                    "LA": b"\x7f\xc8",
                    "RGB": b"\x10\x20\x30",
                    "HSV": b"\x10\x20\x30",
                    "YCbCr": b"\x10\x20\x30",
                    "RGBA": b"\x10\x20\x30\xc8",
                    "RGBa": b"\x10\x20\x30\xc8",
                    "CMYK": b"\x10\x20\x30\xc8",
                    "I": b"\x10\x20\x30\xc8",
                    "F": b"\x10\x20\x30\xc8",
                    "I;16": b"\x70\x11",
                    "I;16B": b"\x11\x70",
                }
                data = data_by_mode.get(self.mode)
                if data is None:
                    raise ValueError(
                        f"valid-frombytes edge unsupported for mode {self.mode}"
                    )
                return self.inline_bytes(
                    f"valid-{slug(self.mode)}-data",
                    data,
                    "application/octet-stream",
                )

        if parameter_id == "font" and self.scenario_font is not None:
            if "font" in value_types and "path" not in value_types:
                return binding(self.ensure_font())
            return self.ref("font", self.scenario_font, "font/ttf")

        if variant_value is not inspect_missing:
            if (
                parameter_id == "filter"
                and isinstance(variant_value, str)
                and operation_key("PIL.ImageFilter", variant_value)
                in self.operations
            ):
                return binding(self.ensure_filter_instance(variant_value))
            if variant_value == "image":
                return binding(
                    self.ensure_image(label=slug(parameter_id))
                )
            if parameter_id == "font" and variant_value == "path":
                return self.ref(
                    "font",
                    "font/fonts/DejaVuSans.ttf",
                    "font/ttf",
                )
            if parameter_id == "font" and variant_value == "bytes":
                font_path = self.assets_root / "font/fonts/DejaVuSans.ttf"
                return self.inline_bytes(
                    "font-bytes",
                    font_path.read_bytes(),
                    "font/ttf",
                )
            if parameter_id == "font" and variant_value == "stream":
                return self.builtin("font-stream", "font-byte-stream")
            if variant_value == "callable":
                return self.builtin(
                    f"{slug(parameter_id)}-callable",
                    "identity-callable",
                )
            value = self.concrete_variant(parameter_id, variant_value)
            if value == "path":
                return self.builtin(
                    f"{slug(parameter_id)}-path",
                    "temporary-output-path",
                )
            if value == "bytes":
                return self.inline_bytes(
                    f"{slug(parameter_id)}-bytes",
                    b"\x00\x01\x02\x03",
                    "application/octet-stream",
                )
            enum_values = {
                "NEAREST": 0,
                "LANCZOS": 1,
                "BILINEAR": 2,
                "BICUBIC": 3,
                "BOX": 4,
                "HAMMING": 5,
            }
            if (
                isinstance(value, str)
                and value in enum_values
                and self.primary_operation == "transform"
                and parameter_id == "resample"
            ):
                # Pillow's transform accepts the integer Resampling enum only;
                # convert the symbolic generator default so the canonical
                # resample workflow exercises a real filter instead of an
                # error-only string.
                value = enum_values[value]
            if (
                isinstance(value, str)
                and value == "NEAREST"
                and self.primary_operation == "transpose"
                and parameter_id == "method"
            ):
                # Pillow's Image.transpose accepts the integer Transpose enum;
                # a resampling name is only the generic generator fallback.
                value = 0
            if (
                isinstance(value, str)
                and value in enum_values
                and "integer" in value_types
                and "string" not in value_types
                and "enum" not in value_types
            ):
                value = enum_values[value]
            transpose_values = {
                "FLIP_LEFT_RIGHT": 0,
                "FLIP_TOP_BOTTOM": 1,
                "ROTATE_90": 2,
                "ROTATE_180": 3,
                "ROTATE_270": 4,
                "TRANSPOSE": 5,
                "TRANSVERSE": 6,
            }
            if (
                isinstance(value, str)
                and value in transpose_values
                and self.primary_operation == "transpose"
                and parameter_id == "method"
            ):
                # Deprecated parameter combinations preserve symbolic enum
                # names in the manifest; execute the public IntEnum value.
                value = transpose_values[value]
            if value is not None:
                return literal(value)

        if (
            self.primary_operation == "transform"
            and self.primary_surface == "PIL.Image.Image"
            and parameter_id in {"data", "resample"}
        ):
            if parameter_id == "data":
                # Canonical transform workflows need real AFFINE data; the
                # identity matrix exercises the full geometry pipeline.
                return literal([1.0, 0.0, 0.0, 0.0, 1.0, 0.0])
            return literal(0)

        override = self.edge_override(parameter_id, value_types)
        if override is not None:
            return literal(override)

        if name == "fp" and self.edge:
            if self.edge in {
                "invalid-bytes",
                "empty-bytes",
                "embedded-null-bytes",
                "non-null-bytes",
                "webp-corrupt-vp8-bitstream",
                "webp-truncated-riff",
                "webp-invalid-riff-header",
            }:
                edge_bytes = {
                    "invalid-bytes": b"\x00invalid",
                    "empty-bytes": b"",
                    "embedded-null-bytes": b"/tmp\x00image.png",
                    "non-null-bytes": b"missing-image-path",
                    "webp-corrupt-vp8-bitstream": b"RIFF\x10\x00\x00\x00WEBPVP8 \x00\x00\x00\x00",
                    "webp-truncated-riff": b"RIFF\x08\x00\x00\x00WEBP",
                    "webp-invalid-riff-header": b"NOTRIFF",
                }[self.edge]
                return self.inline_bytes(
                    f"{slug(self.edge)}-input",
                    edge_bytes,
                    "application/octet-stream",
                )
            if self.edge == "invalid-path":
                return self.missing("invalid-input-path", "missing/invalid-output")
            if self.edge == "read-only-directory":
                return self.builtin("read-only-directory", "read-only-directory")

        if name in {
            "image",
            "im",
            "im1",
            "im2",
            "image1",
            "image2",
            "bitmap",
            "palette",
        } or "image" in value_types:
            if name == "bitmap" and self.scenario_bitmap_mode is not None:
                return binding(
                    self.ensure_image(
                        mode=self.scenario_bitmap_mode,
                        label="bitmap",
                    )
                )
            if name == "im" and self.scenario_im_mode is not None:
                return binding(
                    self.ensure_image(
                        mode=self.scenario_im_mode,
                        label="im",
                    )
                )
            if name == "alpha" and self.scenario_mask_mode is not None:
                # putalpha accepts an "L"/"1" alpha layer; honor the scenario
                # mask mode so the bound argument is a compatible mask.
                return binding(
                    self.ensure_image(
                        mode=self.scenario_mask_mode,
                        label="alpha",
                    )
                )
            if name == "mask" and self.scenario_mask_mode is not None:
                return binding(
                    self.ensure_image(
                        mode=self.scenario_mask_mode,
                        label="mask",
                    )
                )
            return binding(self.ensure_image(label=slug(parameter_id)))
        if name == "mask":
            if self.scenario_mask_mode is not None:
                return binding(
                    self.ensure_image(
                        mode=self.scenario_mask_mode,
                        label="mask",
                    )
                )
            return binding(self.ensure_image(mode="L", label="mask"))
        if name == "font":
            if "font" in value_types and "path" not in value_types:
                return binding(self.ensure_font())
            return self.ref(
                "font",
                "font/fonts/DejaVuSans.ttf",
                "font/ttf",
            )
        if name in {"filename"}:
            if self.primary_surface.startswith("PIL.ImageFont"):
                pilfont_path = self.scenario_asset or "font/pilfont/courb08.pil"
                return self.ref(
                    "pilfont",
                    pilfont_path,
                    "application/x-pilfont",
                )
            return self.builtin("input-path", "encoded-png-input-path")
        if name == "fp":
            if self.primary_operation == "open":
                if "nonexistent" in (self.edge or ""):
                    return self.missing(
                        "missing-input",
                        "missing/does-not-exist.png",
                    )
                return self.builtin(
                    "encoded-input",
                    f"encoded-{self.image_format.lower()}-input",
                )
            if (
                self.primary_operation == "save"
                and self.edge == "no-extension"
            ):
                return self.builtin(
                    "output-no-extension",
                    "temporary-output-no-extension-path",
                )
            return self.builtin("output", "temporary-output-path")
        if name in {"shape"} and self.primary_surface.startswith(
            "PIL.ImageDraw"
        ):
            return self.outline_value()
        if name == "filter":
            return binding(self.ensure_filter_instance("BLUR"))
        if name in {"obj"}:
            return literal(
                {
                    "protocol": "array-interface",
                    "shape": [2, 2, 3],
                    "typestr": "|u1",
                    "data_base64": "AAAAAAAAAAAAAAAA",
                }
            )
        if name in {"deformer"}:
            return literal(
                {
                    "protocol": "getmesh",
                    "mesh": [[[0, 0, 16, 16], [0, 0, 0, 16, 16, 16, 16, 0]]],
                }
            )
        if name in {"lut", "table", "kernel"}:
            if name == "kernel":
                return literal([0, 0, 0, 0, 1, 0, 0, 0, 0])
            if name == "table":
                return literal([0.0] * 24)
            return literal(list(range(256)))
        if name in {"matrix"}:
            return literal([1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0])
        if name in {"dest_map"}:
            return literal(list(range(256)))
        if name in {"data"} and "sequence" in value_types:
            return literal([0, 1, 2, 3])
        if name in {"text"}:
            return literal("" if "empty" in (self.edge or "") else "Hello")
        if name in {"mode"}:
            return literal(self.mode)
        if name in {"format"}:
            return literal(self.image_format)
        if name in {"size"}:
            if self.primary_surface == "PIL.ImageFilter":
                if self.primary_operation == "Kernel":
                    return literal([3, 3])
                if self.primary_operation == "Color3DLUT":
                    return literal(2)
                if self.primary_operation in {
                    "MaxFilter",
                    "MinFilter",
                    "MedianFilter",
                    "ModeFilter",
                    "RankFilter",
                }:
                    return literal(3)
            if self.primary_operation == "Color3DLUT":
                return literal(2)
            if "sequence" not in value_types:
                return literal(20 if "number" in value_types else 3)
            return literal([16, 16])
        if name in {"xy", "center", "translate", "dest", "source"}:
            return literal([0, 0])
        if name in {"box"}:
            return literal([0, 0, 8, 8])
        if name in {"bands"}:
            return literal([])
        if name in {"color", "fill", "fillcolor", "outline", "stroke_fill"}:
            if value_types == {"null"}:
                return literal(None)
            if "integer" in value_types or "number" in value_types:
                return literal(0)
            if "string" in value_types:
                return literal("#204080")
            if "sequence" in value_types:
                return literal([32, 64, 128])
            return literal(None)
        if name in {"factor", "alpha", "scale", "sigma", "angle", "radius"}:
            if "image" in value_types:
                return binding(
                    self.ensure_image(label=slug(parameter_id))
                )
            if "sequence" in value_types and "number" not in value_types:
                return literal([2, 2])
            if "integer" in value_types and "number" not in value_types:
                return literal(2)
            return literal(1.0)
        if name in {
            "width",
            "height",
            "colors",
            "distance",
            "bits",
            "threshold",
            "percent",
            "rank",
            "n_sides",
            "frame",
            "index",
            "size_index",
            "stroke_width",
        }:
            return literal(1)
        if name in {"method", "resample", "dither", "orientation", "layout_engine"}:
            if "integer" in value_types and not (
                {"string", "enum"} & value_types
            ):
                return literal(0)
            if name == "method" and self.primary_operation == "transform":
                # Pillow's transform method is an integer enum (AFFINE=0,
                # EXTENT=1, PERSPECTIVE=2, QUAD=3, MESH=4); the generic
                # "NEAREST" default above is a resample name and would make
                # every canonical transform case an error-only workflow.
                return literal(0)
            if name == "method" and self.primary_operation == "transpose":
                # Pillow's transpose method is an integer enum. Keep the
                # canonical input public and valid for both oracle and target.
                return literal(0)
            return literal("NEAREST")
        if name in {"args"}:
            return literal(["identity"])
        if name in {"kwargs", "params"}:
            return literal({})
        if "boolean" in value_types:
            return literal(False)
        if "integer" in value_types:
            return literal(1)
        if "number" in value_types:
            return literal(1.0)
        if "string" in value_types or "enum" in value_types or "path" in value_types:
            return literal("value")
        if "bytes" in value_types:
            if any(
                token in (self.edge or "")
                for token in ("corrupt", "invalid", "truncated")
            ):
                return self.inline_bytes(
                    f"{slug(parameter_id)}-invalid",
                    b"\x00invalid",
                    "application/octet-stream",
                )
            return self.inline_bytes(
                f"{slug(parameter_id)}-bytes",
                b"\x00\x01\x02\x03",
                "application/octet-stream",
            )
        if "sequence" in value_types:
            return literal([])
        if "mapping" in value_types or "record" in value_types:
            return literal({})
        if "stream" in value_types:
            return self.builtin(
                f"{slug(parameter_id)}-stream",
                "in-memory-byte-stream",
            )
        if "font" in value_types:
            return binding(self.ensure_font())
        if "handle" in value_types:
            return self.outline_value()
        if "null" in value_types:
            return literal(None)
        return literal(1)

    def primary_arguments(
        self,
        operation: dict[str, Any],
    ) -> dict[str, dict[str, Any]]:
        parameters = parameter_index(operation)
        focus = requirement_focus(self.requirement, parameters)
        variant = requirement_variant(self.requirement)
        arguments: dict[str, dict[str, Any]] = {}
        for parameter in operation["source"]["parameters"]:
            parameter_id = parameter["id"]
            if parameter["style"] == "receiver":
                continue
            omission = parameter["omission"]
            required = omission["kind"] == "required"
            if (
                parameter_id in variant
                and variant[parameter_id] == "default"
                and not required
            ):
                continue
            selected = required or parameter_id == focus or parameter_id in variant
            if parameter_id in self.scenario_values:
                selected = True
            if self.scenario_font is not None and parameter_id == "font":
                selected = True
            if self.requirement["dimension"] == "format" and parameter_id == "format":
                selected = True
            if self.requirement["dimension"] == "mode" and parameter_id == "mode":
                selected = True
            if (
                self.requirement["dimension"] in {"boundary", "error_path"}
                and self.edge
                and parameter["style"] != "receiver"
            ):
                selected = True
            if (
                self.primary_surface == "PIL.Image"
                and self.primary_operation == "eval"
                and parameter_id == "args"
            ):
                selected = True
            if not selected:
                continue
            variant_value = variant.get(parameter_id, inspect_missing)
            arguments[parameter_id] = self.descriptor_for(
                parameter, variant_value=variant_value
            )
        if self.requirement["dimension"] == "parameter" and focus is not None:
            focused = operation["source"]["parameters"]
            focused_parameter = next(
                (item for item in focused if item["id"] == focus),
                None,
            )
            if (
                focused_parameter is not None
                and set(focused_parameter["value_types"]) == {"boolean"}
                and focused_parameter["omission"].get("kind") == "literal"
                and focused_parameter["omission"].get("value") is False
            ):
                # A focused boolean parameter must exercise its non-default
                # value; the generic fallback would otherwise re-emit the
                # omission default and never reach the True branch.
                arguments[focus] = literal(True)
        return arguments

    def build(self) -> tuple[list[dict[str, Any]], list[dict[str, Any]], list[str]]:
        operation = self.operations[
            operation_key(self.primary_surface, self.primary_operation)
        ]
        if self.scenario_chain == "pilfont-load-getmask":
            if self.primary_surface != "PIL.ImageFont.ImageFont":
                raise ValueError(
                    "PILfont chains require PIL.ImageFont.ImageFont methods"
                )
            receiver_step = self.add_step(
                "PIL.ImageFont",
                "load",
                receiver=None,
                arguments={
                    "filename": self.ref(
                        "pilfont",
                        self.scenario_asset or "font/pilfont/courb08.pil",
                        "application/x-pilfont",
                    )
                },
                step_id="setup-pilfont",
            )
            call_id = self.add_step(
                self.primary_surface,
                self.primary_operation,
                receiver=binding(receiver_step),
                arguments=self.primary_arguments(operation),
                step_id="call",
            )
            return self.assets, self.steps, [call_id]

        if self.scenario_chain is not None:
            if self.primary_surface not in {
                "PIL.Image.Image",
                "PIL.ImageDraw.ImageDraw",
                "PIL.ImageOps",
            }:
                raise ValueError(
                    "scenario chains require supported image or image-draw methods"
                )

            chain = self.scenario_chain
            observation_step: str | None = None
            if chain == "resize-verify":
                image_step = self.ensure_image(mode="RGB")
                receiver_step = self.add_step(
                    "PIL.Image.Image",
                    "resize",
                    receiver=binding(image_step),
                    arguments={
                        "size": literal([8, 8]),
                        "resample": literal(0),
                    },
                    step_id="setup-resize",
                )
            elif chain == "truthy-non-image-mask":
                image_step = self.ensure_image()
                non_image_step = self.add_step(
                    "PIL.Image.Image",
                    "getextrema",
                    receiver=binding(image_step),
                    arguments={},
                    step_id="setup-non-image-mask",
                )
                self.scenario_values["mask"] = binding(non_image_step)
                receiver_step = image_step
            elif chain == "image-color-input":
                image_step = self.ensure_image()
                color_step = self.ensure_image(label="color")
                self.scenario_values["color"] = binding(color_step)
                receiver_step = image_step
            elif chain == "none-centering-input":
                image_step = self.ensure_image(mode="RGB")
                none_step = self.add_step(
                    "PIL.Image.Image",
                    "putpixel",
                    receiver=binding(image_step),
                    arguments={
                        "xy": literal([0, 0]),
                        "value": literal([1, 2, 3]),
                    },
                    step_id="setup-none-centering",
                )
                self.scenario_values["centering"] = binding(none_step)
                receiver_step = image_step
            elif chain == "opened-rgb-resize-verify":
                image_step = self.ensure_image(mode="RGB")
                receiver_step = self.add_step(
                    "PIL.Image.Image",
                    "resize",
                    receiver=binding(image_step),
                    arguments={
                        "size": literal([8, 8]),
                        "resample": literal(0),
                    },
                    step_id="setup-resize",
                )
            elif chain == "p-resize-verify":
                image_step = self.ensure_image(mode="P")
                receiver_step = self.add_step(
                    "PIL.Image.Image",
                    "resize",
                    receiver=binding(image_step),
                    arguments={
                        "size": literal([8, 8]),
                        "resample": literal(0),
                    },
                    step_id="setup-resize",
                )
            elif chain == "p-resize-no-palette-load":
                image_step = self.ensure_image(mode="P")
                receiver_step = self.add_step(
                    "PIL.Image.Image",
                    "resize",
                    receiver=binding(image_step),
                    arguments={
                        "size": literal([8, 8]),
                        "resample": literal(0),
                    },
                    step_id="setup-resize",
                )
            elif chain == "p-putpalette-resize":
                image_step = self.ensure_image(mode="P")
                self.add_step(
                    "PIL.Image.Image",
                    "putpalette",
                    receiver=binding(image_step),
                    arguments={
                        "data": literal([10, 20, 30, 40, 50, 60]),
                        "rawmode": literal("RGB"),
                    },
                    step_id="setup-putpalette",
                )
                receiver_step = self.add_step(
                    "PIL.Image.Image",
                    "resize",
                    receiver=binding(image_step),
                    arguments={
                        "size": literal([8, 8]),
                        "resample": literal(0),
                    },
                    step_id="setup-resize",
                )
            elif chain == "p-resize-convert-verify":
                image_step = self.ensure_image(mode="P")
                resized_step = self.add_step(
                    "PIL.Image.Image",
                    "resize",
                    receiver=binding(image_step),
                    arguments={
                        "size": literal([8, 8]),
                        "resample": literal(0),
                    },
                    step_id="setup-resize",
                )
                receiver_step = self.add_step(
                    "PIL.Image.Image",
                    "convert",
                    receiver=binding(resized_step),
                    arguments={"mode": literal("RGB")},
                    step_id="setup-convert",
                )
            elif chain == "p-resize-putalpha-verify":
                image_step = self.ensure_image(mode="P")
                resized_step = self.add_step(
                    "PIL.Image.Image",
                    "resize",
                    receiver=binding(image_step),
                    arguments={
                        "size": literal([8, 8]),
                        "resample": literal(0),
                    },
                    step_id="setup-resize",
                )
                self.add_step(
                    "PIL.Image.Image",
                    "putalpha",
                    receiver=binding(resized_step),
                    arguments={"alpha": literal(192)},
                    step_id="setup-putalpha",
                )
                receiver_step = resized_step
            elif chain == "quantize-palette":
                image_step = self.ensure_image(mode="RGB")
                palette_step = self.ensure_image(mode="P", label="palette")
                self.add_step(
                    "PIL.Image.Image",
                    "putpalette",
                    receiver=binding(palette_step),
                    arguments={
                        "data": literal([0, 0, 0, 255, 0, 0]),
                        "rawmode": literal("RGB"),
                    },
                    step_id="setup-quantize-palette",
                )
                self.scenario_values["palette"] = binding(palette_step)
                receiver_step = image_step
            elif chain == "quantize-palette-empty":
                image_step = self.ensure_image(mode="RGB")
                palette_step = self.ensure_image(mode="P", label="palette")
                # Image.new("P", ...) is a valid palette argument even before
                # putpalette attaches entries. Keep this public edge separate
                # so the target must preserve Pillow's empty-palette result.
                self.scenario_values["palette"] = binding(palette_step)
                receiver_step = image_step
            elif chain == "quantize-palette-unsupported-source":
                image_step = self.ensure_image(mode="RGBA")
                palette_step = self.ensure_image(mode="P", label="palette")
                self.add_step(
                    "PIL.Image.Image",
                    "putpalette",
                    receiver=binding(palette_step),
                    arguments={
                        "data": literal([0, 0, 0, 255, 0, 0]),
                        "rawmode": literal("RGB"),
                    },
                    step_id="setup-quantize-palette-unsupported-source",
                )
                self.scenario_values["palette"] = binding(palette_step)
                receiver_step = image_step
            elif chain in {
                "quantize-load",
                "quantize-save",
                "quantize-verify",
            }:
                image_step = self.ensure_image(mode="RGB")
                receiver_step = self.add_step(
                    "PIL.Image.Image",
                    "quantize",
                    receiver=binding(image_step),
                    arguments={
                        "colors": literal(4),
                        "method": literal(0),
                        "kmeans": literal(0),
                    },
                    step_id="setup-quantize",
                )
            elif chain == "p-resize-putalpha-load":
                image_step = self.ensure_image(mode="P")
                resized_step = self.add_step(
                    "PIL.Image.Image",
                    "resize",
                    receiver=binding(image_step),
                    arguments={
                        "size": literal([8, 8]),
                        "resample": literal(0),
                    },
                    step_id="setup-resize",
                )
                self.add_step(
                    "PIL.Image.Image",
                    "putalpha",
                    receiver=binding(resized_step),
                    arguments={"alpha": literal(192)},
                    step_id="setup-putalpha",
                )
                receiver_step = resized_step
            elif chain == "p-resize-getchannel":
                image_step = self.ensure_image(mode="P")
                receiver_step = self.add_step(
                    "PIL.Image.Image",
                    "resize",
                    receiver=binding(image_step),
                    arguments={
                        "size": literal([8, 8]),
                        "resample": literal(0),
                    },
                    step_id="setup-resize",
                )
            elif chain == "p-resize-resize":
                image_step = self.ensure_image(mode="P")
                receiver_step = self.add_step(
                    "PIL.Image.Image",
                    "resize",
                    receiver=binding(image_step),
                    arguments={
                        "size": literal([8, 8]),
                        "resample": literal(0),
                    },
                    step_id="setup-resize",
                )
            elif chain == "resize-crop":
                image_step = self.ensure_image(mode="RGB")
                receiver_step = self.add_step(
                    "PIL.Image.Image",
                    "resize",
                    receiver=binding(image_step),
                    arguments={
                        "size": literal([8, 8]),
                        "resample": literal(0),
                    },
                    step_id="setup-resize",
                )
            elif chain == "resize-copy":
                image_step = self.ensure_image(mode="RGB")
                resized_step = self.add_step(
                    "PIL.Image.Image",
                    "resize",
                    receiver=binding(image_step),
                    arguments={
                        "size": literal([8, 8]),
                        "resample": literal(0),
                    },
                    step_id="setup-resize",
                )
                receiver_step = resized_step
            elif chain == "p-putalpha-convert":
                image_step = self.ensure_image(mode="P")
                self.add_step(
                    "PIL.Image.Image",
                    "putalpha",
                    receiver=binding(image_step),
                    arguments={"alpha": literal(192)},
                    step_id="setup-putalpha",
                )
                receiver_step = image_step
            elif chain == "p-putpalette-putalpha-convert":
                image_step = self.ensure_image(mode="P")
                self.add_step(
                    "PIL.Image.Image",
                    "putpalette",
                    receiver=binding(image_step),
                    arguments={
                        "data": literal([10, 20, 30, 40, 50, 60]),
                        "rawmode": literal("RGB"),
                    },
                    step_id="setup-putpalette",
                )
                self.add_step(
                    "PIL.Image.Image",
                    "putalpha",
                    receiver=binding(image_step),
                    arguments={"alpha": literal(192)},
                    step_id="setup-putalpha",
                )
                receiver_step = image_step
            elif chain == "pa-putpalette-convert":
                image_step = self.ensure_image(mode="PA")
                self.add_step(
                    "PIL.Image.Image",
                    "putpalette",
                    receiver=binding(image_step),
                    arguments={
                        "data": literal([10, 20, 30, 40, 50, 60]),
                        "rawmode": literal("RGB"),
                    },
                    step_id="setup-pa-putpalette",
                )
                receiver_step = image_step
            elif chain == "p-putpalette-putalpha-resize":
                image_step = self.ensure_image(mode="P")
                self.add_step(
                    "PIL.Image.Image",
                    "putpalette",
                    receiver=binding(image_step),
                    arguments={
                        "data": literal([10, 20, 30, 40, 50, 60]),
                        "rawmode": literal("RGB"),
                    },
                    step_id="setup-putpalette",
                )
                self.add_step(
                    "PIL.Image.Image",
                    "putalpha",
                    receiver=binding(image_step),
                    arguments={"alpha": literal(192)},
                    step_id="setup-putalpha",
                )
                receiver_step = image_step
            elif chain == "p-putpalette-remap":
                image_step = self.ensure_image(mode="P")
                self.add_step(
                    "PIL.Image.Image",
                    "putpalette",
                    receiver=binding(image_step),
                    arguments={
                        "data": literal(
                            [10, 20, 30, 128, 40, 50, 60, 255]
                        ),
                        "rawmode": literal("RGBA"),
                    },
                    step_id="setup-putpalette",
                )
                receiver_step = image_step
            elif chain == "p-table-transparency":
                image_asset = self.inline_bytes(
                    "p-table-transparency",
                    indexed_png_with_palette_alpha(),
                    "image/png",
                )
                receiver_step = self.add_step(
                    "PIL.Image",
                    "open",
                    receiver=None,
                    arguments={"fp": image_asset},
                    step_id="setup-table-transparency",
                )
            elif chain == "p-duplicate-transparency":
                image_asset = self.inline_bytes(
                    "p-duplicate-transparency",
                    indexed_png_with_duplicate_transparent_indices(),
                    "image/png",
                )
                receiver_step = self.add_step(
                    "PIL.Image",
                    "open",
                    receiver=None,
                    arguments={"fp": image_asset},
                    step_id="setup-duplicate-transparency",
                )
            elif chain in {
                "p-full-palette-index-transparency-putpixel",
                "p-full-palette-index-transparency-apply",
            }:
                image_asset = self.inline_bytes(
                    "p-full-palette-index-transparency",
                    indexed_png_with_full_palette_index_alpha(),
                    "image/png",
                )
                receiver_step = self.add_step(
                    "PIL.Image",
                    "open",
                    receiver=None,
                    arguments={"fp": image_asset},
                    step_id="setup-full-palette-index-transparency",
                )
            elif chain == "p-transparency-putalpha-apply":
                image_step = self.ensure_image(mode="P")
                self.add_step(
                    "PIL.Image.Image",
                    "putalpha",
                    receiver=binding(image_step),
                    arguments={"alpha": literal(192)},
                    step_id="setup-transparency-putalpha",
                )
                receiver_step = image_step
            elif chain == "p-transparency-resize-apply":
                image_step = self.ensure_image(mode="P")
                receiver_step = self.add_step(
                    "PIL.Image.Image",
                    "resize",
                    receiver=binding(image_step),
                    arguments={
                        "size": literal([2, 2]),
                        "resample": literal(0),
                    },
                    step_id="setup-transparency-resize",
                )
            elif chain == "p-transparency-resize-bilinear-apply":
                image_step = self.ensure_image(mode="P")
                receiver_step = self.add_step(
                    "PIL.Image.Image",
                    "resize",
                    receiver=binding(image_step),
                    arguments={
                        "size": literal([2, 2]),
                        "resample": literal(2),
                    },
                    step_id="setup-transparency-resize-bilinear",
                )
            elif chain == "p-transparency-load-apply":
                image_step = self.ensure_image(mode="P")
                self.add_step(
                    "PIL.Image.Image",
                    "load",
                    receiver=binding(image_step),
                    arguments={},
                    step_id="setup-transparency-load",
                )
                receiver_step = image_step
            elif chain in {
                "p-transparency-putpalette-apply",
                "p-transparency-putpalette-short",
            }:
                image_step = self.ensure_image(mode="P")
                self.add_step(
                    "PIL.Image.Image",
                    "putpalette",
                    receiver=binding(image_step),
                    arguments={
                        "data": literal(
                            [10, 20, 30]
                            if chain == "p-transparency-putpalette-short"
                            else [10, 20, 30, 40, 50, 60]
                        ),
                        "rawmode": literal("RGB"),
                    },
                    step_id="setup-transparency-putpalette",
                )
                receiver_step = image_step
            elif chain == "opened-p-load-getpalette":
                image_step = self.ensure_image(mode="P")
                self.add_step(
                    "PIL.Image.Image",
                    "load",
                    receiver=binding(image_step),
                    arguments={},
                    step_id="setup-opened-p-load",
                )
                receiver_step = image_step
            elif chain == "opened-p-load-save":
                image_step = self.ensure_image(mode="P")
                self.add_step(
                    "PIL.Image.Image",
                    "load",
                    receiver=binding(image_step),
                    arguments={},
                    step_id="setup-opened-p-load",
                )
                receiver_step = image_step
            elif chain == "opened-p-putpalette-getpalette":
                image_step = self.ensure_image(mode="P")
                self.add_step(
                    "PIL.Image.Image",
                    "putpalette",
                    receiver=binding(image_step),
                    arguments={
                        "data": literal([10, 20, 30, 40, 50, 60]),
                        "rawmode": literal("RGB"),
                    },
                    step_id="setup-opened-p-putpalette",
                )
                receiver_step = image_step
            elif chain == "p-short-palette-save":
                image_step = self.ensure_image(mode="P")
                self.add_step(
                    "PIL.Image.Image",
                    "putpalette",
                    receiver=binding(image_step),
                    arguments={
                        "data": literal([1, 2, 3, 4, 5, 6]),
                        "rawmode": literal("RGB"),
                    },
                    step_id="setup-putpalette",
                )
                receiver_step = image_step
            elif chain == "p-short-palette-resize-save":
                image_step = self.ensure_image(mode="P")
                self.add_step(
                    "PIL.Image.Image",
                    "putpalette",
                    receiver=binding(image_step),
                    arguments={
                        "data": literal([1, 2, 3, 4, 5, 6]),
                        "rawmode": literal("RGB"),
                    },
                    step_id="setup-putpalette",
                )
                receiver_step = self.add_step(
                    "PIL.Image.Image",
                    "resize",
                    receiver=binding(image_step),
                    arguments={
                        "size": literal([8, 8]),
                        "resample": literal(0),
                    },
                    step_id="setup-resize",
                )
            elif chain == "p-pipeline-paste":
                destination_step = self.ensure_image(mode="RGB")
                source_step = self.ensure_image(mode="P", label="source")
                flipped_step = self.add_step(
                    "PIL.ImageOps",
                    "flip",
                    receiver=None,
                    arguments={"image": binding(source_step)},
                    step_id="setup-pipeline-flip",
                )
                self.scenario_values["im"] = binding(flipped_step)
                receiver_step = destination_step
            elif chain == "p-putpalette-pipeline-paste":
                destination_step = self.ensure_image(mode="RGB")
                source_step = self.ensure_image(mode="P", label="source")
                self.add_step(
                    "PIL.Image.Image",
                    "putpalette",
                    receiver=binding(source_step),
                    arguments={
                        "data": literal([10, 20, 30, 40, 50, 60]),
                        "rawmode": literal("RGB"),
                    },
                    step_id="setup-source-putpalette",
                )
                flipped_step = self.add_step(
                    "PIL.ImageOps",
                    "flip",
                    receiver=None,
                    arguments={"image": binding(source_step)},
                    step_id="setup-pipeline-flip",
                )
                self.scenario_values["im"] = binding(flipped_step)
                receiver_step = destination_step
            elif chain == "paste-box-image-mask-conflict":
                destination_step = self.ensure_image(mode="RGB")
                source_step = self.ensure_image(mode="RGB", label="im")
                box_step = self.ensure_image(mode="L", label="box")
                mask_step = self.ensure_image(mode="L", label="mask")
                self.scenario_values["im"] = binding(source_step)
                self.scenario_values["box"] = binding(box_step)
                self.scenario_values["mask"] = binding(mask_step)
                receiver_step = destination_step
            elif chain == "rgba-destination-paste":
                receiver_step = self.ensure_image(mode="RGBa")
            elif chain == "p-invert-save":
                image_step = self.ensure_image(mode="P")
                receiver_step = self.add_step(
                    "PIL.ImageChops",
                    "invert",
                    receiver=None,
                    arguments={"image": binding(image_step)},
                    step_id="setup-chops-invert",
                )
            elif chain == "p-full-palette-putpixel":
                image_step = self.ensure_image(mode="P")
                self.add_step(
                    "PIL.Image.Image",
                    "putpalette",
                    receiver=binding(image_step),
                    arguments={
                        "data": literal(
                            [component for index in range(256) for component in (index, index, index)]
                        ),
                        "rawmode": literal("RGB"),
                    },
                    step_id="setup-putpalette",
                )
                receiver_step = image_step
            elif chain in {
                "p-attached-palette-putpixel",
                "p-attached-palette-bitmap",
                "p-attached-palette-text",
            }:
                image_step = self.ensure_image(mode="P")
                self.add_step(
                    "PIL.Image.Image",
                    "putpalette",
                    receiver=binding(image_step),
                    arguments={
                        "data": literal([10, 20, 30, 40, 50, 60]),
                        "rawmode": literal("RGB"),
                    },
                    step_id="setup-putpalette",
                )
                if self.primary_surface == "PIL.ImageDraw.ImageDraw":
                    receiver_step = self.add_step(
                        "PIL.ImageDraw",
                        "Draw",
                        receiver=None,
                        arguments={"im": binding(image_step)},
                        step_id="setup-draw",
                    )
                    observation_step = image_step
                else:
                    receiver_step = image_step
            elif chain == "p-bitmap-putpixel":
                image_step = self.ensure_image(mode="P")
                draw_step = self.add_step(
                    "PIL.ImageDraw",
                    "Draw",
                    receiver=None,
                    arguments={"im": binding(image_step)},
                    step_id="setup-draw",
                )
                bitmap_step = self.ensure_image(mode="L", label="bitmap")
                self.add_step(
                    "PIL.ImageDraw.ImageDraw",
                    "bitmap",
                    receiver=binding(draw_step),
                    arguments={
                        "xy": literal([0, 0]),
                        "bitmap": binding(bitmap_step),
                        "fill": literal(7),
                    },
                    step_id="setup-bitmap",
                )
                receiver_step = image_step
            elif chain == "p-full-palette-exhausted-putpixel":
                image_step = self.ensure_image(mode="P")
                self.add_step(
                    "PIL.Image.Image",
                    "putpalette",
                    receiver=binding(image_step),
                    arguments={
                        "data": literal(
                            [
                                component
                                for index in range(256)
                                for component in (index, index, index)
                            ]
                        ),
                        "rawmode": literal("RGB"),
                    },
                    step_id="setup-putpalette",
                )
                self.add_step(
                    "PIL.Image.Image",
                    "putdata",
                    receiver=binding(image_step),
                    arguments={"data": literal(list(range(256)))},
                    step_id="setup-putdata",
                )
                receiver_step = image_step
            elif chain == "p-mode-filter":
                image_step = self.ensure_image(mode="P")
                filter_step = self.add_step(
                    "PIL.ImageFilter",
                    "ModeFilter",
                    receiver=None,
                    arguments={"size": literal(3)},
                    step_id="setup-filter-mode",
                )
                self.scenario_values["filter"] = binding(filter_step)
                receiver_step = image_step
            elif chain in {
                "palette-getpalette-rgbx",
                "palette-getpalette-channel",
                "palette-getpalette-channel-g",
                "palette-getpalette-channel-b",
                "palette-getpalette-channel-invalid",
                "palette-getpalette-alpha-rgbx",
                "palette-transparency-convert",
            }:
                image_step = self.ensure_image(mode="P")
                if chain in {
                    "palette-getpalette-alpha-rgbx",
                    "palette-transparency-convert",
                }:
                    palette_data = [10, 20, 30, 128, 40, 50, 60, 255]
                    palette_rawmode = "RGBA"
                else:
                    palette_data = [10, 20, 30, 40, 50, 60]
                    palette_rawmode = "RGB"
                self.add_step(
                    "PIL.Image.Image",
                    "putpalette",
                    receiver=binding(image_step),
                    arguments={
                        "data": literal(palette_data),
                        "rawmode": literal(palette_rawmode),
                    },
                    step_id="setup-putpalette",
                )
                receiver_step = image_step
            else:
                raise ValueError(f"unknown scenario chain: {chain}")

            # ImageOps operations are module-level functions.  Their chained
            # setup still keeps the image in the ``image`` argument, but the
            # final call must not turn that image into a method receiver.
            call_receiver = (
                None
                if self.primary_surface == "PIL.ImageOps"
                else binding(receiver_step)
            )
            call_id = self.add_step(
                self.primary_surface,
                self.primary_operation,
                receiver=call_receiver,
                arguments=self.primary_arguments(operation),
                step_id="call",
            )
            observations = [call_id]
            if self.scenario_observe_receiver:
                observation_operation = (
                    "convert"
                    if self.primary_operation == "apply_transparency"
                    else "tobytes"
                )
                observation_arguments = (
                    {"mode": literal("RGBA")}
                    if observation_operation == "convert"
                    else {}
                )
                observations.append(
                    self.add_step(
                        "PIL.Image.Image",
                        observation_operation,
                        receiver=binding(observation_step or receiver_step),
                        arguments=observation_arguments,
                        step_id="observe-receiver",
                    )
                )
            return self.assets, self.steps, observations

        receiver = self.receiver_for(self.primary_surface)
        if (
            self.primary_surface == "PIL.ImageFilter.Color3DLUT"
            and self.primary_operation == "generate"
        ):
            # ``generate`` is a public classmethod; call it on the class
            # rather than constructing an unnecessary receiver instance.
            receiver = None
        arguments = self.primary_arguments(operation)

        if (
            self.primary_surface == "PIL.Image"
            and self.primary_operation == "merge"
            and self.edge not in {
                "mode-band-mismatch",
                "invalid-mode",
                "invalid-band-item",
            }
        ):
            # ``Image.merge`` takes a sequence of single-band Image objects.
            # A single binding is useful for error coverage but cannot reach
            # the core merge pipeline, so construct the sequence through the
            # public Image.new endpoint and bind each result explicitly.
            band_count = {
                "L": 1,
                "LA": 2,
                "RGB": 3,
                "RGBA": 4,
                "CMYK": 4,
            }.get(self.mode)
            if band_count is None:
                raise ValueError(f"merge workflow does not support mode {self.mode}")
            if self.edge == "wrong-band-count":
                band_count -= 1
            band_steps = []
            for index in range(band_count):
                band_steps.append(
                    self.add_step(
                        "PIL.Image",
                        "new",
                        receiver=None,
                        arguments={
                            "mode": literal("L"),
                            "size": literal(self.scenario_size or [16, 16]),
                            "color": literal(
                                17
                                if self.edge == "merge-rgb-nonzero"
                                else 0
                            ),
                        },
                        step_id=f"setup-band-{index + 1}",
                    )
                )
            arguments["mode"] = literal(self.mode)
            arguments["bands"] = bindings(band_steps)
        elif (
            self.primary_surface == "PIL.Image"
            and self.primary_operation == "merge"
            and self.edge == "invalid-band-item"
        ):
            # Preserve valid arity so the public input reaches the core's
            # invalid-item validation instead of stopping at shape checks.
            arguments["mode"] = literal(self.mode)
            arguments["bands"] = literal([None])
        call_id = self.add_step(
            self.primary_surface,
            self.primary_operation,
            receiver=receiver,
            arguments=arguments,
            step_id="call",
        )
        observations = [call_id]

        iterator_im = self.scenario_values.get("im")
        invalid_iterator_image = (
            iterator_im is not None
            and iterator_im.get("kind") == "literal"
            and iterator_im.get("value") is None
        )
        if (
            self.primary_surface == "PIL.ImageSequence"
            and self.primary_operation == "Iterator"
            and not invalid_iterator_image
        ):
            # The constructor itself is only a handle allocation. Seek the
            # public image receiver once so this workflow exercises the
            # frame-0 path without depending on a private iterator protocol.
            observations.append(
                self.add_step(
                    "PIL.Image.Image",
                    "seek",
                    receiver=binding(self.ensure_image()),
                    arguments={"frame": literal(0)},
                    step_id="observe-frame-zero",
                )
            )

        if (
            self.primary_surface == "PIL.ImageSequence.Iterator"
            and self.primary_operation == "__next__"
        ):
            # A real iterator workflow must compare both the first-frame
            # result and the public StopIteration boundary. The second call
            # is input-only parity evidence, not a unit-test assertion.
            observations.append(
                self.add_step(
                    self.primary_surface,
                    "__next__",
                    receiver=receiver,
                    arguments={},
                    step_id="observe-stop-iteration",
                )
            )

        if self.scenario_observe_receiver:
            if (
                self.primary_surface == "PIL.ImageFont.FreeTypeFont"
                and self.primary_operation in {
                    "set_variation_by_axes",
                    "set_variation_by_name",
                }
            ):
                # Variation setters return None. Observe the same public font
                # through getlength so the case verifies the applied
                # coordinates rather than only the setter's signature.
                observations.append(
                    self.add_step(
                        "PIL.ImageFont.FreeTypeFont",
                        "getlength",
                        receiver=receiver,
                        arguments={"text": literal("AV")},
                        step_id="observe-receiver",
                    )
                )
            else:
                # Mutating Image.Image methods return None. Observe the
                # original receiver through a public image endpoint so their
                # deferred writes are exercised by behavioral parity cases.
                observations.append(
                    self.add_step(
                        "PIL.Image.Image",
                        "tobytes",
                        receiver=binding(self.ensure_image()),
                        arguments={},
                        step_id="observe-receiver",
                    )
                )

        if self.scenario_observe_stat_properties:
            # A list passed to ImageStat.Stat is a public precomputed
            # histogram, so observing the properties is necessary to make
            # this an actual behavioral parity workflow rather than a
            # constructor-only handle check.
            for property_name in (
                "count",
                "sum",
                "sum2",
                "mean",
                "median",
                "rms",
                "var",
                "stddev",
                "extrema",
            ):
                observations.append(
                    self.add_step(
                        "PIL.ImageStat.Stat",
                        property_name,
                        receiver=binding(call_id),
                        arguments={},
                        step_id=f"observe-{slug(property_name)}",
                    )
                )

        if self.scenario_observe_result is not None:
            # Observe a returned public object through the declared public
            # operation. ``getdata`` uses ``bytes(ImagingCore)`` here; other
            # operations materialize returned images through an Image method.
            observations.append(
                self.add_step(
                    "PIL.Image.Image",
                    self.scenario_observe_result,
                    receiver=binding(call_id),
                    arguments={},
                    step_id="observe-result",
                )
            )

        if (
            self.primary_surface == "PIL.ImageDraw.ImageDraw"
            and (
                self.primary_operation == "shape"
                or (
                    self.primary_operation == "text"
                    and self.scenario_font is not None
                )
            )
        ):
            # ``shape`` mutates the receiver's image and returns None. Observe
            # the image bytes through the public tobytes endpoint so these
            # cases are behavioral parity evidence rather than signature-only
            # cases.
            image_step = self.ensure_image()
            observations.append(
                self.add_step(
                    "PIL.Image.Image",
                    "tobytes",
                    receiver=binding(image_step),
                    arguments={},
                    step_id="observe-image",
                )
            )

        if self.primary_surface == "PIL.ImageFilter" and operation["kind"] == "type":
            image_step = self.ensure_image()
            filtered = self.add_step(
                "PIL.Image.Image",
                "filter",
                receiver=binding(image_step),
                arguments={"filter": binding(call_id)},
                step_id="apply-filter",
            )
            observations.append(filtered)

        if self.primary_surface == "PIL.ImageEnhance" and operation["kind"] == "type":
            method_surface = f"PIL.ImageEnhance.{self.primary_operation}"
            method_operation = self.operations.get(
                operation_key(method_surface, "enhance")
            )
            if method_operation is not None:
                image_step = self.ensure_image()
                enhance_args = self.required_arguments(method_operation)
                enhanced = self.add_step(
                    method_surface,
                    "enhance",
                    receiver=binding(call_id),
                    arguments=enhance_args,
                    step_id="apply-enhance",
                )
                observations.append(enhanced)

        return self.assets, self.steps, observations

def build_parity_case(
    surface: str,
    operation: dict[str, Any],
    requirement: dict[str, Any],
    operations: dict[tuple[str, str], dict[str, Any]],
    assets_root: Path,
    *,
    case_id: str | None = None,
    scenario_values: dict[str, dict[str, Any]] | None = None,
    scenario_mode: str | None = None,
    scenario_draw_mode: str | None = None,
    scenario_edge: str | None = None,
    scenario_pixel: Any | None = None,
    scenario_font: str | None = None,
    scenario_font_size: float | None = None,
    scenario_transposed_orientation: Any | None = None,
    scenario_bitmap_mode: str | None = None,
    scenario_bitmap_color: Any | None = None,
    scenario_size: list[int] | None = None,
    scenario_im_mode: str | None = None,
    scenario_mask_mode: str | None = None,
    scenario_asset: str | None = None,
    scenario_inline_image: str | None = None,
    scenario_inline_mask_image: str | None = None,
    scenario_exif_variant: str | None = None,
    scenario_noise_seed: int | None = None,
    scenario_chain: str | None = None,
    scenario_observe_result: str | None = None,
    scenario_observe_receiver: bool = False,
    scenario_observe_stat_properties: bool = False,
    scenario_outline_curve: bool = False,
    scenario_outline_empty: bool = False,
) -> dict[str, Any]:
    prefix = operation_prefix(surface, operation["id"])
    suffix = requirement["id"].removeprefix(prefix + ".")
    canonical_case_id = f"{prefix}.{slug(suffix)}"
    if (
        surface == "PIL.ImageOps"
        and operation["id"] == "exif_transpose"
        and "jpeg-exif" in requirement["id"]
    ):
        # The EXIF orientation branches are only reachable when the workflow
        # opens a container that actually carries an EXIF payload, so the
        # canonical input-family cases open the committed JPEG fixture.
        scenario_asset = scenario_asset or "image/exif-orientation6.jpg"
        if requirement["id"].endswith("in-place"):
            scenario_values = {
                **(scenario_values or {}),
                "in_place": literal(True),
            }
    builder = WorkflowBuilder(
        operations=operations,
        primary_surface=surface,
        primary_operation=operation["id"],
        requirement=requirement,
        assets_root=assets_root,
        scenario_values=scenario_values or {},
        scenario_mode=scenario_mode,
        scenario_draw_mode=scenario_draw_mode,
        scenario_edge=scenario_edge,
        scenario_pixel=scenario_pixel,
        scenario_font=scenario_font,
        scenario_font_size=scenario_font_size,
        scenario_transposed_orientation=scenario_transposed_orientation,
        scenario_bitmap_mode=scenario_bitmap_mode,
        scenario_bitmap_color=scenario_bitmap_color,
        scenario_size=scenario_size,
        scenario_im_mode=scenario_im_mode,
        scenario_mask_mode=scenario_mask_mode,
        scenario_asset=scenario_asset,
        scenario_inline_image=scenario_inline_image,
        scenario_inline_mask_image=scenario_inline_mask_image,
        scenario_exif_variant=scenario_exif_variant,
        scenario_noise_seed=scenario_noise_seed,
        scenario_chain=scenario_chain,
        scenario_observe_result=scenario_observe_result,
        scenario_observe_receiver=scenario_observe_receiver,
        scenario_observe_stat_properties=scenario_observe_stat_properties,
        scenario_outline_curve=scenario_outline_curve,
        scenario_outline_empty=scenario_outline_empty,
    )
    assets, steps, observations = builder.build()
    return {
        "case_id": case_id or canonical_case_id,
        "surface": surface,
        "operation": operation["id"],
        "covers": [requirement["id"]],
        "target_profiles": [TARGET_PROFILE],
        "assets": assets,
        "steps": steps,
        "observations": observations,
    }


def build_nuanced_cases(
    manifest: dict[str, Any],
    operations: dict[tuple[str, str], dict[str, Any]],
    assets_root: Path,
    surface_id: str,
) -> list[dict[str, Any]]:
    """Add a small reviewed set of high-signal interaction stimuli.

    These cases are intentionally not generated once per requirement.  They
    exercise values that broad parameter/default matrices routinely miss:
    Unicode and multiline text, nontrivial geometry, resampling/rotation,
    valid color syntax, and a real filter kernel.  They reuse existing
    requirements; coverage selection remains tied to the canonical case for
    each requirement while parity executes these additional workflows.
    """

    specs: tuple[dict[str, Any], ...] = (
        {
            "surface": "PIL.Image",
            "operation": "fromarray",
            "requirement_suffix": "behavior.default",
            "name": "buffer-backed-rgb-values",
            "values": {
                "obj": literal(
                    {
                        "protocol": "numpy-array",
                        "shape": [2, 2, 3],
                        "typestr": "|u1",
                        "data_base64": "AQIDBAUGBwgJCgsM",
                    }
                )
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "fromarray",
            "requirement_suffix": "parameter.obj",
            "name": "buffer-backed-luma-1d",
            "values": {
                "obj": literal(
                    {
                        "protocol": "numpy-array",
                        "shape": [4],
                        "typestr": "|u1",
                        "data_base64": "AQIDBA==",
                    }
                )
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "fromarray",
            "requirement_suffix": "parameter.obj",
            "name": "unsupported-dtype",
            "values": {
                "obj": literal(
                    {
                        "protocol": "numpy-array",
                        "shape": [2, 2],
                        "typestr": "|u8",
                        "data_base64": base64.b64encode(bytes(32)).decode("ascii"),
                    }
                )
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "fromarray",
            "requirement_suffix": "parameter.obj",
            "name": "scalar-array-empty-shape",
            "values": {
                "obj": literal(
                    {
                        "protocol": "numpy-array",
                        "shape": [],
                        "typestr": "|u1",
                        "data_base64": "AQ==",
                    }
                )
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "fromarray",
            "requirement_suffix": "parameter.obj",
            "name": "bytes-object-rejected",
            "values": {"obj": bytes_literal([0, 1, 2, 3])},
        },
        {
            "surface": "PIL.Image",
            "operation": "fromarray",
            "requirement_suffix": "parameter.obj",
            "name": "height-overflow",
            "values": {
                "obj": literal(
                    {
                        "protocol": "buffered-array-interface",
                        "shape": [4_294_967_296],
                        "typestr": "|u1",
                        "data_base64": "AA==",
                    }
                )
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "fromarray",
            "requirement_suffix": "parameter.obj",
            "name": "width-overflow",
            "values": {
                "obj": literal(
                    {
                        "protocol": "buffered-array-interface",
                        "shape": [1, 4_294_967_296],
                        "typestr": "|u1",
                        "data_base64": "AA==",
                    }
                )
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "point",
            "requirement_suffix": "mode.l",
            "name": "l-replicated-lut",
            "mode": "L",
            "edge": "nonzero-pixel",
            "pixel": 200,
            "observe_result": "tobytes",
            "values": {"lut": literal(list(range(256)))},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "point",
            "requirement_suffix": "mode.rgb",
            "name": "rgb-replicated-lut",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [200, 100, 50],
            "observe_result": "tobytes",
            "values": {"lut": literal(list(range(256)))},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "point",
            "requirement_suffix": "parameter.lut",
            "name": "rgb-expanded-lut",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [200, 100, 50],
            "observe_result": "tobytes",
            "values": {
                "lut": literal([index % 256 for index in range(768)]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "point",
            "requirement_suffix": "mode.la",
            "name": "la-replicated-lut",
            "mode": "LA",
            "edge": "nonzero-pixel",
            "pixel": [200, 128],
            "observe_result": "tobytes",
            "values": {"lut": literal(list(range(256)))},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "point",
            "requirement_suffix": "parameter.lut",
            "name": "rgb-callable-lut",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [200, 100, 50],
            "observe_result": "tobytes",
            "values": {"lut": literal(["clamp-shift-callable"])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "point",
            "requirement_suffix": "parameter.lut",
            "name": "rgb-invalid-lut-length",
            "mode": "RGB",
            "values": {"lut": literal([0])},
        },
        {
            "surface": "PIL.ImageSequence",
            "operation": "Iterator",
            "requirement_suffix": "behavior.default",
            "name": "seek-frame-zero",
            "mode": "L",
        },
        {
            "surface": "PIL.ImageSequence",
            "operation": "Iterator",
            "requirement_suffix": "behavior.default",
            "name": "mode1-frame-zero",
            "mode": "1",
        },
        {
            "surface": "PIL.ImageSequence.Iterator",
            "operation": "__next__",
            "requirement_suffix": "behavior.default",
            "name": "opened-single-frame-gif",
            "scenario_asset": "image/p-small.gif",
        },
        {
            "surface": "PIL.ImageSequence",
            "operation": "Iterator",
            "requirement_suffix": "parameter.im",
            "name": "invalid-im-no-seek",
            "values": {"im": literal(None)},
        },
        {
            "surface": "PIL.ImageFont.ImageFont",
            "operation": "getmask",
            "requirement_suffix": "behavior.default",
            "name": "loaded-pilfont-mode1-mask",
            "chain": "pilfont-load-getmask",
        },
        {
            "surface": "PIL.ImageFont.ImageFont",
            "operation": "getmask",
            "requirement_suffix": "behavior.default",
            "name": "loaded-pilfont-luma-mask",
            "chain": "pilfont-load-getmask",
            "scenario_asset": "font/pilfont/courb08_l.pil",
        },
        {
            "surface": "PIL.ImageFont.ImageFont",
            "operation": "getmask",
            "requirement_suffix": "behavior.default",
            "name": "loaded-pilfont-luma-empty-mask",
            "chain": "pilfont-load-getmask",
            "scenario_asset": "font/pilfont/courb08_l.pil",
            "values": {"text": literal("")},
        },
        {
            "surface": "PIL.ImageFont.ImageFont",
            "operation": "getmask",
            "requirement_suffix": "behavior.default",
            "name": "loaded-pilfont-nonlatin-error",
            "chain": "pilfont-load-getmask",
            "values": {"text": literal("🙂")},
        },
        {
            "surface": "PIL.ImageFont.ImageFont",
            "operation": "getmask",
            "requirement_suffix": "behavior.default",
            "name": "loaded-pilfont-bmp-nonlatin-error",
            "chain": "pilfont-load-getmask",
            "values": {"text": literal("☃")},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getbbox",
            "requirement_suffix": "parameter.text",
            "name": "unicode-multiline",
            "values": {"text": literal("A\u0301\nAV🙂")},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getbbox",
            "requirement_suffix": "parameter.text",
            "name": "bytes-latin1",
            "values": {"text": bytes_literal([65, 233])},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getbbox",
            "requirement_suffix": "parameter.anchor",
            "name": "bytes-latin1-anchor",
            "values": {
                "text": bytes_literal([65, 233]),
                "anchor": literal("la"),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getbbox",
            "requirement_suffix": "parameter.text",
            "name": "empty-default-route",
            "values": {"text": literal("")},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "font_variant",
            "requirement_suffix": "parameter.encoding",
            "name": "encoding-unicode-charmap",
            "values": {"encoding": literal("unic")},
        },
        {
            "surface": "PIL.ImageFont",
            "operation": "truetype",
            "requirement_suffix": "parameter.encoding",
            "name": "encoding-unicode-charmap",
            "values": {"encoding": literal("unic")},
        },
        {
            "surface": "PIL.ImageFont",
            "operation": "truetype",
            "requirement_suffix": "parameter.encoding",
            "name": "encoding-symbol-charmap",
            "values": {"encoding": literal("symb")},
        },
        {
            "surface": "PIL.ImageFont",
            "operation": "truetype",
            "requirement_suffix": "parameter.encoding",
            "name": "encoding-adobe-latin1-charmap",
            "values": {"encoding": literal("lat1")},
        },
        {
            "surface": "PIL.ImageFont",
            "operation": "truetype",
            "requirement_suffix": "parameter.encoding",
            "name": "encoding-unknown-charmap",
            "values": {"encoding": literal("unknown")},
        },
        {
            "surface": "PIL.Image",
            "operation": "open",
            "requirement_suffix": "parameter.formats",
            "name": "formats-accepted",
            "values": {"formats": literal(["PNG"])},
        },
        {
            "surface": "PIL.Image",
            "operation": "open",
            "requirement_suffix": "parameter.formats",
            "name": "formats-rejected",
            "values": {"formats": literal(["JPEG"])},
        },
        {
            "surface": "PIL.Image",
            "operation": "open",
            "requirement_suffix": "parameter.formats",
            "name": "formats-single-string",
            "values": {"formats": literal("PNG")},
        },
        {
            "surface": "PIL.Image",
            "operation": "open",
            "requirement_suffix": "parameter.mode",
            "name": "read-mode",
            "values": {"mode": literal("r")},
        },
        {
            "surface": "PIL.Image",
            "operation": "open",
            "requirement_suffix": "parameter.mode",
            "name": "invalid-mode",
            "values": {"mode": literal("w")},
        },
        {
            "surface": "PIL.Image",
            "operation": "open",
            "requirement_suffix": "parameter.mode",
            "name": "non-string-mode",
            "values": {"mode": literal(1)},
        },
        {
            "surface": "PIL.Image",
            "operation": "open",
            "requirement_suffix": "parameter.fp",
            "name": "embedded-null-bytes",
            "edge": "embedded-null-bytes",
        },
        {
            "surface": "PIL.Image",
            "operation": "open",
            "requirement_suffix": "parameter.fp",
            "name": "non-null-bytes",
            "edge": "non-null-bytes",
        },
        # Exercise the public ImageEnhance constructor plus enhance() call on
        # the modes rejected by the Rust core. These are behavioral parity
        # workflows, not direct probes: the mode is created through
        # PIL.Image.new and the rejection is observed at the constructor or
        # enhance() boundary, matching Pillow's class-specific timing.
        *(
            {
                "surface": "PIL.ImageEnhance",
                "operation": class_name,
                "requirement_suffix": "behavior.default",
                "name": f"mode-{mode.lower()}-reject",
                "mode": mode,
            }
            for class_name in ("Brightness", "Color", "Contrast", "Sharpness")
            for mode in ("1", "P")
        ),
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getlength",
            "requirement_suffix": "parameter.text",
            "name": "kerning-pair",
            "values": {"text": literal("AV")},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getlength",
            "requirement_suffix": "parameter.text",
            "name": "bytes-latin1",
            "values": {"text": bytes_literal([65, 233])},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getlength",
            "requirement_suffix": "parameter.mode",
            "name": "bytes-latin1-mode",
            "values": {
                "text": bytes_literal([65, 233]),
                "mode": literal("L"),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getlength",
            "requirement_suffix": "parameter.text",
            "name": "empty-default-route",
            "values": {"text": literal("")},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask",
            "requirement_suffix": "parameter.text",
            "name": "empty-default-route",
            "values": {"text": literal("")},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask",
            "requirement_suffix": "parameter.start",
            "name": "fractional-start",
            "values": {
                "text": literal("AV"),
                "start": literal([0.5, 0.75]),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask",
            "requirement_suffix": "parameter.text",
            "name": "bytes-latin1",
            "values": {"text": bytes_literal([65, 233])},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask",
            "requirement_suffix": "parameter.mode",
            "name": "bytes-latin1-mode",
            "values": {
                "text": bytes_literal([65, 233]),
                "mode": literal("L"),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask",
            "requirement_suffix": "parameter.start",
            "name": "bytes-latin1-start",
            "values": {
                "text": bytes_literal([65, 233]),
                "start": literal([0.5, 0.75]),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask",
            "requirement_suffix": "parameter.start",
            "name": "bytes-latin1-start-stroked",
            "values": {
                "text": bytes_literal([65, 233]),
                "stroke_width": literal(1),
                "start": literal([0.5, 0.75]),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask",
            "requirement_suffix": "parameter.text",
            "name": "sbit-gray-private-base",
            "font": "font/fonts/sbit-gray-format1.ttf",
            "values": {"text": literal("\ue000")},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask",
            "requirement_suffix": "parameter.text",
            "name": "sbit-gray2-private-base",
            "font": "font/fonts/sbit-gray2-format1.ttf",
            "values": {"text": literal("\ue000")},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask",
            "requirement_suffix": "parameter.text",
            "name": "sbit-gray4-private-base",
            "font": "font/fonts/sbit-gray4-format1.ttf",
            "values": {"text": literal("\ue000")},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask",
            "requirement_suffix": "parameter.text",
            "name": "sbit-bgra-private-base",
            "font": "font/fonts/sbit-bgra-format1.ttf",
            "values": {"text": literal("\ue000")},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask",
            "requirement_suffix": "parameter.text",
            "name": "sbit-mono-private-base",
            "font": "font/fonts/sbit-mono-format1.ttf",
            "values": {"text": literal("\ue000")},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask",
            "requirement_suffix": "parameter.text",
            "name": "sbit-composite-mono-private-component",
            "font": "font/fonts/sbit-composite-mono-carry-success-format8.ttf",
            "values": {"text": literal("\ue001")},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask",
            "requirement_suffix": "parameter.text",
            "name": "embedded-strike-private-base",
            "font": "font/fonts/embedded-strike-color-or-sbit.ttf",
            "values": {"text": literal("A")},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask",
            "requirement_suffix": "parameter.text",
            "name": "sbit-missing-small-metrics-private-base",
            "font": "font/fonts/sbit-missing-small-metrics-width.ttf",
            "values": {"text": literal("\ue000")},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask",
            "requirement_suffix": "parameter.text",
            "name": "sbit-composite-gray2-private-component",
            "font": "font/fonts/sbit-composite-gray2-success-format8.ttf",
            "values": {"text": literal("\ue001")},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask",
            "requirement_suffix": "parameter.text",
            "name": "sbit-composite-gray4-private-component",
            "font": "font/fonts/sbit-composite-gray4-success-format8.ttf",
            "values": {"text": literal("\ue001")},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask",
            "requirement_suffix": "parameter.text",
            "name": "sbit-composite-bgra-private-component",
            "font": "font/fonts/sbit-composite-bgra-success-format8.ttf",
            "values": {"text": literal("\ue001")},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.text",
            "name": "multiline-stroked",
            "values": {
                "text": literal("A\nV"),
                "stroke_width": literal(1),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.text",
            "name": "empty-stroked",
            "values": {
                "text": literal(""),
                "stroke_width": literal(1),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.text",
            "name": "empty-default-route",
            "values": {"text": literal("")},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.text",
            "name": "oversized-text-error",
            "values": {"text": text_repeat_literal("A", 1_000_001)},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.text",
            "name": "sbit-cbdt-stroked",
            "font": "font/fonts/sbit-cblc-cbdt-gray-format1.ttf",
            "values": {
                "text": literal("\ue000"),
                "stroke_width": literal(1),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.text",
            "name": "space-default-route",
            "values": {"text": literal(" ")},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.text",
            "name": "space-stroked-route",
            "values": {
                "text": literal(" "),
                "stroke_width": literal(1),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.anchor",
            "name": "valid-anchor-route",
            "values": {
                "text": literal("A"),
                "anchor": literal("la"),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.text",
            "name": "bytes-latin1",
            "values": {"text": bytes_literal([65, 233])},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.stroke-width",
            "name": "bytes-latin1-stroked",
            "values": {
                "text": bytes_literal([65, 233]),
                "stroke_width": literal(1),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.start",
            "name": "bytes-latin1-start",
            "values": {
                "text": bytes_literal([65, 233]),
                "start": literal([0.5, 0.75]),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.start",
            "name": "bytes-latin1-start-stroked",
            "values": {
                "text": bytes_literal([65, 233]),
                "stroke_width": literal(1),
                "start": literal([0.5, 0.75]),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.mode",
            "name": "mode-rgba-error",
            "values": {
                "text": literal("A"),
                "mode": literal("RGBA"),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.mode",
            "name": "mode-rgba-embedded-bgra",
            "font": "font/fonts/sbit-bgra-format1.ttf",
            "values": {
                "text": literal("\ue000"),
                "mode": literal("RGBA"),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.mode",
            "name": "mode-rgba-embedded-strike",
            "font": "font/fonts/embedded-strike-color-or-sbit.ttf",
            "values": {
                "text": literal("A"),
                "mode": literal("RGBA"),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.start",
            "name": "mode-rgba-embedded-bgra-start",
            "font": "font/fonts/sbit-bgra-format1.ttf",
            "values": {
                "text": literal("\ue000"),
                "mode": literal("RGBA"),
                "start": literal([0.5, 0.75]),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.start",
            "name": "mode-rgba-embedded-strike-clipped",
            "font": "font/fonts/embedded-strike-color-or-sbit.ttf",
            "values": {
                "text": literal("A"),
                "mode": literal("RGBA"),
                "start": literal([-1.0, -1.0]),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.start",
            "name": "mode-rgba-embedded-bgra-collapse",
            "font": "font/fonts/sbit-bgra-format1.ttf",
            "values": {
                "text": literal("\ue000"),
                "mode": literal("RGBA"),
                "start": literal([-100.0, -100.0]),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.stroke-width",
            "name": "mode-rgba-embedded-bgra-stroked",
            "font": "font/fonts/sbit-bgra-format1.ttf",
            "values": {
                "text": literal("\ue000"),
                "mode": literal("RGBA"),
                "stroke_width": literal(1),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "get_variation_axes",
            "requirement_suffix": "behavior.default",
            "name": "variable-font",
            "font": "font/fonts/variable-name-platform1-fallback.ttf",
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "get_variation_names",
            "requirement_suffix": "behavior.default",
            "name": "variable-font",
            "font": "font/fonts/variable-name-platform1-fallback.ttf",
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "get_variation_axes",
            "requirement_suffix": "behavior.default",
            "name": "named-instances",
            "font": "font/fonts/variable-named-instances.ttf",
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "get_variation_names",
            "requirement_suffix": "behavior.default",
            "name": "named-instances",
            "font": "font/fonts/variable-named-instances.ttf",
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "get_variation_names",
            "requirement_suffix": "behavior.default",
            "name": "windows-name-fallback",
            "font": "font/fonts/variable-name-windows-fallback.ttf",
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "get_variation_axes",
            "requirement_suffix": "behavior.default",
            "name": "malformed-axis-size",
            "font": "font/fonts/fvar-axis-size-short.ttf",
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "get_variation_names",
            "requirement_suffix": "behavior.default",
            "name": "malformed-instance-array",
            "font": "font/fonts/fvar-instance-array-short.ttf",
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "get_variation_axes",
            "requirement_suffix": "behavior.default",
            "name": "type1-mm",
            "font": "font/fonts/type1-mm-two-axis.pfb",
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "get_variation_names",
            "requirement_suffix": "behavior.default",
            "name": "type1-mm",
            "font": "font/fonts/type1-mm-two-axis.pfb",
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "set_variation_by_axes",
            "requirement_suffix": "behavior.default",
            "name": "type1-mm",
            "font": "font/fonts/type1-mm-two-axis.pfb",
            "values": {"axes": literal([100.0, 400.0])},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "get_variation_axes",
            "requirement_suffix": "behavior.default",
            "name": "non-variable-error",
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getbbox",
            "requirement_suffix": "parameter.direction",
            "name": "unsupported-direction",
            "values": {
                "text": literal("A"),
                "direction": literal("rtl"),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getbbox",
            "requirement_suffix": "parameter.features",
            "name": "unsupported-features",
            "values": {
                "text": literal("A"),
                "features": literal(["liga"]),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getlength",
            "requirement_suffix": "parameter.language",
            "name": "unsupported-language",
            "values": {
                "text": literal("A"),
                "language": literal("en"),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.stroke-width",
            "name": "fractional-stroke",
            "values": {
                "text": literal("AV"),
                "stroke_width": literal(1.5),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.start",
            "name": "fractional-start",
            "values": {
                "text": literal("AV"),
                "start": literal([0.5, 0.75]),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.start",
            "name": "negative-start-clipped",
            "values": {
                "text": literal("A"),
                "start": literal([-1.0, -1.0]),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.start",
            "name": "negative-start-vertical-clipped",
            "values": {
                "text": literal("A"),
                "start": literal([0.0, -10.0]),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.start",
            "name": "zero-height-mask",
            "values": {
                "text": literal("Aj"),
                "start": literal([0.0, -19.0]),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.start",
            "name": "vertical-glyph-clipped",
            "values": {
                "text": literal("Aj"),
                "start": literal([0.0, -18.0]),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.start",
            "name": "negative-start-collapse",
            "values": {
                "text": literal("A"),
                "start": literal([-100.0, -100.0]),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.start",
            "name": "negative-start-height-collapse",
            "values": {
                "text": literal("A"),
                "start": literal([0.0, -100.0]),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.start",
            "name": "negative-start-stroked-collapse",
            "values": {
                "text": literal("A"),
                "stroke_width": literal(1),
                "start": literal([-100.0, -100.0]),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.start",
            "name": "negative-start-stroked-height-collapse",
            "values": {
                "text": literal("A"),
                "stroke_width": literal(1),
                "start": literal([0.0, -100.0]),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getmask2",
            "requirement_suffix": "parameter.mode",
            "name": "mode-one",
            "values": {
                "text": literal("AV"),
                "mode": literal("1"),
            },
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "set_variation_by_axes",
            "requirement_suffix": "behavior.default",
            "name": "variable-font",
            "font": "font/fonts/variable-name-platform1-fallback.ttf",
            "values": {"axes": literal([100.0, 600.0])},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "set_variation_by_axes",
            "requirement_suffix": "behavior.default",
            "name": "non-variable-font",
            "values": {"axes": literal([100.0])},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "set_variation_by_axes",
            "requirement_suffix": "behavior.default",
            "name": "variable-font-positive-axis-overflow",
            "font": "font/fonts/variable-name-platform1-fallback.ttf",
            "values": {"axes": literal([3.4028235e38, 3.4028235e38])},
            "observe_receiver": True,
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "set_variation_by_axes",
            "requirement_suffix": "parameter.axes",
            "name": "variable-font-extra-axis",
            "font": "font/fonts/variable-name-platform1-fallback.ttf",
            "values": {"axes": literal([100.0, 600.0, 100.0])},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "set_variation_by_name",
            "requirement_suffix": "behavior.default",
            "name": "variable-font",
            "font": "font/fonts/variable-name-platform1-fallback.ttf",
            "values": {"name": literal("Bold")},
            "observe_receiver": True,
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "set_variation_by_name",
            "requirement_suffix": "behavior.default",
            "name": "variable-font-unknown-instance",
            "font": "font/fonts/variable-name-platform1-fallback.ttf",
            "values": {"name": literal("DefinitelyMissing")},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "set_variation_by_name",
            "requirement_suffix": "behavior.default",
            "name": "named-instance-thin",
            "font": "font/fonts/variable-named-instances.ttf",
            "values": {"name": literal("Thin")},
            "observe_receiver": True,
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "font_variant",
            "requirement_suffix": "parameter.size",
            "name": "variable-font-size",
            "font": "font/fonts/variable-name-platform1-fallback.ttf",
            "values": {"size": literal(30)},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getbbox",
            "requirement_suffix": "parameter.anchor",
            "name": "invalid-anchor",
            "values": {
                "text": literal("A"),
                "anchor": literal("bad-anchor"),
            },
        },
        {
            "surface": "PIL.ImageFont",
            "operation": "truetype",
            "requirement_suffix": "parameter.size",
            "name": "zero-size",
            "values": {"size": literal(0)},
        },
        {
            "surface": "PIL.ImageFont",
            "operation": "truetype",
            "requirement_suffix": "parameter.font",
            "name": "malformed-cff-table",
            "font": "font/fonts/cff-malformed-short-header.otf",
        },
        {
            "surface": "PIL.ImageFont",
            "operation": "truetype",
            "requirement_suffix": "parameter.font",
            "name": "malformed-cff-name-index",
            "font": "font/fonts/cff-malformed-name-index-offsets-out-of-order.otf",
        },
        {
            "surface": "PIL.ImageFont",
            "operation": "truetype",
            "requirement_suffix": "parameter.size",
            "name": "fractional-size",
            "values": {"size": literal(20.5)},
        },
        {
            "surface": "PIL.ImageFont",
            "operation": "truetype",
            "requirement_suffix": "parameter.size",
            "name": "negative-fractional-size",
            "values": {"size": literal(-5.5)},
        },
        {
            "surface": "PIL.ImageFont",
            "operation": "truetype",
            "requirement_suffix": "parameter.size",
            "name": "oversized-size",
            "values": {"size": literal(50000)},
        },
        {
            "surface": "PIL.ImageFont.TransposedFont",
            "operation": "getmask",
            "requirement_suffix": "behavior.default",
            "name": "rotate-90",
            "orientation": 2,
        },
        {
            "surface": "PIL.ImageFont.TransposedFont",
            "operation": "getbbox",
            "requirement_suffix": "behavior.default",
            "name": "rotate-270",
            "orientation": 4,
        },
        {
            "surface": "PIL.ImageFont.TransposedFont",
            "operation": "getlength",
            "requirement_suffix": "behavior.default",
            "name": "rotate-90-length-error",
            "orientation": 2,
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "parameter.text",
            "name": "unicode-anchor",
            "values": {
                "text": literal("A\u0301🙂"),
                "anchor": literal("mm"),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "parameter.text",
            "name": "bytes-latin1",
            "font": "font/fonts/DejaVuSans.ttf",
            "values": {"text": bytes_literal([65, 233])},
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "explicit-truetype-font",
            "font": "font/fonts/DejaVuSans.ttf",
            "values": {
                "text": literal("Hello"),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "parameter.embedded-color",
            "name": "embedded-bgra-rgb",
            "font": "font/fonts/sbit-bgra-format1.ttf",
            "values": {
                "text": literal("\ue000"),
                "fill": literal([200, 10, 20]),
                "embedded_color": literal(True),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "parameter.embedded-color",
            "name": "embedded-color-l-mode-error",
            "mode": "L",
            "values": {
                "text": literal("Hello"),
                "embedded_color": literal(True),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "parameter.embedded-color",
            "name": "embedded-bgra-rgba",
            "mode": "RGBA",
            "font": "font/fonts/sbit-bgra-format1.ttf",
            "values": {
                "text": literal("\ue000"),
                "fill": literal([200, 10, 20, 200]),
                "embedded_color": literal(True),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "parameter.stroke-width",
            "name": "embedded-bgra-rgba-stroked",
            "mode": "RGBA",
            "font": "font/fonts/sbit-bgra-format1.ttf",
            "values": {
                "text": literal("\ue000"),
                "fill": literal([200, 10, 20, 200]),
                "embedded_color": literal(True),
                "stroke_width": literal(1),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "multiline_text",
            "requirement_suffix": "parameter.text",
            "name": "three-line-spacing",
            "values": {
                "text": literal("A\nB\nC"),
                "spacing": literal(3),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "multiline_text",
            "requirement_suffix": "parameter.text",
            "name": "bytes-latin1",
            "font": "font/fonts/DejaVuSans.ttf",
            "values": {
                "text": bytes_literal([65, 233, 10, 66]),
                "spacing": literal(3),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "multiline_text",
            "requirement_suffix": "parameter.text",
            "name": "empty-line-spacing",
            "values": {
                "text": literal("A\n\nB"),
                "spacing": literal(3),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "multiline_text",
            "requirement_suffix": "parameter.font-size",
            "name": "explicit-font-size",
            "font": "font/fonts/DejaVuSans.ttf",
            "values": {
                "text": literal("Hello\nworld"),
                "font_size": literal(24),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "multiline_textbbox",
            "requirement_suffix": "parameter.text",
            "name": "three-line-spacing",
            "values": {
                "text": literal("A\nBB\nC"),
                "spacing": literal(3),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "multiline_textbbox",
            "requirement_suffix": "parameter.text",
            "name": "bytes-latin1",
            "font": "font/fonts/DejaVuSans.ttf",
            "values": {
                "text": bytes_literal([65, 233, 10, 66]),
                "spacing": literal(3),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "multiline_textbbox",
            "requirement_suffix": "parameter.align",
            "name": "centered-lines",
            "values": {
                "text": literal("A\nBBB\nC"),
                "spacing": literal(2),
                "align": literal("center"),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "multiline_textbbox",
            "requirement_suffix": "parameter.align",
            "name": "right-aligned-lines",
            "values": {
                "text": literal("A\nBBB\nC"),
                "spacing": literal(2),
                "align": literal("right"),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "textbbox",
            "requirement_suffix": "parameter.text",
            "name": "unicode-anchor",
            "values": {
                "text": literal("A\u0301🙂"),
                "anchor": literal("mm"),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "textbbox",
            "requirement_suffix": "parameter.text",
            "name": "bytes-latin1",
            "font": "font/fonts/DejaVuSans.ttf",
            "values": {"text": bytes_literal([65, 233])},
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "textlength",
            "requirement_suffix": "parameter.text",
            "name": "bytes-latin1",
            "font": "font/fonts/DejaVuSans.ttf",
            "values": {"text": bytes_literal([65, 233])},
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-la",
            "mode": "LA",
            "bitmap_mode": "L",
            "bitmap_color": 255,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rectangle",
            "requirement_suffix": "behavior.default",
            "name": "explicit-draw-mode-l-on-rgb",
            "mode": "RGB",
            "draw_mode": "L",
            "observe_receiver": True,
            "values": {
                "xy": literal([2, 2, 8, 8]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rectangle",
            "requirement_suffix": "behavior.default",
            "name": "explicit-draw-mode-rgba-on-rgb",
            "mode": "RGB",
            "draw_mode": "RGBA",
            "observe_receiver": True,
            "values": {
                "xy": literal([2, 2, 8, 8]),
                "fill": literal([255, 0, 0, 128]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-la-invalid-component-count",
            "mode": "LA",
            "bitmap_mode": "L",
            "bitmap_color": 255,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255, 255, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-cmyk",
            "mode": "CMYK",
            "bitmap_mode": "L",
            "bitmap_color": 255,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255, 0, 0, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-p",
            "mode": "P",
            "bitmap_mode": "L",
            "bitmap_color": 255,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-p-tuple-fill",
            "mode": "P",
            "bitmap_mode": "L",
            "bitmap_color": 255,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255, 0, 0]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "raw-p-no-palette-fallback",
            "mode": "P",
            "edge": "raw-p-no-palette",
            "observe_receiver": True,
            "bitmap_mode": "L",
            "bitmap_color": 128,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal(7),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "attached-palette",
            "chain": "p-attached-palette-bitmap",
            "mode": "P",
            "observe_receiver": True,
            "bitmap_mode": "L",
            "bitmap_color": 128,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal(7),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "attached-palette-opaque-mask",
            "chain": "p-attached-palette-bitmap",
            "mode": "P",
            "bitmap_mode": "L",
            "bitmap_color": 255,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal(7),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "attached-palette-zero-mask",
            "chain": "p-attached-palette-bitmap",
            "mode": "P",
            "bitmap_mode": "L",
            "bitmap_color": 0,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal(7),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-rgba-rgba-mask",
            "mode": "RGBA",
            "bitmap_mode": "RGBA",
            "bitmap_color": 255,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255, 0, 0, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-l-one-mask",
            "mode": "L",
            "bitmap_mode": "1",
            "bitmap_color": 1,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-l-zero-one-mask",
            "mode": "L",
            "bitmap_mode": "1",
            "bitmap_color": 0,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-1-one-mask",
            "mode": "1",
            "bitmap_mode": "1",
            "bitmap_color": 1,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-l-l-mask",
            "mode": "L",
            "bitmap_mode": "L",
            "bitmap_color": 255,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-l-zero-mask",
            "mode": "L",
            "bitmap_mode": "L",
            "bitmap_color": 0,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-l-half-mask",
            "mode": "L",
            "bitmap_mode": "L",
            "bitmap_color": 128,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-one-half-mask",
            "mode": "1",
            "bitmap_mode": "L",
            "bitmap_color": 128,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-la-half-mask",
            "mode": "LA",
            "bitmap_mode": "L",
            "bitmap_color": 128,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255, 128]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-la-zero-mask",
            "mode": "LA",
            "bitmap_mode": "L",
            "bitmap_color": 0,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-la-opaque-destination",
            "mode": "LA",
            "edge": "nonzero-pixel",
            "pixel": [0, 255],
            "bitmap_mode": "L",
            "bitmap_color": 128,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-p-half-mask",
            "mode": "P",
            "bitmap_mode": "L",
            "bitmap_color": 128,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-p-zero-mask",
            "mode": "P",
            "bitmap_mode": "L",
            "bitmap_color": 0,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-cmyk-half-mask",
            "mode": "CMYK",
            "bitmap_mode": "L",
            "bitmap_color": 128,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255, 0, 0, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-cmyk-zero-mask",
            "mode": "CMYK",
            "bitmap_mode": "L",
            "bitmap_color": 0,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255, 0, 0, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-i-half-mask",
            "mode": "I",
            "bitmap_mode": "L",
            "bitmap_color": 128,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-i-one-mask",
            "mode": "I",
            "bitmap_mode": "L",
            "bitmap_color": 255,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-f-half-mask",
            "mode": "F",
            "bitmap_mode": "L",
            "bitmap_color": 128,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-f-zero-mask",
            "mode": "F",
            "bitmap_mode": "L",
            "bitmap_color": 0,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-f-mask-out-of-bounds",
            "mode": "F",
            "bitmap_mode": "L",
            "bitmap_color": 128,
            "values": {
                "xy": literal([-2, -2]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-f-one-mask",
            "mode": "F",
            "bitmap_mode": "L",
            "bitmap_color": 255,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-rgb-l-mask",
            "mode": "RGB",
            "bitmap_mode": "L",
            "bitmap_color": 255,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255, 0, 0, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-rgb-half-mask",
            "mode": "RGB",
            "bitmap_mode": "L",
            "bitmap_color": 128,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255, 0, 0, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-rgb-one-mask",
            "mode": "RGB",
            "bitmap_mode": "1",
            "bitmap_color": 1,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255, 0, 0, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-rgba-half-mask",
            "mode": "RGBA",
            "bitmap_mode": "L",
            "bitmap_color": 128,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255, 0, 0, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-rgb-rgba-mask",
            "mode": "RGB",
            "bitmap_mode": "RGBA",
            "bitmap_color": 255,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255, 0, 0, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-rgb-rgba-lowercase-mask",
            "mode": "RGB",
            "bitmap_mode": "RGBa",
            "bitmap_color": [16, 32, 64, 128],
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255, 0, 0, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-f-tuple-fill-error",
            "mode": "F",
            "bitmap_mode": "L",
            "bitmap_color": 255,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255, 0, 0, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-l-tuple-fill-error",
            "mode": "L",
            "bitmap_mode": "L",
            "bitmap_color": 255,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255, 0]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "raw-p-no-palette-fallback",
            "mode": "P",
            "edge": "raw-p-no-palette",
            "observe_receiver": True,
            "values": {
                "text": literal("A"),
                "fill": literal(7),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "attached-palette",
            "chain": "p-attached-palette-text",
            "mode": "P",
            "observe_receiver": True,
            "values": {
                "text": literal("A"),
                "fill": literal(1),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "attached-palette-negative-position",
            "chain": "p-attached-palette-text",
            "mode": "P",
            "values": {
                "xy": literal([-3, -3]),
                "text": literal("A"),
                "fill": literal(1),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "canvas-rgb-negative-position",
            "mode": "RGB",
            "values": {
                "xy": literal([-3, -3]),
                "text": literal("A"),
                "fill": literal([255, 0, 0]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "canvas-la-negative-position",
            "mode": "LA",
            "values": {
                "xy": literal([-3, -3]),
                "text": literal("A"),
                "fill": literal([255, 128]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "canvas-cmyk-negative-position",
            "mode": "CMYK",
            "values": {
                "xy": literal([-3, -3]),
                "text": literal("A"),
                "fill": literal([255, 0, 0, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "canvas-i-negative-position",
            "mode": "I",
            "values": {
                "xy": literal([-3, -3]),
                "text": literal("A"),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "canvas-f-negative-position",
            "mode": "F",
            "values": {
                "xy": literal([-3, -3]),
                "text": literal("A"),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "explicit-font-canvas-1",
            "font": "font/fonts/DejaVuSans.ttf",
            "mode": "1",
            "values": {
                "text": literal("A"),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "explicit-font-canvas-l",
            "font": "font/fonts/DejaVuSans.ttf",
            "mode": "L",
            "values": {
                "text": literal("A"),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "explicit-font-canvas-la",
            "font": "font/fonts/DejaVuSans.ttf",
            "mode": "LA",
            "values": {
                "text": literal("A"),
                "fill": literal([255, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "explicit-font-canvas-p",
            "font": "font/fonts/DejaVuSans.ttf",
            "mode": "P",
            "values": {
                "text": literal("A"),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "explicit-font-canvas-i",
            "font": "font/fonts/DejaVuSans.ttf",
            "mode": "I",
            "values": {
                "text": literal("A"),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "explicit-font-canvas-f",
            "font": "font/fonts/DejaVuSans.ttf",
            "mode": "F",
            "values": {
                "text": literal("A"),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "explicit-font-canvas-cmyk",
            "font": "font/fonts/DejaVuSans.ttf",
            "mode": "CMYK",
            "values": {
                "text": literal("A"),
                "fill": literal([255, 0, 0, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "explicit-font-canvas-ycbcr",
            "font": "font/fonts/DejaVuSans.ttf",
            "mode": "YCbCr",
            "values": {
                "text": literal("A"),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "explicit-font-canvas-hsv",
            "font": "font/fonts/DejaVuSans.ttf",
            "mode": "HSV",
            "values": {
                "text": literal("A"),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "canvas-l",
            "mode": "L",
            "values": {
                "text": literal("A"),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "canvas-1",
            "mode": "1",
            "values": {
                "text": literal("A"),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "canvas-la",
            "mode": "LA",
            "values": {
                "text": literal("A"),
                "fill": literal([255, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "mode.la",
            "name": "canvas-la-opaque-destination",
            "mode": "LA",
            "edge": "nonzero-pixel",
            "pixel": [0, 255],
            "values": {
                "text": literal("A"),
                "fill": literal([255, 128]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "canvas-cmyk",
            "mode": "CMYK",
            "values": {
                "text": literal("A"),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "canvas-p",
            "mode": "P",
            "values": {
                "text": literal("A"),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "canvas-p-small",
            "mode": "P",
            "size": [8, 8],
            "values": {
                "text": literal("A"),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "canvas-l-small",
            "mode": "L",
            "size": [8, 8],
            "values": {
                "text": literal("A"),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "canvas-one-small",
            "mode": "1",
            "size": [8, 8],
            "values": {
                "text": literal("A"),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "canvas-rgba-tuple-fill",
            "mode": "RGBA",
            "values": {
                "text": literal("A"),
                "fill": literal([255, 0, 0, 128]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "canvas-ycbcr",
            "mode": "YCbCr",
            "values": {
                "text": literal("A"),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "canvas-hsv",
            "mode": "HSV",
            "values": {
                "text": literal("A"),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "negative-position",
            "mode": "P",
            "values": {
                "text": literal("A"),
                "xy": literal([-3, -3]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rectangle",
            "requirement_suffix": "behavior.default",
            "name": "nested-box",
            "values": {
                "xy": literal([[0, 0], [8, 8]]),
                "fill": literal([255, 0, 0]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rectangle",
            "requirement_suffix": "behavior.default",
            "name": "malformed-nested-box-error",
            "values": {
                "xy": literal([[0], [8, 8]]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rectangle",
            "requirement_suffix": "behavior.default",
            "name": "too-many-nested-box-points-error",
            "values": {
                "xy": literal([[0, 0], [8, 8], [9, 9]]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rectangle",
            "requirement_suffix": "behavior.default",
            "name": "flat-box-arity-error",
            "values": {
                "xy": literal([0, 0, 8]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "line",
            "requirement_suffix": "behavior.default",
            "name": "flat-points",
            "values": {
                "xy": literal([0, 0, 8, 8]),
                "fill": literal([255, 255, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "line",
            "requirement_suffix": "behavior.default",
            "name": "reversed-slope-directions",
            "values": {
                "xy": literal([12, 10, 2, 1]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "line",
            "requirement_suffix": "behavior.default",
            "name": "partially-clipped-negative-line",
            "values": {
                "xy": literal([-4, -4, 4, 4]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "line",
            "requirement_suffix": "behavior.default",
            "name": "empty-points-no-op",
            "values": {
                "xy": literal([]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "line",
            "requirement_suffix": "behavior.default",
            "name": "single-flat-point-no-op",
            "observe_receiver": True,
            "values": {
                "xy": literal([0, 0]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "line",
            "requirement_suffix": "behavior.default",
            "name": "single-nested-point-no-op",
            "values": {
                "xy": literal([[0, 0]]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "line",
            "requirement_suffix": "behavior.default",
            "name": "odd-flat-points-error",
            "values": {
                "xy": literal([0, 0, 1]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "line",
            "requirement_suffix": "behavior.default",
            "name": "malformed-nested-point-error",
            "values": {
                "xy": literal([[0]]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "line",
            "requirement_suffix": "behavior.default",
            "name": "invalid-sequence-contents-error",
            "values": {
                "xy": literal(["bad", "bad"]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "line",
            "requirement_suffix": "behavior.default",
            "name": "scalar-points-error",
            "values": {
                "xy": literal(1),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "polygon",
            "requirement_suffix": "behavior.default",
            "name": "paired-points",
            "values": {
                "xy": literal([[0, 0], [8, 0], [8, 8], [0, 8]]),
                "fill": literal([0, 255, 0]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "polygon",
            "requirement_suffix": "behavior.default",
            "name": "concave-collinear-clipped",
            "values": {
                "xy": literal([[-2, 2], [4, 2], [4, 5], [8, 5], [8, 12], [-2, 12]]),
                "fill": literal([0, 255, 0]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "polygon",
            "requirement_suffix": "behavior.default",
            "name": "two-points-line",
            "values": {
                "xy": literal([[2, 2], [12, 8]]),
                "fill": literal([255, 0, 0]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "polygon",
            "requirement_suffix": "behavior.default",
            "name": "empty-points-error",
            "values": {
                "xy": literal([]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "polygon",
            "requirement_suffix": "behavior.default",
            "name": "malformed-nested-point-error",
            "values": {
                "xy": literal([[0], [1, 1]]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "polygon",
            "requirement_suffix": "behavior.default",
            "name": "single-nested-point-error",
            "values": {
                "xy": literal([[0, 0]]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "polygon",
            "requirement_suffix": "behavior.default",
            "name": "two-points-outline",
            "values": {
                "xy": literal([[2, 2], [12, 8]]),
                "outline": literal([0, 255, 0]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "shape",
            "requirement_suffix": "behavior.default",
            "name": "filled-and-outlined",
            "values": {
                "fill": literal([255, 0, 0]),
                "outline": literal([0, 255, 0]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "shape",
            "requirement_suffix": "behavior.default",
            "name": "curve-outline",
            "outline_curve": True,
            "values": {"fill": literal([255, 0, 0])},
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "shape",
            "requirement_suffix": "behavior.default",
            "name": "empty-outline-no-op",
            "outline_empty": True,
            "values": {},
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "shape",
            "requirement_suffix": "behavior.default",
            "name": "canvas-l-default-ink",
            "mode": "L",
            "values": {},
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "shape",
            "requirement_suffix": "behavior.default",
            "name": "canvas-i-default-ink",
            "mode": "I",
            "values": {},
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "shape",
            "requirement_suffix": "behavior.default",
            "name": "canvas-f-default-ink",
            "mode": "F",
            "values": {},
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "shape",
            "requirement_suffix": "behavior.default",
            "name": "canvas-p-default-ink",
            "mode": "P",
            "values": {},
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-ycbcr",
            "mode": "YCbCr",
            "bitmap_mode": "L",
            "bitmap_color": 255,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "invalid-rgb-mask",
            "mode": "RGB",
            "bitmap_mode": "RGB",
            "bitmap_color": 255,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([255, 0, 0]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-hsv",
            "mode": "HSV",
            "bitmap_mode": "L",
            "bitmap_color": 255,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([1, 2, 3]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "canvas-ycbcr-zero-mask",
            "mode": "YCbCr",
            "bitmap_mode": "L",
            "bitmap_color": 0,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "partial-rgb-mask-out-of-bounds",
            "mode": "RGB",
            "bitmap_mode": "L",
            "bitmap_color": 128,
            "values": {
                "xy": literal([-2, -2]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "partial-l-mask-out-of-bounds",
            "mode": "L",
            "bitmap_mode": "L",
            "bitmap_color": 128,
            "values": {
                "xy": literal([-2, -2]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "partial-one-mask-out-of-bounds",
            "mode": "1",
            "bitmap_mode": "L",
            "bitmap_color": 128,
            "values": {
                "xy": literal([-2, -2]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "partial-la-mask-out-of-bounds",
            "mode": "LA",
            "bitmap_mode": "L",
            "bitmap_color": 128,
            "values": {
                "xy": literal([-2, -2]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "partial-cmyk-mask-out-of-bounds",
            "mode": "CMYK",
            "bitmap_mode": "L",
            "bitmap_color": 128,
            "values": {
                "xy": literal([-2, -2]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "partial-p-mask-out-of-bounds",
            "mode": "P",
            "bitmap_mode": "L",
            "bitmap_color": 128,
            "values": {
                "xy": literal([-2, -2]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "zero-i-mask",
            "mode": "I",
            "bitmap_mode": "L",
            "bitmap_color": 0,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "bitmap",
            "requirement_suffix": "behavior.default",
            "name": "partial-ycbcr-mask-out-of-bounds",
            "mode": "YCbCr",
            "bitmap_mode": "L",
            "bitmap_color": 128,
            "values": {
                "xy": literal([-2, -2]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "line",
            "requirement_suffix": "behavior.default",
            "name": "canvas-ycbcr-int-fill",
            "mode": "YCbCr",
            "values": {
                "xy": literal([0, 0, 10, 10]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rectangle",
            "requirement_suffix": "behavior.default",
            "name": "canvas-hsv-int-fill",
            "mode": "HSV",
            "values": {
                "xy": literal([0, 0, 8, 8]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "point",
            "requirement_suffix": "behavior.default",
            "name": "flat-points",
            "values": {
                "xy": literal([2, 2, 8, 8]),
                "fill": literal([255, 255, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "point",
            "requirement_suffix": "behavior.default",
            "name": "empty-points-no-op",
            "values": {
                "xy": literal([]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "point",
            "requirement_suffix": "behavior.default",
            "name": "odd-flat-points-error",
            "values": {
                "xy": literal([0, 0, 1]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "point",
            "requirement_suffix": "behavior.default",
            "name": "malformed-nested-point-error",
            "values": {
                "xy": literal([[0]]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "point",
            "requirement_suffix": "behavior.default",
            "name": "invalid-sequence-contents-error",
            "values": {
                "xy": literal(["bad", "bad"]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "point",
            "requirement_suffix": "behavior.default",
            "name": "scalar-points-error",
            "values": {
                "xy": literal(1),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "point",
            "requirement_suffix": "behavior.default",
            "name": "rejected-rgb-empty-component",
            "mode": "RGB",
            "values": {
                "xy": literal([2, 2]),
                "fill": literal("rgb(, 1, 2)"),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "point",
            "requirement_suffix": "behavior.default",
            "name": "rejected-rgb-non-numeric-component",
            "mode": "RGB",
            "values": {
                "xy": literal([2, 2]),
                "fill": literal(["bad"]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "point",
            "requirement_suffix": "behavior.default",
            "name": "l-one-component",
            "mode": "L",
            "observe_receiver": True,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([128]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "point",
            "requirement_suffix": "behavior.default",
            "name": "pa-one-component",
            "mode": "PA",
            "observe_receiver": True,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([7]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "point",
            "requirement_suffix": "behavior.default",
            "name": "pa-default-ink",
            "mode": "PA",
            "observe_receiver": True,
            "values": {
                "xy": literal([2, 2]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "point",
            "requirement_suffix": "behavior.default",
            "name": "pa-two-components",
            "mode": "PA",
            "observe_receiver": True,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([7, 128]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "point",
            "requirement_suffix": "behavior.default",
            "name": "pa-invalid-component-count",
            "mode": "PA",
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([7, 128, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "point",
            "requirement_suffix": "behavior.default",
            "name": "rgb-two-component-error",
            "mode": "RGB",
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([7, 128]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "point",
            "requirement_suffix": "behavior.default",
            "name": "rgb-negative-component-error",
            "mode": "RGB",
            "observe_receiver": True,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([-1, 128, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "point",
            "requirement_suffix": "behavior.default",
            "name": "la-integer-fill",
            "mode": "LA",
            "observe_receiver": True,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal(128),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "point",
            "requirement_suffix": "behavior.default",
            "name": "la-negative-integer-fill",
            "mode": "LA",
            "observe_receiver": True,
            "values": {
                "xy": literal([2, 2]),
                "fill": literal(-1),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "point",
            "requirement_suffix": "behavior.default",
            "name": "la-invalid-component-count",
            "mode": "LA",
            "values": {
                "xy": literal([2, 2]),
                "fill": literal([7, 128, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rounded_rectangle",
            "requirement_suffix": "behavior.default",
            "name": "radius-zero-fallback",
            "values": {
                "xy": literal([0, 0, 10, 10]),
                "radius": literal(0),
                "fill": literal([200, 100, 50]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rounded_rectangle",
            "requirement_suffix": "behavior.default",
            "name": "degenerate-box-fallback",
            "values": {
                "xy": literal([0, 0, 1, 8]),
                "radius": literal(1),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rounded_rectangle",
            "requirement_suffix": "behavior.default",
            "name": "short-box-error",
            "values": {
                "xy": literal([0, 0]),
                "radius": literal(2),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "canvas-i",
            "mode": "I",
            "values": {
                "text": literal("A"),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "canvas-f",
            "mode": "F",
            "values": {
                "text": literal("A"),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "empty-text",
            "font": "font/fonts/DejaVuSans.ttf",
            "values": {
                "text": literal(""),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "zero-size-rgba",
            "mode": "RGBA",
            "edge": "zero-size",
            "font": "font/fonts/DejaVuSans.ttf",
            "values": {
                "text": literal("A"),
                "fill": literal([255, 0, 0, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "la-opaque-destination",
            "mode": "LA",
            "edge": "nonzero-pixel",
            "pixel": [0, 255],
            "font": "font/fonts/DejaVuSans.ttf",
            "values": {
                "text": literal("A"),
                "fill": literal([255, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "i-negative-position",
            "mode": "I",
            "font": "font/fonts/DejaVuSans.ttf",
            "values": {
                "xy": literal([-3, -3]),
                "text": literal("A"),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "line",
            "requirement_suffix": "behavior.default",
            "name": "width-three",
            "values": {
                "xy": literal([0, 0, 12, 8]),
                "fill": literal([255, 255, 255]),
                "width": literal(3),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "line",
            "requirement_suffix": "behavior.default",
            "name": "out-of-bounds",
            "values": {
                "xy": literal([[-4, -4], [20, 20]]),
                "fill": literal(255),
                "width": literal(2),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "line",
            "requirement_suffix": "behavior.default",
            "name": "reverse-axes",
            "values": {
                "xy": literal([[12, 8], [0, 0]]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "line",
            "requirement_suffix": "behavior.default",
            "name": "vertical",
            "values": {
                "xy": literal([[4, 0], [4, 12]]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "line",
            "requirement_suffix": "behavior.default",
            "name": "horizontal",
            "values": {
                "xy": literal([[0, 4], [12, 4]]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "line",
            "requirement_suffix": "behavior.default",
            "name": "steep-negative-direction",
            "values": {
                "xy": literal([[12, 12], [4, 0]]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "line",
            "requirement_suffix": "behavior.default",
            "name": "shallow-negative-y",
            "values": {
                "xy": literal([[0, 12], [12, 4]]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "line",
            "requirement_suffix": "behavior.default",
            "name": "shallow-low-slope",
            "values": {
                "xy": literal([[0, 0], [12, 1]]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "line",
            "requirement_suffix": "behavior.default",
            "name": "steep-high-slope",
            "values": {
                "xy": literal([[0, 0], [1, 12]]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "polygon",
            "requirement_suffix": "behavior.default",
            "name": "horizontal-runs",
            "values": {
                "xy": literal(
                    [
                        [1, 4],
                        [12, 4],
                        [12, 8],
                        [4, 8],
                        [4, 5],
                        [10, 5],
                        [10, 7],
                        [2, 7],
                    ]
                ),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "polygon",
            "requirement_suffix": "behavior.default",
            "name": "out-of-bounds",
            "values": {
                "xy": literal([[-5, 3], [20, 3], [20, 13], [-5, 13]]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "polygon",
            "requirement_suffix": "behavior.default",
            "name": "coalesced-horizontal-increasing",
            "values": {
                "xy": literal([[1, 4], [4, 4], [8, 4], [8, 10], [1, 10]]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "polygon",
            "requirement_suffix": "behavior.default",
            "name": "coalesced-horizontal-decreasing",
            "values": {
                "xy": literal([[8, 4], [4, 4], [1, 4], [1, 10], [8, 10]]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "polygon",
            "requirement_suffix": "behavior.default",
            "name": "horizontal-only",
            "values": {
                "xy": literal([[1, 4], [10, 4], [5, 4]]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "polygon",
            "requirement_suffix": "behavior.default",
            "name": "above-canvas",
            "values": {
                "xy": literal([[-5, -20], [20, -20], [20, -10], [-5, -10]]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "regular_polygon",
            "requirement_suffix": "behavior.default",
            "name": "triangle",
            "values": {
                "bounding_circle": literal([8, 8, 6]),
                "n_sides": literal(3),
                "outline": literal([255, 255, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "regular_polygon",
            "requirement_suffix": "behavior.default",
            "name": "nested-bounding-circle",
            "values": {
                "bounding_circle": literal([[8, 8], 6]),
                "n_sides": literal(5),
                "outline": literal([255, 255, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "regular_polygon",
            "requirement_suffix": "behavior.default",
            "name": "pentagon-rotated",
            "values": {
                "bounding_circle": literal([8, 8, 6]),
                "n_sides": literal(5),
                "rotation": literal(30),
                "outline": literal([255, 255, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "regular_polygon",
            "requirement_suffix": "behavior.default",
            "name": "heptagon-rotated",
            "values": {
                "bounding_circle": literal([8, 8, 6]),
                "n_sides": literal(7),
                "rotation": literal(15),
                "outline": literal([255, 255, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "regular_polygon",
            "requirement_suffix": "behavior.default",
            "name": "rotation-wrap",
            "values": {
                "bounding_circle": literal([8, 8, 6]),
                "n_sides": literal(5),
                "rotation": literal(400),
                "outline": literal([255, 255, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "regular_polygon",
            "requirement_suffix": "behavior.default",
            "name": "short-bounding-circle-error",
            "values": {
                "bounding_circle": literal([8, 8]),
                "n_sides": literal(3),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "regular_polygon",
            "requirement_suffix": "behavior.default",
            "name": "invalid-side-count-error",
            "values": {
                "bounding_circle": literal([8, 8, 4]),
                "n_sides": literal(2),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rectangle",
            "requirement_suffix": "behavior.default",
            "name": "canvas-l",
            "mode": "L",
            "values": {
                "xy": literal([0, 0, 8, 8]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rectangle",
            "requirement_suffix": "behavior.default",
            "name": "canvas-1",
            "mode": "1",
            "values": {
                "xy": literal([0, 0, 8, 8]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rectangle",
            "requirement_suffix": "behavior.default",
            "name": "canvas-la",
            "mode": "LA",
            "values": {
                "xy": literal([0, 0, 8, 8]),
                "fill": literal([255, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rectangle",
            "requirement_suffix": "behavior.default",
            "name": "canvas-cmyk",
            "mode": "CMYK",
            "values": {
                "xy": literal([0, 0, 8, 8]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rectangle",
            "requirement_suffix": "behavior.default",
            "name": "canvas-p",
            "mode": "P",
            "values": {
                "xy": literal([0, 0, 8, 8]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rectangle",
            "requirement_suffix": "behavior.default",
            "name": "canvas-i-integer-fill",
            "mode": "I",
            "observe_receiver": True,
            "values": {
                "xy": literal([0, 0, 8, 8]),
                "fill": literal(123),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rectangle",
            "requirement_suffix": "behavior.default",
            "name": "canvas-f-integer-fill",
            "mode": "F",
            "observe_receiver": True,
            "values": {
                "xy": literal([0, 0, 8, 8]),
                "fill": literal(123),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rectangle",
            "requirement_suffix": "behavior.default",
            "name": "canvas-f-float-fill",
            "mode": "F",
            "observe_receiver": True,
            "values": {
                "xy": literal([0, 0, 8, 8]),
                "fill": literal(1.5),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rectangle",
            "requirement_suffix": "behavior.default",
            "name": "canvas-rgb-float-fill-error",
            "mode": "RGB",
            "values": {
                "xy": literal([0, 0, 8, 8]),
                "fill": literal(1.5),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rectangle",
            "requirement_suffix": "behavior.default",
            "name": "canvas-l-float-fill-error",
            "mode": "L",
            "values": {
                "xy": literal([0, 0, 8, 8]),
                "fill": literal(1.5),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "ellipse",
            "requirement_suffix": "behavior.default",
            "name": "canvas-la",
            "mode": "LA",
            "values": {
                "xy": literal([0, 0, 12, 12]),
                "fill": literal([255, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "polygon",
            "requirement_suffix": "behavior.default",
            "name": "canvas-cmyk",
            "mode": "CMYK",
            "values": {
                "xy": literal([[0, 0], [10, 0], [10, 10], [0, 10]]),
                "fill": literal(255),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rounded_rectangle",
            "requirement_suffix": "behavior.default",
            "name": "radius",
            "values": {
                "xy": literal([0, 0, 10, 10]),
                "radius": literal(4),
                "fill": literal([200, 100, 50]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rounded_rectangle",
            "requirement_suffix": "behavior.default",
            "name": "radius-covers-entire-box",
            "values": {
                "xy": literal([0, 0, 10, 10]),
                "radius": literal(10),
                "fill": literal([200, 100, 50]),
                "outline": literal([255, 255, 255]),
                "width": literal(2),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rounded_rectangle",
            "requirement_suffix": "behavior.default",
            "name": "radius-covers-width-only",
            "values": {
                "xy": literal([0, 0, 6, 14]),
                "radius": literal(3),
                "fill": literal([200, 100, 50]),
                "outline": literal([255, 255, 255]),
                "width": literal(2),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rounded_rectangle",
            "requirement_suffix": "behavior.default",
            "name": "large-outline",
            "values": {
                "xy": literal([0, 0, 14, 10]),
                "radius": literal(3),
                "fill": literal([200, 100, 50]),
                "outline": literal([255, 255, 255]),
                "width": literal(4),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "circle",
            "requirement_suffix": "behavior.default",
            "name": "bbox",
            "values": {
                "xy": literal([0, 0, 12, 12]),
                "outline": literal([255, 255, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "circle",
            "requirement_suffix": "behavior.default",
            "name": "short-center-error",
            "values": {
                "xy": literal([8]),
                "radius": literal(4),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "circle",
            "requirement_suffix": "behavior.default",
            "name": "scalar-center-error",
            "values": {
                "xy": literal(1),
                "radius": literal(4),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "circle",
            "requirement_suffix": "behavior.default",
            "name": "non-sequence-center-error",
            "values": {
                "xy": literal("bad"),
                "radius": literal(4),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "circle",
            "requirement_suffix": "behavior.default",
            "name": "mapping-center-error",
            "values": {
                "xy": literal({}),
                "radius": literal(4),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "circle",
            "requirement_suffix": "behavior.default",
            "name": "none-center-error",
            "values": {
                "xy": literal(None),
                "radius": literal(4),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "polygon",
            "requirement_suffix": "behavior.default",
            "name": "fill-outline",
            "values": {
                "xy": literal([[1, 1], [10, 1], [5, 10]]),
                "fill": literal([255, 0, 0]),
                "outline": literal([255, 255, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "line",
            "requirement_suffix": "behavior.default",
            "name": "wide",
            "values": {
                "xy": literal([[0, 0], [12, 12]]),
                "width": literal(3),
                "fill": literal([255, 0, 0]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "line",
            "requirement_suffix": "behavior.default",
            "name": "wide-joint-curve",
            "values": {
                "xy": literal([[0, 0], [12, 0], [6, 12]]),
                "width": literal(3),
                "joint": literal("curve"),
                "fill": literal([255, 0, 0]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "arc",
            "requirement_suffix": "behavior.default",
            "name": "fill-width",
            "values": {
                "xy": literal([0, 0, 10, 10]),
                "start": literal(0),
                "end": literal(90),
                "fill": literal([255, 0, 0]),
                "width": literal(3),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "arc",
            "requirement_suffix": "behavior.default",
            "name": "full-sweep",
            "values": {
                "xy": literal([0, 0, 12, 10]),
                "start": literal(0),
                "end": literal(360),
                "fill": literal([255, 0, 0]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "arc",
            "requirement_suffix": "behavior.default",
            "name": "empty-sweep",
            "values": {
                "xy": literal([0, 0, 12, 10]),
                "start": literal(45),
                "end": literal(45),
                "fill": literal([255, 0, 0]),
                "width": literal(2),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "arc",
            "requirement_suffix": "behavior.default",
            "name": "wrapped-negative-angles",
            "values": {
                "xy": literal([0, 0, 12, 10]),
                "start": literal(-45),
                "end": literal(45),
                "fill": literal([255, 0, 0]),
                "width": literal(2),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "arc",
            "requirement_suffix": "behavior.default",
            "name": "tall-ellipse-axis-transpose",
            "values": {
                "xy": literal([0, 0, 8, 14]),
                "start": literal(25),
                "end": literal(225),
                "fill": literal([255, 0, 0]),
                "width": literal(2),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "arc",
            "requirement_suffix": "behavior.default",
            "name": "axis-boundary-sweep",
            "values": {
                "xy": literal([0, 0, 14, 8]),
                "start": literal(90),
                "end": literal(270),
                "fill": literal([255, 0, 0]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "chord",
            "requirement_suffix": "behavior.default",
            "name": "fill-outline",
            "values": {
                "xy": literal([0, 0, 10, 10]),
                "start": literal(0),
                "end": literal(180),
                "fill": literal([255, 0, 0]),
                "outline": literal([0, 255, 0]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "chord",
            "requirement_suffix": "behavior.default",
            "name": "full-sweep",
            "values": {
                "xy": literal([0, 0, 12, 10]),
                "start": literal(0),
                "end": literal(360),
                "fill": literal([255, 0, 0]),
                "outline": literal([0, 255, 0]),
                "width": literal(2),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "chord",
            "requirement_suffix": "behavior.default",
            "name": "empty-sweep",
            "values": {
                "xy": literal([0, 0, 12, 10]),
                "start": literal(30),
                "end": literal(30),
                "fill": literal([255, 0, 0]),
                "outline": literal([0, 255, 0]),
                "width": literal(2),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "chord",
            "requirement_suffix": "behavior.default",
            "name": "wrapped-angles",
            "values": {
                "xy": literal([0, 0, 8, 14]),
                "start": literal(300),
                "end": literal(60),
                "fill": literal([255, 0, 0]),
                "outline": literal([0, 255, 0]),
                "width": literal(3),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "pieslice",
            "requirement_suffix": "behavior.default",
            "name": "fill-outline-width",
            "values": {
                "xy": literal([0, 0, 10, 10]),
                "start": literal(45),
                "end": literal(200),
                "fill": literal([255, 0, 0]),
                "outline": literal([0, 0, 255]),
                "width": literal(2),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "pieslice",
            "requirement_suffix": "behavior.default",
            "name": "full-sweep",
            "values": {
                "xy": literal([0, 0, 12, 10]),
                "start": literal(0),
                "end": literal(360),
                "fill": literal([255, 0, 0]),
                "outline": literal([0, 0, 255]),
                "width": literal(2),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "pieslice",
            "requirement_suffix": "behavior.default",
            "name": "empty-sweep",
            "values": {
                "xy": literal([0, 0, 12, 10]),
                "start": literal(30),
                "end": literal(30),
                "fill": literal([255, 0, 0]),
                "outline": literal([0, 0, 255]),
                "width": literal(2),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "pieslice",
            "requirement_suffix": "behavior.default",
            "name": "narrow-sweep",
            "values": {
                "xy": literal([0, 0, 12, 10]),
                "start": literal(10),
                "end": literal(60),
                "fill": literal([255, 0, 0]),
                "outline": literal([0, 0, 255]),
                "width": literal(2),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "pieslice",
            "requirement_suffix": "behavior.default",
            "name": "wide-sweep",
            "values": {
                "xy": literal([0, 0, 12, 10]),
                "start": literal(10),
                "end": literal(220),
                "fill": literal([255, 0, 0]),
                "outline": literal([0, 0, 255]),
                "width": literal(2),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "ellipse",
            "requirement_suffix": "behavior.default",
            "name": "fill-outline-width",
            "values": {
                "xy": literal([0, 0, 12, 10]),
                "fill": literal([255, 0, 0]),
                "outline": literal([0, 0, 255]),
                "width": literal(2),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "ellipse",
            "requirement_suffix": "behavior.default",
            "name": "zero-size",
            "values": {
                "xy": literal([5, 5, 5, 5]),
                "fill": literal([255, 0, 0]),
                "outline": literal([0, 0, 255]),
                "width": literal(1),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "ellipse",
            "requirement_suffix": "behavior.default",
            "name": "reversed-box",
            "values": {
                "xy": literal([10, 8, 2, 2]),
                "fill": literal([255, 0, 0]),
                "outline": literal([0, 0, 255]),
                "width": literal(2),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "ellipse",
            "requirement_suffix": "behavior.default",
            "name": "reversed-y-box",
            "values": {
                "xy": literal([2, 10, 10, 2]),
                "fill": literal([255, 0, 0]),
                "outline": literal([0, 0, 255]),
                "width": literal(2),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rectangle",
            "requirement_suffix": "behavior.default",
            "name": "fill-outline-width",
            "values": {
                "xy": literal([0, 0, 10, 10]),
                "fill": literal([255, 0, 0]),
                "outline": literal([0, 255, 0]),
                "width": literal(2),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rectangle",
            "requirement_suffix": "behavior.default",
            "name": "negative-clipped",
            "values": {
                "xy": literal([-4, -4, 4, 4]),
                "fill": literal([255, 0, 0]),
                "outline": literal([0, 255, 0]),
                "width": literal(2),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "rectangle",
            "requirement_suffix": "behavior.default",
            "name": "fully-off-canvas",
            "values": {
                "xy": literal([20, 20, 24, 24]),
                "fill": literal([255, 0, 0]),
                "outline": literal([0, 255, 0]),
                "width": literal(2),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "polygon",
            "requirement_suffix": "behavior.default",
            "name": "outline-width",
            "values": {
                "xy": literal([[1, 1], [10, 1], [5, 10]]),
                "outline": literal([255, 255, 255]),
                "width": literal(2),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "regular_polygon",
            "requirement_suffix": "behavior.default",
            "name": "rotated-hexagon",
            "values": {
                "bounding_circle": literal([8, 8, 6]),
                "n_sides": literal(6),
                "rotation": literal(15),
                "fill": literal([255, 0, 0]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "point",
            "requirement_suffix": "mode.la",
            "name": "la-canvas",
            "mode": "LA",
            "values": {
                "xy": literal([[2, 2], [4, 4]]),
                "fill": literal([255, 0]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "text",
            "requirement_suffix": "behavior.default",
            "name": "stroked-rgba",
            "mode": "RGBA",
            "values": {
                "text": literal("Hi"),
                "fill": literal([255, 0, 0, 255]),
                "stroke_width": literal(2),
                "stroke_fill": literal([0, 0, 255, 255]),
            },
        },
        {
            "surface": "PIL.ImageDraw.ImageDraw",
            "operation": "multiline_text",
            "requirement_suffix": "behavior.default",
            "name": "centered-anchored",
            "mode": "RGB",
            "values": {
                "text": literal("A\nB"),
                "align": literal("center"),
                "anchor": literal("mm"),
                "fill": literal([255, 0, 0]),
            },
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getrgb",
            "requirement_suffix": "parameter.color",
            "name": "named-css-color",
            "values": {"color": literal("rebeccapurple")},
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getrgb",
            "requirement_suffix": "parameter.color",
            "name": "rgba-syntax",
            "values": {"color": literal("rgba(255, 0, 0, 128)")},
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getrgb",
            "requirement_suffix": "parameter.color",
            "name": "hsl-syntax",
            "values": {"color": literal("hsl(120, 100%, 50%)")},
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getrgb",
            "requirement_suffix": "parameter.color",
            "name": "hex-with-alpha",
            "values": {"color": literal("#ff000080")},
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getrgb",
            "requirement_suffix": "parameter.color",
            "name": "hex-short-alpha",
            "values": {"color": literal("#f00f")},
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getcolor",
            "requirement_suffix": "mode.l",
            "name": "named-l",
            "values": {
                "color": literal("red"),
                "mode": literal("L"),
            },
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getcolor",
            "requirement_suffix": "parameter.mode",
            "name": "named-hsv",
            "values": {
                "color": literal("red"),
                "mode": literal("HSV"),
            },
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getcolor",
            "requirement_suffix": "parameter.mode",
            "name": "green-hsv",
            "values": {
                "color": literal("rgb(0, 255, 0)"),
                "mode": literal("HSV"),
            },
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getcolor",
            "requirement_suffix": "parameter.mode",
            "name": "blue-hsv",
            "values": {
                "color": literal("rgb(0, 0, 255)"),
                "mode": literal("HSV"),
            },
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getcolor",
            "requirement_suffix": "parameter.mode",
            "name": "gray-hsv",
            "values": {
                "color": literal("rgb(128, 128, 128)"),
                "mode": literal("HSV"),
            },
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getcolor",
            "requirement_suffix": "parameter.mode",
            "name": "named-i",
            "values": {
                "color": literal("blue"),
                "mode": literal("I"),
            },
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getcolor",
            "requirement_suffix": "parameter.mode",
            "name": "named-la",
            "values": {
                "color": literal("red"),
                "mode": literal("LA"),
            },
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getcolor",
            "requirement_suffix": "parameter.mode",
            "name": "named-f",
            "values": {
                "color": literal("blue"),
                "mode": literal("F"),
            },
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getcolor",
            "requirement_suffix": "parameter.mode",
            "name": "named-i16",
            "values": {
                "color": literal("green"),
                "mode": literal("I;16"),
            },
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getcolor",
            "requirement_suffix": "parameter.mode",
            "name": "named-i16b",
            "values": {
                "color": literal("white"),
                "mode": literal("I;16B"),
            },
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getcolor",
            "requirement_suffix": "parameter.mode",
            "name": "named-rgba",
            "values": {
                "color": literal("rgba(1, 2, 3, 128)"),
                "mode": literal("RGBA"),
            },
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getcolor",
            "requirement_suffix": "parameter.mode",
            "name": "named-cmyk",
            "values": {
                "color": literal("#204080"),
                "mode": literal("CMYK"),
            },
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getrgb",
            "requirement_suffix": "parameter.color",
            "name": "rejected-rgba-float-alpha",
            "values": {"color": literal("rgba(255, 0, 0, 0.5)")},
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getrgb",
            "requirement_suffix": "parameter.color",
            "name": "rejected-rgba-short",
            "values": {"color": literal("rgba(1,2,3)")},
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getrgb",
            "requirement_suffix": "parameter.color",
            "name": "rejected-transparent",
            "values": {"color": literal("transparent")},
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getrgb",
            "requirement_suffix": "parameter.color",
            "name": "rejected-currentcolor",
            "values": {"color": literal("currentcolor")},
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getrgb",
            "requirement_suffix": "parameter.color",
            "name": "rejected-rgb-unclosed",
            "values": {"color": literal("rgb(1, 2, 3")},
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getrgb",
            "requirement_suffix": "parameter.color",
            "name": "rejected-rgb-nondigit",
            "values": {"color": literal("rgb(1, x, 3)")},
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getrgb",
            "requirement_suffix": "parameter.color",
            "name": "rejected-rgb-empty-component",
            "values": {"color": literal("rgb(, 1, 2)")},
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getrgb",
            "requirement_suffix": "parameter.color",
            "name": "rejected-rgba-unclosed",
            "values": {"color": literal("rgba(1, 2, 3, 4")},
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getrgb",
            "requirement_suffix": "parameter.color",
            "name": "rejected-rgba-nondigit",
            "values": {"color": literal("rgba(1, x, 3, 4)")},
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getrgb",
            "requirement_suffix": "parameter.color",
            "name": "rejected-rgba-empty-component",
            "values": {"color": literal("rgba(1,, 3, 4)")},
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getrgb",
            "requirement_suffix": "parameter.color",
            "name": "rgb-percent",
            "values": {"color": literal("rgb(100%, 50%, 0%)")},
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getrgb",
            "requirement_suffix": "parameter.color",
            "name": "rgb-over-range",
            "values": {"color": literal("rgb(300, 0, 0)")},
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getrgb",
            "requirement_suffix": "parameter.color",
            "name": "rgb-percent-over-range",
            "values": {"color": literal("rgb(101%, 0%, 0%)")},
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getrgb",
            "requirement_suffix": "parameter.color",
            "name": "rgba-over-range",
            "values": {"color": literal("rgba(300, 1, 2, 3)")},
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getrgb",
            "requirement_suffix": "parameter.color",
            "name": "rejected-hsla",
            "values": {"color": literal("hsla(120, 100%, 50%, 0.5)")},
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getrgb",
            "requirement_suffix": "parameter.color",
            "name": "rejected-leading-whitespace",
            "values": {"color": literal(" red")},
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getrgb",
            "requirement_suffix": "parameter.color",
            "name": "rejected-trailing-whitespace",
            "values": {"color": literal("red ")},
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getrgb",
            "requirement_suffix": "parameter.color",
            "name": "rejected-rgb-mixed-percent",
            "values": {"color": literal("rgb(100%, 50, 0%)")},
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getrgb",
            "requirement_suffix": "parameter.color",
            "name": "rejected-rgb-short",
            "values": {"color": literal("rgb(1, 2)")},
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getrgb",
            "requirement_suffix": "parameter.color",
            "name": "rejected-overlong",
            "values": {"color": literal("#" + "f" * 101)},
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getcolor",
            "requirement_suffix": "behavior.default",
            "name": "over-range-hsv",
            "values": {
                "color": literal("rgb(300, 0, 0)"),
                "mode": literal("HSV"),
            },
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getcolor",
            "requirement_suffix": "behavior.default",
            "name": "over-range-l",
            "values": {
                "color": literal("rgb(300, 0, 0)"),
                "mode": literal("L"),
            },
        },
        {
            "surface": "PIL.ImagePalette.ImagePalette",
            "operation": "getcolor",
            "requirement_suffix": "behavior.default",
            "name": "rgb-tuple-append",
            "values": {"color": literal([255, 0, 0])},
        },
        {
            "surface": "PIL.ImagePalette.ImagePalette",
            "operation": "getcolor",
            "requirement_suffix": "behavior.default",
            "name": "rgba-tuple-append",
            "values": {"color": literal([1, 2, 3, 128])},
        },
        {
            "surface": "PIL.ImagePalette.ImagePalette",
            "operation": "getcolor",
            "requirement_suffix": "behavior.default",
            "name": "short-tuple-append",
            "values": {"color": literal([1, 2])},
        },
        {
            "surface": "PIL.ImagePalette.ImagePalette",
            "operation": "getcolor",
            "requirement_suffix": "behavior.default",
            "name": "opaque-rgba-tuple-rgb-mode",
            "values": {"color": literal([1, 2, 3, 255])},
        },
        {
            "surface": "PIL.ImagePalette.ImagePalette",
            "operation": "getcolor",
            "requirement_suffix": "behavior.default",
            "name": "empty-tuple-rejected",
            "values": {"color": literal([])},
        },
        {
            "surface": "PIL.ImagePalette.ImagePalette",
            "operation": "getcolor",
            "requirement_suffix": "behavior.default",
            "name": "full-rgb-palette-existing-color",
            "values": {
                "color": literal([42, 42, 42]),
                "palette": literal(
                    [
                        component
                        for index in range(256)
                        for component in (index, index, index)
                    ]
                ),
            },
        },
        {
            "surface": "PIL.ImagePalette.ImagePalette",
            "operation": "getcolor",
            "requirement_suffix": "behavior.default",
            "name": "full-rgb-palette-exhausted",
            "values": {
                "color": literal([9, 8, 7]),
                "palette": literal(
                    [
                        component
                        for index in range(256)
                        for component in (index, index, index)
                    ]
                ),
            },
        },
        {
            "surface": "PIL.ImagePalette.ImagePalette",
            "operation": "getcolor",
            "requirement_suffix": "behavior.default",
            "name": "rgba-mode-three-tuple",
            "values": {
                "mode": literal("RGBA"),
                "color": literal([1, 2, 3]),
            },
        },
        {
            "surface": "PIL.ImagePalette.ImagePalette",
            "operation": "getcolor",
            "requirement_suffix": "behavior.default",
            "name": "rgba-mode-short-tuple",
            "values": {
                "mode": literal("RGBA"),
                "color": literal([1, 2]),
            },
        },
        {
            "surface": "PIL.ImagePalette.ImagePalette",
            "operation": "getcolor",
            "requirement_suffix": "behavior.default",
            "name": "rgba-mode-four-tuple",
            "values": {
                "mode": literal("RGBA"),
                "color": literal([1, 2, 3, 4]),
            },
        },
        {
            "surface": "PIL.ImagePalette.ImagePalette",
            "operation": "getcolor",
            "requirement_suffix": "behavior.default",
            "name": "full-rgba-palette-existing-color",
            "values": {
                "mode": literal("RGBA"),
                "color": literal([42, 42, 42, 200]),
                "palette": literal(
                    [
                        component
                        for index in range(256)
                        for component in (index, index, index, 200)
                    ]
                ),
            },
        },
        {
            "surface": "PIL.ImagePalette.ImagePalette",
            "operation": "getcolor",
            "requirement_suffix": "behavior.default",
            "name": "full-rgba-palette-exhausted",
            "values": {
                "mode": literal("RGBA"),
                "color": literal([9, 8, 7, 255]),
                "palette": literal(
                    [
                        component
                        for index in range(256)
                        for component in (index, index, index, 255)
                    ]
                ),
            },
        },
        {
            "surface": "PIL.ImagePalette.ImagePalette",
            "operation": "copy",
            "requirement_suffix": "behavior.default",
            "name": "with-palette",
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getcolor",
            "requirement_suffix": "parameter.mode",
            "name": "hex-rgba",
            "values": {
                "color": literal("#ff000080"),
                "mode": literal("RGBA"),
            },
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getcolor",
            "requirement_suffix": "mode.l",
            "name": "rgb-syntax-l",
            "values": {
                "color": literal("rgb(255, 0, 0)"),
                "mode": literal("L"),
            },
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getcolor",
            "requirement_suffix": "parameter.mode",
            "name": "named-one",
            "values": {
                "color": literal("red"),
                "mode": literal("1"),
            },
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getcolor",
            "requirement_suffix": "parameter.mode",
            "name": "hex-la-alpha",
            "values": {
                "color": literal("#ff000080"),
                "mode": literal("LA"),
            },
        },
        {
            "surface": "PIL.ImageColor",
            "operation": "getcolor",
            "requirement_suffix": "parameter.color",
            "name": "hex-rgb-color",
            "values": {
                "color": literal("#204080"),
                "mode": literal("RGB"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "parameter.size",
            "name": "noninteger-ratio-lanczos",
            "observe_result": "tobytes",
            "values": {
                "size": literal([17, 9]),
                "resample": literal(1),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "parameter.resample",
            "name": "p-putpalette-putalpha-bilinear",
            "mode": "P",
            "chain": "p-putpalette-putalpha-resize",
            "observe_result": "tobytes",
            "values": {
                "size": literal([8, 8]),
                "resample": literal(2),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "parameter.resample",
            "name": "box-filter",
            "observe_result": "tobytes",
            "values": {
                "size": literal([7, 5]),
                "resample": literal(4),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "parameter.resample",
            "name": "hamming-filter",
            "observe_result": "tobytes",
            "values": {
                "size": literal([7, 5]),
                "resample": literal(5),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "parameter.resample",
            "name": "box-integer-ratio-boundary",
            "observe_result": "tobytes",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [255, 128, 64],
            "size": [16, 16],
            "values": {
                "size": literal([8, 8]),
                "resample": literal(4),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "parameter.resample",
            "name": "lanczos-kernel-boundaries",
            "observe_result": "tobytes",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [255, 128, 64],
            "size": [9, 8],
            "values": {
                "size": literal([9, 3]),
                "resample": literal(1),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "parameter.resample",
            "name": "hamming-kernel-boundaries",
            "observe_result": "tobytes",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [255, 128, 64],
            "size": [9, 8],
            "values": {
                "size": literal([9, 3]),
                "resample": literal(5),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "mode.rgba",
            "name": "rgba-nearest-nonuniform",
            "observe_result": "tobytes",
            "edge": "nonzero-pixel",
            "pixel": [200, 100, 50, 128],
            "size": [9, 8],
            "values": {
                "size": literal([9, 3]),
                "resample": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "mode.la",
            "name": "la-nearest-nonuniform",
            "observe_result": "tobytes",
            "mode": "LA",
            "edge": "nonzero-pixel",
            "pixel": [200, 128],
            "size": [9, 8],
            "values": {
                "size": literal([9, 3]),
                "resample": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "mode.rgba",
            "name": "rgba-transparent-convolution",
            "observe_result": "tobytes",
            "edge": "nonzero-pixel",
            "pixel": [200, 100, 50, 0],
            "size": [9, 8],
            "values": {
                "size": literal([9, 3]),
                "resample": literal(2),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "mode.la",
            "name": "la-transparent-convolution",
            "observe_result": "tobytes",
            "mode": "LA",
            "edge": "nonzero-pixel",
            "pixel": [200, 0],
            "size": [9, 8],
            "values": {
                "size": literal([9, 3]),
                "resample": literal(2),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "mode.i",
            "name": "i-nearest-nonuniform",
            "observe_result": "tobytes",
            "mode": "I",
            "edge": "nonzero-pixel",
            "pixel": 100000,
            "size": [9, 8],
            "values": {
                "size": literal([9, 3]),
                "resample": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "mode.i",
            "name": "i-convolution-positive",
            "observe_result": "tobytes",
            "mode": "I",
            "edge": "nonzero-pixel",
            "pixel": 100000,
            "size": [9, 8],
            "values": {
                "size": literal([9, 3]),
                "resample": literal(1),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "mode.i",
            "name": "i-convolution-negative",
            "observe_result": "tobytes",
            "mode": "I",
            "edge": "nonzero-pixel",
            "pixel": -100000,
            "size": [9, 8],
            "values": {
                "size": literal([9, 3]),
                "resample": literal(1),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "mode.i",
            "name": "i16n-frombytes-nearest",
            "scenario_inline_image": "i16n-frombytes",
            "observe_result": "tobytes",
            "values": {
                "size": literal([1, 1]),
                "resample": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "mode.i",
            "name": "i16n-frombytes-bilinear",
            "scenario_inline_image": "i16n-frombytes",
            "observe_result": "tobytes",
            "values": {
                "size": literal([3, 3]),
                "resample": literal(2),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "mode.i",
            "name": "i16l-frombytes-bilinear",
            "scenario_inline_image": "i16l-frombytes",
            "observe_result": "tobytes",
            "values": {
                "size": literal([3, 3]),
                "resample": literal(2),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "mode.i",
            "name": "i16b-frombytes-bilinear",
            "scenario_inline_image": "i16b-frombytes",
            "observe_result": "tobytes",
            "values": {
                "size": literal([3, 3]),
                "resample": literal(2),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "thumbnail",
            "requirement_suffix": "mode.rgb",
            "name": "rgb-reducing-downscale",
            "observe_receiver": True,
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [255, 128, 64],
            "size": [16, 16],
            "values": {
                "size": literal([2, 2]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "thumbnail",
            "requirement_suffix": "mode.rgba",
            "name": "rgba-alpha-downscale",
            "observe_receiver": True,
            "edge": "nonzero-pixel",
            "pixel": [200, 100, 50, 128],
            "size": [9, 8],
            "values": {
                "size": literal([4, 3]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "thumbnail",
            "requirement_suffix": "parameter.resample",
            "name": "bilinear-filter-name",
            "observe_receiver": True,
            "edge": "nonzero-pixel",
            "pixel": [255, 128, 64],
            "size": [16, 16],
            "values": {
                "size": literal([7, 5]),
                "resample": literal("BILINEAR"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "thumbnail",
            "requirement_suffix": "parameter.resample",
            "name": "box-filter-name",
            "observe_receiver": True,
            "edge": "nonzero-pixel",
            "pixel": [255, 128, 64],
            "size": [16, 16],
            "values": {
                "size": literal([7, 5]),
                "resample": literal("BOX"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "thumbnail",
            "requirement_suffix": "parameter.resample",
            "name": "hamming-filter-name",
            "observe_receiver": True,
            "edge": "nonzero-pixel",
            "pixel": [255, 128, 64],
            "size": [16, 16],
            "values": {
                "size": literal([7, 5]),
                "resample": literal("HAMMING"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "parameter.resample",
            "name": "unknown-filter",
            "values": {
                "size": literal([7, 5]),
                "resample": literal(999),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "parameter.size",
            "name": "zero-width",
            "values": {"size": literal([0, 5])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "parameter.size",
            "name": "zero-height",
            "values": {"size": literal([5, 0])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "parameter.size",
            "name": "negative-width",
            "values": {"size": literal([-1, 5])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "parameter.size",
            "name": "negative-height",
            "values": {"size": literal([5, -1])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "parameter.size",
            "name": "overflow-width",
            "values": {"size": literal([4294967296, 5])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "parameter.size",
            "name": "overflow-height",
            "values": {"size": literal([5, 4294967296])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "parameter.box",
            "name": "opened-png-without-idat-box",
            "scenario_inline_image": "png-no-idat",
            "values": {
                "size": literal([1, 1]),
                "box": literal([0, 0, 1, 1]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "thumbnail",
            "requirement_suffix": "parameter.resample",
            "name": "unknown-filter",
            "values": {
                "size": literal([7, 5]),
                "resample": literal("NOT_A_FILTER"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "thumbnail",
            "requirement_suffix": "parameter.size",
            "name": "negative-width",
            "values": {"size": literal([-1, 5])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "thumbnail",
            "requirement_suffix": "parameter.size",
            "name": "negative-height",
            "values": {"size": literal([5, -1])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "thumbnail",
            "requirement_suffix": "parameter.size",
            "name": "zero-size-nonempty-source",
            "values": {"size": literal([0, 0])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "thumbnail",
            "requirement_suffix": "parameter.size",
            "name": "negative-height-nonsquare",
            "mode": "RGB",
            "size": [16, 8],
            "edge": "nonzero-pixel",
            "pixel": [255, 128, 64],
            "values": {"size": literal([5, -1])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "thumbnail",
            "requirement_suffix": "parameter.size",
            "name": "zero-size-zero-width-source",
            "size": [0, 8],
            "values": {"size": literal([0, 0])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "thumbnail",
            "requirement_suffix": "parameter.size",
            "name": "zero-size-zero-height-source",
            "size": [8, 0],
            "values": {"size": literal([0, 0])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "thumbnail",
            "requirement_suffix": "parameter.size",
            "name": "negative-height-zero-width-source",
            "size": [0, 8],
            "values": {"size": literal([5, -1])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "thumbnail",
            "requirement_suffix": "parameter.size",
            "name": "negative-height-zero-height-source",
            "size": [8, 0],
            "values": {"size": literal([5, -1])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "thumbnail",
            "requirement_suffix": "parameter.size",
            "name": "negative-height-zero-size-source",
            "size": [0, 0],
            "values": {"size": literal([5, -1])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "thumbnail",
            "requirement_suffix": "parameter.size",
            "name": "negative-height-candidate-zero",
            "mode": "RGB",
            "size": [16, 1],
            "values": {"size": literal([1, -1])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "thumbnail",
            "requirement_suffix": "parameter.size",
            "name": "overflow-width-bound",
            "observe_receiver": True,
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [255, 128, 64],
            "size": [16, 16],
            "values": {"size": literal([4294967296, 5])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "thumbnail",
            "requirement_suffix": "parameter.size",
            "name": "overflow-height-bound",
            "observe_receiver": True,
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [255, 128, 64],
            "size": [16, 16],
            "values": {"size": literal([5, 4294967296])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "thumbnail",
            "requirement_suffix": "parameter.size",
            "name": "opened-png-without-idat-zero-width",
            "scenario_inline_image": "png-no-idat",
            "values": {"size": literal([0, 5])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "thumbnail",
            "requirement_suffix": "parameter.size",
            "name": "opened-png-without-idat-negative-height",
            "scenario_inline_image": "png-no-idat",
            "values": {"size": literal([5, -1])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "reduce",
            "requirement_suffix": "behavior.default",
            "name": "odd-size-factor-three",
            "observe_result": "tobytes",
            "mode": "RGB",
            "values": {
                "size": literal([17, 11]),
                "factor": literal([3, 3]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "reduce",
            "requirement_suffix": "behavior.default",
            "name": "non-square-factors",
            "observe_result": "tobytes",
            "mode": "RGB",
            "values": {
                "size": literal([17, 11]),
                "factor": literal([2, 3]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "reduce",
            "requirement_suffix": "parameter.factor",
            "name": "zero-factor",
            "values": {
                "factor": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "reduce",
            "requirement_suffix": "parameter.factor",
            "name": "float-factor-sequence",
            "values": {
                "factor": literal([2.5, 2.5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "reduce",
            "requirement_suffix": "parameter.factor",
            "name": "invalid-factor-string",
            "values": {
                "factor": literal("2"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "reduce",
            "requirement_suffix": "parameter.factor",
            "name": "wrong-factor-arity",
            "values": {
                "factor": literal([2, 2, 2]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "reduce",
            "requirement_suffix": "parameter.box",
            "name": "wrong-box-arity",
            "values": {
                "factor": literal([2, 2]),
                "box": literal([0, 0]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "reduce",
            "requirement_suffix": "parameter.box",
            "name": "invalid-box-type",
            "values": {
                "factor": literal([2, 2]),
                "box": literal(1),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.method",
            "name": "fast-octree-rgb",
            "observe_result": "tobytes",
            "mode": "RGB",
            "values": {
                "colors": literal(8),
                "method": literal(2),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.method",
            "name": "libimagequant-unavailable",
            "mode": "RGB",
            "values": {
                "colors": literal(8),
                "method": literal(3),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.method",
            "name": "rgba-mediancut-invalid",
            "mode": "RGBA",
            "values": {
                "colors": literal(8),
                "method": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "rotate",
            "requirement_suffix": "parameter.angle",
            "name": "fractional-expanded",
            "observe_result": "tobytes",
            "values": {
                "angle": literal(33.5),
                "expand": literal(True),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "rotate",
            "requirement_suffix": "parameter.resample",
            "name": "invalid-resample-name",
            "mode": "RGB",
            "values": {
                "angle": literal(1),
                "resample": literal("NOT_A_RESAMPLE"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "rotate",
            "requirement_suffix": "parameter.resample",
            "name": "invalid-resample-none",
            "mode": "RGB",
            "values": {
                "angle": literal(1),
                "resample": literal(None),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "rotate",
            "requirement_suffix": "parameter.resample",
            "name": "invalid-resample-lanczos",
            "mode": "RGB",
            "values": {
                "angle": literal(1),
                "resample": literal(1),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "rotate",
            "requirement_suffix": "parameter.resample",
            "name": "invalid-resample-box",
            "mode": "RGB",
            "values": {
                "angle": literal(1),
                "resample": literal(4),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "rotate",
            "requirement_suffix": "parameter.resample",
            "name": "invalid-resample-hamming",
            "mode": "RGB",
            "values": {
                "angle": literal(1),
                "resample": literal(5),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "rotate",
            "requirement_suffix": "parameter.resample",
            "name": "invalid-resample-unknown-code",
            "mode": "RGB",
            "values": {
                "angle": literal(1),
                "resample": literal(999),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "rotate",
            "requirement_suffix": "parameter.expand",
            "name": "non-boolean-expand",
            "mode": "RGB",
            "values": {
                "angle": literal(1),
                "expand": literal(1),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "rotate",
            "requirement_suffix": "parameter.center",
            "name": "invalid-center-arity",
            "mode": "RGB",
            "values": {
                "angle": literal(1),
                "center": literal([0]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "rotate",
            "requirement_suffix": "parameter.translate",
            "name": "invalid-translate-arity",
            "mode": "RGB",
            "values": {
                "angle": literal(1),
                "translate": literal([0]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "rotate",
            "requirement_suffix": "behavior.default",
            "name": "valid-center-translate",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [10, 20, 30],
            "values": {
                "angle": literal(33.5),
                "expand": literal(True),
                "center": literal([8.0, 8.0]),
                "translate": literal([1, -1]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "rotate",
            "requirement_suffix": "parameter.center",
            "name": "valid-center-only",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [10, 20, 30],
            "values": {
                "angle": literal(33.5),
                "expand": literal(True),
                "center": literal([8.0, 8.0]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "rotate",
            "requirement_suffix": "parameter.translate",
            "name": "valid-translate-only",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [10, 20, 30],
            "values": {
                "angle": literal(33.5),
                "expand": literal(True),
                "translate": literal([1, -1]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "rotate",
            "requirement_suffix": "parameter.fillcolor",
            "name": "expanded-red-fill",
            "mode": "RGB",
            "values": {
                "angle": literal(45),
                "expand": literal(True),
                "fillcolor": literal("red"),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "p-affine-scalar-fill",
            "observe_result": "tobytes",
            "mode": "P",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 0, 0, 1, 0]),
                "resample": literal(3),
                "fillcolor": literal(5),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "p-affine-tuple-fill",
            "observe_result": "tobytes",
            "mode": "P",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 0, 0, 1, 0]),
                "fillcolor": literal([10, 20, 30]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "rgb-affine-tuple-fill",
            "observe_result": "tobytes",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 0, 0, 1, 0]),
                "fillcolor": literal([10, 20, 30]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "cmyk-affine-tuple-fill",
            "observe_result": "tobytes",
            "mode": "CMYK",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([1, 2, 3, 4]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "cmyk-affine-three-tuple-fill",
            "observe_result": "tobytes",
            "mode": "CMYK",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([1, 2, 3]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "cmyk-affine-invalid-tuple-fill",
            "mode": "CMYK",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([1, 2]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "cmyk-affine-name-fill",
            "observe_result": "tobytes",
            "mode": "CMYK",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal("red"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "rgb-affine-scalar-negative-fill",
            "observe_result": "tobytes",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal(-1),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "rgba-affine-default-fill",
            "observe_result": "tobytes",
            "mode": "RGBA",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "rgb-affine-clamped-tuple-fill",
            "observe_result": "tobytes",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([-1, 256, 65536]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "l-affine-single-tuple-fill",
            "observe_result": "tobytes",
            "mode": "L",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([7]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "l-affine-invalid-tuple-fill",
            "mode": "L",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([7, 8]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "p-affine-single-tuple-fill",
            "observe_result": "tobytes",
            "mode": "P",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([7]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "la-affine-tuple-fill",
            "observe_result": "tobytes",
            "mode": "LA",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([7, 8]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "rgba-affine-tuple-fill",
            "observe_result": "tobytes",
            "mode": "RGBA",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([7, 8, 9, 10]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "f-affine-scalar-fill",
            "observe_result": "tobytes",
            "mode": "F",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal(2),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "i-affine-scalar-fill",
            "observe_result": "tobytes",
            "mode": "I",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal(258),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "i16-affine-scalar-fill",
            "observe_result": "tobytes",
            "mode": "I;16",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal(258),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "i-affine-invalid-tuple-fill",
            "mode": "I",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([7, 8]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "i16-affine-invalid-tuple-fill",
            "mode": "I;16",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([7, 8]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "rgb-affine-name-fill",
            "observe_result": "tobytes",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal("red"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "rgba-affine-explicit-alpha-name-fill",
            "observe_result": "tobytes",
            "mode": "RGBA",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal("#01020304"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "l-affine-name-fill",
            "observe_result": "tobytes",
            "mode": "L",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal("red"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "la-affine-name-fill",
            "observe_result": "tobytes",
            "mode": "LA",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal("red"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "hsv-affine-name-fill",
            "observe_result": "tobytes",
            "mode": "HSV",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal("red"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "f-affine-name-fill",
            "observe_result": "tobytes",
            "mode": "F",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal("red"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "i-affine-name-fill",
            "observe_result": "tobytes",
            "mode": "I",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal("red"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "i16-affine-name-fill",
            "observe_result": "tobytes",
            "mode": "I;16",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal("red"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "rgb-affine-invalid-tuple-fill",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([7, 8]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "f-affine-invalid-tuple-fill",
            "mode": "F",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([7, 8]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "la-affine-invalid-tuple-fill",
            "mode": "LA",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([7, 8, 9]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "l-affine-scalar-fill",
            "observe_result": "tobytes",
            "mode": "L",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal(7),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "rgb-affine-single-tuple-fill",
            "observe_result": "tobytes",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([7]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "rgba-affine-single-tuple-fill",
            "observe_result": "tobytes",
            "mode": "RGBA",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([7]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "f-affine-single-tuple-fill",
            "observe_result": "tobytes",
            "mode": "F",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([7]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "i-affine-single-tuple-fill",
            "observe_result": "tobytes",
            "mode": "I",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([7]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "i16-affine-single-tuple-fill",
            "observe_result": "tobytes",
            "mode": "I;16",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([7]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "la-affine-single-tuple-fill",
            "observe_result": "tobytes",
            "mode": "LA",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([7]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "p-affine-invalid-tuple-fill",
            "mode": "P",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([7, 8]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "p-affine-nonopaque-tuple-fill",
            "mode": "P",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([1, 2, 3, 4]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "p-affine-opaque-tuple-fill",
            "observe_result": "tobytes",
            "mode": "P",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([1, 2, 3, 255]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "rgba-affine-rgb-name-fill",
            "observe_result": "tobytes",
            "mode": "RGBA",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal("red"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "rgb-affine-explicit-alpha-name-fill",
            "observe_result": "tobytes",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal("#01020304"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "p-affine-name-fill",
            "observe_result": "tobytes",
            "mode": "P",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal("red"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "hsv-affine-tuple-fill",
            "observe_result": "tobytes",
            "mode": "HSV",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([7, 8, 9]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "ycbcr-affine-tuple-fill",
            "observe_result": "tobytes",
            "mode": "YCbCr",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([7, 8, 9]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "rgb-affine-rgba-tuple-fill",
            "observe_result": "tobytes",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([7, 8, 9, 10]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "hsv-affine-rgba-tuple-fill",
            "observe_result": "tobytes",
            "mode": "HSV",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([7, 8, 9, 10]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "ycbcr-affine-rgba-tuple-fill",
            "observe_result": "tobytes",
            "mode": "YCbCr",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 100, 0, 1, 100]),
                "fillcolor": literal([7, 8, 9, 10]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "i16-affine-inbounds-fill",
            "observe_result": "tobytes",
            "mode": "I;16",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 0, 0, 1, 0]),
                "fillcolor": literal(258),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.fillcolor",
            "name": "invalid-fillcolor-type",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 0, 0, 1, 0]),
                "fillcolor": literal([1.5, 2.5, 3.5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.data",
            "name": "invalid-data-type",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(4),
                "data": literal({"invalid": "mesh"}),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.method",
            "name": "extent-method",
            "observe_result": "tobytes",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(1),
                "data": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.data",
            "name": "extent-missing-data",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(1),
                "data": literal(None),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.data",
            "name": "extent-short-data",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(1),
                "data": literal([1, 1, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.data",
            "name": "extent-too-many-data",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(1),
                "data": literal([1, 1, 5, 5, 7]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.method",
            "name": "perspective-method",
            "observe_result": "tobytes",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(2),
                "data": literal([1, 0, 0, 0, 1, 0, 0, 0]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.data",
            "name": "perspective-missing-data",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(2),
                "data": literal(None),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.data",
            "name": "perspective-short-data",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(2),
                "data": literal([1, 0, 0]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.data",
            "name": "quad-short-data",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(3),
                "data": literal([1, 0, 0]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.method",
            "name": "quad-method",
            "observe_result": "tobytes",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(3),
                "data": literal([0, 0, 0, 6, 6, 6, 6, 0]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.data",
            "name": "invalid-scalar-data",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(4),
                "data": literal(1),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.fillcolor",
            "name": "invalid-fillcolor-mapping",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 0, 0, 1, 0]),
                "fillcolor": literal({"invalid": "color"}),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.method",
            "name": "unknown-method",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(5),
                "data": literal([1, 0, 0, 0, 1, 0]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.data",
            "name": "invalid-string-data",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(4),
                "data": literal("mesh"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.data",
            "name": "malformed-mesh-entry",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(4),
                "data": literal([[[0, 0, 6, 6]]]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.data",
            "name": "malformed-mesh-short-bbox",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(4),
                "data": literal([[[0, 0, 6], [0, 0, 6, 0, 6, 6, 0, 6]]]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.data",
            "name": "malformed-mesh-short-quad",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(4),
                "data": literal([[[0, 0, 6, 6], [0, 0, 6, 0, 6, 6, 0]]]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.data",
            "name": "malformed-mesh-too-many-items",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(4),
                "data": literal([
                    [[0, 0, 6, 6], [0, 0, 6, 0, 6, 6, 0, 6], []],
                ]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.data",
            "name": "raw-mesh-short-bbox-after-valid-item",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(4),
                "data": literal([
                    [[0, 0, 6, 6], [0, 0, 6, 0, 6, 6, 0, 6]],
                    [[0, 0, 6], [0, 0, 6, 0, 6, 6, 0, 6]],
                    [[0, 0, 6, 6], [0, 0, 6, 0, 6, 6, 0, 6], []],
                ]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.data",
            "name": "raw-mesh-short-quad-after-valid-item",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(4),
                "data": literal([
                    [[0, 0, 6, 6], [0, 0, 6, 0, 6, 6, 0, 6]],
                    [[0, 0, 6, 6], [0, 0, 6, 0, 6, 6, 0]],
                    [[0, 0, 6, 6], [0, 0, 6, 0, 6, 6, 0, 6], []],
                ]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.data",
            "name": "missing-mesh-data",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(4),
                "data": literal(None),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.data",
            "name": "empty-mesh-data",
            "observe_result": "tobytes",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(4),
                "data": literal([]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.data",
            "name": "empty-mesh-data-p-scalar-fill",
            "observe_result": "tobytes",
            "mode": "P",
            "values": {
                "size": literal([6, 6]),
                "method": literal(4),
                "data": literal([]),
                "fillcolor": literal(5),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.data",
            "name": "short-affine-matrix",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(0),
                "data": literal([1, 0, 0]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.data",
            "name": "flat-mesh-data",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(4),
                "data": literal([1, 0, 0, 0, 1, 0]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "parameter.data",
            "name": "identity-mesh",
            "observe_result": "tobytes",
            "mode": "RGB",
            "values": {
                "size": literal([6, 6]),
                "method": literal(4),
                "data": literal([
                    [[0, 0, 6, 6], [0, 0, 6, 0, 6, 6, 0, 6]],
                ]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "color-l",
            "observe_receiver": True,
            "mode": "L",
            "values": {
                "im": literal(255),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "opened-i16-scalar",
            "observe_receiver": True,
            "mode": "I;16",
            "scenario_inline_image": "l16-tiff",
            "values": {
                "im": literal(7),
                "box": literal([0, 0, 2, 2]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "opened-i16-two-tuple-error",
            "mode": "I;16",
            "scenario_inline_image": "l16-tiff",
            "values": {
                "im": literal([7, 9]),
                "box": literal([0, 0, 2, 2]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "opened-i16n-scalar",
            "observe_receiver": True,
            "mode": "I;16N",
            "scenario_inline_image": "i16n-frombytes",
            "values": {
                "im": literal(7),
                "box": literal([0, 0, 2, 2]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "color-rgb",
            "observe_receiver": True,
            "mode": "RGB",
            "values": {
                "im": literal([255, 0, 0]),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "color-rgba",
            "observe_receiver": True,
            "mode": "RGBA",
            "values": {
                "im": literal([255, 0, 0, 128]),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "color-rgb-tuple-into-cmyk",
            "observe_receiver": True,
            "mode": "CMYK",
            "values": {
                "im": literal([1, 2, 3]),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "color-rgba-tuple-into-rgb",
            "observe_receiver": True,
            "mode": "RGB",
            "values": {
                "im": literal([1, 2, 3, 4]),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "color-la",
            "observe_receiver": True,
            "mode": "LA",
            "values": {
                "im": literal([255, 128]),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "color-p",
            "observe_receiver": True,
            "mode": "P",
            "values": {
                "im": literal(5),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "color-cmyk",
            "observe_receiver": True,
            "mode": "CMYK",
            "values": {
                "im": literal([0, 255, 0, 0]),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "color-i",
            "observe_receiver": True,
            "mode": "I",
            "values": {
                "im": literal(100),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "color-one",
            "observe_receiver": True,
            "mode": "1",
            "values": {
                "im": literal(1),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "rgb-int-fill",
            "observe_receiver": True,
            "mode": "RGB",
            "values": {
                "im": literal(255),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "f-int-fill",
            "observe_receiver": True,
            "mode": "F",
            "values": {
                "im": literal(100),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "l-two-tuple-error",
            "mode": "L",
            "values": {
                "im": literal([255, 128]),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "la-four-tuple-error",
            "mode": "LA",
            "values": {
                "im": literal([255, 0, 0, 128]),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.im",
            "name": "f-five-tuple-error",
            "mode": "F",
            "values": {
                "im": literal([1, 2, 3, 4, 5]),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.im",
            "name": "pa-five-tuple-error",
            "mode": "PA",
            "values": {
                "im": literal([1, 2, 3, 4, 5]),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "rgb-two-tuple-error",
            "mode": "RGB",
            "values": {
                "im": literal([255, 128]),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "i-two-tuple-error",
            "mode": "I",
            "values": {
                "im": literal([1, 2]),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.im",
            "name": "empty-tuple-error",
            "mode": "L",
            "values": {
                "im": literal([]),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.im",
            "name": "none-source-error",
            "mode": "L",
            "values": {
                "im": literal(1.5),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.im",
            "name": "single-tuple-color",
            "observe_receiver": True,
            "mode": "L",
            "values": {
                "im": literal([255]),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.im",
            "name": "string-color-rgb",
            "observe_receiver": True,
            "mode": "RGB",
            "values": {
                "im": literal("red"),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.im",
            "name": "string-color-luma",
            "observe_receiver": True,
            "mode": "L",
            "values": {
                "im": literal("#204080"),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.im",
            "name": "string-color-la",
            "observe_receiver": True,
            "mode": "LA",
            "values": {
                "im": literal("red"),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.im",
            "name": "string-color-rgba",
            "observe_receiver": True,
            "mode": "RGBA",
            "values": {
                "im": literal("red"),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.im",
            "name": "string-color-hsv",
            "observe_receiver": True,
            "mode": "HSV",
            "values": {
                "im": literal("red"),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.im",
            "name": "string-color-error",
            "mode": "RGB",
            "values": {
                "im": literal("not-a-color"),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.im",
            "name": "five-tuple-error",
            "mode": "RGB",
            "values": {
                "im": literal([1, 2, 3, 4, 5]),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.box",
            "name": "box-three-items-error",
            "mode": "RGB",
            "values": {
                "im": literal([255, 0, 0]),
                "box": literal([1, 1, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.box",
            "name": "box-scalar-error",
            "mode": "RGB",
            "values": {
                "im": literal([255, 0, 0]),
                "box": literal(1),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.box",
            "name": "box-noninteger-sequence-error",
            "mode": "RGB",
            "values": {
                "im": literal([255, 0, 0]),
                "box": literal(["left", "top", "right"]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.box",
            "name": "image-inverted-height-box",
            "mode": "RGB",
            "im_mode": "RGB",
            "values": {
                "box": literal([0, 8, 8, 2]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.box",
            "name": "two-coordinate-overflow",
            "mode": "RGB",
            "values": {
                "im": literal([255, 0, 0]),
                "box": literal([2147483648, 0]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.box",
            "name": "image-two-coordinate-x-overflow",
            "mode": "RGB",
            "im_mode": "RGB",
            "values": {
                "box": literal([2147483648, 0]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.box",
            "name": "image-two-coordinate-y-overflow",
            "mode": "RGB",
            "im_mode": "RGB",
            "values": {
                "box": literal([0, 2147483648]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.box",
            "name": "image-box-left-overflow",
            "mode": "RGB",
            "im_mode": "RGB",
            "values": {
                "box": literal([2147483648, 0, 1, 1]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.box",
            "name": "image-box-left-underflow",
            "mode": "RGB",
            "im_mode": "RGB",
            "values": {
                "box": literal([-2147483649, 0, 1, 1]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.box",
            "name": "image-box-top-overflow",
            "mode": "RGB",
            "im_mode": "RGB",
            "values": {
                "box": literal([0, 2147483648, 1, 1]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.box",
            "name": "image-box-right-overflow",
            "mode": "RGB",
            "im_mode": "RGB",
            "values": {
                "box": literal([0, 0, 2147483648, 1]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.box",
            "name": "image-box-bottom-overflow",
            "mode": "RGB",
            "im_mode": "RGB",
            "values": {
                "box": literal([0, 0, 1, 2147483648]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.mask",
            "name": "two-coordinate-color-mask",
            "mode": "RGB",
            "mask_mode": "L",
            "values": {
                "im": literal(255),
                "box": literal([1, 1]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "behavior.default",
            "name": "offset-source",
            "observe_receiver": True,
            "mode": "RGBA",
            "values": {
                "source": literal([1, 1]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "behavior.default",
            "name": "source-scalar-error",
            "mode": "RGBA",
            "values": {
                "source": literal(1),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "l-bytes",
            "observe_receiver": True,
            "mode": "L",
            "values": {
                "data": bytes_literal(
                    [0, 32, 64, 96, 128, 160, 192, 224, 255]
                ),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "opened-i16-values",
            "observe_receiver": True,
            "mode": "I;16",
            "scenario_inline_image": "l16-tiff",
            "values": {
                "data": literal([1, 2, 3, 4]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "i16-bytes-sequence",
            "observe_receiver": True,
            "mode": "I;16",
            "values": {
                "data": bytes_literal([0x70, 0x11, 0x34, 0x12]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "i16l-bytes-sequence",
            "observe_receiver": True,
            "mode": "I;16L",
            "values": {
                "data": bytes_literal([0x70, 0x11, 0x34, 0x12]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "i16b-bytes-sequence",
            "observe_receiver": True,
            "mode": "I;16B",
            "values": {
                "data": bytes_literal([0x11, 0x70, 0x12, 0x34]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "i16n-bytes-sequence",
            "observe_receiver": True,
            "mode": "I;16N",
            "values": {
                "data": bytes_literal([0x70, 0x11, 0x34, 0x12]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "opened-i16-nested-sequence",
            "scenario_inline_image": "l16-tiff",
            "values": {
                "data": literal([[1], [2], [3], [4]]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "opened-i16n-values",
            "observe_receiver": True,
            "mode": "I;16N",
            "scenario_inline_image": "i16n-frombytes",
            "values": {
                "data": literal([1, 2, 3, 4]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "parameter.data",
            "name": "l-too-many-entries",
            "mode": "L",
            "size": [3, 3],
            "values": {"data": literal(list(range(10)))},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "i-mode",
            "observe_receiver": True,
            "mode": "I",
            "values": {
                "data": literal([1, 2, 3, 4, 5, 6, 7, 8, 9]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "f-mode",
            "observe_receiver": True,
            "mode": "F",
            "values": {
                "data": literal([0.5, 1.5, 2.5, 3.5, 4.5, 5.5, 6.5, 7.5, 8.5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "clipped-values",
            "observe_receiver": True,
            "mode": "L",
            "values": {
                "data": literal([300, -5, 128, 0, 255, 1, 2, 3, 4]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "scale-offset",
            "observe_receiver": True,
            "mode": "L",
            "values": {
                "data": literal([1, 2, 3, 4, 5, 6, 7, 8, 9]),
                "scale": literal(2),
                "offset": literal(10),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "linear_gradient",
            "requirement_suffix": "behavior.default",
            "name": "i-mode",
            "values": {"mode": literal("I")},
        },
        {
            "surface": "PIL.Image",
            "operation": "linear_gradient",
            "requirement_suffix": "behavior.default",
            "name": "f-mode",
            "values": {"mode": literal("F")},
        },
        {
            "surface": "PIL.Image",
            "operation": "linear_gradient",
            "requirement_suffix": "behavior.default",
            "name": "rgb-error",
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image",
            "operation": "linear_gradient",
            "requirement_suffix": "behavior.default",
            "name": "invalid-single-character-mode",
            "values": {"mode": literal("X")},
        },
        {
            "surface": "PIL.Image",
            "operation": "radial_gradient",
            "requirement_suffix": "behavior.default",
            "name": "i-mode",
            "values": {"mode": literal("I")},
        },
        {
            "surface": "PIL.Image",
            "operation": "radial_gradient",
            "requirement_suffix": "behavior.default",
            "name": "f-mode",
            "values": {"mode": literal("F")},
        },
        {
            "surface": "PIL.Image",
            "operation": "radial_gradient",
            "requirement_suffix": "behavior.default",
            "name": "rgb-error",
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image",
            "operation": "radial_gradient",
            "requirement_suffix": "behavior.default",
            "name": "invalid-single-character-mode",
            "values": {"mode": literal("X")},
        },
        {
            "surface": "PIL.Image",
            "operation": "effect_mandelbrot",
            "requirement_suffix": "behavior.default",
            "name": "zero-size",
            "values": {
                "size": literal([0, 0]),
                "extent": literal([-2.5, -1.5, 2.5, 1.5]),
                "quality": literal(100),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "effect_mandelbrot",
            "requirement_suffix": "behavior.default",
            "name": "quality-one-error",
            "values": {
                "size": literal([64, 64]),
                "extent": literal([-2.5, -1.5, 2.5, 1.5]),
                "quality": literal(1),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "effect_mandelbrot",
            "requirement_suffix": "behavior.default",
            "name": "quality-200",
            "values": {
                "size": literal([16, 16]),
                "extent": literal([-1.0, -1.0, 1.0, 1.0]),
                "quality": literal(200),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "effect_mandelbrot",
            "requirement_suffix": "behavior.default",
            "name": "zero-height",
            "values": {
                "size": literal([4, 0]),
                "extent": literal([-2.5, -1.5, 2.5, 1.5]),
                "quality": literal(100),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "effect_mandelbrot",
            "requirement_suffix": "behavior.default",
            "name": "negative-height",
            "values": {
                "size": literal([4, 4]),
                "extent": literal([-1.0, 1.0, 1.0, -1.0]),
                "quality": literal(10),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "effect_mandelbrot",
            "requirement_suffix": "parameter.extent",
            "name": "wrong-length",
            "values": {
                "size": literal([4, 4]),
                "extent": literal([0.0, 0.0, 1.0]),
                "quality": literal(10),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "merge",
            "requirement_suffix": "behavior.default",
            "name": "rgb-mode-nonzero",
            "mode": "RGB",
            "edge": "merge-rgb-nonzero",
        },
        {
            "surface": "PIL.Image",
            "operation": "merge",
            "requirement_suffix": "behavior.default",
            "name": "cmyk-mode",
            "mode": "CMYK",
        },
        {
            "surface": "PIL.Image",
            "operation": "merge",
            "requirement_suffix": "behavior.default",
            "name": "invalid-mode",
            "edge": "invalid-mode",
            "values": {"mode": literal("NOT_A_MODE")},
        },
        {
            "surface": "PIL.Image",
            "operation": "merge",
            "requirement_suffix": "behavior.default",
            "name": "wrong-band-count",
            "mode": "RGB",
            "edge": "wrong-band-count",
        },
        {
            "surface": "PIL.Image",
            "operation": "merge",
            "requirement_suffix": "behavior.default",
            "name": "invalid-band-item",
            "mode": "L",
            "edge": "invalid-band-item",
            "values": {"bands": literal([None])},
        },
        {
            "surface": "PIL.Image",
            "operation": "blend",
            "requirement_suffix": "behavior.default",
            "name": "mismatched-sizes",
            "edge": "second-smaller-than-first",
            "values": {"alpha": literal(0.25)},
        },
        {
            "surface": "PIL.Image",
            "operation": "blend",
            "requirement_suffix": "behavior.default",
            "name": "palette-mode-error",
            "mode": "P",
            "values": {"alpha": literal(0.25)},
        },
        {
            "surface": "PIL.Image",
            "operation": "blend",
            "requirement_suffix": "behavior.default",
            "name": "second-palette-mode-error",
            "mode": "RGB",
            "edge": "blend-second-palette",
            "values": {"alpha": literal(0.25)},
        },
        {
            "surface": "PIL.Image",
            "operation": "composite",
            "requirement_suffix": "behavior.default",
            "name": "mask-size-mismatch",
            "mode": "RGB",
            "mask_mode": "L",
            "edge": "mask-size-mismatch",
        },
        {
            "surface": "PIL.Image",
            "operation": "composite",
            "requirement_suffix": "behavior.default",
            "name": "bad-mask-mode",
            "mode": "RGB",
            "mask_mode": "RGB",
        },
        {
            "surface": "PIL.Image",
            "operation": "composite",
            "requirement_suffix": "behavior.default",
            "name": "source-mode-conversion",
            "mode": "RGB",
            "mask_mode": "L",
            "edge": "composite-mode-mismatch",
        },
        {
            "surface": "PIL.Image",
            "operation": "linear_gradient",
            "requirement_suffix": "behavior.default",
            "name": "l-mode",
            "values": {"mode": literal("L")},
        },
        {
            "surface": "PIL.Image",
            "operation": "linear_gradient",
            "requirement_suffix": "behavior.default",
            "name": "one-mode",
            "values": {"mode": literal("1")},
        },
        {
            "surface": "PIL.Image",
            "operation": "linear_gradient",
            "requirement_suffix": "behavior.default",
            "name": "p-mode",
            "values": {"mode": literal("P")},
        },
        {
            "surface": "PIL.Image",
            "operation": "radial_gradient",
            "requirement_suffix": "behavior.default",
            "name": "l-mode",
            "values": {"mode": literal("L")},
        },
        {
            "surface": "PIL.Image",
            "operation": "radial_gradient",
            "requirement_suffix": "behavior.default",
            "name": "one-mode",
            "values": {"mode": literal("1")},
        },
        {
            "surface": "PIL.Image",
            "operation": "radial_gradient",
            "requirement_suffix": "behavior.default",
            "name": "p-mode",
            "values": {"mode": literal("P")},
        },
        {
            "surface": "PIL.Image",
            "operation": "effect_mandelbrot",
            "requirement_suffix": "behavior.default",
            "name": "width-one",
            "values": {
                "size": literal([1, 4]),
                "extent": literal([-1.0, -1.0, 1.0, 1.0]),
                "quality": literal(10),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "effect_mandelbrot",
            "requirement_suffix": "behavior.default",
            "name": "height-one",
            "values": {
                "size": literal([4, 1]),
                "extent": literal([-1.0, -1.0, 1.0, 1.0]),
                "quality": literal(10),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "effect_mandelbrot",
            "requirement_suffix": "behavior.default",
            "name": "negative-extent",
            "values": {
                "size": literal([4, 4]),
                "extent": literal([1.0, -1.0, -1.0, 1.0]),
                "quality": literal(10),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "eval",
            "requirement_suffix": "behavior.default",
            "name": "rgb-replicated-lut",
            "mode": "RGB",
            "values": {
                "args": literal([list(range(256))]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "eval",
            "requirement_suffix": "behavior.default",
            "name": "rgb-expanded-lut",
            "mode": "RGB",
            "values": {
                "args": literal([[index % 256 for index in range(768)]]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "eval",
            "requirement_suffix": "behavior.default",
            "name": "invalid-lut-length",
            "mode": "RGB",
            "values": {"args": literal([[0]])},
        },
        {
            "surface": "PIL.Image",
            "operation": "eval",
            "requirement_suffix": "behavior.default",
            "name": "clamp-shift-callable",
            "mode": "RGB",
            "values": {
                "args": literal(["clamp-shift-callable"]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "eval",
            "requirement_suffix": "behavior.default",
            "name": "l-callable-expanded-path",
            "mode": "L",
            "values": {
                "args": literal(["clamp-shift-callable"]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "composite",
            "requirement_suffix": "behavior.default",
            "name": "one-mask",
            "mode": "RGB",
            "mask_mode": "1",
        },
        {
            "surface": "PIL.Image",
            "operation": "composite",
            "requirement_suffix": "behavior.default",
            "name": "p-output-l-mask",
            "mode": "P",
            "mask_mode": "L",
        },
        {
            "surface": "PIL.Image",
            "operation": "composite",
            "requirement_suffix": "behavior.default",
            "name": "rgba-mask",
            "mode": "RGB",
            "mask_mode": "RGBA",
        },
        {
            "surface": "PIL.ImageStat.Stat",
            "operation": "extrema",
            "requirement_suffix": "behavior.default",
            "name": "p-mode",
            "mode": "P",
        },
        {
            "surface": "PIL.ImageStat.Stat",
            "operation": "extrema",
            "requirement_suffix": "behavior.default",
            "name": "i-mode",
            "mode": "I",
        },
        {
            "surface": "PIL.ImageStat.Stat",
            "operation": "extrema",
            "requirement_suffix": "behavior.default",
            "name": "f-mode",
            "mode": "F",
        },
        {
            "surface": "PIL.ImageStat.Stat",
            "operation": "extrema",
            "requirement_suffix": "behavior.default",
            "name": "i-empty",
            "mode": "I",
            "edge": "zero-size",
        },
        {
            "surface": "PIL.ImageStat.Stat",
            "operation": "extrema",
            "requirement_suffix": "behavior.default",
            "name": "f-empty",
            "mode": "F",
            "edge": "zero-size",
        },
        {
            "surface": "PIL.ImageStat.Stat",
            "operation": "extrema",
            "requirement_suffix": "behavior.default",
            "name": "l-empty-frombytes",
            "mode": "L",
            "edge": "zero-size-frombytes",
        },
        {
            "surface": "PIL.ImageStat.Stat",
            "operation": "extrema",
            "requirement_suffix": "behavior.default",
            "name": "rgb-empty-frombytes",
            "mode": "RGB",
            "edge": "zero-size-frombytes",
        },
        {
            "surface": "PIL.ImageStat.Stat",
            "operation": "extrema",
            "requirement_suffix": "behavior.default",
            "name": "rgba-empty-frombytes",
            "mode": "RGBA",
            "edge": "zero-size-frombytes",
        },
        {
            "surface": "PIL.ImageStat.Stat",
            "operation": "extrema",
            "requirement_suffix": "behavior.default",
            "name": "i-varied",
            "mode": "I",
            "edge": "nonzero-pixel",
            "pixel": 200,
        },
        {
            "surface": "PIL.ImageStat.Stat",
            "operation": "extrema",
            "requirement_suffix": "behavior.default",
            "name": "f-varied",
            "mode": "F",
            "edge": "nonzero-pixel",
            "pixel": 200,
        },
        {
            "surface": "PIL.ImageStat.Stat",
            "operation": "extrema",
            "requirement_suffix": "behavior.default",
            "name": "i-median-scan",
            "mode": "I",
            "edge": "stat-median",
        },
        {
            "surface": "PIL.ImageStat.Stat",
            "operation": "extrema",
            "requirement_suffix": "behavior.default",
            "name": "f-median-scan",
            "mode": "F",
            "edge": "stat-median",
        },
        {
            "surface": "PIL.ImageStat.Stat",
            "operation": "var",
            "requirement_suffix": "mode.l",
            "name": "l-large-uniform-rounding",
            "mode": "L",
            "edge": "uniform-fill",
            "pixel": 182,
            "size": [7292605, 1],
        },
        {
            "surface": "PIL.ImageStat.Stat",
            "operation": "extrema",
            "requirement_suffix": "behavior.default",
            "name": "cmyk-mode",
            "mode": "CMYK",
        },
        {
            "surface": "PIL.ImageStat.Stat",
            "operation": "extrema",
            "requirement_suffix": "behavior.default",
            "name": "one-mode",
            "mode": "1",
        },
        {
            "surface": "PIL.ImageStat.Stat",
            "operation": "extrema",
            "requirement_suffix": "behavior.default",
            "name": "la-mode",
            "mode": "LA",
        },
        {
            "surface": "PIL.ImageStat.Stat",
            "operation": "extrema",
            "requirement_suffix": "behavior.default",
            "name": "rgb-mode",
            "mode": "RGB",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "one-mode",
            "observe_receiver": True,
            "mode": "1",
            "values": {
                "data": literal([0, 1, 0, 1, 0, 1, 0, 1, 0]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "p-index",
            "observe_receiver": True,
            "mode": "P",
            "values": {
                "xy": literal([0, 0]),
                "value": literal(7),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "p-tuple",
            "observe_receiver": True,
            "mode": "P",
            "values": {
                "xy": literal([0, 0]),
                "value": literal([10, 20, 30]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "p-palette-exact-match",
            "observe_receiver": True,
            "mode": "P",
            "values": {
                "xy": literal([0, 0]),
                "value": literal([0, 0, 0]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "p-palette-append",
            "observe_receiver": True,
            "mode": "P",
            "values": {
                "xy": literal([0, 0]),
                "value": literal([15, 25, 35]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "p-rgba-tuple-error",
            "mode": "P",
            "values": {
                "xy": literal([0, 0]),
                "value": literal([10, 20, 30, 40]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "one-tuple-equals-scalar",
            "observe_receiver": True,
            "mode": "RGBA",
            "values": {
                "xy": literal([1, 1]),
                "value": literal([200]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "p-one-tuple-index",
            "observe_receiver": True,
            "mode": "P",
            "values": {
                "xy": literal([1, 1]),
                "value": literal([5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "la-two-tuple",
            "observe_receiver": True,
            "mode": "LA",
            "values": {
                "xy": literal([0, 0]),
                "value": literal([200, 128]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "pa-scalar",
            "observe_receiver": True,
            "mode": "PA",
            "values": {
                "xy": literal([0, 0]),
                "value": literal(7),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "p-indices",
            "observe_receiver": True,
            "mode": "P",
            "values": {
                "data": literal([0, 1, 2, 3, 4, 5, 6, 7, 8]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "rgba-flat",
            "observe_receiver": True,
            "mode": "RGBA",
            "values": {
                "data": literal(
                    [255, 0, 0, 128] * 9
                ),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "la-packed",
            "observe_receiver": True,
            "mode": "LA",
            "values": {
                "data": literal([0x8000007F] * 9),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "rgb-packed",
            "observe_receiver": True,
            "mode": "RGB",
            "values": {
                "data": literal([0x00302010] * 9),
            },
        },
        {
            "surface": "PIL.ImageStat",
            "operation": "Stat",
            "requirement_suffix": "behavior.default",
            "name": "from-histogram-list",
            "observe_stat_properties": True,
            "values": {
                "image_or_list": literal(
                    [10 if index == 5 else 54 if index == 200 else 0 for index in range(256)]
                ),
            },
        },
        {
            "surface": "PIL.ImageStat",
            "operation": "Stat",
            "requirement_suffix": "behavior.default",
            "name": "empty-list",
            "observe_stat_properties": True,
            "values": {"image_or_list": literal([])},
        },
        {
            "surface": "PIL.ImageStat",
            "operation": "Stat",
            "requirement_suffix": "behavior.default",
            "name": "zero-histogram-list",
            "observe_stat_properties": True,
            "values": {"image_or_list": literal([0] * 256)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "save",
            "requirement_suffix": "behavior.default",
            "name": "p-png",
            "mode": "P",
            "values": {"format": literal("PNG")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "save",
            "requirement_suffix": "behavior.default",
            "name": "no-extension-default-format",
            "edge": "no-extension",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "save",
            "requirement_suffix": "behavior.default",
            "name": "i-png",
            "mode": "I",
            "values": {"format": literal("PNG")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "save",
            "requirement_suffix": "behavior.default",
            "name": "i-bmp-error",
            "mode": "I",
            "values": {"format": literal("BMP")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "save",
            "requirement_suffix": "behavior.default",
            "name": "f-png-error",
            "mode": "F",
            "values": {"format": literal("PNG")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "save",
            "requirement_suffix": "behavior.default",
            "name": "cmyk-png-error",
            "mode": "CMYK",
            "values": {"format": literal("PNG")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "save",
            "requirement_suffix": "behavior.default",
            "name": "f-bmp-error",
            "mode": "F",
            "values": {"format": literal("BMP")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "save",
            "requirement_suffix": "behavior.default",
            "name": "cmyk-bmp-error",
            "mode": "CMYK",
            "values": {"format": literal("BMP")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "rgba-tuples",
            "observe_receiver": True,
            "mode": "RGBA",
            "values": {
                "data": literal([[255, 0, 0, 128]] * 9),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "cmyk-tuples",
            "observe_receiver": True,
            "mode": "CMYK",
            "values": {
                "data": literal([[0, 255, 0, 0]] * 9),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "ycbcr-tuples",
            "observe_receiver": True,
            "mode": "YCbCr",
            "values": {
                "data": literal([[16, 128, 128]] * 9),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "hsv-tuples",
            "observe_receiver": True,
            "mode": "HSV",
            "values": {
                "data": literal([[16, 128, 128]] * 9),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "la-tuples",
            "observe_receiver": True,
            "mode": "LA",
            "values": {
                "data": literal([[255, 128]] * 9),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "la-invalid-component-count",
            "mode": "LA",
            "values": {
                "data": literal([[1, 2, 3]] * 9),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "la-scalar-float",
            "mode": "LA",
            "values": {"data": literal([1.5] * 9)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "rgb-invalid-component-count",
            "mode": "RGB",
            "values": {
                "data": literal([[1, 2]] * 9),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "l-nested-sequence",
            "mode": "L",
            "values": {
                "data": literal([[1, 2]] * 9),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "l-from-rgb",
            "observe_receiver": True,
            "mode": "L",
            "im_mode": "RGB",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "rgb-from-rgba",
            "observe_receiver": True,
            "mode": "RGB",
            "im_mode": "RGBA",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "p-from-l",
            "observe_receiver": True,
            "mode": "P",
            "im_mode": "L",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "behavior.default",
            "name": "offset-dest",
            "observe_receiver": True,
            "mode": "RGBA",
            "values": {
                "dest": literal([1, 1]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "behavior.default",
            "name": "source-larger-than-dest",
            "observe_receiver": True,
            "mode": "RGBA",
            "edge": "source-larger-than-dest",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "behavior.default",
            "name": "source-smaller-than-dest",
            "observe_receiver": True,
            "mode": "RGBA",
            "edge": "source-smaller-than-dest",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "behavior.default",
            "name": "source-four-tuple",
            "observe_receiver": True,
            "mode": "RGBA",
            "values": {
                "source": literal([2, 2, 8, 8]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "parameter.source",
            "name": "source-wrong-arity",
            "mode": "RGBA",
            "values": {
                "source": literal([0]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "parameter.source",
            "name": "source-negative-coordinate",
            "mode": "RGBA",
            "values": {
                "source": literal([-1, 0]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "parameter.source",
            "name": "source-x-overflow",
            "mode": "RGBA",
            "values": {
                "source": literal([2147483648, 0]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "parameter.source",
            "name": "source-y-overflow",
            "mode": "RGBA",
            "values": {
                "source": literal([0, 2147483648]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "parameter.source",
            "name": "source-left-overflow",
            "mode": "RGBA",
            "values": {
                "source": literal([2147483648, 0, 1, 1]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "parameter.source",
            "name": "source-top-overflow",
            "mode": "RGBA",
            "values": {
                "source": literal([0, 2147483648, 1, 1]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "parameter.source",
            "name": "source-right-overflow",
            "mode": "RGBA",
            "values": {
                "source": literal([0, 0, 2147483648, 1]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "parameter.source",
            "name": "source-bottom-overflow",
            "mode": "RGBA",
            "values": {
                "source": literal([0, 0, 1, 2147483648]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "parameter.source",
            "name": "source-coordinate-left-overflow",
            "mode": "RGBA",
            "values": {
                "source": literal([2147483648, 0, 2147483649, 1]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "parameter.source",
            "name": "source-coordinate-top-overflow",
            "mode": "RGBA",
            "values": {
                "source": literal([0, 2147483648, 1, 2147483649]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "parameter.source",
            "name": "source-coordinate-right-overflow",
            "mode": "RGBA",
            "values": {
                "source": literal([2147483647, 0, 2147483648, 1]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "parameter.source",
            "name": "source-coordinate-bottom-overflow",
            "mode": "RGBA",
            "values": {
                "source": literal([0, 2147483647, 1, 2147483648]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "parameter.source",
            "name": "source-inverted-box",
            "mode": "RGBA",
            "values": {
                "source": literal([8, 8, 2, 2]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "behavior.default",
            "name": "source-zero-width",
            "observe_receiver": True,
            "mode": "RGBA",
            "values": {
                "source": literal([0, 0, 0, 1]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "behavior.default",
            "name": "source-zero-height",
            "observe_receiver": True,
            "mode": "RGBA",
            "values": {
                "source": literal([0, 0, 1, 0]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "behavior.default",
            "name": "source-zero-size",
            "observe_receiver": True,
            "mode": "RGBA",
            "values": {
                "source": literal([0, 0, 0, 0]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "parameter.dest",
            "name": "dest-wrong-arity",
            "mode": "RGBA",
            "values": {
                "dest": literal([0]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "parameter.dest",
            "name": "dest-x-overflow",
            "mode": "RGBA",
            "values": {
                "dest": literal([2147483648, 0]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "parameter.dest",
            "name": "dest-y-overflow",
            "mode": "RGBA",
            "values": {
                "dest": literal([0, 2147483648]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "parameter.dest",
            "name": "dest-right-overflow",
            "mode": "RGBA",
            "values": {
                "dest": literal([2147483647, 0]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "parameter.dest",
            "name": "dest-bottom-overflow",
            "mode": "RGBA",
            "values": {
                "dest": literal([0, 2147483647]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "behavior.default",
            "name": "source-smaller-offset-dest",
            "observe_receiver": True,
            "mode": "RGBA",
            "edge": "source-smaller-than-dest",
            "values": {
                "dest": literal([1, 1]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "behavior.default",
            "name": "dest-scalar-error",
            "mode": "RGBA",
            "values": {
                "dest": literal(1),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "behavior.default",
            "name": "rgb-dest-mode-error",
            "mode": "RGB",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "behavior.default",
            "name": "rgba-dest-rgb-source-mode-error",
            "mode": "RGBA",
            "im_mode": "RGB",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "color-rgb-four-tuple",
            "mode": "RGB",
            "values": {
                "im": literal([255, 0, 0, 128]),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "degenerate-color-box-height",
            "mode": "RGB",
            "values": {
                "im": literal(255),
                "box": literal([2, 2, 4, 2]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "degenerate-image-box",
            "mode": "RGB",
            "im_mode": "RGB",
            "values": {
                "box": literal([2, 2, 2, 4]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "color-ycbcr-four-tuple",
            "mode": "YCbCr",
            "values": {
                "im": literal([255, 0, 0, 128]),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "color-hsv-four-tuple",
            "mode": "HSV",
            "values": {
                "im": literal([255, 0, 0, 128]),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "color-pa-int",
            "mode": "PA",
            "values": {
                "im": literal(7),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "color-pa-two-tuple",
            "mode": "PA",
            "values": {
                "im": literal([7, 3]),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "pa-from-p",
            "mode": "PA",
            "im_mode": "P",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "pa-from-l",
            "observe_receiver": True,
            "mode": "PA",
            "im_mode": "L",
            "values": {
                "box": literal([0, 0, 16, 16]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "i-two-tuple-mode-error",
            "mode": "I",
            "values": {
                "im": literal([1, 2]),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "behavior.default",
            "name": "mismatched-sizes",
            "mode": "RGBA",
            "edge": "second-smaller-than-first",
        },
        {
            "surface": "PIL.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "behavior.default",
            "name": "rgb-dest-mode-error",
            "mode": "RGB",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "parameter.mode",
            "name": "alpha-conversion",
            "values": {
                "mode": literal("LA"),
                "dither": literal("NONE"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.cmyk",
            "name": "rgb-to-cmyk",
            "mode": "RGB",
            "values": {"mode": literal("CMYK")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.cmyk",
            "name": "cmyk-to-cmyk",
            "mode": "CMYK",
            "values": {"mode": literal("CMYK")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.cmyk",
            "name": "hsv-to-cmyk",
            "mode": "HSV",
            "values": {"mode": literal("CMYK")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.cmyk",
            "name": "ycbcr-to-cmyk",
            "mode": "YCbCr",
            "values": {"mode": literal("CMYK")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.cmyk",
            "name": "i-to-cmyk",
            "mode": "I",
            "values": {"mode": literal("CMYK")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.cmyk",
            "name": "f-to-cmyk",
            "mode": "F",
            "values": {"mode": literal("CMYK")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.cmyk",
            "name": "p-to-cmyk",
            "mode": "P",
            "values": {"mode": literal("CMYK")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "cmyk-to-rgb",
            "mode": "CMYK",
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.l",
            "name": "cmyk-to-l",
            "mode": "CMYK",
            "values": {"mode": literal("L")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.hsv",
            "name": "rgb-to-hsv",
            "mode": "RGB",
            "values": {"mode": literal("HSV")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.hsv",
            "name": "rgb-to-hsv-nonzero",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [200, 100, 50],
            "values": {"mode": literal("HSV")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.hsv",
            "name": "rgb-to-hsv-g-max",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [50, 200, 100],
            "values": {"mode": literal("HSV")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.hsv",
            "name": "rgb-to-hsv-b-max",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [50, 100, 200],
            "values": {"mode": literal("HSV")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "hsv-to-rgb",
            "mode": "HSV",
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "hsv-sector-zero",
            "mode": "HSV",
            "edge": "nonzero-pixel",
            "pixel": [0, 255, 200],
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "hsv-sector-one",
            "mode": "HSV",
            "edge": "nonzero-pixel",
            "pixel": [50, 255, 200],
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "hsv-sector-three",
            "mode": "HSV",
            "edge": "nonzero-pixel",
            "pixel": [150, 255, 200],
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "hsv-sector-four",
            "mode": "HSV",
            "edge": "nonzero-pixel",
            "pixel": [200, 255, 200],
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "hsv-sector-five",
            "mode": "HSV",
            "edge": "nonzero-pixel",
            "pixel": [250, 255, 200],
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.ycbcr",
            "name": "rgb-to-ycbcr",
            "mode": "RGB",
            "values": {"mode": literal("YCbCr")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.ycbcr",
            "name": "rgb-to-ycbcr-nonzero",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [200, 100, 50],
            "values": {"mode": literal("YCbCr")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "ycbcr-to-rgb",
            "mode": "YCbCr",
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "i-to-rgb",
            "mode": "I",
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "f-to-rgb",
            "mode": "F",
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "p-to-rgb",
            "mode": "P",
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "l-to-rgb",
            "mode": "L",
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "one-to-l",
            "mode": "1",
            "values": {"mode": literal("L")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "one-to-rgb",
            "mode": "1",
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "one-to-cmyk",
            "mode": "1",
            "values": {"mode": literal("CMYK")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "ycbcr-to-1",
            "mode": "YCbCr",
            "values": {"mode": literal("1")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "hsv-to-1",
            "mode": "HSV",
            "values": {"mode": literal("1")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "cmyk-to-1",
            "mode": "CMYK",
            "values": {"mode": literal("1")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "i-to-1",
            "mode": "I",
            "values": {"mode": literal("1")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "f-to-1",
            "mode": "F",
            "values": {"mode": literal("1")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "rgb-to-1-dither-none",
            "mode": "RGB",
            "values": {
                "mode": literal("1"),
                "dither": literal("NONE"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "parameter.dither",
            "name": "integer-none-dither-high-luma",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [255, 255, 255],
            "values": {
                "mode": literal("1"),
                "dither": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "cmyk-to-la",
            "mode": "CMYK",
            "values": {"mode": literal("LA")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "cmyk-to-rgba",
            "mode": "CMYK",
            "values": {"mode": literal("RGBA")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "hsv-to-l",
            "mode": "HSV",
            "values": {"mode": literal("L")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "ycbcr-to-l",
            "mode": "YCbCr",
            "values": {"mode": literal("L")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "p-same-mode",
            "mode": "P",
            "values": {"mode": literal("P")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.p",
            "name": "rgb-to-p",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [200, 100, 50],
            "values": {"mode": literal("P")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.p",
            "name": "l-to-p",
            "mode": "L",
            "edge": "nonzero-pixel",
            "pixel": 200,
            "values": {"mode": literal("P")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "parameter.matrix",
            "name": "rgb-matrix-four",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [10, 20, 30],
            "values": {
                "mode": literal("L"),
                "matrix": literal([1, 0, 0, 4]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "parameter.matrix",
            "name": "rgb-matrix-twelve",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [10, 20, 30],
            "values": {
                "mode": literal("RGB"),
                "matrix": literal([1, 0, 0, 4, 0, 1, 0, 5, 0, 0, 1, 6]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "parameter.matrix",
            "name": "rgb-matrix-wrong-length",
            "mode": "RGB",
            "values": {
                "mode": literal("L"),
                "matrix": literal([1, 0]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "unknown-mode",
            "mode": "RGB",
            "values": {"mode": literal("BOGUS")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.rgba",
            "name": "l-to-rgba",
            "mode": "L",
            "values": {"mode": literal("RGBA")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.l",
            "name": "rgba-to-l",
            "mode": "RGBA",
            "values": {"mode": literal("L")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.i",
            "name": "rgb-to-i",
            "mode": "RGB",
            "values": {"mode": literal("I")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.f",
            "name": "rgb-to-f",
            "mode": "RGB",
            "values": {"mode": literal("F")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "la-to-rgb",
            "mode": "LA",
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "behavior.default",
            "name": "nonzero-rgb",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [200, 100, 50],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "behavior.default",
            "name": "green-only-rgb",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [0, 200, 30],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "behavior.default",
            "name": "blue-only-rgb",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [0, 0, 50],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "parameter.alpha-only",
            "name": "alpha-only-rgba",
            "mode": "RGBA",
            "edge": "nonzero-pixel",
            "pixel": [200, 100, 50, 128],
            "values": {"alpha_only": literal(True)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "parameter.alpha-only",
            "name": "transparent-alpha-rgba",
            "mode": "RGBA",
            "edge": "nonzero-pixel",
            "pixel": [200, 100, 50, 0],
            "values": {"alpha_only": literal(True)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "behavior.default",
            "name": "nonzero-cmyk",
            "mode": "CMYK",
            "edge": "nonzero-pixel",
            "pixel": [0, 0, 0, 200],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "behavior.default",
            "name": "nonzero-ycbcr",
            "mode": "YCbCr",
            "edge": "nonzero-pixel",
            "pixel": [200, 128, 128],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "behavior.default",
            "name": "nonzero-hsv",
            "mode": "HSV",
            "edge": "nonzero-pixel",
            "pixel": [200, 128, 128],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "behavior.default",
            "name": "nonzero-i",
            "mode": "I",
            "edge": "nonzero-pixel",
            "pixel": 200,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "behavior.default",
            "name": "nonzero-f",
            "mode": "F",
            "edge": "nonzero-pixel",
            "pixel": 200.5,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getextrema",
            "requirement_suffix": "behavior.default",
            "name": "nonzero-rgba",
            "mode": "RGBA",
            "edge": "nonzero-pixel",
            "pixel": [10, 200, 30, 255],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getextrema",
            "requirement_suffix": "behavior.default",
            "name": "i-large-nonzero",
            "mode": "I",
            "edge": "nonzero-pixel",
            "pixel": 100000,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getextrema",
            "requirement_suffix": "behavior.default",
            "name": "f-nonzero",
            "mode": "F",
            "edge": "nonzero-pixel",
            "pixel": 1.5,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getextrema",
            "requirement_suffix": "behavior.default",
            "name": "i-empty",
            "mode": "I",
            "edge": "zero-size",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getextrema",
            "requirement_suffix": "behavior.default",
            "name": "f-empty",
            "mode": "F",
            "edge": "zero-size",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getextrema",
            "requirement_suffix": "behavior.default",
            "name": "la-empty",
            "mode": "LA",
            "edge": "zero-size",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getextrema",
            "requirement_suffix": "behavior.default",
            "name": "rgba-empty",
            "mode": "RGBA",
            "edge": "zero-size",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getextrema",
            "requirement_suffix": "behavior.default",
            "name": "zero-width",
            "mode": "RGB",
            "edge": "zero-width",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getextrema",
            "requirement_suffix": "behavior.default",
            "name": "zero-height",
            "mode": "RGB",
            "edge": "zero-height",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "histogram",
            "requirement_suffix": "behavior.default",
            "name": "nonzero-rgba",
            "mode": "RGBA",
            "edge": "nonzero-pixel",
            "pixel": [5, 250, 128, 255],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "histogram",
            "requirement_suffix": "parameter.mask",
            "name": "masked-region",
            "mode": "RGB",
            "mask_mode": "L",
            "edge": "nonzero-pixel",
            "pixel": [10, 200, 30],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "histogram",
            "requirement_suffix": "parameter.mask",
            "name": "one-mask",
            "mode": "RGB",
            "mask_mode": "1",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "histogram",
            "requirement_suffix": "parameter.mask",
            "name": "masked-nonzero-rgb",
            "mode": "RGB",
            "mask_mode": "L",
            "edge": "mask-nonzero-pixel",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "histogram",
            "requirement_suffix": "parameter.mask",
            "name": "masked-nonzero-l",
            "mode": "L",
            "mask_mode": "L",
            "edge": "mask-nonzero-pixel",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "histogram",
            "requirement_suffix": "parameter.mask",
            "name": "masked-nonzero-la",
            "mode": "LA",
            "mask_mode": "L",
            "edge": "mask-nonzero-pixel",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "histogram",
            "requirement_suffix": "parameter.mask",
            "name": "masked-nonzero-rgba",
            "mode": "RGBA",
            "mask_mode": "L",
            "edge": "mask-nonzero-pixel",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "entropy",
            "requirement_suffix": "parameter.extrema",
            "name": "masked-region",
            "mode": "RGB",
            "mask_mode": "L",
            "edge": "nonzero-pixel",
            "pixel": [10, 200, 30],
        },
        {
            "surface": "PIL.ImageChops",
            "operation": "invert",
            "requirement_suffix": "behavior.default",
            "name": "la",
            "mode": "LA",
        },
        {
            "surface": "PIL.ImageChops",
            "operation": "invert",
            "requirement_suffix": "behavior.default",
            "name": "rgba",
            "mode": "RGBA",
        },
        {
            "surface": "PIL.ImageChops",
            "operation": "add",
            "requirement_suffix": "behavior.default",
            "name": "scale-offset",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [200, 100, 50],
            "values": {
                "scale": literal(2),
                "offset": literal(1),
            },
        },
        {
            "surface": "PIL.ImageChops",
            "operation": "add",
            "requirement_suffix": "behavior.default",
            "name": "materialized-p",
            "mode": "P",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageChops",
            "operation": "subtract",
            "requirement_suffix": "behavior.default",
            "name": "scale-offset",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [200, 100, 50],
            "values": {
                "scale": literal(2),
                "offset": literal(1),
            },
        },
        {
            "surface": "PIL.ImageChops",
            "operation": "subtract",
            "requirement_suffix": "behavior.default",
            "name": "materialized-p",
            "mode": "P",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageChops",
            "operation": "blend",
            "requirement_suffix": "behavior.default",
            "name": "extrapolate-alpha",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [200, 100, 50],
            "values": {
                "alpha": literal(2),
            },
        },
        {
            "surface": "PIL.ImageChops",
            "operation": "blend",
            "requirement_suffix": "behavior.default",
            "name": "materialized-p",
            "mode": "P",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageChops",
            "operation": "blend",
            "requirement_suffix": "behavior.default",
            "name": "palette-pipeline-second",
            "mode": "P",
            "edge": "palette-pipeline",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "fit",
            "requirement_suffix": "parameter.size",
            "name": "fractional-centering",
            "observe_result": "tobytes",
            "values": {
                "size": literal([13, 7]),
                "centering": literal([0.25, 0.75]),
                "resample": literal("BICUBIC"),
            },
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "fit",
            "requirement_suffix": "parameter.centering",
            "name": "half-centered",
            "values": {
                "size": literal([13, 7]),
                "centering": literal([0.5, 0.25]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "fit",
            "requirement_suffix": "parameter.size",
            "name": "wide-source-crop",
            "values": {
                "size": literal([7, 13]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "fit",
            "requirement_suffix": "parameter.method",
            "name": "numeric-method-default-centering",
            "values": {
                "size": literal([13, 7]),
                "method": literal(0),
                "centering": literal([0.5, 0.5]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "autocontrast",
            "requirement_suffix": "parameter.mask",
            "name": "valid-l-mask",
            "mode": "RGB",
            "mask_mode": "L",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "autocontrast",
            "requirement_suffix": "parameter.mask",
            "name": "mask-size-mismatch",
            "mode": "RGB",
            "mask_mode": "L",
            "edge": "mask-size-mismatch",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "autocontrast",
            "requirement_suffix": "parameter.mask",
            "name": "invalid-mask-type",
            "mode": "RGB",
            "chain": "truthy-non-image-mask",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "equalize",
            "requirement_suffix": "parameter.mask",
            "name": "valid-l-mask",
            "mode": "RGB",
            "mask_mode": "L",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "equalize",
            "requirement_suffix": "parameter.mask",
            "name": "mask-size-mismatch",
            "mode": "RGB",
            "mask_mode": "L",
            "edge": "mask-size-mismatch",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "equalize",
            "requirement_suffix": "parameter.mask",
            "name": "invalid-mask-type",
            "mode": "RGB",
            "chain": "truthy-non-image-mask",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "fit",
            "requirement_suffix": "parameter.centering",
            "name": "short-centering",
            "values": {"centering": literal([0.5])},
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "fit",
            "requirement_suffix": "parameter.centering",
            "name": "long-centering",
            "values": {"centering": literal([0.5, 0.5, 0.5])},
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "fit",
            "requirement_suffix": "parameter.centering",
            "name": "default-centering-pair",
            "values": {"centering": literal([0.5, 0.5])},
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "fit",
            "requirement_suffix": "parameter.centering",
            "name": "invalid-centering-none",
            "mode": "RGB",
            "chain": "none-centering-input",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.centering",
            "name": "fractional-centering",
            "values": {
                "size": literal([20, 12]),
                "centering": literal([0.25, 0.75]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.size",
            "name": "vertical-padding",
            "values": {
                "size": literal([12, 20]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.size",
            "name": "half-rounded-contain",
            "size": [4, 3],
            "values": {
                "size": literal([2, 2]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.method",
            "name": "numeric-method-default-color",
            "values": {
                "size": literal([20, 12]),
                "method": literal(0),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.size",
            "name": "palette-vertical-padding",
            "mode": "P",
            "values": {
                "size": literal([12, 20]),
                "color": literal(5),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.size",
            "name": "palette-alpha-vertical-padding",
            "mode": "PA",
            "values": {
                "size": literal([12, 20]),
                "color": literal([5, 7]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "rgb-color",
            "mode": "RGB",
            "values": {
                "size": literal([20, 12]),
                "color": literal([1, 2, 3]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "rgba-color",
            "mode": "RGBA",
            "values": {
                "size": literal([20, 12]),
                "color": literal([1, 2, 3, 4]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "rgb-color-name",
            "mode": "RGB",
            "values": {
                "size": literal([20, 12]),
                "color": literal("red"),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "rgba-color-name",
            "mode": "RGBA",
            "values": {
                "size": literal([20, 12]),
                "color": literal("red"),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "invalid-color-name",
            "mode": "RGB",
            "values": {
                "size": literal([20, 12]),
                "color": literal("bad"),
            },
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "invalid-color-length",
            "mode": "RGB",
            "values": {
                "size": literal([20, 12]),
                "color": literal([1, 2]),
            },
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "invalid-f-color",
            "mode": "F",
            "values": {
                "color": literal([1, 2]),
                "size": literal([20, 12]),
            },
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "f-color-name",
            "mode": "F",
            "values": {
                "color": literal("red"),
                "size": literal([20, 12]),
            },
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "invalid-l-color",
            "mode": "L",
            "values": {
                "color": literal([1, 2]),
                "size": literal([20, 12]),
            },
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "invalid-la-color",
            "mode": "LA",
            "values": {
                "color": literal([1, 2, 3]),
                "size": literal([20, 12]),
            },
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "la-color-two-components",
            "mode": "LA",
            "values": {
                "color": literal([64, 192]),
                "size": literal([20, 12]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "la-color-name",
            "mode": "LA",
            "values": {
                "color": literal("red"),
                "size": literal([20, 12]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "l-color-name",
            "mode": "L",
            "values": {
                "color": literal("red"),
                "size": literal([20, 12]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "l-one-component",
            "mode": "L",
            "values": {
                "color": literal([64]),
                "size": literal([20, 12]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "la-scalar",
            "mode": "LA",
            "values": {
                "color": literal(64),
                "size": literal([20, 12]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "rgba-explicit-alpha-name",
            "mode": "RGBA",
            "values": {
                "color": literal("#10203080"),
                "size": literal([20, 12]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "hsv-color-name",
            "mode": "HSV",
            "values": {
                "color": literal("red"),
                "size": literal([20, 12]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "p-scalar-color",
            "mode": "P",
            "values": {
                "size": literal([20, 12]),
                "color": literal(5),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "p-three-components",
            "mode": "P",
            "values": {
                "size": literal([20, 12]),
                "color": literal([5, 6, 7]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "p-color-name",
            "mode": "P",
            "values": {
                "size": literal([20, 12]),
                "color": literal("red"),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "p-four-components",
            "mode": "P",
            "values": {
                "size": literal([20, 12]),
                "color": literal([5, 6, 7, 8]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "p-four-components-opaque",
            "mode": "P",
            "values": {
                "size": literal([20, 12]),
                "color": literal([5, 6, 7, 255]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "pa-scalar-color",
            "mode": "PA",
            "values": {
                "size": literal([20, 12]),
                "color": literal(5),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "pa-two-components",
            "mode": "PA",
            "values": {
                "size": literal([20, 12]),
                "color": literal([5, 7]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "rgba-scalar-color",
            "mode": "RGBA",
            "values": {
                "size": literal([20, 12]),
                "color": literal(5),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "invalid-color-type",
            "mode": "RGB",
            "chain": "image-color-input",
            "values": {
                "size": literal([20, 12]),
            },
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "invalid-l-three-components",
            "mode": "L",
            "values": {
                "size": literal([20, 12]),
                "color": literal([1, 2, 3]),
            },
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "invalid-l-four-components",
            "mode": "L",
            "values": {
                "size": literal([20, 12]),
                "color": literal([1, 2, 3, 4]),
            },
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "invalid-la-four-components",
            "mode": "LA",
            "values": {
                "size": literal([20, 12]),
                "color": literal([1, 2, 3, 4]),
            },
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.color",
            "name": "rgba-three-components",
            "mode": "RGBA",
            "values": {
                "size": literal([20, 12]),
                "color": literal([1, 2, 3]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "contain",
            "requirement_suffix": "parameter.method",
            "name": "numeric-method",
            "values": {
                "method": literal(0),
                "size": literal([11, 7]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "contain",
            "requirement_suffix": "parameter.size",
            "name": "wide-target",
            "values": {
                "size": literal([7, 11]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "contain",
            "requirement_suffix": "parameter.size",
            "name": "half-rounded-height",
            "size": [4, 3],
            "values": {
                "size": literal([2, 2]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "contain",
            "requirement_suffix": "parameter.size",
            "name": "half-rounded-even-height",
            "size": [4, 3],
            "values": {
                "size": literal([6, 5]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "cover",
            "requirement_suffix": "parameter.method",
            "name": "numeric-method",
            "values": {
                "method": literal(0),
                "size": literal([11, 7]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "cover",
            "requirement_suffix": "parameter.size",
            "name": "wide-target",
            "values": {
                "size": literal([7, 11]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "cover",
            "requirement_suffix": "parameter.size",
            "name": "half-rounded-height",
            "size": [4, 3],
            "values": {
                "size": literal([2, 1]),
            },
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "autocontrast",
            "requirement_suffix": "mode.l",
            "name": "materialized-l",
            "mode": "L",
            "edge": "nonzero-pixel",
            "pixel": 120,
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "equalize",
            "requirement_suffix": "mode.l",
            "name": "materialized-l",
            "mode": "L",
            "edge": "nonzero-pixel",
            "pixel": 120,
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "equalize",
            "requirement_suffix": "behavior.default",
            "name": "materialized-p",
            "mode": "P",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "equalize",
            "requirement_suffix": "behavior.default",
            "name": "opened-palette",
            "scenario_asset": "image/p-small.png",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "invert",
            "requirement_suffix": "behavior.default",
            "name": "materialized-rgb",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [12, 34, 56],
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "flip",
            "requirement_suffix": "behavior.default",
            "name": "materialized-rgb",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [12, 34, 56],
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "mirror",
            "requirement_suffix": "behavior.default",
            "name": "materialized-rgb",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [12, 34, 56],
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "posterize",
            "requirement_suffix": "parameter.bits",
            "name": "materialized-rgb",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [12, 34, 56],
            "values": {"bits": literal(4)},
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "solarize",
            "requirement_suffix": "parameter.threshold",
            "name": "materialized-l",
            "mode": "L",
            "edge": "nonzero-pixel",
            "pixel": 180,
            "values": {"threshold": literal(100)},
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "grayscale",
            "requirement_suffix": "mode.rgb",
            "name": "materialized-rgb",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [12, 34, 56],
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "expand",
            "requirement_suffix": "parameter.border",
            "name": "materialized-border",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [12, 34, 56],
            "values": {"border": literal(2)},
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "crop",
            "requirement_suffix": "parameter.border",
            "name": "materialized-border",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [12, 34, 56],
            "values": {"border": literal(2)},
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "crop",
            "requirement_suffix": "parameter.border",
            "name": "border-exceeds-image",
            "values": {"border": literal(8)},
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "scale",
            "requirement_suffix": "parameter.factor",
            "name": "materialized-upscale",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [12, 34, 56],
            "values": {"factor": literal(1.5)},
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "contain",
            "requirement_suffix": "parameter.size",
            "name": "materialized-aspect",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [12, 34, 56],
            "values": {"size": literal([11, 7])},
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "cover",
            "requirement_suffix": "parameter.size",
            "name": "materialized-aspect",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [12, 34, 56],
            "values": {"size": literal([11, 7])},
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "pad",
            "requirement_suffix": "parameter.size",
            "name": "materialized-padding",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [12, 34, 56],
            "values": {"size": literal([20, 12])},
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "colorize",
            "requirement_suffix": "behavior.default",
            "name": "two-color",
            "mode": "L",
            "edge": "nonzero-pixel",
            "pixel": 128,
            "observe_result": "tobytes",
            "values": {
                "black": literal("black"),
                "white": literal("white"),
            },
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "colorize",
            "requirement_suffix": "parameter.mid",
            "name": "three-color",
            "mode": "L",
            "edge": "nonzero-pixel",
            "pixel": 128,
            "observe_result": "tobytes",
            "values": {
                "black": literal("black"),
                "white": literal("white"),
                "mid": literal("red"),
                "midpoint": literal(127),
            },
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "colorize",
            "requirement_suffix": "behavior.default",
            "name": "mapped-points",
            "mode": "L",
            "edge": "nonzero-pixel",
            "pixel": 128,
            "observe_result": "tobytes",
            "values": {
                "black": literal([0, 0, 255]),
                "white": literal([255, 255, 0]),
                "blackpoint": literal(50),
                "whitepoint": literal(200),
            },
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "colorize",
            "requirement_suffix": "behavior.default",
            "name": "mode-one",
            "mode": "1",
            "values": {
                "black": literal([0, 0, 0]),
                "white": literal([255, 255, 255]),
            },
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "colorize",
            "requirement_suffix": "behavior.default",
            "name": "invalid-points",
            "mode": "L",
            "values": {
                "black": literal([0, 0, 0]),
                "white": literal([255, 255, 255]),
                "blackpoint": literal(200),
                "whitepoint": literal(50),
            },
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "colorize",
            "requirement_suffix": "behavior.default",
            "name": "invalid-mid-points",
            "mode": "L",
            "values": {
                "black": literal([0, 0, 0]),
                "white": literal([255, 255, 255]),
                "mid": literal("red"),
                "blackpoint": literal(100),
                "midpoint": literal(50),
            },
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "colorize",
            "requirement_suffix": "behavior.default",
            "name": "invalid-midpoint-above-whitepoint",
            "mode": "L",
            "values": {
                "black": literal([0, 0, 0]),
                "white": literal([255, 255, 255]),
                "mid": literal("red"),
                "midpoint": literal(200),
                "whitepoint": literal(100),
            },
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "invert",
            "requirement_suffix": "behavior.default",
            "name": "p-mode",
            "mode": "P",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "exif_transpose",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-orientation6-materialized",
            "scenario_asset": "image/exif-orientation6.jpg",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "exif_transpose",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-le-orientation2-materialized",
            "exif_variant": "le-orientation2",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "exif_transpose",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-le-orientation4-materialized",
            "exif_variant": "le-orientation4",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "exif_transpose",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-le-orientation5-materialized",
            "exif_variant": "le-orientation5",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "exif_transpose",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-le-orientation7-materialized",
            "exif_variant": "le-orientation7",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "exif_transpose",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-le-orientation8-materialized",
            "exif_variant": "le-orientation8",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "exif_transpose",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-be-orientation3-materialized",
            "exif_variant": "be-orientation3",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "exif_transpose",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-be-width-before-orientation-materialized",
            "exif_variant": "be-non-orientation-before-orientation",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "exif_transpose",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-no-orientation",
            "exif_variant": "no-orientation",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "exif_transpose",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-invalid-magic",
            "exif_variant": "invalid-magic",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "exif_transpose",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-invalid-byte-order",
            "exif_variant": "invalid-byte-order",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "exif_transpose",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-short-tiff",
            "exif_variant": "short-tiff",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "exif_transpose",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-short-exif-payload",
            "exif_variant": "short-exif-payload",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "exif_transpose",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-invalid-offset",
            "exif_variant": "invalid-offset",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "exif_transpose",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-truncated-entry",
            "exif_variant": "truncated-entry",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "exif_transpose",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-non-orientation-before-orientation",
            "exif_variant": "non-orientation-before-orientation",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "exif_transpose",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-invalid-orientation",
            "exif_variant": "invalid-orientation",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "exif_transpose",
            "requirement_suffix": "parameter.in-place",
            "name": "jpeg-le-orientation2-in-place",
            "exif_variant": "le-orientation2",
            "values": {"in_place": literal(True)},
            "observe_receiver": True,
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "exif_transpose",
            "requirement_suffix": "behavior.default",
            "name": "tiff-no-orientation",
            "scenario_asset": "image/rgb-small.tiff",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "equalize",
            "requirement_suffix": "behavior.default",
            "name": "unsupported-mode-cmyk",
            "mode": "CMYK",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "equalize",
            "requirement_suffix": "behavior.default",
            "name": "unsupported-mode-i",
            "mode": "I",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "equalize",
            "requirement_suffix": "behavior.default",
            "name": "unsupported-mode-f",
            "mode": "F",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "equalize",
            "requirement_suffix": "behavior.default",
            "name": "unsupported-mode-one",
            "mode": "1",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "equalize",
            "requirement_suffix": "behavior.default",
            "name": "palette-mode",
            "mode": "P",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "autocontrast",
            "requirement_suffix": "behavior.default",
            "name": "unsupported-mode-cmyk",
            "mode": "CMYK",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "autocontrast",
            "requirement_suffix": "behavior.default",
            "name": "unsupported-mode-i",
            "mode": "I",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "autocontrast",
            "requirement_suffix": "behavior.default",
            "name": "unsupported-mode-f",
            "mode": "F",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "autocontrast",
            "requirement_suffix": "behavior.default",
            "name": "unsupported-mode-one",
            "mode": "1",
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "autocontrast",
            "requirement_suffix": "behavior.default",
            "name": "p-mode",
            "mode": "P",
        },
        {
            "surface": "PIL.ImageFilter",
            "operation": "Kernel",
            "requirement_suffix": "behavior.default",
            "name": "three-by-three-edge",
            "values": {
                "size": literal([3, 3]),
                "kernel": literal([0, -1, 0, -1, 5, -1, 0, -1, 0]),
            },
            "mode": "RGB",
        },
        {
            "surface": "PIL.ImageFilter",
            "operation": "Kernel",
            "requirement_suffix": "behavior.default",
            "name": "five-by-five",
            "values": {
                "size": literal([5, 5]),
                "kernel": literal(list(range(1, 26))),
            },
            "mode": "RGB",
        },
        {
            "surface": "PIL.ImageFilter",
            "operation": "Kernel",
            "requirement_suffix": "behavior.default",
            "name": "bad-size",
            "values": {
                "size": literal([4, 4]),
                "kernel": literal([1] * 16),
            },
            "mode": "RGB",
        },
        {
            "surface": "PIL.ImageFilter",
            "operation": "Kernel",
            "requirement_suffix": "behavior.default",
            "name": "short-kernel",
            "values": {
                "size": literal([3, 3]),
                "kernel": literal([1, 2]),
            },
            "mode": "RGB",
        },
        {
            "surface": "PIL.ImageFilter",
            "operation": "Kernel",
            "requirement_suffix": "behavior.default",
            "name": "non-square-size",
            "values": {
                "size": literal([3, 5]),
                "kernel": literal([1.0] * 15),
            },
            "mode": "RGB",
        },
        {
            "surface": "PIL.ImageFilter",
            "operation": "ModeFilter",
            "requirement_suffix": "behavior.default",
            "name": "nonzero-mode-selection",
            "mode": "L",
            "edge": "mode-filter-pattern",
            "values": {"size": literal(3)},
        },
        {
            "surface": "PIL.ImageFilter",
            "operation": "ModeFilter",
            "requirement_suffix": "behavior.default",
            "name": "no-majority",
            "mode": "L",
            "edge": "mode-filter-no-majority",
            "values": {"size": literal(3)},
        },
        {
            "surface": "PIL.ImageFilter",
            "operation": "ModeFilter",
            "requirement_suffix": "behavior.default",
            "name": "explicit-one-mode",
            "mode": "1",
            "edge": "nonzero-pixel",
            "pixel": 1,
            "values": {"size": literal(3)},
        },
        {
            "surface": "PIL.ImageFilter",
            "operation": "UnsharpMask",
            "requirement_suffix": "behavior.default",
            "name": "nonuniform-l-threshold",
            "mode": "L",
            "edge": "nonzero-pixel",
            "pixel": 255,
            "values": {
                "radius": literal(2),
                "percent": literal(200),
                "threshold": literal(0),
            },
        },
        {
            "surface": "PIL.ImageFilter",
            "operation": "UnsharpMask",
            "requirement_suffix": "behavior.default",
            "name": "nonuniform-rgba-threshold",
            "mode": "RGBA",
            "edge": "nonzero-pixel",
            "pixel": [255, 0, 0, 255],
            "values": {
                "radius": literal(2),
                "percent": literal(200),
                "threshold": literal(0),
            },
        },
        {
            "surface": "PIL.ImageFilter",
            "operation": "UnsharpMask",
            "requirement_suffix": "behavior.default",
            "name": "nonuniform-l-in-range-clip",
            "mode": "L",
            "edge": "nonzero-pixel",
            "pixel": 100,
            "values": {
                "radius": literal(2),
                "percent": literal(50),
                "threshold": literal(0),
            },
        },
        {
            "surface": "PIL.ImageFilter",
            "operation": "Color3DLUT",
            "requirement_suffix": "parameter.target-mode",
            "name": "valid-rgba-target",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [255, 128, 64],
            "values": {
                "size": literal(2),
                "table": literal(
                    [
                        0.0, 0.0, 0.0, 0.0,
                        1.0, 0.0, 0.0, 0.0,
                        0.0, 1.0, 0.0, 0.0,
                        1.0, 1.0, 0.0, 0.0,
                        0.0, 0.0, 1.0, 0.0,
                        1.0, 0.0, 1.0, 0.0,
                        0.0, 1.0, 1.0, 0.0,
                        1.0, 1.0, 1.0, 1.0,
                    ]
                ),
                "channels": literal(4),
                "target_mode": literal("RGBA"),
            },
        },
        {
            "surface": "PIL.ImageFilter",
            "operation": "Color3DLUT",
            "requirement_suffix": "behavior.default",
            "name": "wrong-source-mode",
            "mode": "L",
        },
        {
            "surface": "PIL.ImageFilter",
            "operation": "Color3DLUT",
            "requirement_suffix": "parameter.target-mode",
            "name": "target-too-narrow",
            "mode": "RGB",
            "values": {
                "size": literal(2),
                "table": literal(
                    [
                        0.0, 0.0, 0.0, 0.0,
                        1.0, 0.0, 0.0, 0.0,
                        0.0, 1.0, 0.0, 0.0,
                        1.0, 1.0, 0.0, 0.0,
                        0.0, 0.0, 1.0, 0.0,
                        1.0, 0.0, 1.0, 0.0,
                        0.0, 1.0, 1.0, 0.0,
                        1.0, 1.0, 1.0, 1.0,
                    ]
                ),
                "channels": literal(4),
                "target_mode": literal("RGB"),
            },
        },
        {
            "surface": "PIL.ImageFilter",
            "operation": "Color3DLUT",
            "requirement_suffix": "parameter.target-mode",
            "name": "unrecognized-target-mode",
            "mode": "RGB",
            "values": {
                "size": literal(2),
                "table": literal([0.0] * 24),
                "target_mode": literal("XYZ"),
            },
        },
        {
            "surface": "PIL.ImageFilter",
            "operation": "Color3DLUT",
            "requirement_suffix": "parameter.size",
            "name": "tuple-size",
            "mode": "RGB",
            "values": {
                "size": literal([2, 3, 4]),
                "table": literal([0.0] * (2 * 3 * 4 * 3)),
            },
        },
        {
            "surface": "PIL.ImageFilter",
            "operation": "Color3DLUT",
            "requirement_suffix": "parameter.size",
            "name": "invalid-size-shape",
            "mode": "RGB",
            "values": {
                "size": literal([2, 2]),
                "table": literal([0.0] * 24),
            },
        },
        {
            "surface": "PIL.ImageFilter",
            "operation": "Color3DLUT",
            "requirement_suffix": "parameter.size",
            "name": "invalid-size-range",
            "mode": "RGB",
            "values": {
                "size": literal([1, 2, 2]),
                "table": literal([0.0] * 24),
            },
        },
        {
            "surface": "PIL.ImageFilter",
            "operation": "Color3DLUT",
            "requirement_suffix": "parameter.size",
            "name": "invalid-size-middle-dimension",
            "mode": "RGB",
            "values": {
                "size": literal([2, 1, 2]),
                "table": literal([0.0] * 24),
            },
        },
        {
            "surface": "PIL.ImageFilter",
            "operation": "Color3DLUT",
            "requirement_suffix": "parameter.size",
            "name": "invalid-size-last-dimension",
            "mode": "RGB",
            "values": {
                "size": literal([2, 2, 1]),
                "table": literal([0.0] * 24),
            },
        },
        {
            "surface": "PIL.ImageFilter",
            "operation": "Color3DLUT",
            "requirement_suffix": "parameter.table",
            "name": "nested-table",
            "mode": "RGB",
            "values": {
                "size": literal(2),
                "table": literal([[0.0, 0.0, 0.0]] * 8),
            },
        },
        {
            "surface": "PIL.ImageFilter",
            "operation": "Color3DLUT",
            "requirement_suffix": "parameter.table",
            "name": "nested-table-wrong-channel-count",
            "mode": "RGB",
            "values": {
                "size": literal(2),
                "table": literal([[0.0, 0.0]] * 8),
            },
        },
        {
            "surface": "PIL.ImageFilter",
            "operation": "Color3DLUT",
            "requirement_suffix": "parameter.table",
            "name": "short-table",
            "mode": "RGB",
            "values": {
                "size": literal(2),
                "table": literal([0.0]),
            },
        },
        {
            "surface": "PIL.ImageFilter",
            "operation": "Color3DLUT",
            "requirement_suffix": "parameter.table",
            "name": "empty-table",
            "mode": "RGB",
            "values": {
                "size": literal(2),
                "table": literal([]),
            },
        },
        {
            "surface": "PIL.ImageFilter.Color3DLUT",
            "operation": "generate",
            "requirement_suffix": "parameter.callback",
            "name": "short-callback-result",
            "values": {
                "size": literal(2),
                "callback": literal("color3dlut-short-result"),
            },
        },
        {
            "surface": "PIL.ImageFilter.Color3DLUT",
            "operation": "generate",
            "requirement_suffix": "parameter.size",
            "name": "invalid-size",
            "values": {
                "size": literal(1),
            },
        },
        {
            "surface": "PIL.ImageFilter.Color3DLUT",
            "operation": "generate",
            "requirement_suffix": "parameter.channels",
            "name": "invalid-channels",
            "values": {
                "size": literal(2),
                "channels": literal(2),
            },
        },
        {
            "surface": "PIL.ImageFilter.Color3DLUT",
            "operation": "transform",
            "requirement_suffix": "parameter.callback",
            "name": "short-callback-result",
            "values": {
                "callback": literal("color3dlut-short-result"),
            },
        },
        {
            "surface": "PIL.ImageFilter.Color3DLUT",
            "operation": "transform",
            "requirement_suffix": "parameter.channels",
            "name": "rgba-result",
            "values": {
                "callback": literal("color3dlut-transform-rgba"),
                "channels": literal(4),
            },
        },
        {
            "surface": "PIL.ImageFilter.Color3DLUT",
            "operation": "transform",
            "requirement_suffix": "parameter.channels",
            "name": "invalid-channels",
            "values": {
                "channels": literal(2),
            },
        },
        {
            "surface": "PIL.ImageFilter.Color3DLUT",
            "operation": "__repr__",
            "requirement_suffix": "behavior.default",
            "name": "default",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "filter",
            "requirement_suffix": "behavior.default",
            "name": "f-mode",
            "mode": "F",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "filter",
            "requirement_suffix": "behavior.default",
            "name": "invalid-filter",
            "values": {"filter": literal("BOGUS")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "mode.l",
            "name": "nonzero-l",
            "mode": "L",
            "edge": "nonzero-pixel",
            "pixel": 200,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "mode.1",
            "name": "nonzero-1",
            "mode": "1",
            "edge": "nonzero-pixel",
            "pixel": 1,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "mode.p",
            "name": "nonzero-p",
            "mode": "P",
            "edge": "nonzero-pixel",
            "pixel": 1,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "mode.rgb",
            "name": "corner-pixel",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [255, 0, 0],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "mode.la",
            "name": "nonzero-alpha",
            "mode": "LA",
            "edge": "nonzero-pixel",
            "pixel": [200, 128],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "mode.rgba",
            "name": "nonzero-alpha-rgba",
            "mode": "RGBA",
            "edge": "nonzero-pixel",
            "pixel": [200, 100, 50, 128],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "edge.blank-image",
            "name": "blank-rgb",
            "mode": "RGB",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "edge.blank-image",
            "name": "blank-rgba",
            "mode": "RGBA",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "behavior.default",
            "name": "blank-i",
            "mode": "I",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "behavior.default",
            "name": "blank-f",
            "mode": "F",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "behavior.default",
            "name": "zero-width",
            "mode": "RGB",
            "edge": "zero-width",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "behavior.default",
            "name": "zero-height",
            "mode": "RGB",
            "edge": "zero-height",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "parameter.alpha-only",
            "name": "la-zero-rgb-nonzero-alpha",
            "mode": "LA",
            "edge": "nonzero-pixel",
            "pixel": [0, 200],
            "values": {"alpha_only": literal(False)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "parameter.alpha-only",
            "name": "rgba-zero-rgb-nonzero-alpha",
            "mode": "RGBA",
            "edge": "nonzero-pixel",
            "pixel": [0, 0, 0, 200],
            "values": {"alpha_only": literal(False)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "parameter.alpha-only",
            "name": "rgba-nonzero-rgb-zero-alpha",
            "mode": "RGBA",
            "edge": "nonzero-pixel",
            "pixel": [200, 100, 50, 0],
            "values": {"alpha_only": literal(False)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "parameter.alpha-only",
            "name": "rgba-green-only-zero-alpha",
            "mode": "RGBA",
            "edge": "nonzero-pixel",
            "pixel": [0, 200, 0, 0],
            "values": {"alpha_only": literal(False)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "parameter.alpha-only",
            "name": "rgba-blue-only-zero-alpha",
            "mode": "RGBA",
            "edge": "nonzero-pixel",
            "pixel": [0, 0, 50, 0],
            "values": {"alpha_only": literal(False)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "parameter-combination.legacy-003",
            "name": "alpha-only-false-rgb",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [200, 100, 50],
            "values": {"alpha_only": literal(False)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.method",
            "name": "maxcoverage-16",
            "mode": "RGB",
            "edge": "noise-fill",
            "seed": 1,
            "values": {
                "colors": literal(16),
                "method": literal(1),
                "kmeans": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.method",
            "name": "maxcoverage-zero-size",
            "mode": "RGB",
            "edge": "zero-size",
            "values": {
                "colors": literal(4),
                "method": literal(1),
                "kmeans": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.method",
            "name": "maxcoverage-repeated-colors",
            "mode": "RGB",
            "edge": "quantize-repeated-colors",
            "values": {
                "colors": literal(4),
                "method": literal(1),
                "kmeans": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.method",
            "name": "fast-octree-rgba-diverse",
            "mode": "RGBA",
            "edge": "noise-fill",
            "seed": 13,
            "values": {
                "colors": literal(16),
                "method": literal(2),
                "kmeans": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.method",
            "name": "fast-octree-rgba-zero-width",
            "mode": "RGBA",
            "edge": "zero-width",
            "values": {
                "colors": literal(4),
                "method": literal(2),
                "kmeans": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.method",
            "name": "fast-octree-rgba-zero-height",
            "mode": "RGBA",
            "edge": "zero-height",
            "values": {
                "colors": literal(4),
                "method": literal(2),
                "kmeans": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.method",
            "name": "fast-octree-rgba-transparent",
            "mode": "RGBA",
            "edge": "uniform-fill",
            "pixel": [0, 0, 0, 0],
            "values": {
                "colors": literal(4),
                "method": literal(2),
                "kmeans": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.method",
            "name": "fast-octree-rgba-libimagequant",
            "mode": "RGBA",
            "values": {
                "colors": literal(4),
                "method": literal(3),
                "kmeans": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.method",
            "name": "mediancut-zero-size",
            "mode": "RGB",
            "edge": "zero-size",
            "values": {
                "colors": literal(4),
                "method": literal(0),
                "kmeans": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.method",
            "name": "fast-octree-rgba-zero-size",
            "mode": "RGBA",
            "edge": "zero-size",
            "values": {
                "colors": literal(4),
                "method": literal(2),
                "kmeans": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.method",
            "name": "mediancut-single-color",
            "mode": "RGB",
            "edge": "uniform-fill",
            "pixel": [10, 20, 30],
            "values": {
                "colors": literal(16),
                "method": literal(0),
                "kmeans": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.method",
            "name": "mediancut-single-pixel",
            "mode": "RGB",
            "size": [1, 1],
            "edge": "uniform-fill",
            "pixel": [10, 20, 30],
            "values": {
                "colors": literal(16),
                "method": literal(0),
                "kmeans": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.method",
            "name": "median-cut-adaptive-hash-rebuild",
            "mode": "RGB",
            "edge": "quantize-hash-rebuild",
            "values": {
                "colors": literal(4),
                "method": literal(0),
                "kmeans": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.method",
            "name": "median-cut-adaptive-hash-recursive-rebuild",
            "mode": "RGB",
            "size": [65537, 1],
            "edge": "quantize-hash-recursive-rebuild",
            "values": {
                "colors": literal(4),
                "method": literal(0),
                "kmeans": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.method",
            "name": "mediancut-repeated-colors",
            "mode": "RGB",
            "edge": "quantize-repeated-colors",
            "values": {
                "colors": literal(8),
                "method": literal(0),
                "kmeans": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.method",
            "name": "maxcoverage-4",
            "mode": "RGB",
            "edge": "noise-fill",
            "seed": 2,
            "values": {
                "colors": literal(4),
                "method": literal(1),
                "kmeans": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.colors",
            "name": "maxcoverage-32-colors",
            "mode": "RGB",
            "edge": "noise-fill",
            "seed": 3,
            "values": {
                "colors": literal(32),
                "method": literal(1),
                "kmeans": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.kmeans",
            "name": "maxcoverage-kmeans-1",
            "mode": "RGB",
            "edge": "noise-fill",
            "seed": 4,
            "values": {
                "colors": literal(16),
                "method": literal(1),
                "kmeans": literal(1),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.kmeans",
            "name": "maxcoverage-kmeans-2",
            "mode": "RGB",
            "edge": "noise-fill",
            "seed": 5,
            "values": {
                "colors": literal(16),
                "method": literal(1),
                "kmeans": literal(2),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.kmeans",
            "name": "maxcoverage-kmeans-5",
            "mode": "RGB",
            "edge": "noise-fill",
            "seed": 6,
            "values": {
                "colors": literal(16),
                "method": literal(1),
                "kmeans": literal(5),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "effect_spread",
            "requirement_suffix": "mode.p",
            "name": "p-rgba-palette-zero-distance",
            "mode": "P",
            "edge": "effect-spread-p-rgba",
            "observe_result": "tobytes",
            "values": {"distance": literal(0)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "effect_spread",
            "requirement_suffix": "parameter.distance",
            "name": "p-rgba-palette-single-pixel",
            "mode": "P",
            "edge": "effect-spread-p-rgba",
            "size": [1, 1],
            "observe_result": "tobytes",
            "values": {"distance": literal(17)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpalette",
            "requirement_suffix": "parameter.rawmode",
            "name": "la-palette",
            "observe_receiver": True,
            "mode": "P",
            "values": {
                "data": literal([0, 255, 1, 254]),
                "rawmode": literal("LA"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpalette",
            "requirement_suffix": "parameter.rawmode",
            "name": "rgba-palette",
            "observe_receiver": True,
            "mode": "P",
            "values": {
                "data": literal([10, 20, 30, 254, 40, 50, 60, 255]),
                "rawmode": literal("RGBA"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpalette",
            "requirement_suffix": "mode.l",
            "name": "la-palette-l-image",
            "mode": "L",
            "values": {
                "data": literal([0, 255, 1, 254]),
                "rawmode": literal("LA"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpalette",
            "requirement_suffix": "parameter.rawmode",
            "name": "la-receiver",
            "mode": "LA",
            "values": {
                "data": literal([0, 255, 1, 254]),
                "rawmode": literal("LA"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpalette",
            "requirement_suffix": "parameter.rawmode",
            "name": "oversized-rgb-palette",
            "mode": "P",
            "values": {
                "data": literal([1, 2, 3] * 257),
                "rawmode": literal("RGB"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpalette",
            "requirement_suffix": "parameter.rawmode",
            "name": "oversized-rgba-palette",
            "mode": "P",
            "values": {
                "data": literal([1, 2, 3, 4] * 257),
                "rawmode": literal("RGBA"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpalette",
            "requirement_suffix": "parameter.rawmode",
            "name": "oversized-la-palette",
            "mode": "P",
            "values": {
                "data": literal([1, 2] * 257),
                "rawmode": literal("LA"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpalette",
            "requirement_suffix": "parameter.rawmode",
            "name": "invalid-rawmode",
            "mode": "P",
            "values": {
                "data": literal([1, 2, 3]),
                "rawmode": literal("XYZ"),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putalpha",
            "requirement_suffix": "behavior.default",
            "name": "l-mask",
            "observe_receiver": True,
            "mode": "RGBA",
            "mask_mode": "L",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putalpha",
            "requirement_suffix": "behavior.default",
            "name": "cmyk-mask",
            "observe_receiver": True,
            "mode": "CMYK",
            "mask_mode": "L",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putalpha",
            "requirement_suffix": "behavior.default",
            "name": "p-mask",
            "observe_receiver": True,
            "mode": "P",
            "mask_mode": "L",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putalpha",
            "requirement_suffix": "behavior.default",
            "name": "la-mask",
            "observe_receiver": True,
            "mode": "LA",
            "mask_mode": "L",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putalpha",
            "requirement_suffix": "mode.l",
            "name": "l-scalar",
            "observe_receiver": True,
            "mode": "L",
            "values": {"alpha": literal(192)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putalpha",
            "requirement_suffix": "mode.la",
            "name": "la-scalar",
            "observe_receiver": True,
            "mode": "LA",
            "values": {"alpha": literal(192)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putalpha",
            "requirement_suffix": "mode.p",
            "name": "p-scalar",
            "observe_receiver": True,
            "mode": "P",
            "values": {"alpha": literal(192)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putalpha",
            "requirement_suffix": "mode.rgb",
            "name": "rgb-scalar",
            "observe_receiver": True,
            "mode": "RGB",
            "values": {"alpha": literal(192)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putalpha",
            "requirement_suffix": "mode.rgba",
            "name": "rgba-scalar",
            "observe_receiver": True,
            "mode": "RGBA",
            "values": {"alpha": literal(192)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putalpha",
            "requirement_suffix": "behavior.default",
            "name": "pa-scalar",
            "observe_receiver": True,
            "mode": "PA",
            "values": {"alpha": literal(192)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putalpha",
            "requirement_suffix": "mode.cmyk",
            "name": "cmyk-scalar",
            "observe_receiver": True,
            "mode": "CMYK",
            "values": {"alpha": literal(192)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putalpha",
            "requirement_suffix": "behavior.default",
            "name": "i-unsupported",
            "mode": "I",
            "values": {"alpha": literal(192)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putalpha",
            "requirement_suffix": "behavior.default",
            "name": "i-mask-unsupported",
            "mode": "I",
            "mask_mode": "L",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putalpha",
            "requirement_suffix": "behavior.default",
            "name": "f-mask-unsupported",
            "mode": "F",
            "mask_mode": "L",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putalpha",
            "requirement_suffix": "behavior.default",
            "name": "ycbcr-mask-unsupported",
            "mode": "YCbCr",
            "mask_mode": "L",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putalpha",
            "requirement_suffix": "behavior.default",
            "name": "hsv-mask-unsupported",
            "mode": "HSV",
            "mask_mode": "L",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putalpha",
            "requirement_suffix": "behavior.default",
            "name": "f-unsupported",
            "mode": "F",
            "values": {"alpha": literal(192)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putalpha",
            "requirement_suffix": "behavior.default",
            "name": "ycbcr-unsupported",
            "mode": "YCbCr",
            "values": {"alpha": literal(192)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putalpha",
            "requirement_suffix": "behavior.default",
            "name": "hsv-unsupported",
            "mode": "HSV",
            "values": {"alpha": literal(192)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putalpha",
            "requirement_suffix": "behavior.default",
            "name": "one-mask",
            "mode": "RGB",
            "mask_mode": "1",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putalpha",
            "requirement_suffix": "behavior.default",
            "name": "invalid-mask-mode",
            "mode": "RGBA",
            "mask_mode": "RGB",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putalpha",
            "requirement_suffix": "behavior.default",
            "name": "mask-size-mismatch",
            "mode": "RGB",
            "mask_mode": "L",
            "edge": "mask-size-mismatch",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putalpha",
            "requirement_suffix": "behavior.default",
            "name": "invalid-alpha-type",
            "mode": "RGBA",
            "values": {"alpha": literal("not-an-alpha")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "parameter.scale",
            "name": "rgb-tuples-scale-offset",
            "observe_receiver": True,
            "mode": "RGB",
            "values": {
                "data": literal([[1, 2, 3], [4, 5, 6], [7, 8, 9]] * 3),
                "scale": literal(2),
                "offset": literal(1),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "parameter.offset",
            "name": "rgba-clipped-tuples",
            "observe_receiver": True,
            "mode": "RGBA",
            "values": {
                "data": literal([[300, -5, 128, 0], [255, 200, 100, 400], [1, 2, 3, 4]] * 3),
                "offset": literal(0.5),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "save",
            "requirement_suffix": "format.png",
            "name": "rgb-nonzero",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [200, 100, 50],
            "values": {"format": literal("PNG")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "save",
            "requirement_suffix": "format.png",
            "name": "p-short-palette",
            "mode": "P",
            "chain": "p-short-palette-save",
            "values": {"format": literal("PNG")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "save",
            "requirement_suffix": "format.png",
            "name": "p-short-palette-resize",
            "mode": "P",
            "chain": "p-short-palette-resize-save",
            "values": {"format": literal("PNG")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "save",
            "requirement_suffix": "format.png",
            "name": "opened-p-load-save",
            "scenario_asset": "image/p-small.png",
            "chain": "opened-p-load-save",
            "values": {"format": literal("PNG")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "save",
            "requirement_suffix": "format.png",
            "name": "p-invert-pipeline",
            "mode": "P",
            "chain": "p-invert-save",
            "values": {"format": literal("PNG")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "filter",
            "requirement_suffix": "mode.p",
            "name": "p-mode-filter",
            "mode": "P",
            "chain": "p-mode-filter",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbands",
            "requirement_suffix": "behavior.default",
            "name": "cmyk",
            "mode": "CMYK",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbands",
            "requirement_suffix": "behavior.default",
            "name": "ycbcr",
            "mode": "YCbCr",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbands",
            "requirement_suffix": "behavior.default",
            "name": "hsv",
            "mode": "HSV",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbands",
            "requirement_suffix": "behavior.default",
            "name": "i-mode",
            "mode": "I",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbands",
            "requirement_suffix": "behavior.default",
            "name": "f-mode",
            "mode": "F",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbands",
            "requirement_suffix": "behavior.default",
            "name": "pa-mode",
            "mode": "PA",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbands",
            "requirement_suffix": "behavior.default",
            "name": "rgba-premultiplied-mode",
            "mode": "RGBa",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbands",
            "requirement_suffix": "behavior.default",
            "name": "i16-mode",
            "mode": "I;16",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbands",
            "requirement_suffix": "behavior.default",
            "name": "i16l-mode",
            "mode": "I;16L",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbands",
            "requirement_suffix": "behavior.default",
            "name": "i16b-mode",
            "mode": "I;16B",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbands",
            "requirement_suffix": "behavior.default",
            "name": "i16n-mode",
            "mode": "I;16N",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.box",
            "name": "color-two-tuple-box",
            "mode": "RGB",
            "values": {
                "im": literal([255, 0, 0]),
                "box": literal([0, 0]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "color-zero-region",
            "mode": "RGB",
            "values": {
                "im": literal([255, 0, 0]),
                "box": literal([1, 1, 1, 1]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "edge.mask-size-mismatch",
            "name": "region-mask-mismatch",
            "mode": "RGBA",
            "im_mode": "RGBA",
            "mask_mode": "L",
            "edge": "mask-size-mismatch",
            "values": {
                "box": literal([0, 0, 16, 16]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.mask",
            "name": "truthy-non-image-mask",
            "mode": "RGB",
            "chain": "truthy-non-image-mask",
            "values": {
                "box": literal([0, 0, 4, 4]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "mode.pa",
            "name": "p-source-into-pa",
            "mode": "PA",
            "im_mode": "P",
            "values": {
                "box": literal([2, 2, 6, 6]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "p-pipeline-source",
            "mode": "RGB",
            "chain": "p-pipeline-paste",
            "observe_receiver": True,
            "values": {
                "box": literal([0, 0, 16, 16]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "p-pipeline-source-with-palette",
            "mode": "RGB",
            "chain": "p-putpalette-pipeline-paste",
            "observe_receiver": True,
            "values": {
                "box": literal([0, 0, 16, 16]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "rgba-destination-scalar",
            "scenario_inline_image": "rgba-frombytes",
            "chain": "rgba-destination-paste",
            "observe_receiver": True,
            "values": {
                "im": literal(7),
                "box": literal([0, 0, 1, 1]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "load",
            "requirement_suffix": "behavior.default",
            "name": "png-rgb-opened",
            "scenario_asset": "image/rgb-small.png",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "load",
            "requirement_suffix": "behavior.default",
            "name": "png-rgba-opened",
            "scenario_asset": "image/rgba-small.png",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "load",
            "requirement_suffix": "behavior.default",
            "name": "png-l-opened",
            "scenario_asset": "image/l-small.png",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "load",
            "requirement_suffix": "behavior.default",
            "name": "png-p-opened",
            "scenario_asset": "image/p-small.png",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "load",
            "requirement_suffix": "behavior.default",
            "name": "png-p-transparency",
            "scenario_asset": "image/p-transparency.png",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "load",
            "requirement_suffix": "behavior.default",
            "name": "gif-p-opened",
            "scenario_asset": "image/p-small.gif",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "load",
            "requirement_suffix": "behavior.default",
            "name": "p-resize-pipeline",
            "mode": "P",
            "chain": "p-resize-verify",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "load",
            "requirement_suffix": "behavior.default",
            "name": "p-resize-without-palette",
            "mode": "P",
            "edge": "raw-p-no-palette",
            "chain": "p-resize-no-palette-load",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "load",
            "requirement_suffix": "behavior.default",
            "name": "p-putpalette-resize-pipeline",
            "mode": "P",
            "chain": "p-putpalette-resize",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "load",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-rgb-opened",
            "scenario_asset": "image/rgb-small.jpg",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "verify",
            "requirement_suffix": "behavior.default",
            "name": "png-rgb-opened",
            "scenario_asset": "image/rgb-small.png",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "verify",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-rgb-opened",
            "scenario_asset": "image/rgb-small.jpg",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "behavior.default",
            "name": "png-rgba-opened",
            "scenario_asset": "image/rgba-small.png",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "behavior.default",
            "name": "opened-png-without-idat",
            "scenario_inline_image": "png-no-idat",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "crop",
            "requirement_suffix": "behavior.default",
            "name": "opened-png-without-idat",
            "scenario_inline_image": "png-no-idat",
            "values": {"box": literal([0, 0, 1, 1])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getcolors",
            "requirement_suffix": "behavior.default",
            "name": "opened-png-without-idat",
            "scenario_inline_image": "png-no-idat",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbands",
            "requirement_suffix": "behavior.default",
            "name": "opened-png-without-idat",
            "scenario_inline_image": "png-no-idat",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getdata",
            "requirement_suffix": "behavior.default",
            "name": "opened-png-without-idat",
            "scenario_inline_image": "png-no-idat",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getprojection",
            "requirement_suffix": "behavior.default",
            "name": "opened-png-without-idat",
            "scenario_inline_image": "png-no-idat",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "entropy",
            "requirement_suffix": "behavior.default",
            "name": "opened-png-without-idat",
            "scenario_inline_image": "png-no-idat",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpixel",
            "requirement_suffix": "behavior.default",
            "name": "opened-png-without-idat",
            "scenario_inline_image": "png-no-idat",
            "values": {"xy": literal([0, 0])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getbbox",
            "requirement_suffix": "behavior.default",
            "name": "png-p-transparency",
            "scenario_asset": "image/p-transparency.png",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getextrema",
            "requirement_suffix": "behavior.default",
            "name": "png-rgba-opened",
            "scenario_asset": "image/rgba-small.png",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getextrema",
            "requirement_suffix": "behavior.default",
            "name": "opened-png-without-idat",
            "scenario_inline_image": "png-no-idat",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "save",
            "requirement_suffix": "format.png",
            "name": "opened-rgb",
            "scenario_asset": "image/rgb-small.png",
            "values": {"format": literal("PNG")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "histogram",
            "requirement_suffix": "mode.rgb",
            "name": "nonzero-rgb",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [200, 100, 50],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "histogram",
            "requirement_suffix": "mode.rgb",
            "name": "opened-png-without-idat",
            "scenario_inline_image": "png-no-idat",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "histogram",
            "requirement_suffix": "mode.i16",
            "name": "i16-frombytes",
            "scenario_inline_image": "i16-frombytes",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "histogram",
            "requirement_suffix": "mode.i16",
            "name": "i16l-frombytes",
            "scenario_inline_image": "i16l-frombytes",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "histogram",
            "requirement_suffix": "mode.i16",
            "name": "i16b-frombytes",
            "scenario_inline_image": "i16b-frombytes",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "histogram",
            "requirement_suffix": "mode.i16",
            "name": "i16n-frombytes",
            "scenario_inline_image": "i16n-frombytes",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "histogram",
            "requirement_suffix": "parameter.mask",
            "name": "mask-size-mismatch",
            "mode": "RGB",
            "mask_mode": "L",
            "edge": "mask-size-mismatch",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "histogram",
            "requirement_suffix": "parameter.mask",
            "name": "bad-mask-mode",
            "mode": "RGB",
            "mask_mode": "RGB",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "histogram",
            "requirement_suffix": "parameter.mask",
            "name": "truthy-non-image-mask",
            "mode": "RGB",
            "chain": "truthy-non-image-mask",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "histogram",
            "requirement_suffix": "parameter.mask",
            "name": "opened-png-without-idat-mask",
            "mode": "RGB",
            "scenario_inline_mask_image": "png-no-idat",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.l",
            "name": "from-one-to-l",
            "mode": "1",
            "edge": "nonzero-pixel",
            "pixel": 1,
            "values": {"mode": literal("L")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.rgb",
            "name": "from-one-to-rgb",
            "mode": "1",
            "edge": "nonzero-pixel",
            "pixel": 1,
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "parameter.dither",
            "name": "integer-none-dither",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [200, 100, 50],
            "values": {"mode": literal("1"), "dither": literal(0)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "parameter.dither",
            "name": "integer-floydsteinberg-dither",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [200, 100, 50],
            "values": {"mode": literal("1"), "dither": literal(1)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "parameter.dither",
            "name": "integer-floydsteinberg-dither-high-luma",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [255, 255, 255],
            "values": {"mode": literal("1"), "dither": literal(1)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "parameter.dither",
            "name": "string-dither-error",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [200, 100, 50],
            "values": {"mode": literal("1"), "dither": literal("BOGUS")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "parameter.palette",
            "name": "explicit-none-palette",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [200, 100, 50],
            "values": {"mode": literal("P"), "palette": literal(None)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "parameter.mode",
            "name": "non-string-mode-error",
            "mode": "RGB",
            "values": {"mode": literal(1)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "parameter.dither",
            "name": "non-integer-dither-error",
            "mode": "RGB",
            "values": {"mode": literal("1"), "dither": literal([1])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "l-to-pa",
            "mode": "L",
            "values": {"mode": literal("PA")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.1",
            "name": "pa-to-one",
            "mode": "PA",
            "edge": "nonzero-pixel",
            "pixel": [200, 255],
            "values": {"mode": literal("1")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.1",
            "name": "pa-palette-to-one",
            "mode": "PA",
            "edge": "nonzero-pixel",
            "pixel": [1, 255],
            "chain": "pa-putpalette-convert",
            "values": {"mode": literal("1")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.1",
            "name": "pa-short-palette-to-one",
            "mode": "PA",
            "edge": "nonzero-pixel",
            "pixel": [2, 255],
            "chain": "pa-putpalette-convert",
            "values": {"mode": literal("1")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.rgba",
            "name": "raw-p-without-palette-to-rgba",
            "mode": "P",
            "edge": "raw-p-no-palette",
            "values": {"mode": literal("RGBA")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.l",
            "name": "from-cmyk-to-l",
            "mode": "CMYK",
            "edge": "nonzero-pixel",
            "pixel": [100, 0, 0, 0],
            "values": {"mode": literal("L")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.rgba",
            "name": "from-cmyk-to-rgba",
            "mode": "CMYK",
            "values": {"mode": literal("RGBA")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.l",
            "name": "from-ycbcr",
            "mode": "YCbCr",
            "edge": "nonzero-pixel",
            "pixel": [100, 150, 200],
            "values": {"mode": literal("L")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.rgb",
            "name": "from-hsv-to-rgb",
            "mode": "HSV",
            "edge": "nonzero-pixel",
            "pixel": [100, 200, 150],
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.rgb",
            "name": "from-i-to-rgb",
            "mode": "I",
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.rgb",
            "name": "from-f-to-rgb",
            "mode": "F",
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.l",
            "name": "from-f-to-l",
            "mode": "F",
            "values": {"mode": literal("L")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.method",
            "name": "mediancut-16",
            "mode": "RGB",
            "edge": "noise-fill",
            "seed": 7,
            "values": {
                "colors": literal(16),
                "method": literal(0),
                "kmeans": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.method",
            "name": "mediancut-4",
            "mode": "RGB",
            "edge": "noise-fill",
            "seed": 8,
            "values": {
                "colors": literal(4),
                "method": literal(0),
                "kmeans": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.colors",
            "name": "mediancut-32-colors",
            "mode": "RGB",
            "edge": "noise-fill",
            "seed": 9,
            "values": {
                "colors": literal(32),
                "method": literal(0),
                "kmeans": literal(0),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.kmeans",
            "name": "mediancut-kmeans-1",
            "mode": "RGB",
            "edge": "noise-fill",
            "seed": 10,
            "values": {
                "colors": literal(16),
                "method": literal(0),
                "kmeans": literal(1),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.kmeans",
            "name": "mediancut-kmeans-2",
            "mode": "RGB",
            "edge": "noise-fill",
            "seed": 11,
            "values": {
                "colors": literal(16),
                "method": literal(0),
                "kmeans": literal(2),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "behavior.default",
            "name": "mediancut-default",
            "mode": "RGB",
            "edge": "noise-fill",
            "seed": 12,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.colors",
            "name": "invalid-color-count",
            "mode": "RGB",
            "values": {"colors": literal(0)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.palette",
            "name": "palette-image",
            "mode": "RGB",
            "chain": "quantize-palette",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.palette",
            "name": "palette-image-red",
            "mode": "RGB",
            "edge": "uniform-fill",
            "pixel": [255, 0, 0],
            "chain": "quantize-palette",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.dither",
            "name": "palette-image-red-no-dither",
            "mode": "RGB",
            "edge": "uniform-fill",
            "pixel": [255, 0, 0],
            "chain": "quantize-palette",
            "values": {"dither": literal(0)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.palette",
            "name": "palette-image-empty",
            "mode": "RGB",
            "chain": "quantize-palette-empty",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.palette",
            "name": "palette-image-unsupported-rgba",
            "mode": "RGBA",
            "chain": "quantize-palette-unsupported-source",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "quantize",
            "requirement_suffix": "parameter.kmeans",
            "name": "negative-kmeans",
            "mode": "RGB",
            "values": {"kmeans": literal(-1)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "apply_transparency",
            "requirement_suffix": "behavior.default",
            "name": "png-p-transparency",
            "scenario_asset": "image/p-transparency.png",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "info",
            "requirement_suffix": "behavior.default",
            "name": "p-transparency-table",
            "mode": "P",
            "chain": "p-table-transparency",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "info",
            "requirement_suffix": "behavior.default",
            "name": "opened-jpeg-format-info",
            "scenario_asset": "image/rgb-small.jpg",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "info",
            "requirement_suffix": "behavior.default",
            "name": "opened-bmp-format-info",
            "scenario_asset": "image/rgb-small.bmp",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "info",
            "requirement_suffix": "behavior.default",
            "name": "opened-gif-format-info",
            "scenario_asset": "image/p-small.gif",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "info",
            "requirement_suffix": "behavior.default",
            "name": "opened-webp-format-info",
            "scenario_asset": "image/rgb-small.webp",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "apply_transparency",
            "requirement_suffix": "behavior.default",
            "name": "png-p-transparency-table",
            "mode": "P",
            "chain": "p-table-transparency",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "apply_transparency",
            "requirement_suffix": "behavior.default",
            "name": "png-p-transparency-resized",
            "scenario_asset": "image/p-transparency.png",
            "chain": "p-transparency-resize-apply",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "apply_transparency",
            "requirement_suffix": "behavior.default",
            "name": "png-p-transparency-resized-bilinear",
            "scenario_asset": "image/p-transparency.png",
            "chain": "p-transparency-resize-bilinear-apply",
            "observe_receiver": True,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "apply_transparency",
            "requirement_suffix": "behavior.default",
            "name": "png-p-transparency-loaded",
            "scenario_asset": "image/p-transparency.png",
            "chain": "p-transparency-load-apply",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "apply_transparency",
            "requirement_suffix": "behavior.default",
            "name": "png-p-transparency-putpalette",
            "scenario_asset": "image/p-transparency.png",
            "chain": "p-transparency-putpalette-apply",
            "observe_receiver": True,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "apply_transparency",
            "requirement_suffix": "behavior.default",
            "name": "png-p-transparency-putpalette-short",
            "scenario_asset": "image/p-transparency.png",
            "chain": "p-transparency-putpalette-short",
            "observe_receiver": True,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "apply_transparency",
            "requirement_suffix": "behavior.default",
            "name": "png-p-single-index-transparency",
            "mode": "P",
            "chain": "p-full-palette-index-transparency-apply",
            "observe_receiver": True,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "apply_transparency",
            "requirement_suffix": "behavior.default",
            "name": "p-transparency-pa-boundary-putalpha",
            "scenario_asset": "image/p-transparency.png",
            "chain": "p-transparency-putalpha-apply",
            "observe_receiver": True,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.rgba",
            "name": "opened-p-transparency",
            "scenario_asset": "image/p-transparency.png",
            "values": {"mode": literal("RGBA")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "parameter.mode",
            "name": "opened-p-transparency-auto",
            "scenario_asset": "image/p-transparency.png",
            "values": {"mode": literal(None)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "parameter.mode",
            "name": "opened-p-auto",
            "scenario_asset": "image/p-small.png",
            "values": {"mode": literal(None)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.rgba",
            "name": "opened-p-transparency-table",
            "mode": "P",
            "chain": "p-table-transparency",
            "values": {"mode": literal("RGBA")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.rgb",
            "name": "opened-p-transparency-table-to-rgb",
            "mode": "P",
            "chain": "p-table-transparency",
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.la",
            "name": "opened-p-transparency-table-to-la",
            "mode": "P",
            "chain": "p-table-transparency",
            "values": {"mode": literal("LA")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "opened-p-transparency-table-to-pa",
            "mode": "P",
            "chain": "p-table-transparency",
            "values": {"mode": literal("PA")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.rgb",
            "name": "opened-p-transparency-to-rgb",
            "scenario_asset": "image/p-transparency.png",
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.la",
            "name": "opened-p-transparency-to-la",
            "scenario_asset": "image/p-transparency.png",
            "values": {"mode": literal("LA")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.l",
            "name": "opened-p-transparency-to-l",
            "scenario_asset": "image/p-transparency.png",
            "values": {"mode": literal("L")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "opened-p-to-pa",
            "scenario_asset": "image/p-small.png",
            "values": {"mode": literal("PA")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "opened-p-transparency-to-pa",
            "scenario_asset": "image/p-transparency.png",
            "values": {"mode": literal("PA")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "histogram",
            "requirement_suffix": "mode.rgba",
            "name": "opened-rgba",
            "scenario_asset": "image/rgba-small.png",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "entropy",
            "requirement_suffix": "mode.l",
            "name": "opened-l",
            "scenario_asset": "image/l-small.png",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "save",
            "requirement_suffix": "format.bmp",
            "name": "opened-rgb-bmp",
            "scenario_asset": "image/rgb-small.png",
            "values": {"format": literal("BMP")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpalette",
            "requirement_suffix": "behavior.default",
            "name": "opened-p",
            "scenario_asset": "image/p-small.png",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpalette",
            "requirement_suffix": "behavior.default",
            "name": "raw-p-without-palette",
            "mode": "P",
            "edge": "raw-p-no-palette",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpalette",
            "requirement_suffix": "behavior.default",
            "name": "raw-pa-without-palette",
            "mode": "PA",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpalette",
            "requirement_suffix": "behavior.default",
            "name": "opened-p-after-load",
            "scenario_asset": "image/p-small.png",
            "chain": "opened-p-load-getpalette",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpalette",
            "requirement_suffix": "behavior.default",
            "name": "opened-p-after-putpalette",
            "scenario_asset": "image/p-small.png",
            "chain": "opened-p-putpalette-getpalette",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpalette",
            "requirement_suffix": "parameter.rawmode",
            "name": "rgb-none-rgba",
            "mode": "RGB",
            "values": {"rawmode": literal("RGBA")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpalette",
            "requirement_suffix": "parameter.rawmode",
            "name": "opened-p-rgba",
            "scenario_asset": "image/p-small.png",
            "values": {"rawmode": literal("RGBA")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpalette",
            "requirement_suffix": "parameter.rawmode",
            "name": "opened-p-transparency-rgba",
            "scenario_asset": "image/p-transparency.png",
            "values": {"rawmode": literal("RGBA")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getchannel",
            "requirement_suffix": "behavior.default",
            "name": "opened-rgba-alpha",
            "scenario_asset": "image/rgba-small.png",
            "values": {"channel": literal(3)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getchannel",
            "requirement_suffix": "behavior.default",
            "name": "named-rgb-green",
            "mode": "RGB",
            "values": {"channel": literal("G")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getchannel",
            "requirement_suffix": "behavior.default",
            "name": "named-rgba-alpha",
            "mode": "RGBA",
            "values": {"channel": literal("A")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getchannel",
            "requirement_suffix": "behavior.default",
            "name": "negative-index",
            "mode": "RGBA",
            "values": {"channel": literal(-1)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getchannel",
            "requirement_suffix": "behavior.default",
            "name": "numeric-out-of-range",
            "mode": "RGB",
            "values": {"channel": literal(3)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getchannel",
            "requirement_suffix": "behavior.default",
            "name": "invalid-selector-type",
            "mode": "RGBA",
            "values": {"channel": literal([0])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getchannel",
            "requirement_suffix": "behavior.default",
            "name": "pa-direct-index-channel",
            "mode": "PA",
            "values": {"channel": literal(0)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "split",
            "requirement_suffix": "behavior.default",
            "name": "opened-rgba",
            "scenario_asset": "image/rgba-small.png",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "behavior.default",
            "name": "opened-rgb",
            "scenario_asset": "image/rgb-small.png",
            "values": {"size": literal([4, 4])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "behavior.default",
            "name": "p-pipeline-resize-resize",
            "mode": "P",
            "chain": "p-resize-resize",
            "values": {"size": literal([4, 4]), "resample": literal(0)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "thumbnail",
            "requirement_suffix": "parameter.size",
            "name": "zero-width",
            "values": {"size": literal([0, 5])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "thumbnail",
            "requirement_suffix": "parameter.size",
            "name": "zero-height",
            "values": {"size": literal([5, 0])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "thumbnail",
            "requirement_suffix": "behavior.default",
            "name": "p-forces-nearest",
            "observe_receiver": True,
            "mode": "P",
            "values": {
                "size": literal([4, 4]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "resize",
            "requirement_suffix": "behavior.default",
            "name": "f-mode-specialized-path",
            "observe_result": "tobytes",
            "mode": "F",
            "values": {
                "size": literal([4, 4]),
                "resample": literal(3),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "thumbnail",
            "requirement_suffix": "behavior.default",
            "name": "f-mode-specialized-path",
            "observe_receiver": True,
            "mode": "F",
            "values": {
                "size": literal([4, 4]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getcolors",
            "requirement_suffix": "behavior.default",
            "name": "opened-rgba",
            "scenario_asset": "image/rgba-small.png",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getcolors",
            "requirement_suffix": "mode.rgb",
            "name": "rgb-nonzero",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [10, 20, 30],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getcolors",
            "requirement_suffix": "behavior.default",
            "name": "opened-p",
            "scenario_asset": "image/p-small.png",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getcolors",
            "requirement_suffix": "behavior.default",
            "name": "cmyk-nonzero",
            "mode": "CMYK",
            "edge": "nonzero-pixel",
            "pixel": [10, 20, 30, 40],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getcolors",
            "requirement_suffix": "behavior.default",
            "name": "ycbcr-nonzero",
            "mode": "YCbCr",
            "edge": "nonzero-pixel",
            "pixel": [16, 128, 64],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getcolors",
            "requirement_suffix": "behavior.default",
            "name": "hsv-nonzero",
            "mode": "HSV",
            "edge": "nonzero-pixel",
            "pixel": [64, 128, 255],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getcolors",
            "requirement_suffix": "behavior.default",
            "name": "i16-mode",
            "mode": "I;16",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getcolors",
            "requirement_suffix": "behavior.default",
            "name": "i16l-mode",
            "mode": "I;16L",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getcolors",
            "requirement_suffix": "behavior.default",
            "name": "i16b-mode",
            "mode": "I;16B",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getcolors",
            "requirement_suffix": "behavior.default",
            "name": "i16n-mode",
            "mode": "I;16N",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getcolors",
            "requirement_suffix": "mode.la",
            "name": "la-nonzero",
            "mode": "LA",
            "edge": "nonzero-pixel",
            "pixel": [12, 200],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getcolors",
            "requirement_suffix": "behavior.default",
            "name": "i-nonzero",
            "mode": "I",
            "edge": "nonzero-pixel",
            "pixel": 200,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getcolors",
            "requirement_suffix": "behavior.default",
            "name": "i-large-nonzero",
            "mode": "I",
            "edge": "nonzero-pixel",
            "pixel": 100000,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getcolors",
            "requirement_suffix": "behavior.default",
            "name": "f-nonzero",
            "mode": "F",
            "edge": "nonzero-pixel",
            "pixel": 1.5,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getcolors",
            "requirement_suffix": "parameter.maxcolors",
            "name": "i-two-colors-limit",
            "mode": "I",
            "edge": "nonzero-pixel",
            "pixel": 200,
            "values": {"maxcolors": literal(1)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getcolors",
            "requirement_suffix": "parameter.maxcolors",
            "name": "f-two-colors-limit",
            "mode": "F",
            "edge": "nonzero-pixel",
            "pixel": 1.5,
            "values": {"maxcolors": literal(1)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getcolors",
            "requirement_suffix": "parameter.maxcolors",
            "name": "l-two-colors-limit",
            "mode": "L",
            "edge": "nonzero-pixel",
            "pixel": 200,
            "values": {"maxcolors": literal(1)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getcolors",
            "requirement_suffix": "mode.la",
            "name": "la-odd-nonzero",
            "mode": "LA",
            "edge": "nonzero-pixel",
            "pixel": [13, 200],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getdata",
            "requirement_suffix": "behavior.default",
            "name": "opened-rgb",
            "scenario_asset": "image/rgb-small.png",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getdata",
            "requirement_suffix": "behavior.default",
            "name": "bytes-luma",
            "mode": "L",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getdata",
            "requirement_suffix": "behavior.default",
            "name": "bytes-rgb-multiband",
            "mode": "RGB",
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getdata",
            "requirement_suffix": "behavior.default",
            "name": "bytes-i-out-of-range",
            "mode": "I",
            "edge": "nonzero-pixel",
            "pixel": 100000,
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getdata",
            "requirement_suffix": "behavior.default",
            "name": "bytes-f-float",
            "mode": "F",
            "edge": "nonzero-pixel",
            "pixel": 1.5,
            "observe_result": "tobytes",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getdata",
            "requirement_suffix": "behavior.default",
            "name": "negative-band",
            "mode": "RGBA",
            "values": {"band": literal(-1)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getdata",
            "requirement_suffix": "behavior.default",
            "name": "i-large-nonzero",
            "mode": "I",
            "edge": "nonzero-pixel",
            "pixel": 100000,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getdata",
            "requirement_suffix": "behavior.default",
            "name": "f-nonzero",
            "mode": "F",
            "edge": "nonzero-pixel",
            "pixel": 1.5,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getdata",
            "requirement_suffix": "behavior.default",
            "name": "i-invalid-band",
            "mode": "I",
            "edge": "invalid-band",
            "values": {"band": literal(99)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getprojection",
            "requirement_suffix": "behavior.default",
            "name": "opened-rgb",
            "scenario_asset": "image/rgb-small.png",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getprojection",
            "requirement_suffix": "behavior.default",
            "name": "rgb-red-only",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [1, 0, 0],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getprojection",
            "requirement_suffix": "behavior.default",
            "name": "rgb-blue-only",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [0, 0, 1],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getprojection",
            "requirement_suffix": "behavior.default",
            "name": "rgba-alpha-only",
            "mode": "RGBA",
            "edge": "nonzero-pixel",
            "pixel": [0, 0, 0, 1],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getprojection",
            "requirement_suffix": "behavior.default",
            "name": "la-alpha-only",
            "mode": "LA",
            "edge": "nonzero-pixel",
            "pixel": [0, 1],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getprojection",
            "requirement_suffix": "behavior.default",
            "name": "la-luma-only",
            "mode": "LA",
            "edge": "nonzero-pixel",
            "pixel": [1, 0],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getprojection",
            "requirement_suffix": "behavior.default",
            "name": "i-negative-nonzero",
            "mode": "I",
            "edge": "nonzero-pixel",
            "pixel": -1,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getprojection",
            "requirement_suffix": "behavior.default",
            "name": "f-negative-nonzero",
            "mode": "F",
            "edge": "nonzero-pixel",
            "pixel": -1.5,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getprojection",
            "requirement_suffix": "behavior.default",
            "name": "hsv-blue-only",
            "mode": "HSV",
            "edge": "nonzero-pixel",
            "pixel": [0, 0, 1],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getprojection",
            "requirement_suffix": "behavior.default",
            "name": "ycbcr-blue-only",
            "mode": "YCbCr",
            "edge": "nonzero-pixel",
            "pixel": [0, 0, 1],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getprojection",
            "requirement_suffix": "behavior.default",
            "name": "cmyk-channel-only",
            "mode": "CMYK",
            "edge": "nonzero-pixel",
            "pixel": [0, 0, 0, 1],
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getprojection",
            "requirement_suffix": "behavior.default",
            "name": "l-nonzero",
            "mode": "L",
            "edge": "nonzero-pixel",
            "pixel": 1,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getprojection",
            "requirement_suffix": "behavior.default",
            "name": "one-nonzero",
            "mode": "1",
            "edge": "nonzero-pixel",
            "pixel": 1,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getprojection",
            "requirement_suffix": "behavior.default",
            "name": "p-nonzero",
            "mode": "P",
            "edge": "nonzero-pixel",
            "pixel": 1,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "tobitmap",
            "requirement_suffix": "behavior.default",
            "name": "nonzero-l",
            "mode": "L",
            "edge": "nonzero-pixel",
            "pixel": 200,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "tobitmap",
            "requirement_suffix": "mode.1",
            "name": "nonzero-1",
            "mode": "1",
            "edge": "nonzero-pixel",
            "pixel": 1,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getextrema",
            "requirement_suffix": "behavior.default",
            "name": "opened-rgb",
            "scenario_asset": "image/rgb-small.png",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "crop",
            "requirement_suffix": "behavior.default",
            "name": "opened-rgb",
            "scenario_asset": "image/rgb-small.png",
            "values": {"box": literal([1, 1, 7, 7])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "crop",
            "requirement_suffix": "behavior.default",
            "name": "resize-pipeline",
            "mode": "RGB",
            "edge": "nonzero-pixel",
            "pixel": [12, 34, 56],
            "chain": "resize-crop",
            "values": {"box": literal([1, 1, 6, 6])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "crop",
            "requirement_suffix": "behavior.default",
            "name": "box-omitted",
            "mode": "RGB",
            "values": {"box": literal(None)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "crop",
            "requirement_suffix": "behavior.default",
            "name": "reversed-box",
            "mode": "RGB",
            "values": {"box": literal([8, 8, 2, 2])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "crop",
            "requirement_suffix": "behavior.default",
            "name": "reversed-vertical-box",
            "mode": "RGB",
            "values": {"box": literal([2, 8, 8, 2])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "crop",
            "requirement_suffix": "behavior.default",
            "name": "zero-height-box",
            "mode": "RGB",
            "values": {"box": literal([0, 0, 8, 0])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "crop",
            "requirement_suffix": "behavior.default",
            "name": "disjoint-box",
            "mode": "RGB",
            "values": {"box": literal([32, 32, 40, 40])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "crop",
            "requirement_suffix": "behavior.default",
            "name": "vertically-disjoint-box",
            "mode": "RGB",
            "values": {"box": literal([2, 32, 8, 40])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "crop",
            "requirement_suffix": "behavior.default",
            "name": "top-padded-box",
            "mode": "RGB",
            "values": {"box": literal([2, -2, 8, 8])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "crop",
            "requirement_suffix": "behavior.default",
            "name": "negative-left-box",
            "mode": "RGB",
            "values": {"box": literal([-2, 2, 8, 8])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "crop",
            "requirement_suffix": "behavior.default",
            "name": "negative-right-box",
            "mode": "RGB",
            "values": {"box": literal([2, 2, -1, 8])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "crop",
            "requirement_suffix": "behavior.default",
            "name": "negative-bottom-box",
            "mode": "RGB",
            "values": {"box": literal([2, 2, 8, -1])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "crop",
            "requirement_suffix": "behavior.default",
            "name": "bottom-padded-box",
            "mode": "RGB",
            "values": {"box": literal([2, 2, 8, 20])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "crop",
            "requirement_suffix": "mode.p",
            "name": "padded-palette",
            "mode": "P",
            "values": {"box": literal([-2, -2, 20, 20])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "rotate",
            "requirement_suffix": "behavior.default",
            "name": "opened-rgb",
            "scenario_asset": "image/rgb-small.png",
            "values": {"angle": literal(90)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "rotate",
            "requirement_suffix": "mode.l",
            "name": "mode-l-90-non-expand",
            "mode": "L",
            "values": {"angle": literal(90)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "rotate",
            "requirement_suffix": "mode.la",
            "name": "mode-la-90-non-expand",
            "mode": "LA",
            "values": {"angle": literal(90)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "rotate",
            "requirement_suffix": "mode.rgba",
            "name": "mode-rgba-90-non-expand",
            "mode": "RGBA",
            "values": {"angle": literal(90)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "rotate",
            "requirement_suffix": "behavior.default",
            "name": "opened-rgb-270-expand",
            "scenario_asset": "image/rgb-small.png",
            "values": {"angle": literal(270), "expand": literal(True)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transpose",
            "requirement_suffix": "behavior.default",
            "name": "opened-rgb",
            "scenario_asset": "image/rgb-small.png",
            "values": {"method": literal(0)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transpose",
            "requirement_suffix": "parameter.method",
            "name": "unknown-integer",
            "mode": "RGB",
            "values": {"method": literal(99)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "copy",
            "requirement_suffix": "behavior.default",
            "name": "opened-rgb",
            "scenario_asset": "image/rgb-small.png",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "copy",
            "requirement_suffix": "behavior.default",
            "name": "resize-pipeline",
            "chain": "resize-copy",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "point",
            "requirement_suffix": "behavior.default",
            "name": "opened-rgb",
            "scenario_asset": "image/rgb-small.png",
            "values": {"lut": literal([(i * 2) % 256 for i in range(256)])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "point",
            "requirement_suffix": "parameter.lut",
            "name": "expanded-rgb-lut",
            "mode": "RGB",
            "values": {"lut": literal([(i * 3) % 256 for i in range(256)] * 3)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "filter",
            "requirement_suffix": "behavior.default",
            "name": "opened-rgb",
            "scenario_asset": "image/rgb-small.png",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.l",
            "name": "opened-rgb",
            "scenario_asset": "image/rgb-small.png",
            "values": {"mode": literal("L")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.rgb",
            "name": "opened-rgba",
            "scenario_asset": "image/rgba-small.png",
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.rgb",
            "name": "opened-l",
            "scenario_asset": "image/l-small.png",
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "load",
            "requirement_suffix": "behavior.default",
            "name": "bmp-rgb-opened",
            "scenario_asset": "image/rgb-small.bmp",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "load",
            "requirement_suffix": "behavior.default",
            "name": "tiff-rgb-opened",
            "scenario_asset": "image/rgb-small.tiff",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "load",
            "requirement_suffix": "behavior.default",
            "name": "webp-rgb-opened",
            "scenario_asset": "image/rgb-small.webp",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "load",
            "requirement_suffix": "behavior.default",
            "name": "webp-rgba-opened",
            "scenario_asset": "image/rgba-small.webp",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "verify",
            "requirement_suffix": "behavior.default",
            "name": "bmp-rgb-opened",
            "scenario_asset": "image/rgb-small.bmp",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "verify",
            "requirement_suffix": "behavior.default",
            "name": "tiff-rgb-opened",
            "scenario_asset": "image/rgb-small.tiff",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "verify",
            "requirement_suffix": "behavior.default",
            "name": "webp-rgb-opened",
            "scenario_asset": "image/rgb-small.webp",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "verify",
            "requirement_suffix": "behavior.default",
            "name": "webp-rgba-opened",
            "scenario_asset": "image/rgba-small.webp",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "entropy",
            "requirement_suffix": "parameter.mask",
            "name": "masked-nonzero",
            "mode": "RGB",
            "mask_mode": "L",
            "edge": "mask-nonzero-pixel",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "entropy",
            "requirement_suffix": "parameter.mask",
            "name": "masked-nonzero-l",
            "mode": "L",
            "mask_mode": "L",
            "edge": "mask-nonzero-pixel",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "entropy",
            "requirement_suffix": "parameter.mask",
            "name": "masked-nonzero-la",
            "mode": "LA",
            "mask_mode": "L",
            "edge": "mask-nonzero-pixel",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "entropy",
            "requirement_suffix": "parameter.mask",
            "name": "one-mask",
            "mode": "RGB",
            "mask_mode": "1",
            "edge": "mask-nonzero-pixel",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "entropy",
            "requirement_suffix": "parameter.mask",
            "name": "mask-size-mismatch",
            "mode": "L",
            "mask_mode": "L",
            "edge": "mask-size-mismatch",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "entropy",
            "requirement_suffix": "parameter.mask",
            "name": "bad-mask-mode",
            "mode": "L",
            "mask_mode": "RGB",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "entropy",
            "requirement_suffix": "parameter.mask",
            "name": "truthy-non-image-mask",
            "mode": "L",
            "chain": "truthy-non-image-mask",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "p-mode-rgb-color",
            "mode": "P",
            "values": {
                "xy": literal([1, 1]),
                "value": literal([255, 0, 0]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "f-float",
            "mode": "F",
            "values": {
                "xy": literal([0, 0]),
                "value": literal(1.25),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "rgb-float-error",
            "mode": "RGB",
            "values": {
                "xy": literal([0, 0]),
                "value": literal(1.25),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "l-float-error",
            "mode": "L",
            "values": {
                "xy": literal([0, 0]),
                "value": literal(1.25),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "invalid-component-count",
            "mode": "RGB",
            "values": {
                "xy": literal([0, 0]),
                "value": literal([1, 2, 3, 4, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "invalid-component-type-rgb",
            "mode": "RGB",
            "values": {
                "xy": literal([0, 0]),
                "value": literal([1.5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "invalid-component-type-l",
            "mode": "L",
            "values": {
                "xy": literal([0, 0]),
                "value": literal([1.5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "l-negative-integer",
            "mode": "L",
            "values": {
                "xy": literal([0, 0]),
                "value": literal(-1),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "rgba-premultiplied-scalar",
            "mode": "RGBa",
            "values": {
                "xy": literal([0, 0]),
                "value": literal(200),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.im",
            "name": "la-source-into-rgb",
            "mode": "RGB",
            "im_mode": "LA",
            "values": {"box": literal([0, 0, 4, 4])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.im",
            "name": "rgba-source-into-rgb",
            "mode": "RGB",
            "im_mode": "RGBA",
            "values": {"box": literal([0, 0, 4, 4])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.mask",
            "name": "rgba-mask",
            "mode": "RGB",
            "im_mode": "RGB",
            "mask_mode": "RGBA",
            "values": {"box": literal([0, 0, 16, 16])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.mask",
            "name": "la-mask",
            "mode": "RGB",
            "im_mode": "RGB",
            "mask_mode": "LA",
            "values": {"box": literal([0, 0, 16, 16])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.mask",
            "name": "box-image-and-mask-conflict",
            "mode": "RGB",
            "chain": "paste-box-image-mask-conflict",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.im",
            "name": "l-source-into-rgb",
            "mode": "RGB",
            "im_mode": "L",
            "values": {"box": literal([0, 0, 4, 4])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.im",
            "name": "scalar-color-rgb",
            "mode": "RGB",
            "values": {
                "im": literal(255),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "parameter.im",
            "name": "scalar-color-la",
            "mode": "LA",
            "values": {
                "im": literal(255),
                "box": literal([1, 1, 5, 5]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "split",
            "requirement_suffix": "behavior.default",
            "name": "p-mode",
            "mode": "P",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "split",
            "requirement_suffix": "behavior.default",
            "name": "opened-p",
            "scenario_asset": "image/p-small.png",
        },
        {
            "surface": "PIL.Image",
            "operation": "frombytes",
            "requirement_suffix": "mode.1",
            "name": "valid-packed",
            "mode": "1",
            "edge": "valid-frombytes",
        },
        {
            "surface": "PIL.Image",
            "operation": "frombytes",
            "requirement_suffix": "mode.l",
            "name": "valid-l",
            "mode": "L",
            "edge": "valid-frombytes",
        },
        {
            "surface": "PIL.Image",
            "operation": "frombytes",
            "requirement_suffix": "mode.la",
            "name": "valid-la",
            "mode": "LA",
            "edge": "valid-frombytes",
        },
        {
            "surface": "PIL.Image",
            "operation": "frombytes",
            "requirement_suffix": "mode.p",
            "name": "valid-p",
            "mode": "P",
            "edge": "valid-frombytes",
        },
        {
            "surface": "PIL.Image",
            "operation": "frombytes",
            "requirement_suffix": "mode.rgb",
            "name": "valid-rgb",
            "mode": "RGB",
            "edge": "valid-frombytes",
        },
        {
            "surface": "PIL.Image",
            "operation": "frombytes",
            "requirement_suffix": "parameter.data",
            "name": "valid-rgba-lowercase",
            "mode": "RGBa",
            "edge": "valid-frombytes",
        },
        {
            "surface": "PIL.Image",
            "operation": "frombytes",
            "requirement_suffix": "mode.cmyk",
            "name": "valid-cmyk",
            "mode": "CMYK",
            "edge": "valid-frombytes",
        },
        {
            "surface": "PIL.Image",
            "operation": "frombytes",
            "requirement_suffix": "mode.rgba",
            "name": "valid-rgba",
            "mode": "RGBA",
            "edge": "valid-frombytes",
        },
        {
            "surface": "PIL.Image",
            "operation": "frombytes",
            "requirement_suffix": "parameter.data",
            "name": "valid-hsv",
            "mode": "HSV",
            "edge": "valid-frombytes",
        },
        {
            "surface": "PIL.Image",
            "operation": "frombytes",
            "requirement_suffix": "parameter.mode",
            "name": "valid-i",
            "mode": "I",
            "edge": "valid-frombytes",
        },
        {
            "surface": "PIL.Image",
            "operation": "frombytes",
            "requirement_suffix": "parameter.mode",
            "name": "valid-f",
            "mode": "F",
            "edge": "valid-frombytes",
        },
        {
            "surface": "PIL.Image",
            "operation": "frombytes",
            "requirement_suffix": "parameter.mode",
            "name": "valid-i16n",
            "mode": "I;16N",
            "values": {
                "size": literal([2, 2]),
                "data": bytes_literal([0, 0, 1, 0, 2, 0, 3, 0]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "frombytes",
            "requirement_suffix": "parameter.mode",
            "name": "valid-i16",
            "mode": "I;16",
            "values": {
                "size": literal([1, 1]),
                "data": bytes_literal([0x70, 0x11]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "frombytes",
            "requirement_suffix": "parameter.mode",
            "name": "valid-i16l",
            "mode": "I;16L",
            "values": {
                "size": literal([1, 1]),
                "data": bytes_literal([0x70, 0x11]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "frombytes",
            "requirement_suffix": "parameter.mode",
            "name": "valid-i16b",
            "mode": "I;16B",
            "values": {
                "size": literal([1, 1]),
                "data": bytes_literal([0x11, 0x70]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "frombytes",
            "requirement_suffix": "parameter.mode",
            "name": "valid-ycbcr",
            "mode": "YCbCr",
            "edge": "valid-frombytes",
        },
        {
            "surface": "PIL.Image",
            "operation": "frombytes",
            "requirement_suffix": "behavior.default",
            "name": "invalid-mode",
            "edge": "invalid-mode",
            "values": {
                "size": literal([1, 1]),
                "data": bytes_literal([0]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "frombytes",
            "requirement_suffix": "behavior.default",
            "name": "zero-size",
            "mode": "L",
            "edge": "zero-size",
            "values": {
                "data": bytes_literal([]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "frombytes",
            "requirement_suffix": "behavior.default",
            "name": "zero-size-p",
            "mode": "P",
            "edge": "zero-size",
            "values": {
                "data": bytes_literal([]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "frombytes",
            "requirement_suffix": "behavior.default",
            "name": "zero-size-i16n",
            "mode": "I;16N",
            "edge": "zero-size",
            "values": {
                "data": bytes_literal([]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "frombytes",
            "requirement_suffix": "behavior.default",
            "name": "zero-width",
            "mode": "L",
            "values": {
                "size": literal([0, 1]),
                "data": bytes_literal([]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "frombytes",
            "requirement_suffix": "behavior.default",
            "name": "zero-height",
            "mode": "L",
            "values": {
                "size": literal([1, 0]),
                "data": bytes_literal([]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "frombytes",
            "requirement_suffix": "mode.l",
            "name": "valid-l",
            "mode": "L",
            "edge": "valid-frombytes",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "frombytes",
            "requirement_suffix": "parameter.data",
            "name": "valid-rgb",
            "mode": "RGB",
            "edge": "valid-frombytes",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "frombytes",
            "requirement_suffix": "mode.la",
            "name": "valid-la",
            "mode": "LA",
            "edge": "valid-frombytes",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "frombytes",
            "requirement_suffix": "mode.rgba",
            "name": "valid-rgba",
            "mode": "RGBA",
            "edge": "valid-frombytes",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "frombytes",
            "requirement_suffix": "parameter.data",
            "name": "valid-rgba-lowercase",
            "mode": "RGBa",
            "edge": "valid-frombytes",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "frombytes",
            "requirement_suffix": "parameter.data",
            "name": "valid-packed",
            "mode": "1",
            "edge": "valid-frombytes",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "frombytes",
            "requirement_suffix": "parameter.data",
            "name": "valid-p",
            "mode": "P",
            "edge": "valid-frombytes",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "frombytes",
            "requirement_suffix": "parameter.data",
            "name": "valid-cmyk",
            "mode": "CMYK",
            "edge": "valid-frombytes",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "frombytes",
            "requirement_suffix": "parameter.data",
            "name": "valid-i",
            "mode": "I",
            "edge": "valid-frombytes",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "frombytes",
            "requirement_suffix": "parameter.data",
            "name": "valid-f",
            "mode": "F",
            "edge": "valid-frombytes",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "frombytes",
            "requirement_suffix": "parameter.data",
            "name": "valid-i16",
            "mode": "I;16",
            "edge": "valid-frombytes",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "frombytes",
            "requirement_suffix": "parameter.data",
            "name": "valid-i16b",
            "mode": "I;16B",
            "edge": "valid-frombytes",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "frombytes",
            "requirement_suffix": "parameter.data",
            "name": "valid-ycbcr",
            "mode": "YCbCr",
            "edge": "valid-frombytes",
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "tuple-p",
            "values": {
                "mode": literal("P"),
                "size": literal([2, 2]),
                "color": literal([10, 20, 30]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "scalar-p",
            "values": {
                "mode": literal("P"),
                "size": literal([2, 2]),
                "color": literal(7),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.mode",
            "name": "scalar-pa",
            "values": {
                "mode": literal("PA"),
                "size": literal([2, 2]),
                "color": literal(7),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "behavior.default",
            "name": "omitted-p-color",
            "values": {
                "mode": literal("P"),
                "size": literal([2, 2]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "rgb-string",
            "values": {
                "mode": literal("RGB"),
                "size": literal([2, 2]),
                "color": literal("rgb(1, 2, 3)"),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "rgb-percent-string",
            "values": {
                "mode": literal("RGB"),
                "size": literal([2, 2]),
                "color": literal("rgb(100%, 50%, 0%)"),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "rgba-string",
            "values": {
                "mode": literal("RGBA"),
                "size": literal([2, 2]),
                "color": literal("rgba(1, 2, 3, 128)"),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "rejected-rgba-short-string",
            "values": {
                "mode": literal("RGB"),
                "size": literal([2, 2]),
                "color": literal("rgba(1, 2, 3)"),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "rejected-rgba-nondigit-string",
            "values": {
                "mode": literal("RGB"),
                "size": literal([2, 2]),
                "color": literal("rgba(1, x, 3, 4)"),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "rejected-rgba-empty-component-string",
            "values": {
                "mode": literal("RGB"),
                "size": literal([2, 2]),
                "color": literal("rgba(1,, 3, 4)"),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "rejected-rgb-short-string",
            "values": {
                "mode": literal("RGB"),
                "size": literal([2, 2]),
                "color": literal("rgb(1, 2)"),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "rejected-rgb-unclosed-string",
            "values": {
                "mode": literal("RGB"),
                "size": literal([2, 2]),
                "color": literal("rgb(1, 2, 3"),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "rejected-rgb-nondigit-string",
            "values": {
                "mode": literal("RGB"),
                "size": literal([2, 2]),
                "color": literal("rgb(1, x, 3)"),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "rejected-rgb-empty-percent-component",
            "values": {
                "mode": literal("RGB"),
                "size": literal([2, 2]),
                "color": literal("rgb(%, 1%, 2%)"),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "rejected-leading-space-string",
            "values": {
                "mode": literal("RGB"),
                "size": literal([2, 2]),
                "color": literal(" red"),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "integer-scalar-i",
            "values": {
                "mode": literal("I"),
                "size": literal([2, 2]),
                "color": literal(-123456),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "tuple-rgb-i",
            "values": {
                "mode": literal("I"),
                "size": literal([2, 2]),
                "color": literal([1, 2, 3]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "tuple-rgb-f",
            "values": {
                "mode": literal("F"),
                "size": literal([2, 2]),
                "color": literal([1, 2, 3]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "tuple-rgb-i16",
            "values": {
                "mode": literal("I;16"),
                "size": literal([2, 1]),
                "color": literal([1, 2, 3]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "tuple-rgb-l",
            "values": {
                "mode": literal("L"),
                "size": literal([2, 2]),
                "color": literal([1, 2, 3]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "tuple-rgb-one",
            "values": {
                "mode": literal("1"),
                "size": literal([2, 2]),
                "color": literal([1, 2, 3]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "integer-scalar-i16",
            "values": {
                "mode": literal("I;16"),
                "size": literal([2, 1]),
                "color": literal(70000),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "integer-scalar-i16l",
            "values": {
                "mode": literal("I;16L"),
                "size": literal([2, 1]),
                "color": literal(70000),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "integer-scalar-i16b",
            "values": {
                "mode": literal("I;16B"),
                "size": literal([2, 1]),
                "color": literal(70000),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "integer-scalar-i16n",
            "values": {
                "mode": literal("I;16N"),
                "size": literal([2, 1]),
                "color": literal(70000),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "float-scalar-f",
            "values": {
                "mode": literal("F"),
                "size": literal([2, 2]),
                "color": literal(1.25),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "tuple-la",
            "values": {
                "mode": literal("LA"),
                "size": literal([2, 1]),
                "color": literal([17, 203]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "tuple-rgba",
            "values": {
                "mode": literal("RGBA"),
                "size": literal([2, 1]),
                "color": literal([1, 2, 3, 4]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "p-opaque-rgba-tuple",
            "values": {
                "mode": literal("P"),
                "size": literal([2, 2]),
                "color": literal([10, 20, 30, 255]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "p-transparent-rgba-error",
            "values": {
                "mode": literal("P"),
                "size": literal([2, 2]),
                "color": literal([10, 20, 30, 128]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "p-la-tuple",
            "values": {
                "mode": literal("P"),
                "size": literal([2, 2]),
                "color": literal([10, 128]),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "p-string-color",
            "values": {
                "mode": literal("P"),
                "size": literal([2, 2]),
                "color": literal("red"),
            },
        },
        {
            "surface": "PIL.Image",
            "operation": "new",
            "requirement_suffix": "parameter.color",
            "name": "none-color-rgb",
            "values": {
                "mode": literal("RGB"),
                "size": literal([2, 1]),
                "color": literal(None),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "rgba-rgb-tuple",
            "mode": "RGBA",
            "values": {
                "im": literal([255, 0, 0]),
                "box": literal([1, 1, 3, 3]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getexif",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-exif",
            "scenario_asset": "image/exif-orientation6.jpg",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getexif",
            "requirement_suffix": "behavior.default",
            "name": "tiff-container",
            "scenario_asset": "image/rgb-small.tiff",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getexif",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-without-exif",
            "scenario_asset": "image/rgb-small.jpg",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getexif",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-le-orientation2",
            "exif_variant": "le-orientation2",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getexif",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-no-orientation",
            "exif_variant": "no-orientation",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getexif",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-invalid-magic",
            "exif_variant": "invalid-magic",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getexif",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-no-exif-prefix",
            "exif_variant": "no-exif-prefix",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getexif",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-standalone-soi",
            "exif_variant": "standalone-soi",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getexif",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-standalone-rst0",
            "exif_variant": "standalone-rst0",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getexif",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-eoi-before-app1",
            "exif_variant": "eoi-before-app1",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getexif",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-empty-app1",
            "exif_variant": "empty-app1",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getexif",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-short-app1-length",
            "exif_variant": "short-app1-length",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getexif",
            "requirement_suffix": "behavior.default",
            "name": "jpeg-no-eoi",
            "exif_variant": "no-eoi",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getexif",
            "requirement_suffix": "behavior.default",
            "name": "png-without-exif",
            "scenario_asset": "image/rgb-small.png",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getim",
            "requirement_suffix": "behavior.default",
            "name": "rgb-default",
            "mode": "RGB",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getxmp",
            "requirement_suffix": "behavior.default",
            "name": "rgb-default",
            "mode": "RGB",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getxmp",
            "requirement_suffix": "mode.l",
            "name": "l-mode",
            "mode": "L",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "has_transparency_data",
            "requirement_suffix": "behavior.default",
            "name": "rgba-mode",
            "mode": "RGBA",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "has_transparency_data",
            "requirement_suffix": "behavior.default",
            "name": "la-mode",
            "mode": "LA",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "has_transparency_data",
            "requirement_suffix": "behavior.default",
            "name": "p-transparency",
            "scenario_asset": "image/p-transparency.png",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpalette",
            "requirement_suffix": "parameter.rawmode",
            "name": "attached-rgbx",
            "chain": "palette-getpalette-rgbx",
            "values": {"rawmode": literal("RGBX")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpalette",
            "requirement_suffix": "parameter.rawmode",
            "name": "attached-channel-r",
            "chain": "palette-getpalette-channel",
            "values": {"rawmode": literal("R")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpalette",
            "requirement_suffix": "parameter.rawmode",
            "name": "attached-channel-g",
            "chain": "palette-getpalette-channel-g",
            "values": {"rawmode": literal("G")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpalette",
            "requirement_suffix": "parameter.rawmode",
            "name": "attached-channel-b",
            "chain": "palette-getpalette-channel-b",
            "values": {"rawmode": literal("B")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpalette",
            "requirement_suffix": "parameter.rawmode",
            "name": "attached-channel-invalid",
            "chain": "palette-getpalette-channel-invalid",
            "values": {"rawmode": literal("XYZ")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpalette",
            "requirement_suffix": "parameter.rawmode",
            "name": "attached-alpha-rgbx",
            "chain": "palette-getpalette-alpha-rgbx",
            "values": {"rawmode": literal("RGBX")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpalette",
            "requirement_suffix": "behavior.default",
            "name": "attached-alpha-auto-rawmode",
            "chain": "palette-getpalette-alpha-rgbx",
            "values": {"rawmode": literal(None)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpalette",
            "requirement_suffix": "behavior.default",
            "name": "opened-transparency-auto-rawmode",
            "scenario_asset": "image/p-transparency.png",
            "values": {"rawmode": literal(None)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpalette",
            "requirement_suffix": "behavior.default",
            "name": "opened-duplicate-transparency-auto-rawmode",
            "mode": "P",
            "chain": "p-duplicate-transparency",
            "values": {"rawmode": literal(None)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "parameter.mode",
            "name": "attached-palette-transparency-table",
            "mode": "P",
            "chain": "palette-transparency-convert",
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "behavior.default",
            "name": "attached-palette-transparency-default-mode",
            "mode": "P",
            "chain": "palette-transparency-convert",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.la",
            "name": "p-to-la-without-transparency",
            "mode": "P",
            "values": {"mode": literal("LA")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.la",
            "name": "p-to-la-with-palette-transparency",
            "mode": "P",
            "chain": "palette-transparency-convert",
            "values": {"mode": literal("LA")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "verify",
            "requirement_suffix": "behavior.default",
            "name": "resize-pipeline",
            "chain": "resize-verify",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "verify",
            "requirement_suffix": "behavior.default",
            "name": "opened-rgb-resize-pipeline",
            "scenario_asset": "image/rgb-small.png",
            "chain": "opened-rgb-resize-verify",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "verify",
            "requirement_suffix": "behavior.default",
            "name": "p-resize-pipeline",
            "mode": "P",
            "chain": "p-resize-verify",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "verify",
            "requirement_suffix": "behavior.default",
            "name": "p-putpalette-resize-pipeline",
            "mode": "P",
            "chain": "p-putpalette-resize",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "verify",
            "requirement_suffix": "behavior.default",
            "name": "p-resize-convert-pipeline",
            "mode": "P",
            "chain": "p-resize-convert-verify",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "verify",
            "requirement_suffix": "behavior.default",
            "name": "p-resize-putalpha-pipeline",
            "mode": "P",
            "chain": "p-resize-putalpha-verify",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "load",
            "requirement_suffix": "behavior.default",
            "name": "quantized-pipeline",
            "chain": "quantize-load",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "verify",
            "requirement_suffix": "behavior.default",
            "name": "quantized-pipeline",
            "chain": "quantize-verify",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "save",
            "requirement_suffix": "format.png",
            "name": "quantized-pipeline",
            "chain": "quantize-save",
            "values": {"format": literal("PNG")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "load",
            "requirement_suffix": "behavior.default",
            "name": "p-resize-putalpha",
            "chain": "p-resize-putalpha-load",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.rgba",
            "name": "p-putalpha-to-rgba",
            "mode": "P",
            "chain": "p-putalpha-convert",
            "values": {"mode": literal("RGBA")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getchannel",
            "requirement_suffix": "behavior.default",
            "name": "p-resize-preserves-p-channel",
            "mode": "P",
            "chain": "p-resize-getchannel",
            "values": {"channel": literal(0)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.rgba",
            "name": "p-putpalette-putalpha-to-rgba",
            "mode": "P",
            "chain": "p-putpalette-putalpha-convert",
            "observe_receiver": True,
            "values": {"mode": literal("RGBA")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.1",
            "name": "p-putpalette-putalpha-to-one",
            "mode": "P",
            "chain": "p-putpalette-putalpha-convert",
            "values": {"mode": literal("1")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getchannel",
            "requirement_suffix": "behavior.default",
            "name": "pa-index-channel",
            "mode": "P",
            "chain": "p-putpalette-putalpha-convert",
            "values": {"channel": literal(0)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getchannel",
            "requirement_suffix": "behavior.default",
            "name": "pa-alpha-channel",
            "mode": "P",
            "chain": "p-putpalette-putalpha-convert",
            "values": {"channel": literal(1)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "remap_palette",
            "requirement_suffix": "parameter-combination.legacy-001",
            "name": "attached-alpha-remap",
            "mode": "P",
            "chain": "p-putpalette-remap",
            "values": {"dest_map": literal([0, 1])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "remap_palette",
            "requirement_suffix": "parameter.source-palette",
            "name": "explicit-rgba-source-palette",
            "mode": "P",
            "values": {
                "dest_map": literal([0, 1]),
                "source_palette": literal([1, 2, 3, 128] * 193),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "remap_palette",
            "requirement_suffix": "parameter.dest-map",
            "name": "oversized-dest-map",
            "mode": "P",
            "values": {"dest_map": literal([0] * 257)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "tobytes",
            "requirement_suffix": "parameter.args",
            "name": "rgb-bgr-raw",
            "mode": "RGB",
            "values": {
                "encoder_name": literal("raw"),
                "args": literal(["BGR"]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "tobytes",
            "requirement_suffix": "parameter.args",
            "name": "rgba-bgra-raw",
            "mode": "RGBA",
            "values": {
                "encoder_name": literal("raw"),
                "args": literal(["BGRA"]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "tobytes",
            "requirement_suffix": "parameter.args",
            "name": "rgb-rgba-raw",
            "mode": "RGB",
            "edge": "uniform-fill",
            "pixel": [1, 2, 3],
            "values": {
                "encoder_name": literal("raw"),
                "args": literal(["RGBA"]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "tobytes",
            "requirement_suffix": "parameter.args",
            "name": "rgb-invalid-raw-mode",
            "mode": "RGB",
            "values": {
                "encoder_name": literal("raw"),
                "args": literal(["LA"]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "tobytes",
            "requirement_suffix": "parameter.encoder-name",
            "name": "unknown-encoder-error",
            "mode": "RGB",
            "values": {"encoder_name": literal("foo")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "tobytes",
            "requirement_suffix": "behavior.default",
            "name": "i-mode",
            "mode": "I",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "tobytes",
            "requirement_suffix": "behavior.default",
            "name": "f-mode",
            "mode": "F",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "tobytes",
            "requirement_suffix": "behavior.default",
            "name": "i16-mode",
            "mode": "I;16",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "tobytes",
            "requirement_suffix": "behavior.default",
            "name": "i16b-mode",
            "mode": "I;16B",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "tobytes",
            "requirement_suffix": "behavior.default",
            "name": "one-packed-nonzero",
            "mode": "1",
            "edge": "nonzero-pixel",
            "pixel": 255,
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "i-scalar",
            "observe_receiver": True,
            "mode": "I",
            "values": {
                "xy": literal([0, 0]),
                "value": literal(200),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "f-scalar",
            "observe_receiver": True,
            "mode": "F",
            "values": {
                "xy": literal([0, 0]),
                "value": literal(200),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "ycbcr-scalar",
            "observe_receiver": True,
            "mode": "YCbCr",
            "values": {
                "xy": literal([0, 0]),
                "value": literal(200),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "hsv-scalar",
            "observe_receiver": True,
            "mode": "HSV",
            "values": {
                "xy": literal([0, 0]),
                "value": literal(200),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "p-full-palette-replace",
            "observe_receiver": True,
            "mode": "P",
            "chain": "p-full-palette-putpixel",
            "values": {
                "xy": literal([0, 0]),
                "value": literal([9, 8, 7]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpixel",
            "requirement_suffix": "behavior.default",
            "name": "y-out-of-bounds",
            "mode": "L",
            "edge": "y-out-of-bounds",
            "values": {"xy": literal([0, 16])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpixel",
            "requirement_suffix": "behavior.default",
            "name": "i-y-out-of-bounds",
            "mode": "I",
            "edge": "y-out-of-bounds",
            "values": {"xy": literal([0, 16])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpixel",
            "requirement_suffix": "behavior.default",
            "name": "f-y-out-of-bounds",
            "mode": "F",
            "edge": "y-out-of-bounds",
            "values": {"xy": literal([0, 16])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpixel",
            "requirement_suffix": "behavior.default",
            "name": "i-x-out-of-bounds",
            "mode": "I",
            "edge": "x-out-of-bounds",
            "values": {"xy": literal([16, 0])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpixel",
            "requirement_suffix": "behavior.default",
            "name": "f-x-out-of-bounds",
            "mode": "F",
            "edge": "x-out-of-bounds",
            "values": {"xy": literal([16, 0])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpixel",
            "requirement_suffix": "behavior.default",
            "name": "i-large-nonzero",
            "mode": "I",
            "edge": "nonzero-pixel",
            "pixel": 100000,
            "values": {"xy": literal([2, 3])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpixel",
            "requirement_suffix": "behavior.default",
            "name": "f-nonzero",
            "mode": "F",
            "edge": "nonzero-pixel",
            "pixel": 1.5,
            "values": {"xy": literal([2, 3])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "getpixel",
            "requirement_suffix": "behavior.default",
            "name": "ycbcr-nonzero",
            "mode": "YCbCr",
            "edge": "nonzero-pixel",
            "pixel": [100, 150, 200],
            "values": {"xy": literal([2, 3])},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "p-y-out-of-bounds",
            "mode": "P",
            "edge": "y-out-of-bounds",
            "values": {"xy": literal([0, 16]), "value": literal(7)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "p-x-out-of-bounds",
            "mode": "P",
            "values": {"xy": literal([16, 0]), "value": literal(7)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "i-y-out-of-bounds",
            "mode": "I",
            "edge": "y-out-of-bounds",
            "values": {"xy": literal([0, 16]), "value": literal(200)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "i-x-out-of-bounds",
            "mode": "I",
            "values": {"xy": literal([16, 0]), "value": literal(200)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "f-y-out-of-bounds",
            "mode": "F",
            "edge": "y-out-of-bounds",
            "values": {"xy": literal([0, 16]), "value": literal(200)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "f-x-out-of-bounds",
            "mode": "F",
            "values": {"xy": literal([16, 0]), "value": literal(200)},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "p-attached-palette-exact-match",
            "observe_receiver": True,
            "mode": "P",
            "chain": "p-attached-palette-putpixel",
            "values": {
                "xy": literal([0, 0]),
                "value": literal([10, 20, 30]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "p-full-palette-exhausted",
            "mode": "P",
            "chain": "p-full-palette-exhausted-putpixel",
            "values": {
                "xy": literal([0, 0]),
                "value": literal([9, 8, 7]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "raw-p-no-palette-tuple",
            "mode": "P",
            "edge": "raw-p-no-palette",
            "values": {
                "xy": literal([0, 0]),
                "value": literal([9, 8, 7]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "p-after-bitmap-no-palette-tuple",
            "observe_receiver": True,
            "mode": "P",
            "edge": "raw-p-no-palette",
            "chain": "p-bitmap-putpixel",
            "values": {
                "xy": literal([0, 0]),
                "value": literal([9, 8, 7]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "rgb-y-out-of-bounds-tuple",
            "mode": "RGB",
            "edge": "y-out-of-bounds",
            "values": {
                "xy": literal([0, 16]),
                "value": literal([1, 2, 3]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putpixel",
            "requirement_suffix": "behavior.default",
            "name": "p-full-palette-transparent-slot",
            "observe_receiver": True,
            "mode": "P",
            "chain": "p-full-palette-index-transparency-putpixel",
            "values": {
                "xy": literal([0, 0]),
                "value": literal([9, 8, 7]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "convert",
            "requirement_suffix": "mode.rgb",
            "name": "raw-p-without-palette-to-rgb",
            "mode": "P",
            "edge": "raw-p-no-palette",
            "values": {"mode": literal("RGB")},
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "pa-tuples",
            "observe_receiver": True,
            "mode": "PA",
            "values": {
                "data": literal([[1, 255], [2, 128], [3, 0], [4, 64]]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "pa-invalid-component-count",
            "mode": "PA",
            "values": {
                "data": literal([[1, 2, 3]] * 4),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "rgb-four-tuples",
            "observe_receiver": True,
            "mode": "RGB",
            "values": {
                "data": literal([[1, 2, 3, 4], [5, 6, 7, 8]]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "rgba-three-tuples",
            "observe_receiver": True,
            "mode": "RGBA",
            "values": {
                "data": literal([[1, 2, 3], [4, 5, 6]]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "l-nan-clips-high",
            "observe_receiver": True,
            "mode": "L",
            "values": {
                "data": literal([float("nan")]),
            },
        },
    )

    requirements: dict[tuple[str, str], dict[str, dict[str, Any]]] = {}
    for surface in manifest["surfaces"]:
        for operation in surface["operations"]:
            prefix = operation_prefix(surface["id"], operation["id"])
            requirements[(surface["id"], operation["id"])] = {
                item["id"].removeprefix(prefix + "."): item
                for item in operation["requirements"]
            }

    cases: list[dict[str, Any]] = []
    for spec in specs:
        if spec["surface"] != surface_id:
            continue
        key = (spec["surface"], spec["operation"])
        operation = operations.get(key)
        if operation is None:
            raise ValueError(f"nuanced case references unknown operation: {key}")
        requirement = requirements[key].get(spec["requirement_suffix"])
        if requirement is None:
            raise ValueError(
                f"nuanced case requirement missing: {key}"
                f".{spec['requirement_suffix']}"
            )
        prefix = operation_prefix(*key)
        cases.append(
            build_parity_case(
                spec["surface"],
                operation,
                requirement,
                operations,
                assets_root,
                case_id=f"{prefix}.nuanced.{slug(spec['name'])}",
                scenario_values=spec.get("values"),
                scenario_mode=spec.get("mode"),
                scenario_draw_mode=spec.get("draw_mode"),
                scenario_edge=spec.get("edge"),
                scenario_pixel=spec.get("pixel"),
                scenario_font=spec.get("font"),
                scenario_font_size=spec.get("font_size"),
                scenario_transposed_orientation=spec.get("orientation"),
                scenario_bitmap_mode=spec.get("bitmap_mode"),
                scenario_bitmap_color=spec.get("bitmap_color"),
                scenario_size=spec.get("size"),
                scenario_im_mode=spec.get("im_mode"),
                scenario_mask_mode=spec.get("mask_mode"),
                scenario_noise_seed=spec.get("seed"),
                scenario_asset=spec.get("scenario_asset"),
                scenario_inline_image=spec.get("scenario_inline_image"),
                scenario_inline_mask_image=spec.get("scenario_inline_mask_image"),
                scenario_exif_variant=spec.get("exif_variant"),
                scenario_chain=spec.get("chain"),
                scenario_observe_result=spec.get("observe_result"),
                scenario_observe_receiver=spec.get("observe_receiver", False),
                scenario_observe_stat_properties=spec.get(
                    "observe_stat_properties", False
                ),
                scenario_outline_curve=spec.get("outline_curve", False),
                scenario_outline_empty=spec.get("outline_empty", False),
            )
        )
    return cases


def build_crash_quarantine_cases(
    manifest: dict[str, Any],
    operations: dict[tuple[str, str], dict[str, Any]],
    assets_root: Path,
) -> list[dict[str, Any]]:
    """Build input-only cases that are retained outside active execution.

    These workflows are deliberately not part of ``manifest.input_index``.
    They are preserved as reproducible stimuli for later crash analysis after
    the source adapter can report an isolated crash status safely.
    """

    requirements: dict[tuple[str, str], dict[str, dict[str, Any]]] = {}
    for surface in manifest["surfaces"]:
        for operation in surface["operations"]:
            key = (surface["id"], operation["id"])
            prefix = operation_prefix(*key)
            requirements[key] = {
                item["id"].removeprefix(prefix + "."): item
                for item in operation["requirements"]
            }

    cases: list[dict[str, Any]] = []
    for spec in CRASH_QUARANTINE_SPECS:
        key = (spec["surface"], spec["operation"])
        operation = operations.get(key)
        if operation is None:
            raise ValueError(f"crash quarantine case references unknown operation: {key}")
        requirement = requirements[key].get(spec["requirement_suffix"])
        if requirement is None:
            raise ValueError(
                f"crash quarantine requirement missing: {key}"
                f".{spec['requirement_suffix']}"
            )
        prefix = operation_prefix(*key)
        cases.append(
            build_parity_case(
                spec["surface"],
                operation,
                requirement,
                operations,
                assets_root,
                case_id=f"{prefix}.nuanced.{slug(spec['name'])}",
                scenario_font=spec.get("font"),
                scenario_observe_receiver=spec.get("observe_receiver", False),
            )
        )
    return cases


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(payload, indent=2, sort_keys=False) + "\n",
        encoding="utf-8",
    )


def build_inputs(
    manifest: dict[str, Any],
    output_root: Path,
    active_assets_root: Path,
) -> dict[str, int]:
    operations = operation_index(manifest)
    indexed = {
        lane: set(paths)
        for lane, paths in manifest["input_index"].items()
    }
    generated = {"parity": set(), "coverage": set(), "benchmark": set()}
    case_by_requirement: dict[str, str] = {}
    counts = {
        "parity_cases": 0,
        "coverage_plans": 0,
        "benchmark_workloads": 0,
        "benchmark_suites": 0,
    }
    crash_quarantine_cases = build_crash_quarantine_cases(
        manifest,
        operations,
        active_assets_root,
    )
    write_json(
        output_root / CRASH_QUARANTINE_RELATIVE,
        {
            "schema": "migration-parity/crash-quarantine-input@1",
            "status": "quarantined",
            "active": False,
            "execution": "manual",
            "reason": CRASH_QUARANTINE_REASON,
            "cases": crash_quarantine_cases,
        },
    )

    for surface in manifest["surfaces"]:
        surface_id = surface["id"]
        storage_slug = surface["storage_slug"]
        parity_cases: list[dict[str, Any]] = []
        parity_candidates: list[dict[str, Any]] = []
        coverage_requirements: list[str] = []
        benchmark_requirements: list[
            tuple[dict[str, Any], dict[str, Any]]
        ] = []
        component_ids: list[str] = []

        for operation in surface["operations"]:
            for component_id in operation["coverage"].get("component_ids", []):
                if component_id not in component_ids:
                    component_ids.append(component_id)
            for requirement in operation["requirements"]:
                if "parity" in requirement["lanes"]:
                    case = build_parity_case(
                        surface_id,
                        operation,
                        requirement,
                        operations,
                        active_assets_root,
                    )
                    parity_candidates.append(case)
                if "coverage" in requirement["lanes"]:
                    coverage_requirements.append(requirement["id"])
                if "benchmark" in requirement["lanes"]:
                    benchmark_requirements.append((operation, requirement))

        parity_cases, local_case_by_requirement, duplicate_count = (
            merge_duplicate_cases(parity_candidates)
        )
        for requirement_id, case_id in local_case_by_requirement.items():
            if requirement_id in case_by_requirement:
                raise ValueError(f"requirement mapped twice: {requirement_id}")
            case_by_requirement[requirement_id] = case_id
        nuanced_cases = build_nuanced_cases(
            manifest,
            operations,
            active_assets_root,
            surface_id,
        )
        existing_signatures = {case_signature(case) for case in parity_cases}
        added_nuanced_cases = 0
        appended_nuanced: list[dict[str, Any]] = []
        for nuanced_case in nuanced_cases:
            signature = case_signature(nuanced_case)
            if signature not in existing_signatures:
                parity_cases.append(nuanced_case)
                existing_signatures.add(signature)
                appended_nuanced.append(nuanced_case)
                added_nuanced_cases += 1
        counts.setdefault("nuanced_parity_cases", 0)
        counts["nuanced_parity_cases"] += added_nuanced_cases

        parity_relative = f"inputs/parity/{storage_slug}.json"
        parity_path = output_root / parity_relative
        write_json(
            parity_path,
            {
                "schema": "migration-parity/parity-input@1",
                "cases": parity_cases,
            },
        )
        generated["parity"].add(parity_relative)
        counts["parity_cases"] += len(parity_cases)
        counts.setdefault("duplicate_parity_cases", 0)
        counts["duplicate_parity_cases"] += duplicate_count

        selected_cases = [
            case_by_requirement[requirement_id]
            for requirement_id in coverage_requirements
        ]
        selected_cases.extend(
            case["case_id"]
            for case in appended_nuanced
            if any(
                requirement_id in coverage_requirements
                for requirement_id in case["covers"]
            )
        )
        selected_cases = list(dict.fromkeys(selected_cases))
        # Coverage is intentionally input-only: do not add direct native
        # probes to compensate for paths that lack a public parity workflow.
        # Those paths remain visible as uncovered until a real public input
        # can reach them or they are documented as unreachable.
        command_ids = []
        coverage_relative = f"inputs/coverage/{storage_slug}.json"
        coverage_path = output_root / coverage_relative
        write_json(
            coverage_path,
            {
                "schema": "migration-parity/coverage-input@1",
                "plans": [
                    {
                        "plan_id": f"{storage_slug}.coverage-plan",
                        "covers": coverage_requirements,
                        "target_profile": TARGET_PROFILE,
                        "selectors": {
                            "parity_case_ids": selected_cases,
                            "command_ids": command_ids,
                        },
                        "component_ids": component_ids,
                        "command_id": "coverage",
                    }
                ],
            },
        )
        generated["coverage"].add(coverage_relative)
        counts["coverage_plans"] += 1

        workloads: list[dict[str, Any]] = []
        members: list[dict[str, Any]] = []
        for operation, requirement in benchmark_requirements:
            case_id = case_by_requirement.get(requirement["id"])
            if case_id is None:
                raise ValueError(
                    f"benchmark requirement lacks correctness case: "
                    f"{requirement['id']}"
                )
            workload_id = (
                f"{storage_slug}.{slug(operation['id'])}.standard"
            )
            workloads.append(
                {
                    "workload_id": workload_id,
                    "covers": [requirement["id"]],
                    "subjects": [
                        {"kind": "oracle", "id": "pillow"},
                        {"kind": "target_profile", "id": TARGET_PROFILE},
                    ],
                    "input": {
                        "kind": "parity_case",
                        "case_id": case_id,
                    },
                    "measurement": {
                        "boundary": "observed_steps",
                        "step_ids": ["call"],
                        "metrics": operation["benchmark"]["metrics"],
                        "warmup_iterations": 5,
                        "measurement_iterations": 20,
                        "samples": 5,
                        "concurrency": 1,
                        "cache_state": "warm",
                        "correctness_gate": "parity_pass",
                    },
                }
            )
            members.append({"workload_id": workload_id, "weight": 1})
        benchmark_relative = f"inputs/benchmark/{storage_slug}.json"
        benchmark_path = output_root / benchmark_relative
        suites = (
            [
                {
                    "suite_id": f"{storage_slug}.benchmark-suite",
                    "description": (
                        f"Equal-weight public workloads for {surface_id}."
                    ),
                    "members": members,
                }
            ]
            if members
            else []
        )
        write_json(
            benchmark_path,
            {
                "schema": "migration-parity/benchmark-input@1",
                "workloads": workloads,
                "suites": suites,
            },
        )
        generated["benchmark"].add(benchmark_relative)
        counts["benchmark_workloads"] += len(workloads)
        counts["benchmark_suites"] += len(suites)

    for lane in ("parity", "coverage", "benchmark"):
        if generated[lane] != indexed[lane]:
            raise ValueError(
                f"{lane} input index drift: "
                f"missing={sorted(indexed[lane] - generated[lane])}, "
                f"extra={sorted(generated[lane] - indexed[lane])}"
            )
    return counts


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--manifest",
        type=Path,
        default=DEFAULT_MANIFEST,
        help="Fixed manifest@2 path.",
    )
    parser.add_argument(
        "--output-root",
        type=Path,
        default=DEFAULT_OUTPUT_ROOT,
        help="Fixture root receiving indexed input documents.",
    )
    args = parser.parse_args()
    manifest = load_manifest(args.manifest.resolve())
    counts = build_inputs(
        manifest,
        args.output_root.resolve(),
        FIXTURE_ROOT / "assets",
    )
    print(json.dumps(counts, sort_keys=True))


if __name__ == "__main__":
    main()
