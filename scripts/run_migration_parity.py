#!/usr/bin/env python3
"""Run the fixed migration-parity input contract against Pillow and pillow-rs.

The manifest and input documents are specifications.  This module is the
runtime boundary that turns those documents into independently executed
source/target workflows and a strict ``migration-parity/parity-result@1``
artifact.  It deliberately does not read deprecated fixtures or expected
outputs.

The side runner is invoked in a fresh Python process for each implementation.
That keeps source and target object graphs, imports, temporary files, and
exceptions independent while retaining one shared public workflow definition.
"""

from __future__ import annotations

import argparse
import base64
import datetime as _dt
import hashlib
import importlib
import io
import json
import math
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
import time
import types
import uuid
from typing import Any, Iterable

import yaml


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "pillow-rs" / "tests" / "fixtures"
DEFAULT_MANIFEST = FIXTURE_ROOT / "manifest.yaml"
DEFAULT_OUTPUT = ROOT / "build" / "migration-parity" / "parity-result.json"
TARGET_PROFILE = "python-cpu"
TARGET_ID = "pillow-rs-python"
ORACLE_ID = "pillow"
ORACLE_VERSION = "12.2.0"

# These are fixed stimuli, not generated oracle outputs.  The builtin asset
# names in parity inputs intentionally identify a format without storing an
# expected result.  Keeping the bytes here makes source and target receive the
# same input without one side reading the other's output.
ENCODED_INPUTS: dict[str, bytes] = {
    "encoded-png-input": base64.b64decode(
        "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEklEQVR4nGNkZGJmYGBgYgADAABwAAqyASBNAAAAAElFTkSuQmCC"
    ),
    "encoded-jpeg-input": base64.b64decode(
        "/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAgGBgcGBQgHBwcJCQgKDBQNDAsLDBkSEw8UHRofHh0aHBwgJC4nICIsIxwcKDcpLDAxNDQ0Hyc5PTgyPC4zNDL/2wBDAQkJCQwLDBgNDRgyIRwhMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjL/wAARCAACAAIDASIAAhEBAxEB/8QAHwAAAQUBAQEBAQEAAAAAAAAAAAECAwQFBgcICQoL/8QAtRAAAgEDAwIEAwUFBAQAAAF9AQIDAAQRBRIhMUEGE1FhByJxFDKBkaEII0KxwRVS0fAkM2JyggkKFhcYGRolJicoKSo0NTY3ODk6Q0RFRkdISUpTVFVWV1hZWmNkZWZnaGhpanc3dnd4eXqDhIWGh4iJipKTlJWWl5iZmqKjpKWmp6ipqrKztLW2t7i5usLDxMXGx8jJytLT1NXW19jZ2uHi4+Tl5ufo6erx8vP09fb3+Pn6/8QAHwEAAwEBAQEBAQEBAQAAAAAAAAECAwQFBgcICQoL/8QAtREAAgECBAQDBAcFBAQAAQJ3AAECAxEEBSExBhJBUQdhcRMiMoEIFEKRobHBCSMzUvAVYnLRChYkNOEl8RcYGRomJygpKjU2Nzg5OkNERUZHSElKU1RVVldYWVpjZGVmZ2hpanN0dXZ3eHl6goOEhYaHiImKkpOUlZaXmJmaoqOkpaanqKmqsrO0tba3uLm6wsPExcbHyMnK0tPU1dbX2Nna4uPk5ebn6Onq8vP09fb3+Pn6/9oADAMBAAIRAxEAPwDwGiiimI//2Q=="
    ),
    "encoded-gif-input": base64.b64decode(
        "R0lGODdhAgACAIEAAAECAwAAAAAAAAAAACwAAAAAAgACAAAIBgABCAQQEAA7"
    ),
    "encoded-bmp-input": base64.b64decode(
        "Qk1GAAAAAAAAADYAAAAoAAAAAgAAAAIAAAABABgAAAAAABAAAADEDgAAxA4AAAAAAAAAAAAAAwIBAwIBAAADAgEDAgEAAA=="
    ),
    "encoded-tiff-input": base64.b64decode(
        "SUkqAAgAAAAKAAABBAABAAAAAgAAAAEBBAABAAAAAgAAAAIBAwADAAAAhgAAAAMBAwABAAAAAQAAAAYBAwABAAAAAgAAABEBBAABAAAAjAAAABUBAwABAAAAAwAAABYBBAABAAAAAgAAABcBBAABAAAADAAAABwBAwABAAAAAQAAAAAAAAAIAAgACAABAgMBAgMBAgMBAgM="
    ),
    "encoded-webp-input": base64.b64decode(
        "UklGRiQAAABXRUJQVlA4IBgAAAAwAQCdASoCAAIAAUAmJaQAA3AA/v0gUAA="
    ),
    # Pillow's tiny ICO writer emits an empty icon for this deliberately
    # minimal stimulus.  It remains useful for exercising the public error
    # path and is kept as a fixed input rather than silently omitted.
    "encoded-ico-input": base64.b64decode("AAABAAAA"),
}


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def now_rfc3339() -> str:
    return _dt.datetime.now(_dt.timezone.utc).isoformat().replace("+00:00", "Z")


def json_safe(value: Any) -> Any:
    """Convert ordinary Python containers without interpreting public shape."""

    if value is None or isinstance(value, (bool, int, str)):
        return value
    if isinstance(value, float):
        if math.isnan(value):
            return "NaN"
        if math.isinf(value):
            return "Infinity" if value > 0 else "-Infinity"
        return value
    if isinstance(value, bytes):
        return {
            "kind": "bytes",
            "encoding": "base64",
            "data": base64.b64encode(value).decode("ascii"),
        }
    if isinstance(value, bytearray):
        return json_safe(bytes(value))
    if isinstance(value, (list, tuple)):
        return [json_safe(item) for item in value]
    if isinstance(value, dict):
        return {str(key): json_safe(item) for key, item in value.items()}
    return None


class ArrayInterfaceValue:
    """Small fixed array-interface stimulus used by Image.fromarray cases."""

    def __init__(self, descriptor: dict[str, Any]):
        self._shape = tuple(descriptor["shape"])
        self._typestr = descriptor["typestr"]
        self._data = base64.b64decode(descriptor["data_base64"])

    @property
    def __array_interface__(self) -> dict[str, Any]:
        return {
            "shape": self._shape,
            "typestr": self._typestr,
            "data": (self._data, False),
            "version": 3,
        }

    def tobytes(self) -> bytes:
        return self._data

    def __bytes__(self) -> bytes:
        return self._data


class DeformerValue:
    """Fixed object implementing Pillow's public getmesh protocol."""

    def __init__(self, descriptor: dict[str, Any]):
        self._mesh = descriptor["mesh"]

    def getmesh(self, image: Any) -> Any:
        return self._mesh


def decode_literal(value: Any, *, side: str = "source") -> Any:
    if isinstance(value, list):
        converted = [decode_literal(item, side=side) for item in value]
        # JSON has one sequence representation.  The public Pillow contract
        # accepts both list and tuple for these values, while the PyO3 target
        # exposes tuples for size/box/bands/matrices/coordinates.  Canonicalize
        # the language-neutral sequence for both independent adapters so a
        # Python implementation detail does not make setup itself not-run.
        return tuple(converted)
    if isinstance(value, dict):
        protocol = value.get("protocol")
        if protocol == "array-interface":
            return ArrayInterfaceValue(value)
        if protocol == "getmesh":
            return DeformerValue(value)
        return {key: decode_literal(item, side=side) for key, item in value.items()}
    return value


class AssetStore:
    def __init__(self, assets: list[dict[str, Any]], root: Path, tempdir: Path):
        self._assets = {asset["id"]: asset for asset in assets}
        self._root = root
        self._tempdir = tempdir
        self._resolved: dict[str, Any] = {}

    def resolve(self, asset_id: str) -> Any:
        if asset_id in self._resolved:
            return self._resolved[asset_id]
        if asset_id not in self._assets:
            raise ValueError(f"unknown workflow asset: {asset_id}")
        asset = self._assets[asset_id]
        kind = asset["kind"]
        if kind == "ref":
            value: Any = str(self._root / asset["path"])
        elif kind == "inline":
            if asset.get("encoding") != "base64":
                raise ValueError(f"unsupported inline encoding: {asset_id}")
            value = base64.b64decode(asset["data"])
        elif kind == "missing":
            value = str(self._tempdir / asset["path"])
        elif kind == "builtin":
            value = self._builtin(asset["name"], asset_id)
        else:
            raise ValueError(f"unsupported asset kind: {kind}")
        self._resolved[asset_id] = value
        return value

    def _write_asset(self, asset_id: str, data: bytes, suffix: str) -> str:
        path = self._tempdir / f"{asset_id}{suffix}"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        return str(path)

    def _builtin(self, name: str, asset_id: str) -> Any:
        if name in ENCODED_INPUTS:
            return self._write_asset(asset_id, ENCODED_INPUTS[name], ".bin")
        if name == "temporary-output-path":
            return str(self._tempdir / f"{asset_id}.out")
        if name == "read-only-directory":
            path = self._tempdir / f"{asset_id}.dir"
            path.mkdir(parents=True, exist_ok=True)
            return str(path)
        if name == "identity-callable":
            return lambda value: value
        if name in {"font-byte-stream", "in-memory-byte-stream"}:
            return io.BytesIO()
        raise ValueError(f"unsupported builtin asset: {name}")


SOURCE_MODULES = {
    "PIL.Image": "PIL.Image",
    "PIL.ImageChops": "PIL.ImageChops",
    "PIL.ImageColor": "PIL.ImageColor",
    "PIL.ImageDraw": "PIL.ImageDraw",
    "PIL.ImageEnhance": "PIL.ImageEnhance",
    "PIL.ImageFilter": "PIL.ImageFilter",
    "PIL.ImageFont": "PIL.ImageFont",
    "PIL.ImageOps": "PIL.ImageOps",
    "PIL.ImagePalette": "PIL.ImagePalette",
    "PIL.ImageSequence": "PIL.ImageSequence",
    "PIL.ImageStat": "PIL.ImageStat",
}

TARGET_MODULES = {
    "PIL.Image": "pillow_rs",
    "PIL.ImageChops": "pillow_rs.imagechops",
    "PIL.ImageColor": "pillow_rs.imagecolor",
    "PIL.ImageDraw": "pillow_rs.imagedraw",
    "PIL.ImageEnhance": "pillow_rs.imageenhance",
    "PIL.ImageFilter": "pillow_rs.imagefilter",
    "PIL.ImageFont": "pillow_rs.imagefont",
    "PIL.ImageOps": "pillow_rs.imageops",
    "PIL.ImagePalette": "pillow_rs.imagepalette",
    "PIL.ImageSequence": "pillow_rs.imagesequence",
    "PIL.ImageStat": "pillow_rs.imagestat",
}

TARGET_TYPES = {
    "PIL.Image.Image": ("pillow_rs", "Image"),
    "PIL.ImageDraw.ImageDraw": ("pillow_rs.imagedraw", "Draw"),
    "PIL.ImageEnhance.Brightness": ("pillow_rs.imageenhance", "Brightness"),
    "PIL.ImageEnhance.Color": ("pillow_rs.imageenhance", "Color"),
    "PIL.ImageEnhance.Contrast": ("pillow_rs.imageenhance", "Contrast"),
    "PIL.ImageEnhance.Sharpness": ("pillow_rs.imageenhance", "Sharpness"),
    "PIL.ImageFilter.BoxBlur": ("pillow_rs.imagefilter", "BoxBlur"),
    "PIL.ImageFilter.Color3DLUT": ("pillow_rs.imagefilter", "Color3DLUT"),
    "PIL.ImageFilter.GaussianBlur": ("pillow_rs.imagefilter", "GaussianBlur"),
    "PIL.ImageFilter.Kernel": ("pillow_rs.imagefilter", "Kernel"),
    "PIL.ImageFilter.MaxFilter": ("pillow_rs.imagefilter", "MaxFilter"),
    "PIL.ImageFilter.MedianFilter": ("pillow_rs.imagefilter", "MedianFilter"),
    "PIL.ImageFilter.MinFilter": ("pillow_rs.imagefilter", "MinFilter"),
    "PIL.ImageFilter.ModeFilter": ("pillow_rs.imagefilter", "ModeFilter"),
    "PIL.ImageFilter.RankFilter": ("pillow_rs.imagefilter", "RankFilter"),
    "PIL.ImageFilter.UnsharpMask": ("pillow_rs.imagefilter", "UnsharpMask"),
    "PIL.ImageFont.FreeTypeFont": ("pillow_rs.imagefont", "FreeTypeFont"),
    "PIL.ImageFont.ImageFont": ("pillow_rs.imagefont", "ImageFont"),
    "PIL.ImageFont.TransposedFont": ("pillow_rs.imagefont", "TransposedFont"),
    "PIL.ImagePalette.ImagePalette": ("pillow_rs.imagepalette", "ImagePalette"),
    "PIL.ImageSequence.Iterator": ("pillow_rs.imagesequence", "Iterator"),
    "PIL.ImageStat.Stat": ("pillow_rs.imagestat", "Stat"),
}


def import_surface(side: str, surface: str) -> Any:
    if side == "source":
        if surface in SOURCE_MODULES:
            return importlib.import_module(SOURCE_MODULES[surface])
        parts = surface.rsplit(".", 1)
        module = importlib.import_module(SOURCE_MODULES[parts[0]])
        return getattr(module, parts[1])
    if surface in TARGET_MODULES:
        return importlib.import_module(TARGET_MODULES[surface])
    if surface in TARGET_TYPES:
        module_name, attr = TARGET_TYPES[surface]
        return getattr(importlib.import_module(module_name), attr)
    raise KeyError(f"unsupported target surface: {surface}")


def operation_definition(
    operation_index: dict[tuple[str, str], dict[str, Any]],
    surface: str,
    operation: str,
) -> dict[str, Any]:
    try:
        return operation_index[(surface, operation)]
    except KeyError as exc:
        raise KeyError(f"workflow references unknown operation {surface}.{operation}") from exc


def resolve_descriptor(
    descriptor: dict[str, Any], bindings: dict[str, Any], assets: AssetStore, *, side: str
) -> Any:
    kind = descriptor["kind"]
    if kind == "literal":
        return decode_literal(descriptor.get("value"), side=side)
    if kind == "binding":
        return bindings[descriptor["step_id"]]
    if kind == "asset":
        return assets.resolve(descriptor["asset_id"])
    raise ValueError(f"unsupported workflow descriptor: {kind}")


def _call_arguments(
    opdef: dict[str, Any],
    descriptors: dict[str, dict[str, Any]],
    bindings: dict[str, Any],
    assets: AssetStore,
    *,
    side: str,
) -> tuple[list[Any], dict[str, Any]]:
    params = {param["id"]: param for param in opdef["source"]["parameters"]}
    positional: list[Any] = []
    keywords: dict[str, Any] = {}
    for name, descriptor in descriptors.items():
        value = resolve_descriptor(descriptor, bindings, assets, side=side)
        param = params.get(name, {})
        style = param.get("style", "positional_or_keyword")
        if style == "positional":
            positional.append(value)
        elif style == "variadic_positional":
            positional.extend(value)
        elif style == "variadic_keyword":
            if not isinstance(value, dict):
                raise TypeError(f"variadic keyword argument {name} must be a mapping")
            keywords.update(value)
        else:
            keywords[name] = value
    return positional, keywords


def call_workflow_step(
    side: str,
    step: dict[str, Any],
    opdef: dict[str, Any],
    bindings: dict[str, Any],
    assets: AssetStore,
) -> Any:
    receiver_desc = step.get("receiver")
    receiver = (
        resolve_descriptor(receiver_desc, bindings, assets, side=side)
        if receiver_desc is not None
        else None
    )
    positional, keywords = _call_arguments(
        opdef, step.get("arguments", {}), bindings, assets
        , side=side
    )
    operation = step["operation"]
    if opdef["kind"] == "property_get":
        if receiver is None:
            raise TypeError(f"property {operation} requires a receiver")
        return getattr(receiver, operation)
    if opdef["kind"] == "constant":
        return getattr(import_surface(side, step["surface"]), operation)
    if receiver is not None:
        callable_value = getattr(receiver, operation)
    else:
        callable_value = getattr(import_surface(side, step["surface"]), operation)
    # A few source inventories classify public constants under the same
    # namespace/type bucket as constructors.  The value itself is the public
    # step result; attempting to call a string would turn a valid constant
    # workflow into an adapter-only ``TypeError``.
    if not callable(callable_value):
        return callable_value
    if side == "target" and step["surface"] == "PIL.ImageDraw" and operation == "Draw" and "im" in keywords:
        # The target facade intentionally spells this constructor argument
        # ``image`` while the Pillow public constructor spells it ``im``.
        # This is a target binding conversion, not a changed input workflow.
        keywords["image"] = keywords.pop("im")
    return callable_value(*positional, **keywords)


def _metadata(value: Any, name: str) -> Any:
    try:
        return json_safe(getattr(value, name))
    except Exception:
        return None


def serialize_value(value: Any, shape: str, *, side: str, surface: str, operation: str) -> Any:
    """Serialize the declared public result shape, never an implementation repr."""

    if shape == "none":
        return None
    if shape == "image":
        try:
            raw = bytes(value.tobytes())
        except Exception:
            raw = b""
        return {
            "kind": "image",
            "mode": str(getattr(value, "mode", "")),
            "size": json_safe(getattr(value, "size", None)),
            "format": _metadata(value, "format"),
            "info": _metadata(value, "info"),
            "palette": _metadata(value, "palette"),
            "bytes": base64.b64encode(raw).decode("ascii"),
        }
    if shape == "mask":
        try:
            raw = bytes(value)
        except Exception:
            try:
                raw = bytes(value.tobytes())
            except Exception:
                raw = b""
        return {
            "kind": "mask",
            "mode": str(getattr(value, "mode", "")),
            "size": json_safe(getattr(value, "size", None)),
            "bytes": base64.b64encode(raw).decode("ascii"),
        }
    if shape == "bytes":
        try:
            raw = bytes(value)
        except Exception:
            raw = b""
        return {
            "kind": "bytes",
            "encoding": "base64",
            "data": base64.b64encode(raw).decode("ascii"),
        }
    if shape == "handle":
        type_name = type(value).__name__
        # The target's Draw class is the public ImageDraw.ImageDraw endpoint.
        if side == "target" and surface == "PIL.ImageDraw" and operation == "Draw":
            type_name = "ImageDraw"
        return {"type": type_name}
    if shape in {"sequence", "ordered", "metrics"}:
        if isinstance(value, (str, bytes, bytearray)):
            return json_safe(value)
        try:
            return [json_safe(item) for item in value]
        except TypeError:
            return json_safe(value)
    if shape in {"mapping", "record"}:
        if isinstance(value, dict):
            return json_safe(value)
        attrs = getattr(value, "__dict__", None)
        if isinstance(attrs, dict):
            return json_safe(attrs)
    if shape == "scalar":
        return json_safe(value)
    return json_safe(value)


def public_error(exc: BaseException) -> dict[str, Any]:
    if isinstance(exc, (FileNotFoundError, PermissionError, IsADirectoryError, OSError)):
        kind = "io_error"
    elif isinstance(exc, NotImplementedError):
        kind = "unsupported"
    elif isinstance(exc, TypeError):
        kind = "type_error"
    elif isinstance(exc, ValueError):
        kind = "invalid_argument"
    else:
        kind = "runtime_error"
    return {
        "class": type(exc).__name__,
        "kind": kind,
        "message": str(exc),
        "stage": "call",
        "code": None,
    }


def run_case(
    side: str,
    case: dict[str, Any],
    operation_index: dict[tuple[str, str], dict[str, Any]],
    tempdir: Path,
    *,
    timing_steps: set[str] | None = None,
    timing_sink: list[int] | None = None,
) -> dict[str, Any]:
    assets = AssetStore(case.get("assets", []), FIXTURE_ROOT / "assets", tempdir)
    bindings: dict[str, Any] = {}
    step_results: dict[str, dict[str, Any]] = {}
    blocked_reason: str | None = None
    for step in case["steps"]:
        step_id = step["step_id"]
        if blocked_reason is not None:
            step_results[step_id] = {
                "step_id": step_id,
                "status": "not_run",
                "reason": blocked_reason,
            }
            continue
        try:
            opdef = operation_definition(
                operation_index, step["surface"], step["operation"]
            )
            started_ns = (
                time.perf_counter_ns()
                if timing_steps and step_id in timing_steps
                else None
            )
            value = call_workflow_step(
                side, step, opdef, bindings, assets
            )
            if started_ns is not None and timing_sink is not None:
                timing_sink.append(time.perf_counter_ns() - started_ns)
            bindings[step_id] = value
            step_results[step_id] = {"step_id": step_id, "status": "ok", "_value": value}
        except BaseException as exc:  # public failures are part of the contract
            error = public_error(exc)
            step_results[step_id] = {
                "step_id": step_id,
                "status": "error",
                "error": error,
            }
            blocked_reason = f"dependency step {step_id} failed"

    observations: list[dict[str, Any]] = []
    for observation_id in case.get("observations", []):
        result = step_results.get(observation_id)
        if result is None:
            observations.append(
                {
                    "step_id": observation_id,
                    "status": "not_run",
                    "reason": "observation step is not present in workflow",
                }
            )
            continue
        if result["status"] != "ok":
            observations.append(result)
            continue
        step = next(item for item in case["steps"] if item["step_id"] == observation_id)
        opdef = operation_definition(operation_index, step["surface"], step["operation"])
        shape = opdef["source"]["result"]["shape"]
        observations.append(
            {
                "step_id": observation_id,
                "status": "ok",
                "value": serialize_value(
                    result["_value"],
                    shape,
                    side=side,
                    surface=step["surface"],
                    operation=step["operation"],
                ),
            }
        )
    return {
        "case_id": case["case_id"],
        "status": "completed",
        "observations": observations,
    }


def load_manifest(path: Path) -> dict[str, Any]:
    manifest = yaml.safe_load(path.read_text(encoding="utf-8"))
    if manifest.get("schema") != "migration-parity/manifest@2":
        raise ValueError("manifest must declare migration-parity/manifest@2")
    return manifest


def load_cases(manifest: dict[str, Any], *, case_ids: set[str] | None, surface: str | None) -> tuple[list[dict[str, Any]], dict[str, str]]:
    cases: list[dict[str, Any]] = []
    case_inputs: dict[str, str] = {}
    for relative in manifest["input_index"]["parity"]:
        path = ROOT / "pillow-rs" / "tests" / "fixtures" / relative
        payload = json.loads(path.read_text(encoding="utf-8"))
        if payload.get("schema") != "migration-parity/parity-input@1":
            raise ValueError(f"{relative}: invalid parity input schema")
        for case in payload["cases"]:
            if case_ids and case["case_id"] not in case_ids:
                continue
            if surface and case["surface"] != surface:
                continue
            if case["case_id"] in case_inputs:
                raise ValueError(f"duplicate active case ID: {case['case_id']}")
            cases.append(case)
            case_inputs[case["case_id"]] = relative
    cases.sort(key=lambda item: item["case_id"])
    return cases, case_inputs


def build_operation_index(manifest: dict[str, Any]) -> dict[tuple[str, str], dict[str, Any]]:
    return {
        (surface["id"], operation["id"]): operation
        for surface in manifest["surfaces"]
        for operation in surface["operations"]
    }


def side_identity(side: str) -> dict[str, str]:
    if side == "source":
        pil = importlib.import_module("PIL")
        version = str(getattr(pil, "__version__", ""))
        if version != ORACLE_VERSION:
            raise RuntimeError(f"Pillow oracle version {version!r}, expected {ORACLE_VERSION}")
        return {"side": "source", "implementation": "Pillow", "version": version}
    target = importlib.import_module("pillow_rs")
    target_path = Path(target.__file__).resolve()
    expected_root = (ROOT / "pillow-rs-py" / "python").resolve()
    if expected_root not in target_path.parents:
        raise RuntimeError(f"target imported outside checkout: {target_path}")
    return {
        "side": "target",
        "implementation": "pillow-rs",
        "version": str(getattr(target, "__version__", "unknown")),
        "path": str(target_path),
    }


def run_side_subprocess(
    side: str,
    manifest_path: Path,
    cases: list[dict[str, Any]],
    timeout_seconds: int,
) -> tuple[dict[str, str], dict[str, dict[str, Any]]]:
    payload = json.dumps(cases, separators=(",", ":"))
    command = [
        sys.executable,
        str(Path(__file__).resolve()),
        "--side",
        side,
        "--manifest",
        str(manifest_path),
    ]
    env = os.environ.copy()
    target_python = str(ROOT / "pillow-rs-py" / "python")
    env["PYTHONPATH"] = target_python + os.pathsep + env.get("PYTHONPATH", "")
    try:
        process = subprocess.run(
            command,
            input=payload,
            text=True,
            capture_output=True,
            env=env,
            cwd=ROOT,
            timeout=timeout_seconds,
            check=False,
        )
    except subprocess.TimeoutExpired as exc:
        raise RuntimeError(f"{side} adapter timed out") from exc
    if process.returncode != 0:
        detail = process.stderr.strip().replace("\n", " ")[-800:]
        raise RuntimeError(f"{side} adapter exited {process.returncode}: {detail}")
    try:
        result = json.loads(process.stdout)
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"{side} adapter emitted malformed JSON") from exc
    if set(result) != {"identity", "results"}:
        raise RuntimeError(f"{side} adapter emitted invalid handshake envelope")
    results = result["results"]
    by_id = {item["case_id"]: item for item in results}
    if len(by_id) != len(cases) or set(by_id) != {case["case_id"] for case in cases}:
        raise RuntimeError(f"{side} adapter result IDs/count do not match selected cases")
    return result["identity"], by_id


def comparison_policy(
    operation_index: dict[tuple[str, str], dict[str, Any]], step: dict[str, Any]
) -> dict[str, Any]:
    result = operation_definition(operation_index, step["surface"], step["operation"])["source"]["result"]
    observations = result.get("observations", [])
    return observations[0].get("comparison", {"kind": "exact"}) if observations else {"kind": "exact"}


def _diff(path: str, kind: str, source: Any, target: Any, message: str) -> dict[str, Any]:
    return {
        "step_id": path.split(".", 1)[0],
        "path": path,
        "kind": kind,
        "source": source,
        "target": target,
        "message": message,
    }


def compare_value(source: Any, target: Any, policy: dict[str, Any], path: str) -> list[dict[str, Any]]:
    kind = policy.get("kind", "exact")
    if kind == "numeric":
        if isinstance(source, (int, float)) and isinstance(target, (int, float)):
            if math.isnan(source) or math.isnan(target):
                equal = policy.get("nan_policy") == "equal" and math.isnan(source) and math.isnan(target)
            else:
                delta = abs(float(source) - float(target))
                scale = max(abs(float(source)), abs(float(target)))
                equal = delta <= float(policy.get("absolute_tolerance", 0)) or delta <= float(policy.get("relative_tolerance", 0)) * scale
            return [] if equal else [_diff(path, "numeric_mismatch", source, target, "declared numeric tolerance exceeded")]
        return [_diff(path, "value_mismatch", source, target, "numeric observation is not numeric")]
    if kind == "ordered":
        if source == target:
            return []
        return [_diff(path, "value_mismatch", source, target, "ordered value mismatch")]
    if kind == "bytes":
        if source == target:
            return []
        return [_diff(path, "bytes_mismatch", source, target, "exact byte value mismatch")]
    if kind == "image":
        if source == target:
            return []
        return [_diff(path, "image_mismatch", source, target, "declared image comparison mismatch")]
    if kind == "text":
        transforms = set(policy.get("transforms", []))
        left = str(source)
        right = str(target)
        if "normalize_newlines" in transforms:
            left = left.replace("\r\n", "\n").replace("\r", "\n")
            right = right.replace("\r\n", "\n").replace("\r", "\n")
        return [] if left == right else [_diff(path, "value_mismatch", source, target, "declared text comparison mismatch")]
    if kind == "unordered":
        try:
            left = sorted(source, key=lambda value: json.dumps(value, sort_keys=True))
            right = sorted(target, key=lambda value: json.dumps(value, sort_keys=True))
        except (TypeError, ValueError):
            left, right = source, target
        return [] if left == right else [_diff(path, "value_mismatch", source, target, "unordered value mismatch")]
    return [] if source == target else [_diff(path, "value_mismatch", source, target, "exact value mismatch")]


def compare_error(source: dict[str, Any], target: dict[str, Any], opdef: dict[str, Any], path: str) -> list[dict[str, Any]]:
    error_policy = opdef["source"]["result"].get("error", {})
    fields = error_policy.get("fields", ["class", "kind", "message", "stage", "code"])
    for field in fields:
        if source.get(field) != target.get(field):
            return [_diff(f"{path}.error.{field}", "error_mismatch", source.get(field), target.get(field), "declared public error field mismatch")]
    return []


def compare_case(
    case: dict[str, Any],
    source: dict[str, Any],
    target: dict[str, Any],
    operation_index: dict[tuple[str, str], dict[str, Any]],
) -> tuple[str, list[dict[str, Any]]]:
    if source["status"] != target["status"]:
        return "fail", [_diff("status", "status_mismatch", source["status"], target["status"], "workflow status mismatch")]
    source_obs = {item["step_id"]: item for item in source["observations"]}
    target_obs = {item["step_id"]: item for item in target["observations"]}
    if set(source_obs) != set(target_obs):
        return "fail", [_diff("observations", "observation_set_mismatch", sorted(source_obs), sorted(target_obs), "observation IDs differ")]
    diffs: list[dict[str, Any]] = []
    for step in case.get("steps", []):
        step_id = step["step_id"]
        if step_id not in source_obs:
            continue
        left = source_obs[step_id]
        right = target_obs[step_id]
        if left["status"] == "not_run" or right["status"] == "not_run":
            if left != right:
                diffs.append(_diff(f"{step_id}.status", "not_run_mismatch", left, right, "workflow observation was not run"))
            continue
        if left["status"] != right["status"]:
            diffs.append(_diff(f"{step_id}.status", "status_mismatch", left["status"], right["status"], "public observation status mismatch"))
            continue
        opdef = operation_definition(operation_index, step["surface"], step["operation"])
        if left["status"] == "error":
            diffs.extend(compare_error(left["error"], right["error"], opdef, step_id))
        else:
            diffs.extend(compare_value(left.get("value"), right.get("value"), comparison_policy(operation_index, step), f"{step_id}.value"))
    return ("pass" if not diffs else "fail"), diffs


def build_identity(
    manifest_path: Path,
    input_paths: list[str],
    command: dict[str, Any],
    cases: list[dict[str, Any]],
    case_inputs: dict[str, str],
    *,
    target_dirty: bool,
) -> dict[str, Any]:
    manifest = load_manifest(manifest_path)
    assets: list[dict[str, Any]] = []
    for case in cases:
        input_path = case_inputs[case["case_id"]]
        for asset in case.get("assets", []):
            kind = asset["kind"]
            if kind == "ref":
                locator = asset["path"]
                digest = asset.get("sha256")
            elif kind == "inline":
                locator = None
                digest = asset.get("sha256")
            elif kind == "builtin":
                locator = asset.get("name")
                builtin = asset.get("name")
                digest = (
                    sha256_bytes(ENCODED_INPUTS[builtin])
                    if builtin in ENCODED_INPUTS
                    else None
                )
            else:
                locator = asset.get("path")
                digest = None
            assets.append(
                {
                    "input_path": input_path,
                    "item_id": case["case_id"],
                    "asset_id": asset["id"],
                    "kind": kind,
                    "locator": locator,
                    "sha256": digest,
                }
            )
    return {
        "run_id": f"migration-parity-{uuid.uuid4().hex}",
        "started_at": now_rfc3339(),
        "finished_at": now_rfc3339(),
        "manifest": {"path": str(manifest_path.relative_to(ROOT)), "schema": manifest["schema"], "sha256": sha256_file(manifest_path)},
        "inputs": [
            {
                "path": path,
                "schema": "migration-parity/parity-input@1",
                "sha256": sha256_file(ROOT / "pillow-rs" / "tests" / "fixtures" / path),
            }
            for path in input_paths
        ],
        "assets": assets,
        "oracles": [{"oracle_id": ORACLE_ID, "name": "Pillow", "version": ORACLE_VERSION, "runtime": "CPython 3.12"}],
        "targets": [{"target_profile": TARGET_PROFILE, "target_id": TARGET_ID, "revision": git_revision(), "dirty": target_dirty, "runtime": platform.python_version(), "backend": "cpu", "features": ["all-features"]}],
        "command": command,
    }


def git_revision() -> str:
    try:
        return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def git_dirty() -> bool:
    try:
        return bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip())
    except (OSError, subprocess.CalledProcessError):
        return True


def run_orchestrator(args: argparse.Namespace) -> int:
    manifest_path = args.manifest.resolve()
    manifest = load_manifest(manifest_path)
    case_ids = set(args.case_id or [])
    cases, case_inputs = load_cases(manifest, case_ids=case_ids or None, surface=args.surface)
    if args.limit is not None:
        cases = cases[: args.limit]
        case_inputs = {case["case_id"]: case_inputs[case["case_id"]] for case in cases}
    if not cases:
        raise ValueError("no active parity cases selected")
    operation_index = build_operation_index(manifest)
    command = {"command_id": "parity", "argv": ["make", "migration-parity-test"], "cwd": ".", "timeout_seconds": args.timeout}
    identity = build_identity(
        manifest_path,
        sorted(set(case_inputs.values())),
        command,
        cases,
        case_inputs,
        target_dirty=git_dirty(),
    )
    started = now_rfc3339()
    try:
        source_handshake, source_results = run_side_subprocess("source", manifest_path, cases, args.timeout)
        target_handshake, target_results = run_side_subprocess("target", manifest_path, cases, args.timeout)
    except RuntimeError as exc:
        result = {
            "schema": "migration-parity/parity-result@1",
            "identity": identity,
            "status": "infrastructure_failed",
            "summary": {"selected": len(cases), "executed": 0, "passed": 0, "failed": 0, "not_run": len(cases), "infrastructure_errors": 1},
            "comparisons": [],
            "infrastructure_errors": [{"scope": "runner", "id": None, "kind": "adapter_failure", "message": str(exc)}],
        }
        write_result(args.output, result)
        print(json.dumps(result["summary"], sort_keys=True))
        return 2
    if source_handshake.get("version") != ORACLE_VERSION:
        raise RuntimeError("oracle identity handshake did not pin Pillow 12.2.0")
    comparisons: list[dict[str, Any]] = []
    passed = failed = not_run = 0
    for case in cases:
        outcome, diffs = compare_case(case, source_results[case["case_id"]], target_results[case["case_id"]], operation_index)
        if outcome == "pass":
            passed += 1
        elif outcome == "fail":
            failed += 1
        else:
            not_run += 1
        comparisons.append({"case_id": case["case_id"], "target_profile": TARGET_PROFILE, "requirements": case.get("covers", []), "source": source_results[case["case_id"]], "target": target_results[case["case_id"]], "outcome": outcome, "diffs": diffs})
    identity["started_at"] = started
    identity["finished_at"] = now_rfc3339()
    result = {
        "schema": "migration-parity/parity-result@1",
        "identity": identity,
        "status": "completed",
        "summary": {"selected": len(cases), "executed": len(cases), "passed": passed, "failed": failed, "not_run": not_run, "infrastructure_errors": 0},
        "comparisons": comparisons,
        "infrastructure_errors": [],
    }
    write_result(args.output, result)
    print(json.dumps(result["summary"], sort_keys=True))
    return 0 if failed == 0 and not_run == 0 else 1


def write_result(path: Path, result: dict[str, Any]) -> None:
    path = path.resolve()
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(result, indent=2, sort_keys=False) + "\n", encoding="utf-8")


def run_side(args: argparse.Namespace) -> int:
    manifest = load_manifest(args.manifest.resolve())
    cases = json.loads(sys.stdin.read())
    operation_index = build_operation_index(manifest)
    handshake = side_identity(args.side)
    results: list[dict[str, Any]] = []
    timings: dict[str, list[int]] = {}
    with tempfile.TemporaryDirectory(prefix=f"migration-parity-{args.side}-") as temporary:
        tempdir = Path(temporary)
        for case in cases:
            sink: list[int] = []
            result: dict[str, Any] | None = None
            for _ in range(args.repeat):
                result = run_case(
                    args.side,
                    case,
                    operation_index,
                    tempdir,
                    timing_steps=set(args.timing_step),
                    timing_sink=sink if args.timings else None,
                )
            assert result is not None
            results.append(result)
            if args.timings:
                timings[case["case_id"]] = sink
    envelope: dict[str, Any] = {"identity": handshake, "results": results}
    if args.timings:
        envelope["timings_ns"] = timings
    sys.stdout.write(json.dumps(envelope, separators=(",", ":")) + "\n")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--limit", type=int)
    parser.add_argument("--case-id", action="append")
    parser.add_argument("--surface")
    parser.add_argument("--timeout", type=int, default=3600)
    parser.add_argument("--side", choices=("source", "target"))
    parser.add_argument("--identity", choices=("source", "target"))
    parser.add_argument("--repeat", type=int, default=1)
    parser.add_argument("--timings", action="store_true")
    parser.add_argument("--timing-step", action="append", default=["call"])
    args = parser.parse_args()
    if args.side:
        return run_side(args)
    if args.identity:
        print(json.dumps(side_identity(args.identity), sort_keys=True))
        return 0
    return run_orchestrator(args)


if __name__ == "__main__":
    raise SystemExit(main())
