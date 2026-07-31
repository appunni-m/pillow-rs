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
import re
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


def slug(value: str) -> str:
    return re.sub(r"[^A-Za-z0-9]+", "-", value).strip("-").lower()


def literal(value: Any) -> dict[str, Any]:
    return {"kind": "literal", "value": value}


def binding(step_id: str) -> dict[str, str]:
    return {"kind": "binding", "step_id": step_id}


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
    return validate_fixed_manifest(manifest, manifest_path=path)


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
    _outline_step: str | None = None
    scenario_values: dict[str, dict[str, Any]] = field(default_factory=dict)
    scenario_mode: str | None = None
    scenario_edge: str | None = None
    scenario_pixel: Any | None = None
    scenario_font: str | None = None
    scenario_font_size: float | None = None
    scenario_transposed_orientation: Any | None = None
    scenario_bitmap_mode: str | None = None
    scenario_bitmap_color: int | None = None
    scenario_size: list[int] | None = None
    scenario_im_mode: str | None = None
    scenario_mask_mode: str | None = None
    scenario_asset: str | None = None

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
        cache_key = f"{label}:{requested_mode}"
        if cache_key in self._image_steps:
            return self._image_steps[cache_key]
        if self.scenario_asset is not None:
            # Stimulus workflows that open an encoded container (for example
            # the JPEG-with-EXIF `ImageOps.exif_transpose` cases) build the
            # primary image from a committed asset instead of `Image.new`.
            fp_descriptor = self.ref(
                f"{label}-asset",
                self.scenario_asset,
                "image/jpeg",
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
        elif self.edge == "mask-size-mismatch" and label == "mask":
            size = [8, 8]
        step_id = self.add_step(
            "PIL.Image",
            "new",
            receiver=None,
            arguments={
                "mode": literal(requested_mode),
                "size": literal(size),
                "color": literal(
                    self.scenario_bitmap_color
                    if label == "bitmap" and self.scenario_bitmap_color is not None
                    else 0
                ),
            },
            step_id=self.next_step_id(f"setup-{label}"),
        )
        self._image_steps[cache_key] = step_id
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
        elif self.edge == "nonzero-pixel" and label == "image":
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

    def ensure_outline(self) -> str:
        if self._outline_step is None:
            self._outline_step = self.add_step(
                "PIL.ImageDraw",
                "Outline",
                receiver=None,
                arguments={},
                step_id=self.next_step_id("setup-outline"),
            )
        return self._outline_step

    def receiver_for(self, surface: str) -> dict[str, str] | None:
        if surface == "PIL.Image.Image":
            return binding(self.ensure_image())
        if surface == "PIL.ImageDraw.ImageDraw":
            image_step = self.ensure_image()
            draw_step = self.add_step(
                "PIL.ImageDraw",
                "Draw",
                receiver=None,
                arguments={"im": binding(image_step)},
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
            palette = self.add_step(
                "PIL.ImagePalette",
                "ImagePalette",
                receiver=None,
                arguments={},
                step_id=self.next_step_id("setup-palette"),
            )
            return binding(palette)
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
        if name == "format" and edge.startswith("webp-"):
            return "WEBP"
        if name == "format" and edge == "unsupported-format":
            return "NOT_A_FORMAT"
        if edge == "too-many-colors" and name == "maxcolors":
            return 1
        if edge in {"out-of-bounds", "negative-coords"} and name == "xy":
            return [16, 16] if edge == "out-of-bounds" else [-1, -1]
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

        for scenario_key, descriptor in self.scenario_values.items():
            if scenario_key == parameter_id:
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
                return descriptor

        if parameter_id == "font" and self.scenario_font is not None:
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
                and value in enum_values
                and "integer" in value_types
                and "string" not in value_types
                and "enum" not in value_types
            ):
                value = enum_values[value]
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
                "webp-corrupt-vp8-bitstream",
                "webp-truncated-riff",
                "webp-invalid-riff-header",
            }:
                edge_bytes = {
                    "invalid-bytes": b"\x00invalid",
                    "empty-bytes": b"",
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
                return self.ref(
                    "pilfont",
                    "font/pilfont/courb08.pil",
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
            return self.builtin("output", "temporary-output-path")
        if name in {"shape"} and self.primary_surface.startswith(
            "PIL.ImageDraw"
        ):
            return binding(self.ensure_outline())
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
            return binding(self.ensure_outline())
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
        receiver = self.receiver_for(self.primary_surface)
        arguments = self.primary_arguments(operation)
        call_id = self.add_step(
            self.primary_surface,
            self.primary_operation,
            receiver=receiver,
            arguments=arguments,
            step_id="call",
        )
        observations = [call_id]

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
    scenario_edge: str | None = None,
    scenario_pixel: Any | None = None,
    scenario_font: str | None = None,
    scenario_font_size: float | None = None,
    scenario_transposed_orientation: Any | None = None,
    scenario_bitmap_mode: str | None = None,
    scenario_bitmap_color: int | None = None,
    scenario_size: list[int] | None = None,
    scenario_im_mode: str | None = None,
    scenario_mask_mode: str | None = None,
    scenario_asset: str | None = None,
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
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getbbox",
            "requirement_suffix": "parameter.text",
            "name": "unicode-multiline",
            "values": {"text": literal("A\u0301\nAV🙂")},
        },
        {
            "surface": "PIL.ImageFont.FreeTypeFont",
            "operation": "getlength",
            "requirement_suffix": "parameter.text",
            "name": "kerning-pair",
            "values": {"text": literal("AV")},
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
            "operation": "set_variation_by_name",
            "requirement_suffix": "behavior.default",
            "name": "variable-font",
            "font": "font/fonts/variable-name-platform1-fallback.ttf",
            "values": {"name": literal("Bold")},
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
            "name": "two-points-outline",
            "values": {
                "xy": literal([[2, 2], [12, 8]]),
                "outline": literal([0, 255, 0]),
            },
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
            "name": "named-i",
            "values": {
                "color": literal("blue"),
                "mode": literal("I"),
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
            "name": "rgb-percent",
            "values": {"color": literal("rgb(100%, 50%, 0%)")},
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
            "values": {
                "size": literal([17, 9]),
                "resample": literal(1),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "rotate",
            "requirement_suffix": "parameter.angle",
            "name": "fractional-expanded",
            "values": {
                "angle": literal(33.5),
                "expand": literal(True),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "transform",
            "requirement_suffix": "behavior.default",
            "name": "p-affine-scalar-fill",
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
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "color-l",
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
            "name": "color-rgb",
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
            "name": "color-la",
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
            "operation": "alpha_composite",
            "requirement_suffix": "behavior.default",
            "name": "offset-source",
            "mode": "RGBA",
            "values": {
                "source": literal([1, 1]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "i-mode",
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
            "operation": "eval",
            "requirement_suffix": "behavior.default",
            "name": "rgb-replicated-lut",
            "mode": "RGB",
            "values": {
                "args": literal(list(range(256))),
            },
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
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "p-indices",
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
            "mode": "RGBA",
            "values": {
                "data": literal(
                    [255, 0, 0, 128] * 9
                ),
            },
        },
        {
            "surface": "PIL.ImageStat",
            "operation": "Stat",
            "requirement_suffix": "behavior.default",
            "name": "from-histogram-list",
            "values": {
                "image_or_list": literal(
                    [10 if index == 5 else 54 if index == 200 else 0 for index in range(256)]
                ),
            },
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
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "rgba-tuples",
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
            "mode": "CMYK",
            "values": {
                "data": literal([[0, 255, 0, 0]] * 9),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "putdata",
            "requirement_suffix": "behavior.default",
            "name": "la-tuples",
            "mode": "LA",
            "values": {
                "data": literal([[255, 128]] * 9),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "l-from-rgb",
            "mode": "L",
            "im_mode": "RGB",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "rgb-from-rgba",
            "mode": "RGB",
            "im_mode": "RGBA",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "paste",
            "requirement_suffix": "behavior.default",
            "name": "p-from-l",
            "mode": "P",
            "im_mode": "L",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "behavior.default",
            "name": "offset-dest",
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
            "mode": "RGBA",
            "edge": "source-larger-than-dest",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "behavior.default",
            "name": "source-smaller-than-dest",
            "mode": "RGBA",
            "edge": "source-smaller-than-dest",
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "behavior.default",
            "name": "source-four-tuple",
            "mode": "RGBA",
            "values": {
                "source": literal([2, 2, 8, 8]),
            },
        },
        {
            "surface": "PIL.Image.Image",
            "operation": "alpha_composite",
            "requirement_suffix": "behavior.default",
            "name": "source-smaller-offset-dest",
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
            "requirement_suffix": "behavior.default",
            "name": "hsv-to-rgb",
            "mode": "HSV",
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
            "operation": "getextrema",
            "requirement_suffix": "behavior.default",
            "name": "nonzero-rgba",
            "mode": "RGBA",
            "edge": "nonzero-pixel",
            "pixel": [10, 200, 30, 255],
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
            "surface": "PIL.ImageOps",
            "operation": "fit",
            "requirement_suffix": "parameter.size",
            "name": "fractional-centering",
            "values": {
                "size": literal([13, 7]),
                "centering": literal([0.25, 0.75]),
                "resample": literal("BICUBIC"),
            },
        },
        {
            "surface": "PIL.ImageOps",
            "operation": "colorize",
            "requirement_suffix": "behavior.default",
            "name": "two-color",
            "mode": "L",
            "edge": "nonzero-pixel",
            "pixel": 128,
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
            "operation": "invert",
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
        if "image-font" in component_ids and surface_id == "PIL.ImageFont.FreeTypeFont":
            command_ids = ["coverage-font-native"]
        elif "image-ops" in component_ids:
            # The EXIF parsers and the core colorize guards are unreachable
            # through the public Pillow surface; exercise them through the
            # maintained native coverage command.
            command_ids = ["coverage-imageops-native"]
        elif "image-sequence" in component_ids:
            # The iterator protocol (``__iter__``/``__next__``) has no public
            # manifest endpoint; exercise it through the maintained native
            # coverage command.
            command_ids = ["coverage-imagesequence-native"]
        elif "image-core" in component_ids:
            # Module-level helpers (fromarray variants, merge, gradients,
            # resize/crop/rotate/convert wrappers) have no Pillow oracle
            # endpoint; exercise them through the maintained native command.
            command_ids = ["coverage-imagecore-native"]
        elif "image-draw" in component_ids:
            # `Draw.shape` requires a real Outline built through the
            # move/line/close protocol the parity generator does not emit.
            command_ids = ["coverage-imagedraw-native"]
        elif "image-color" in component_ids:
            # The getcolor mode matrix and rejected function-form variants
            # are not selected by the parity plan.
            command_ids = ["coverage-imagecolor-native"]
        elif "image-palette" in component_ids:
            # The palette append/lookup shapes and save surfaces are not
            # selected by the parity plan.
            command_ids = ["coverage-imagepalette-native"]
        else:
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
