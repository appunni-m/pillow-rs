#!/usr/bin/env python3
"""Build the fixed project-wide migration-parity manifest.

This is a deterministic migration aid for the initial manifest@2 conversion.
It joins the frozen deprecated authority inventory with the pinned Pillow
12.2.0 public signatures and the current public pillow_rs facade signatures.
It writes specification only: no test outcomes, counts, percentages, timings,
coverage observations, or current target revision.
"""

from __future__ import annotations

import argparse
import enum
import importlib
import inspect
import json
import re
import sys
from collections import defaultdict
from pathlib import Path
from typing import Any, Iterable

import yaml

from migration_parity_inventory import (
    AUTHORITY_PATH,
    EXPECTED_AUTHORITY_SHA256,
    EXPECTED_PILLOW_VERSION,
    Endpoint,
    LegacyRef,
    derive_inventory,
    load_authority,
)


WORKSPACE_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = (
    WORKSPACE_ROOT / "pillow-rs" / "tests" / "fixtures" / "manifest.yaml"
)
PYTHON_FACADE_ROOT = WORKSPACE_ROOT / "pillow-rs-py" / "python"
TARGET_PROFILE = "python-cpu"
BENCHMARK_BACKENDS = ("cpu", "simd", "gpu")
TARGET_ID = "pillow-rs-python"
ORACLE_ID = "pillow"

# The legacy authority records the Qt-only ImageQt endpoints as ignored for
# the current coverage campaign. Keep them in the active public inventory so
# the host-integration surface remains visible, but do not claim that the
# CPU/SIMD/GPU coverage lane measures them without a Qt host profile.
COVERAGE_NOT_APPLICABLE_ENDPOINTS: dict[str, str] = {
    "PIL.Image.Image::toqimage": (
        "Qt host integration requires an optional Qt binding and is outside "
        "the CPU/SIMD/GPU campaign."
    ),
    "PIL.Image.Image::toqpixmap": (
        "Qt host integration requires an optional Qt binding and is outside "
        "the CPU/SIMD/GPU campaign."
    ),
}

ALLOWED_VALUE_TYPES = {
    "null",
    "boolean",
    "integer",
    "number",
    "string",
    "bytes",
    "path",
    "enum",
    "sequence",
    "mapping",
    "record",
    "image",
    "font",
    "stream",
    "handle",
    "any_json",
}

COMPONENTS: dict[str, tuple[str, ...]] = {
    "image-core": (
        "pillow-rs-py/python/pillow_rs/image.py",
        "pillow-rs-py/python/pillow_rs/operations.py",
        "pillow-rs/src/image.rs",
        "pillow-rs/src/pipeline.rs",
        "pillow-rs/src/ops/module_fns.rs",
        "pillow-rs/src/ops/analysis.rs",
        "pillow-rs/src/ops/array.rs",
        "pillow-rs/src/ops/convert.rs",
        "pillow-rs/src/ops/crop.rs",
        "pillow-rs/src/ops/paste.rs",
        "pillow-rs/src/ops/quantize.rs",
        "pillow-rs/src/ops/resize.rs",
        "pillow-rs/src/ops/rotate.rs",
        "pillow-rs/src/ops/split.rs",
        "pillow-rs/src/ops/transform.rs",
        "pillow-rs/src/ops/transpose.rs",
        "pillow-rs/src/compute/pool_cpu/ops/geometry.rs",
    ),
    "image-draw": (
        "pillow-rs-py/python/pillow_rs/imagedraw.py",
        "pillow-rs/src/draw/mod.rs",
    ),
    "image-filter": (
        "pillow-rs-py/python/pillow_rs/imagefilter.py",
        "pillow-rs/src/ops/filter.rs",
        "pillow-rs/src/ops/param_filters.rs",
    ),
    "image-enhance": (
        "pillow-rs-py/python/pillow_rs/imageenhance.py",
        "pillow-rs/src/ops/enhance.rs",
    ),
    "image-ops": (
        "pillow-rs-py/python/pillow_rs/imageops.py",
        "pillow-rs/src/ops/imageops.rs",
    ),
    "image-chops": (
        "pillow-rs-py/python/pillow_rs/imagechops.py",
        "pillow-rs/src/ops/chops.rs",
    ),
    "image-color": (
        "pillow-rs-py/python/pillow_rs/imagecolor.py",
        "pillow-rs/src/color.rs",
    ),
    "image-palette": (
        "pillow-rs-py/python/pillow_rs/imagepalette.py",
        "pillow-rs/src/color.rs",
    ),
    "image-font": (
        "pillow-rs-py/python/pillow_rs/imagefont.py",
        "pillow-rs/src/font/mod.rs",
        "pillow-rs/src/font/imagingft.rs",
        "pillow-rs/src/font/pilfont.rs",
        "pillow-rs/src/lib.rs",
    ),
    "image-stat": (
        "pillow-rs-py/python/pillow_rs/imagestat.py",
        "pillow-rs/src/ops/analysis.rs",
    ),
    "image-sequence": (
        "pillow-rs-py/python/pillow_rs/imagesequence.py",
        "pillow-rs/src/image_sequence.rs",
    ),
}


def slug(value: str) -> str:
    normalized = re.sub(r"[^A-Za-z0-9]+", "-", value).strip("-")
    return normalized.lower()


def requirement_prefix(endpoint: Endpoint) -> str:
    return f"{endpoint.surface}.{endpoint.operation}"


def add_unique(values: list[str], value: str) -> None:
    if value not in values:
        values.append(value)


def json_default(value: Any) -> Any:
    if isinstance(value, enum.Enum):
        raise TypeError("enum defaults use sentinels")
    if value is Ellipsis:
        raise TypeError("ellipsis defaults use sentinels")
    if isinstance(value, tuple):
        return [json_default(item) for item in value]
    if isinstance(value, list):
        return [json_default(item) for item in value]
    if isinstance(value, dict):
        return {
            str(key): json_default(item) for key, item in value.items()
        }
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    raise TypeError(f"non-JSON default {value!r}")


def default_value_type(value: Any) -> str:
    if value is None:
        return "null"
    if type(value) is bool:
        return "boolean"
    if type(value) is int:
        return "integer"
    if type(value) is float:
        return "number"
    if type(value) is str:
        return "string"
    if isinstance(value, (bytes, bytearray, memoryview)):
        return "bytes"
    if isinstance(value, (tuple, list)):
        return "sequence"
    if isinstance(value, dict):
        return "mapping"
    if isinstance(value, enum.Enum):
        return "enum"
    return "any_json"


def annotation_value_types(annotation: Any, parameter_name: str) -> list[str]:
    text = "" if annotation is inspect.Parameter.empty else str(annotation)
    lower = text.lower()
    types: list[str] = []
    is_outline = "_outline" in lower or re.search(r"\boutline\b", lower)

    def include(value_type: str) -> None:
        if value_type in ALLOWED_VALUE_TYPES:
            add_unique(types, value_type)

    if re.search(r"\bnone\b|nonetype|null", lower):
        include("null")
    if re.search(r"\bbool\b", lower):
        include("boolean")
    if re.search(r"\bint\b", lower):
        include("integer")
    if re.search(r"\bfloat\b|\bdouble\b|\bnumber\b", lower):
        include("number")
    if re.search(r"\bstr\b|anystr|imagetext|literal\[", lower):
        include("string")
    if re.search(r"\bbytes\b|bytearray|memoryview", lower):
        include("bytes")
    if "path" in lower:
        include("path")
    if re.search(r"\bio\[|binaryio|textio|file", lower):
        include("stream")
    if not is_outline and re.search(r"\bimage\b|imagefile|imagingcore", lower):
        include("image")
    if is_outline:
        include("handle")
    if re.search(r"\bfont\b|freetypefont|transposedfont", lower):
        include("font")
    if re.search(r"\bsequence\b|\blist\b|\btuple\b|\bcoords\b", lower):
        include("sequence")
    if re.search(r"\bmapping\b|\bdict\b", lower):
        include("mapping")
    if re.search(r"arrayinterface|numpy|protocol|typed", lower):
        include("record")
    if re.search(r"callable|handler|deformer", lower):
        include("handle")
        # A callable cannot be serialized as a JSON literal.  Active input
        # workflows represent this public value with a fixed named builtin
        # asset, so keep the source-neutral descriptor inside the fixed
        # manifest vocabulary as well as the public handle type.
        include("any_json")
    if "_ink" in lower:
        # Pillow's public ``_Ink`` annotation omits numeric colors, but the
        # ImageDraw runtime accepts real numbers for F-mode canvases and
        # rejects them mode-specifically elsewhere. Keep that input family in
        # the manifest so the input-only parity lane can exercise both paths.
        include("number")
        include("integer")
        include("string")
        include("sequence")
    if re.search(
        r"resampling|transpose|dither|palette|transform|layout|enum", lower
    ):
        include("enum")
    if parameter_name.lower() == "orientation":
        # Pillow's TransposedFont orientation is an Image.Transpose integer
        # enum (or None); callers pass ints such as 2 (ROTATE_90).
        include("integer")
        include("enum")
    if parameter_name.lower() == "dither":
        # Pillow exposes Dither as an IntEnum.  The Python binding accepts the
        # enum's integer value, while a string spelling is rejected before the
        # core conversion path.  Keep both the source enum and its public
        # integer representation in the input contract. A sequence remains in
        # scope so invalid host types exercise the public TypeError path.
        include("integer")
        include("enum")
        include("sequence")
    if re.search(r"\bany\b|\bobject\b", lower):
        include("any_json")

    name = parameter_name.lower()
    if name in {"fp", "filename"}:
        include("path")
        include("stream")
    if not types:
        if name in {"self", "cls"}:
            include("handle")
        elif name in {
            "image",
            "im",
            "im1",
            "im2",
            "image1",
            "image2",
            "mask",
            "bitmap",
            "palette",
        }:
            include("image")
        elif name in {"font"}:
            include("font")
            include("path")
            include("bytes")
            include("stream")
        elif name in {"fp", "filename"}:
            include("path")
            include("stream")
        elif name in {"text", "mode", "format", "encoding", "name"}:
            include("string")
        elif name in {
            "size",
            "xy",
            "box",
            "center",
            "translate",
            "features",
            "axes",
            "bands",
            "data",
        }:
            include("sequence")
        else:
            include("any_json")
    return types


def omission_for(parameter: inspect.Parameter) -> dict[str, Any]:
    if parameter.kind is inspect.Parameter.VAR_POSITIONAL:
        return {"kind": "literal", "value": []}
    if parameter.kind is inspect.Parameter.VAR_KEYWORD:
        return {"kind": "literal", "value": {}}
    if parameter.default is inspect.Parameter.empty:
        return {"kind": "required"}
    try:
        return {"kind": "literal", "value": json_default(parameter.default)}
    except TypeError:
        return {
            "kind": "sentinel",
            "name": getattr(parameter.default, "name", repr(parameter.default)),
            "semantics": (
                f"Use the public runtime default for {parameter.name} exactly."
            ),
        }


def parameter_style(
    parameter: inspect.Parameter,
    *,
    is_method: bool,
    position: int,
) -> str:
    if is_method and position == 0 and parameter.name in {"self", "cls"}:
        return "receiver"
    return {
        inspect.Parameter.POSITIONAL_ONLY: "positional",
        inspect.Parameter.POSITIONAL_OR_KEYWORD: "positional_or_keyword",
        inspect.Parameter.KEYWORD_ONLY: "keyword",
        inspect.Parameter.VAR_POSITIONAL: "variadic_positional",
        inspect.Parameter.VAR_KEYWORD: "variadic_keyword",
    }[parameter.kind]


def parameters_from_signature(
    signature: inspect.Signature,
    *,
    kind: str,
) -> list[dict[str, Any]]:
    parameters: list[dict[str, Any]] = []
    is_method = kind in {"method", "property_get", "property_set"}
    for position, parameter in enumerate(signature.parameters.values()):
        if parameter.kind is inspect.Parameter.VAR_POSITIONAL:
            value_types = ["sequence"]
        elif parameter.kind is inspect.Parameter.VAR_KEYWORD:
            value_types = ["mapping"]
        else:
            value_types = annotation_value_types(
                parameter.annotation, parameter.name
            )
        if parameter.default is not inspect.Parameter.empty:
            add_unique(value_types, default_value_type(parameter.default))
        parameters.append(
            {
                "id": parameter.name,
                "style": parameter_style(
                    parameter, is_method=is_method, position=position
                ),
                "value_types": value_types,
                "omission": omission_for(parameter),
            }
        )
    return parameters


def dynamic_property_signature(endpoint: Endpoint) -> inspect.Signature:
    receiver = inspect.Parameter(
        "self", inspect.Parameter.POSITIONAL_ONLY
    )
    return inspect.Signature(parameters=[receiver])


def import_path(path: str) -> Any:
    parts = path.split(".")
    for length in range(len(parts), 0, -1):
        module_name = ".".join(parts[:length])
        try:
            value: Any = importlib.import_module(module_name)
        except ModuleNotFoundError as error:
            if not isinstance(error.name, str) or not module_name.startswith(
                error.name
            ):
                raise
            continue
        for part in parts[length:]:
            value = getattr(value, part)
        return value
    raise ModuleNotFoundError(path)


def source_object(endpoint: Endpoint) -> Any | None:
    try:
        return import_path(endpoint.source_path)
    except AttributeError:
        if endpoint.kind == "property_get":
            parent = endpoint.source_path.rsplit(".", 1)[0]
            import_path(parent)
            return None
        raise


def target_candidates(endpoint: Endpoint) -> list[str]:
    surface = endpoint.surface
    operation = endpoint.operation
    if surface == "PIL.Image":
        return [f"pillow_rs.{operation}", f"pillow_rs.Image.{operation}"]
    if surface == "PIL.Image.Image":
        return [f"pillow_rs.Image.{operation}"]
    if surface == "PIL.ImageDraw":
        return [f"pillow_rs.ImageDraw.{operation}"]
    if surface == "PIL.ImageDraw.ImageDraw":
        return [f"pillow_rs.ImageDraw.Draw.{operation}"]
    if surface == "PIL.ImagePalette":
        return [f"pillow_rs.ImagePalette.{operation}"]
    if surface == "PIL.ImagePalette.ImagePalette":
        return [f"pillow_rs.ImagePalette.ImagePalette.{operation}"]
    if surface.startswith("PIL.ImageEnhance."):
        class_name = surface.rsplit(".", 1)[1]
        return [f"pillow_rs.ImageEnhance.{class_name}.{operation}"]
    if surface == "PIL.ImageEnhance":
        return [f"pillow_rs.ImageEnhance.{operation}"]
    if surface == "PIL.ImageFont":
        return [f"pillow_rs.ImageFont.{operation}"]
    if surface.startswith("PIL.ImageFont."):
        class_name = surface.rsplit(".", 1)[1]
        return [f"pillow_rs.ImageFont.{class_name}.{operation}"]
    if surface.startswith("PIL.ImageFilter."):
        class_name = surface.rsplit(".", 1)[1]
        return [f"pillow_rs.imagefilter.{class_name}.{operation}"]
    if surface == "PIL.ImageStat":
        return [f"pillow_rs.ImageStat.{operation}"]
    if surface == "PIL.ImageStat.Stat":
        return [f"pillow_rs.ImageStat.Stat.{operation}"]
    module = surface.removeprefix("PIL.")
    return [f"pillow_rs.{module}.{operation}"]


def resolve_target(endpoint: Endpoint) -> tuple[str, Any | None]:
    errors: list[str] = []
    for path in target_candidates(endpoint):
        try:
            return path, import_path(path)
        except AttributeError as error:
            errors.append(f"{path}: {error}")
            if endpoint.kind == "property_get":
                parent = path.rsplit(".", 1)[0]
                try:
                    import_path(parent)
                except AttributeError:
                    continue
                return path, None
    raise ValueError(
        f"{endpoint.id}: no public target binding: {'; '.join(errors)}"
    )


def callable_signature(
    endpoint: Endpoint,
    value: Any | None,
) -> inspect.Signature:
    if endpoint.kind in {"property_get", "constant"}:
        if endpoint.kind == "constant":
            return inspect.Signature()
        return dynamic_property_signature(endpoint)
    if value is None or not callable(value):
        raise ValueError(f"{endpoint.id}: expected callable public endpoint")
    try:
        return inspect.signature(value)
    except ValueError as error:
        if endpoint.source_path == "PIL.ImageDraw.Outline":
            return inspect.Signature()
        raise ValueError(
            f"{endpoint.id}: public signature is not inspectable"
        ) from error


def signature_text(
    endpoint: Endpoint,
    signature: inspect.Signature,
) -> str:
    return f"{endpoint.operation}{signature}"


def return_annotation_text(signature: inspect.Signature) -> str:
    annotation = signature.return_annotation
    if annotation is inspect.Signature.empty:
        return ""
    return str(annotation).lower()


def result_shape(
    endpoint: Endpoint,
    signature: inspect.Signature,
) -> tuple[str, list[str]]:
    if endpoint.kind == "constant":
        return "scalar", ["integer"]
    if endpoint.kind == "property_get":
        if endpoint.operation in {"size", "extrema", "count", "sum", "sum2",
                                  "mean", "median", "rms", "var", "stddev"}:
            return "sequence", ["sequence"]
        if endpoint.operation in {"info"}:
            return "mapping", ["mapping"]
        if endpoint.operation in {"format"}:
            return "scalar", ["string", "null"]
        if endpoint.operation in {"mode"}:
            return "scalar", ["string"]
        return "scalar", ["integer"]
    if endpoint.kind == "type":
        return "handle", ["handle"]

    if endpoint.source_path == "PIL.ImageSequence.Iterator.__iter__":
        # Pillow annotates this protocol method with the iterator class name;
        # the parity value is still a public handle, not a sequence to drain.
        return "handle", ["handle"]

    annotation = return_annotation_text(signature)
    if "imagingcore" in annotation:
        return "mask", ["record"]
    if re.search(r"\bimage\b|imagefile", annotation):
        return "image", ["image", "null"] if re.search(r"\bnone\b", annotation) else ["image"]
    if re.search(r"\bbytes\b|bytearray|memoryview", annotation):
        return "bytes", ["bytes", "null"] if re.search(r"\bnone\b", annotation) else ["bytes"]
    if re.search(r"\biterator\b", annotation):
        return "iterator", ["sequence"]
    if re.search(r"\bdict\b|\bmapping\b", annotation):
        return "mapping", ["mapping"]
    if re.search(r"\bnone\b", annotation):
        nullable_types = ["null"]
        if re.search(r"\btuple\b|\blist\b|\bsequence\b", annotation):
            nullable_types.append("sequence")
        if re.search(r"\bfloat\b", annotation):
            nullable_types.append("number")
        if re.search(r"\bint\b", annotation):
            nullable_types.append("integer")
        if "pixelaccess" in annotation:
            return "handle", ["handle", "null"]
        if len(nullable_types) > 1:
            return "value", nullable_types
        return "none", ["null"]
    if re.search(r"\btuple\b|\blist\b|\bsequence\b", annotation):
        return "sequence", ["sequence"]
    if re.search(r"\bfloat\b", annotation):
        return "scalar", ["number"]
    if re.search(r"\bint\b", annotation):
        return "scalar", ["integer"]
    if re.search(r"\bbool\b", annotation):
        return "scalar", ["boolean"]
    if re.search(r"\bstr\b", annotation):
        return "scalar", ["string"]
    if any(
        token in annotation
        for token in ("font", "pixelaccess", "capsule", "outline")
    ):
        return "handle", ["handle"]

    mutators = {
        "apply_transparency",
        "close",
        "frombytes",
        "paste",
        "putalpha",
        "putdata",
        "putpalette",
        "putpixel",
        "save",
        "seek",
        "set_variation_by_axes",
        "set_variation_by_name",
        "thumbnail",
        "verify",
    }
    if endpoint.operation in mutators:
        return "none", ["null"]
    return "record", ["record"]


def comparison_for(shape: str, value_types: list[str]) -> dict[str, Any]:
    if shape == "image":
        return {
            "kind": "image",
            "pixel_mode": "exact",
            "maximum_channel_delta": 0,
            "metadata_mode": "exact",
            "reason": None,
        }
    if shape in {"bytes", "encoded_file"}:
        return {"kind": "bytes"}
    if shape in {"sequence", "iterator"}:
        return {"kind": "ordered"}
    if value_types in (["number"], ["integer"]):
        return {
            "kind": "numeric",
            "absolute_tolerance": 0,
            "relative_tolerance": 0,
            "nan_policy": "forbidden",
        }
    return {"kind": "exact"}


def result_contract(
    endpoint: Endpoint,
    signature: inspect.Signature,
) -> dict[str, Any]:
    shape, value_types = result_shape(endpoint, signature)
    path = "value.type" if shape == "handle" else "value"
    observed_types = (
        ["string", "null"] if "null" in value_types else ["string"]
    ) if shape == "handle" else value_types
    return {
        "shape": shape,
        "observations": [
            {
                "path": path,
                "value_types": observed_types,
                "comparison": comparison_for(shape, observed_types),
            }
        ],
        "error": {
            "fields": ["class", "kind", "message", "stage", "code"],
            "message": {
                "mode": "exact",
                "transforms": [],
                "reason": None,
            },
        },
    }


def authority_row_map() -> dict[str, dict[str, Any]]:
    authority = load_authority()
    rows: dict[str, dict[str, Any]] = {}
    for module, module_data in authority["modules"].items():
        for section in ("class_methods", "methods", "properties", "functions"):
            for entry in module_data.get(section, []):
                name = entry["name"]
                rows[LegacyRef(module, section, None, name).id] = dict(entry)
        for class_entry in module_data.get("classes", []):
            class_name = class_entry["name"]
            rows[
                LegacyRef(module, "classes", None, class_name).id
            ] = dict(class_entry)
            inherited = {
                key: value
                for key, value in class_entry.items()
                if key
                in {
                    "status",
                    "supported_targets",
                    "supported_modes",
                    "supported_formats",
                }
            }
            for section in (
                "class_methods",
                "methods",
                "properties",
                "functions",
            ):
                for entry in class_entry.get(section, []):
                    data = {"name": entry} if isinstance(entry, str) else dict(entry)
                    merged = {**inherited, **data}
                    rows[
                        LegacyRef(
                            module, section, class_name, merged["name"]
                        ).id
                    ] = merged
    return rows


def merged_legacy_values(
    endpoint: Endpoint,
    row_map: dict[str, dict[str, Any]],
    field: str,
) -> list[Any]:
    result: list[Any] = []
    for reference in endpoint.legacy_refs:
        value = row_map[reference.id].get(field, [])
        values = value if isinstance(value, list) else [value]
        for item in values:
            if item not in result:
                result.append(item)
    return result


def requirement(
    requirement_id: str,
    dimension: str,
    description: str,
    lanes: list[str],
) -> dict[str, Any]:
    return {
        "id": requirement_id,
        "dimension": dimension,
        "description": description,
        "lanes": lanes,
        "target_profiles": [TARGET_PROFILE],
    }


def looks_like_error(edge: str) -> bool:
    return any(
        token in edge.lower()
        for token in (
            "corrupt",
            "empty",
            "error",
            "invalid",
            "missing",
            "negative",
            "read_only",
            "truncated",
            "unsupported",
            "zero",
        )
    )


def operation_requirements(
    endpoint: Endpoint,
    parameters: list[dict[str, Any]],
    row_map: dict[str, dict[str, Any]],
    *,
    benchmark_applicable: bool,
) -> list[dict[str, Any]]:
    prefix = requirement_prefix(endpoint)
    requirements = [
        requirement(
            f"{prefix}.behavior.default",
            "success_path",
            f"Default public behavior of {endpoint.source_path}.",
            ["parity", "coverage"],
        )
    ]
    for parameter in parameters:
        if parameter["style"] == "receiver":
            continue
        requirements.append(
            requirement(
                f"{prefix}.parameter.{slug(parameter['id'])}",
                "parameter",
                (
                    f"Public parameter {parameter['id']} with its declared "
                    "type union and omission semantics."
                ),
                ["parity", "coverage"],
            )
        )
    for mode in merged_legacy_values(
        endpoint, row_map, "supported_modes"
    ):
        requirements.append(
            requirement(
                f"{prefix}.mode.{slug(str(mode))}",
                "mode",
                f"Public behavior for image mode {mode!r}.",
                ["parity", "coverage"],
            )
        )
    for image_format in merged_legacy_values(
        endpoint, row_map, "supported_formats"
    ):
        requirements.append(
            requirement(
                f"{prefix}.format.{slug(str(image_format))}",
                "format",
                f"Public behavior for image format {image_format!r}.",
                ["parity", "coverage"],
            )
        )
    for index, variant in enumerate(
        merged_legacy_values(endpoint, row_map, "param_variants"), start=1
    ):
        requirements.append(
            requirement(
                f"{prefix}.parameter-combination.legacy-{index:03d}",
                "parameter_combination",
                (
                    "Deprecated-authority parameter combination: "
                    f"{json.dumps(variant, sort_keys=True, separators=(',', ':'))}"
                ),
                ["parity", "coverage"],
            )
        )
    for edge in merged_legacy_values(endpoint, row_map, "edge_cases"):
        requirements.append(
            requirement(
                f"{prefix}.edge.{slug(str(edge))}",
                "error_path" if looks_like_error(str(edge)) else "boundary",
                f"Deprecated-authority edge case {edge!r}.",
                ["parity", "coverage"],
            )
        )
    if benchmark_applicable:
        requirements.append(
            requirement(
                f"{prefix}.performance.standard",
                "performance",
                f"Deterministic standard workload for {endpoint.source_path}.",
                ["parity", "benchmark"],
            )
        )
    ids = [item["id"] for item in requirements]
    if len(ids) != len(set(ids)):
        raise ValueError(f"{endpoint.id}: duplicate requirement IDs")
    return requirements


def component_for(endpoint: Endpoint) -> str:
    surface = endpoint.surface
    if surface.startswith("PIL.ImageDraw"):
        return "image-draw"
    if surface.startswith("PIL.ImageFilter"):
        return "image-filter"
    if surface.startswith("PIL.ImageEnhance"):
        return "image-enhance"
    if surface.startswith("PIL.ImageOps"):
        return "image-ops"
    if surface.startswith("PIL.ImageChops"):
        return "image-chops"
    if surface.startswith("PIL.ImageColor"):
        return "image-color"
    if surface.startswith("PIL.ImagePalette"):
        return "image-palette"
    if surface.startswith("PIL.ImageFont"):
        return "image-font"
    if surface.startswith("PIL.ImageStat"):
        return "image-stat"
    if surface.startswith("PIL.ImageSequence"):
        return "image-sequence"
    return "image-core"


def surface_kind(surface: str) -> str:
    type_surfaces = {
        "PIL.Image.Image",
        "PIL.ImageDraw.ImageDraw",
        "PIL.ImageEnhance.Brightness",
        "PIL.ImageEnhance.Color",
        "PIL.ImageEnhance.Contrast",
        "PIL.ImageEnhance.Sharpness",
        "PIL.ImageFont.FreeTypeFont",
        "PIL.ImageFont.ImageFont",
        "PIL.ImageFont.TransposedFont",
        "PIL.ImagePalette.ImagePalette",
        "PIL.ImageSequence.Iterator",
        "PIL.ImageStat.Stat",
    }
    return "type" if surface in type_surfaces else "namespace"


def operation_contract(
    endpoint: Endpoint,
    row_map: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    source = source_object(endpoint)
    source_signature = callable_signature(endpoint, source)
    parameters = parameters_from_signature(
        source_signature, kind=endpoint.kind
    )
    if endpoint.source_path == "PIL.ImageSequence.Iterator":
        # Pillow's runtime contract deliberately validates the seek protocol
        # instead of relying on the Image.Image annotation. Keep null in the
        # fixed input vocabulary so the public AttributeError path can be
        # exercised without inventing a target-only test object.
        for parameter in parameters:
            if parameter["id"] == "im":
                add_unique(parameter["value_types"], "null")
    if endpoint.surface == "PIL.ImageDraw.ImageDraw":
        # ImageDraw receives arbitrary Python objects at these boundaries and
        # reports public TypeError/ValueError diagnostics for malformed
        # coordinates. Keep the broad input vocabulary so the Rust-owned
        # normalizers' Invalid variants can be exercised by input-only parity.
        for parameter in parameters:
            if parameter["id"] == "xy":
                add_unique(parameter["value_types"], "any_json")
    if endpoint.source_path == "PIL.Image.Image.convert":
        # Pillow validates the public mode argument before conversion. Keep an
        # integer in this endpoint's input vocabulary so that Rust owns the
        # invalid host-type diagnostic without broadening other mode APIs.
        for parameter in parameters:
            if parameter["id"] == "mode":
                add_unique(parameter["value_types"], "integer")
            if parameter["id"] == "palette":
                # Pillow accepts an explicit None at runtime even though the
                # annotation documents Palette.WEB as the default sentinel.
                add_unique(parameter["value_types"], "null")
    if endpoint.source_path == "PIL.Image.Image.reduce":
        # Pillow's factor conversion reports the concrete host type for
        # invalid values. Keep a string in this endpoint's vocabulary so the
        # Rust core owns that public diagnostic without broadening other
        # integer parameters.
        for parameter in parameters:
            if parameter["id"] == "factor":
                add_unique(parameter["value_types"], "string")
    if endpoint.source_path == "PIL.Image.Image.getchannel":
        # The documented selector is int | str, but Pillow also exposes a
        # stable TypeError for other host values. Keep JSON values available
        # only for this negative parity lane so the Rust core owns that
        # conversion diagnostic without changing the public signature.
        for parameter in parameters:
            if parameter["id"] == "channel":
                add_unique(parameter["value_types"], "any_json")
    if endpoint.source_path == "PIL.Image.Image.putalpha":
        # The documented alpha value is Image | int, but the public binding
        # still receives arbitrary Python objects and reports a stable
        # TypeError for invalid host values. Keep a scalar string available
        # only for this negative parity lane so Rust owns that diagnostic
        # without broadening unrelated alpha parameters.
        for parameter in parameters:
            if parameter["id"] == "alpha":
                add_unique(parameter["value_types"], "string")
    if endpoint.source_path == "PIL.Image.Image.transform":
        # The transform implementation accepts a broad runtime sequence and
        # reports invalid host values from its native conversion path. Keep
        # the scalar and mapping edge values available only for this endpoint.
        for parameter in parameters:
            if parameter["id"] == "data":
                add_unique(parameter["value_types"], "integer")
            if parameter["id"] == "fillcolor":
                add_unique(parameter["value_types"], "any_json")
    benchmark_applicable = endpoint.kind != "constant"
    requirements = operation_requirements(
        endpoint,
        parameters,
        row_map,
        benchmark_applicable=benchmark_applicable,
    )
    coverage_exclusion_reason = COVERAGE_NOT_APPLICABLE_ENDPOINTS.get(endpoint.id)
    target_path, target = resolve_target(endpoint)
    if (
        target is not None
        and not callable(target)
        and endpoint.kind not in {"constant", "property_get"}
    ):
        target_signature_text = (
            f"{endpoint.operation}: {type(target).__name__} public value"
        )
        target_support = {
            "status": "partial",
            "reason": (
                "The target exposes a selector value rather than the source "
                "callable public type; filter application is observable but "
                "direct type behavior is not equivalent."
            ),
            "missing_requirements": [
                item["id"]
                for item in requirements
                if "parity" in item["lanes"]
            ],
        }
    else:
        target_signature = callable_signature(endpoint, target)
        target_signature_text = signature_text(endpoint, target_signature)
        target_support = {"status": "supported"}
    contract: dict[str, Any] = {
        "id": endpoint.operation,
        "kind": endpoint.kind,
        "classification": endpoint.classification,
        "lifecycle": {"status": "current"},
        "source": {
            "oracle_id": ORACLE_ID,
            "path": endpoint.source_path,
            "signature": signature_text(endpoint, source_signature),
            "parameters": parameters,
            "result": result_contract(endpoint, source_signature),
        },
        "targets": [
            {
                "target_id": TARGET_ID,
                "path": target_path,
                "signature": target_signature_text,
                "support": target_support,
            }
        ],
        "requirements": requirements,
        "parity": {
            "applicability": "required",
            "target_profiles": [TARGET_PROFILE],
        },
        "coverage": (
            {
                "applicability": "not_applicable",
                "reason": coverage_exclusion_reason,
            }
            if coverage_exclusion_reason is not None
            else {
                "applicability": "required",
                "target_profiles": [TARGET_PROFILE],
                "component_ids": [component_for(endpoint)],
            }
        ),
    }
    if benchmark_applicable:
        contract["benchmark"] = {
            "applicability": "required",
            "target_profiles": [TARGET_PROFILE],
            "metrics": ["latency", "throughput"],
        }
    else:
        contract["benchmark"] = {
            "applicability": "not_applicable",
            "reason": (
                "Reading a process-wide constant has no meaningful isolated "
                "runtime workload."
            ),
        }
    return contract


def coverage_components() -> list[dict[str, Any]]:
    components: list[dict[str, Any]] = []
    for component_id, paths in COMPONENTS.items():
        missing = [
            path for path in paths if not (WORKSPACE_ROOT / path).is_file()
        ]
        if missing:
            raise ValueError(
                f"{component_id}: coverage paths do not exist: {missing}"
            )
        components.append(
            {
                "id": component_id,
                "target_profile": TARGET_PROFILE,
                "paths": list(paths),
                "dimensions": ["function", "line", "branch", "region"],
                "thresholds": [
                    {"dimension": dimension, "minimum_percent": 100}
                    for dimension in ("function", "line", "branch", "region")
                ],
            }
        )
    return components


def command(
    command_id: str,
    make_target: str,
    timeout_seconds: int,
) -> dict[str, Any]:
    return {
        "id": command_id,
        "argv": ["make", make_target],
        "cwd": ".",
        "timeout_seconds": timeout_seconds,
    }


def build_manifest() -> dict[str, Any]:
    if str(PYTHON_FACADE_ROOT) not in sys.path:
        sys.path.insert(0, str(PYTHON_FACADE_ROOT))
    import PIL
    import pillow_rs

    if PIL.__version__ != EXPECTED_PILLOW_VERSION:
        raise ValueError(
            f"manifest build requires Pillow {EXPECTED_PILLOW_VERSION}, "
            f"observed {PIL.__version__}"
        )
    if not isinstance(pillow_rs.__version__, str):
        raise ValueError("pillow_rs public identity has no string version")

    endpoints, _ = derive_inventory()
    row_map = authority_row_map()
    grouped: dict[str, list[Endpoint]] = defaultdict(list)
    for endpoint in endpoints:
        grouped[endpoint.surface].append(endpoint)

    surfaces = []
    input_index = {"parity": [], "coverage": [], "benchmark": []}
    for surface, surface_endpoints in sorted(grouped.items()):
        storage_slug = slug(surface)
        for lane in input_index:
            input_index[lane].append(
                f"inputs/{lane}/{storage_slug}.json"
            )
        surfaces.append(
            {
                "id": surface,
                "kind": surface_kind(surface),
                "source_path": surface,
                "storage_slug": storage_slug,
                "operations": [
                    operation_contract(endpoint, row_map)
                    for endpoint in sorted(
                        surface_endpoints,
                        key=lambda item: item.operation,
                    )
                ],
            }
        )

    return {
        "schema": "migration-parity/manifest@2",
        "scope": {
            "id": "pillow-rs-selected-public-contract",
            "mode": "full",
            "inventory": {
                "authority": "scripts/migration_parity_inventory.py",
                "revision": (
                    f"sha256:{EXPECTED_AUTHORITY_SHA256}"
                    "+inventory-rule@1"
                ),
                "command_id": "inventory",
            },
        },
        "oracles": [
            {
                "id": ORACLE_ID,
                "name": "Pillow",
                "version": EXPECTED_PILLOW_VERSION,
                "runtime": "CPython 3.12",
                "identity_command_id": "oracle-identity",
                "contract": "Public observable PIL behavior",
                "components": [
                    {"id": "freetype", "name": "FreeType", "version": "2.14.3"},
                    {"id": "libjpeg", "name": "libjpeg-turbo", "version": "6.2"},
                    {"id": "openjpeg", "name": "OpenJPEG", "version": "2.5.4"},
                    {"id": "zlib", "name": "zlib-ng", "version": "1.3.1.zlib-ng"},
                    {"id": "libtiff", "name": "libtiff", "version": "4.7.1"},
                    {"id": "libwebp", "name": "libwebp", "version": "1.6.0"},
                ],
            }
        ],
        "targets": [
            {
                "id": TARGET_ID,
                "name": "pillow-rs",
                "runtime": "CPython public pillow_rs facade backed by Rust",
                "identity_command_id": "target-identity",
                "contract": (
                    "Public Pillow-compatible image and font behavior through "
                    "the pillow_rs package"
                ),
            }
        ],
        "target_profiles": [
            {
                "id": f"python-{backend}",
                "target_id": TARGET_ID,
                "backend": backend,
                "features": ["all-features"],
            }
            for backend in BENCHMARK_BACKENDS
        ],
        "commands": [
            command("inventory", "migration-parity-inventory", 60),
            command("oracle-identity", "migration-parity-oracle-identity", 60),
            command("target-identity", "migration-parity-target-identity", 300),
            command("parity", "migration-parity-test", 3600),
            command("coverage", "migration-parity-coverage", 3600),
            command("coverage-rust", "migration-parity-coverage-rust", 7200),
            command(
                "coverage-font-native",
                "migration-parity-font-native-coverage",
                600,
            ),
            command(
                "coverage-imageops-native",
                "migration-parity-imageops-native-coverage",
                600,
            ),
            command(
                "coverage-imagesequence-native",
                "migration-parity-imagesequence-native-coverage",
                600,
            ),
            command(
                "coverage-imagecore-native",
                "migration-parity-imagecore-native-coverage",
                600,
            ),
            command(
                "coverage-imagedraw-native",
                "migration-parity-imagedraw-native-coverage",
                600,
            ),
            command(
                "coverage-imagecolor-native",
                "migration-parity-imagecolor-native-coverage",
                600,
            ),
            command(
                "coverage-imagepalette-native",
                "migration-parity-imagepalette-native-coverage",
                600,
            ),
            command("benchmark", "migration-parity-benchmark", 7200),
            command("aggregate", "migration-parity-aggregate", 300),
            command("docs", "migration-parity-docs", 300),
            command("drift", "migration-parity-drift-check", 300),
        ],
        "interfaces": {
            "parity": {
                "input_schema": "migration-parity/parity-input@1",
                "result_schema": "migration-parity/parity-result@1",
                "command_id": "parity",
            },
            "coverage": {
                "input_schema": "migration-parity/coverage-input@1",
                "result_schema": "migration-parity/coverage-result@1",
                "command_id": "coverage",
            },
            "benchmark": {
                "input_schema": "migration-parity/benchmark-input@1",
                "result_schema": "migration-parity/benchmark-result@1",
                "command_id": "benchmark",
            },
            "aggregation": {
                "input_schemas": [
                    "migration-parity/parity-result@1",
                    "migration-parity/coverage-result@1",
                    "migration-parity/benchmark-result@1",
                ],
                "result_schema": "migration-parity/status-report@1",
                "command_id": "aggregate",
            },
        },
        "input_index": input_index,
        "coverage_components": coverage_components(),
        "surfaces": surfaces,
        "documentation": {
            "command_id": "docs",
            "specification_outputs": [
                "docs/generated/migration-parity-public-contract.md"
            ],
            "evidence_outputs": [
                "docs/generated/migration-parity-status.md",
                "docs/generated/migration-coverage-status.md",
                "docs/generated/migration-benchmark-status.md",
            ],
        },
    }


class LiteralDumper(yaml.SafeDumper):
    pass


def dump_manifest(manifest: dict[str, Any]) -> str:
    return yaml.dump(
        manifest,
        Dumper=LiteralDumper,
        sort_keys=False,
        allow_unicode=True,
        width=100,
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--output",
        type=Path,
        default=DEFAULT_OUTPUT,
        help="Manifest output path.",
    )
    args = parser.parse_args()
    manifest = build_manifest()
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(dump_manifest(manifest), encoding="utf-8")


if __name__ == "__main__":
    main()
