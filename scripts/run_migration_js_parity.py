#!/usr/bin/env python3
"""Run the public migration-parity workflows through the JS/WASM facade.

The active workflow documents are shared with the Python lane.  Every selected
workflow is sent to JavaScript, including cases whose values or operations are
not yet implemented by the current wasm-bindgen API.  Those cases must still
produce a completed workflow result (normally a structured public target
error), so the execution denominator is honest.  A separate diagnostic report
retains stable hints about known facade limits; those hints do not filter cases
and are never a pending count.

Pillow remains the oracle: every selected case is executed by the existing
source adapter and by one selected WASM host, then compared with the
canonical parity comparison policy.  The Node and browser hosts use the same
workflow adapter and the same input payload.  Browser WebGPU capability is
reported separately; a capability probe is not counted as a shader dispatch.
"""

from __future__ import annotations

import argparse
import base64
import collections
import copy
import datetime as _dt
import hashlib
import json
import math
from pathlib import Path
import subprocess
import sys
from typing import Any

try:
    from run_migration_parity import (
        ENCODED_INPUTS,
        FIXTURE_ROOT,
        build_operation_index,
        compare_case,
        load_cases,
        load_manifest,
        receipt_terminal_complete,
        run_side_subprocess,
    )
except ModuleNotFoundError:  # imported as ``scripts.run_migration_js_parity``
    from scripts.run_migration_parity import (
        ENCODED_INPUTS,
        FIXTURE_ROOT,
        build_operation_index,
        compare_case,
        load_cases,
        load_manifest,
        receipt_terminal_complete,
        run_side_subprocess,
    )


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "pillow-rs" / "tests" / "fixtures" / "manifest.yaml"
DEFAULT_OUTPUT = ROOT / "build" / "migration-parity" / "js-wasm-parity-result.json"
DEFAULT_BROWSER_OUTPUT = ROOT / "build" / "migration-parity" / "browser-wasm-parity-result.json"
NODE_RUNNER = ROOT / "pillow-rs-js" / "scripts" / "run_parity.mjs"
BROWSER_RUNNER = ROOT / "pillow-rs-js" / "scripts" / "run_browser_parity.mjs"
DEFAULT_CHUNK_SIZE = 128


IMAGE_METHODS = {
    "alpha_composite",
    "apply_transparency",
    "box_blur",
    "close",
    "convert",
    "copy",
    "crop",
    "draft",
    "effect_spread",
    "entropy",
    "filter",
    "gaussian_blur",
    "getbands",
    "getbbox",
    "getchannel",
    "getcolors",
    "getdata",
    "getextrema",
    "getpalette",
    "getprojection",
    "get_flattened_data",
    "histogram",
    "load",
    "max_filter",
    "median_filter",
    "min_filter",
    "mode_filter",
    "point",
    "putalpha",
    "putdata",
    "putpalette",
    "putpixel",
    "quantize",
    "reduce",
    "remap_palette",
    "resize",
    "rotate",
    "split",
    "thumbnail",
    "tobitmap",
    "tobytes",
    "transform",
    "transpose",
    "unsharp_mask",
    "verify",
}
IMAGE_PROPERTIES = {"height", "mode", "size", "width"}
CHOPS_METHODS = {
    "add",
    "add_modulo",
    "blend",
    "composite",
    "constant",
    "darker",
    "difference",
    "duplicate",
    "hard_light",
    "invert",
    "lighter",
    "logical_and",
    "logical_or",
    "logical_xor",
    "multiply",
    "offset",
    "overlay",
    "screen",
    "soft_light",
    "subtract",
    "subtract_modulo",
}
OPS_METHODS = {
    "autocontrast",
    "contain",
    "cover",
    "crop",
    "equalize",
    "expand",
    "fit",
    "flip",
    "grayscale",
    "invert",
    "mirror",
    "pad",
    "posterize",
    "scale",
    "solarize",
}
DRAW_METHODS = {
    "arc",
    "chord",
    "circle",
    "ellipse",
    "line",
    "pieslice",
    "point",
    "polygon",
    "rectangle",
    "rounded_rectangle",
}
# ``alpha_composite`` is an Image.Image mutator in the current JS binding;
# there is no static ``PIL.Image.alpha_composite`` export to dispatch here.
IMAGE_FUNCTIONS = {"blend", "composite", "merge"}

# These operations consume the process-global Darwin-compatible RNG in the
# Rust core.  Restarting the JS host between ordinary chunks resets that
# state, so workflows containing either operation must be replayed together in
# one host process.  Other workflows do not consume this RNG and can remain
# chunked for failure isolation.
PROCESS_GLOBAL_STATE_OPS = {
    ("PIL.Image", "effect_noise"),
    ("PIL.Image.Image", "effect_spread"),
}


def now() -> str:
    return _dt.datetime.now(_dt.timezone.utc).isoformat().replace("+00:00", "Z")


def case_digest(cases: list[dict[str, Any]]) -> str:
    ids = sorted(case["case_id"] for case in cases)
    return hashlib.sha256(("\n".join(ids) + "\n").encode()).hexdigest()


def uses_process_global_state(case: dict[str, Any]) -> bool:
    return any(
        (step["surface"], step["operation"]) in PROCESS_GLOBAL_STATE_OPS
        for step in case.get("steps", [])
    )


def plain_json(value: Any) -> bool:
    """Return whether a literal has no host protocol or callable semantics."""

    if value is None or isinstance(value, (bool, int, float, str)):
        return True
    if isinstance(value, list):
        return all(plain_json(item) for item in value)
    return False


def literal_value(descriptor: dict[str, Any]) -> Any:
    return descriptor.get("value") if descriptor.get("kind") == "literal" else None


def js_json_value(value: Any) -> Any:
    """Encode non-finite public numbers without relying on non-JSON tokens."""

    if isinstance(value, float) and not math.isfinite(value):
        return {
            "__pillow_rs_nonfinite_number__": (
                "NaN" if math.isnan(value) else "Infinity" if value > 0 else "-Infinity"
            )
        }
    if isinstance(value, list):
        return [js_json_value(item) for item in value]
    if isinstance(value, dict):
        return {key: js_json_value(item) for key, item in value.items()}
    return value


def js_asset_payload(
    cases: list[dict[str, Any]],
) -> tuple[list[dict[str, Any]], dict[str, dict[str, Any]]]:
    """Embed input assets for the JS hosts without embedding oracle outputs.

    Python can resolve a ``ref`` asset as a path and can create its small
    callable/file builtins locally. Node and a browser page cannot read the
    repository or a Python temporary directory, so the host-neutral runner
    sends the original stimulus bytes (or an explicit protocol marker) in the
    batch envelope. Asset IDs are shared by generated cases; duplicate
    definitions must remain byte-for-byte identical.
    """

    payload: dict[str, dict[str, Any]] = {}
    target_cases: list[dict[str, Any]] = []
    assets_root = FIXTURE_ROOT / "assets"

    def add(asset_id: str, value: dict[str, Any]) -> None:
        existing = payload.get(asset_id)
        if existing is not None and existing != value:
            raise ValueError(f"asset id has conflicting JS payloads: {asset_id}")
        payload[asset_id] = value

    for case in cases:
        # Asset IDs are only unique within a workflow.  The shared JS
        # envelope is batch-scoped, so two unrelated workflows may both use
        # names such as ``image-asset`` for different bytes.  Give each
        # workflow a private transport key and rewrite only the descriptors
        # sent to the JS host.  The source case and its identity remain
        # untouched, so this is transport normalization rather than a fixture
        # change.
        target_case = copy.deepcopy(case)
        asset_ids: dict[str, str] = {}
        rewritten_assets: list[dict[str, Any]] = []
        for asset in case.get("assets", []):
            source_id = asset["id"]
            transport_id = f"{case['case_id']}::{source_id}"
            asset_ids[source_id] = transport_id
            rewritten_asset = dict(asset)
            rewritten_asset["id"] = transport_id
            rewritten_assets.append(rewritten_asset)
        target_case["assets"] = rewritten_assets

        # Pillow treats a bytes object passed as Image.open(fp) as a
        # filesystem path, while the same bytes passed to decoder/data
        # parameters are an in-memory buffer. Preserve that distinction in
        # the JS transport without changing the public fixture.
        open_fp_asset_ids = {
            step["arguments"]["fp"]["asset_id"]
            for step in case.get("steps", [])
            if step.get("surface") == "PIL.Image"
            and step.get("operation") == "open"
            and isinstance(step.get("arguments", {}).get("fp"), dict)
            and step["arguments"]["fp"].get("kind") == "asset"
        }

        def rewrite_descriptor(value: Any) -> Any:
            if isinstance(value, dict):
                if value.get("kind") == "asset":
                    rewritten = dict(value)
                    asset_id = value.get("asset_id")
                    if asset_id in asset_ids:
                        rewritten["asset_id"] = asset_ids[asset_id]
                    return rewritten
                return {key: rewrite_descriptor(item) for key, item in value.items()}
            if isinstance(value, list):
                return [rewrite_descriptor(item) for item in value]
            return value

        target_case["steps"] = rewrite_descriptor(target_case.get("steps", []))
        target_cases.append(target_case)

        for asset in case.get("assets", []):
            source_id = asset["id"]
            asset_id = asset_ids[source_id]
            kind = asset["kind"]
            if kind == "inline":
                if asset.get("encoding") != "base64":
                    raise ValueError(f"unsupported inline encoding: {asset_id}")
                add(
                    asset_id,
                    {
                        # AssetStore materializes inline encoded images before
                        # passing them to Image.open; transport those as image
                        # bytes so the JS facade decodes them.  Non-image
                        # inline bytes remain Python ``bytes`` path inputs.
                        "kind": (
                            "path-bytes"
                            if source_id in open_fp_asset_ids
                            and not asset.get("media_type", "").startswith("image/")
                            else "bytes"
                        ),
                        "data_base64": asset["data"],
                        "media_type": asset.get("media_type"),
                    },
                )
            elif kind == "ref":
                path = assets_root / asset["path"]
                if not path.is_file():
                    raise ValueError(f"missing JS input asset: {asset['path']}")
                if asset.get("media_type") == "application/x-pilfont":
                    glyph_path = next(
                        (
                            path.with_suffix(extension)
                            for extension in (".png", ".gif", ".pbm")
                            if path.with_suffix(extension).is_file()
                        ),
                        None,
                    )
                    if glyph_path is None:
                        raise ValueError(
                            f"missing PILfont glyph asset for {asset['path']}"
                        )
                    add(
                        asset_id,
                        {
                            "kind": "pilfont",
                            "metrics_base64": base64.b64encode(
                                path.read_bytes()
                            ).decode("ascii"),
                            "glyph_base64": base64.b64encode(
                                glyph_path.read_bytes()
                            ).decode("ascii"),
                        },
                    )
                else:
                    add(
                        asset_id,
                        {
                            "kind": "bytes",
                            "data_base64": base64.b64encode(path.read_bytes()).decode(
                                "ascii"
                            ),
                            "media_type": asset.get("media_type"),
                        },
                    )
            elif kind == "missing":
                add(asset_id, {"kind": "missing", "path": asset.get("path")})
            elif kind == "builtin":
                name = asset.get("name")
                if name in ENCODED_INPUTS:
                    add(
                        asset_id,
                        {
                            "kind": "bytes",
                            "data_base64": base64.b64encode(
                                ENCODED_INPUTS[name]
                            ).decode("ascii"),
                        },
                    )
                elif name and name.startswith("encoded-") and name.endswith(
                    "-input-stream"
                ):
                    encoded_name = name.removesuffix("-stream")
                    if encoded_name not in ENCODED_INPUTS:
                        raise ValueError(f"unknown encoded builtin asset: {name}")
                    add(
                        asset_id,
                        {
                            "kind": "bytes",
                            "data_base64": base64.b64encode(
                                ENCODED_INPUTS[encoded_name]
                            ).decode("ascii"),
                        },
                    )
                elif name in {
                    "identity-callable",
                    "clamp-shift-callable",
                    "point-affine-shift-callable",
                    "point-affine-scale-callable",
                    "point-byte-float-callable",
                    "color3dlut-generate-identity",
                    "color3dlut-transform-identity",
                    "color3dlut-transform-rgba",
                    "color3dlut-short-result",
                }:
                    add(asset_id, {"kind": "callable", "name": name})
                elif name in {"font-byte-stream", "in-memory-byte-stream"}:
                    add(asset_id, {"kind": "bytes", "data_base64": ""})
                elif name in {
                    "temporary-output-path",
                    "temporary-output-no-extension-path",
                    "read-only-directory",
                }:
                    add(asset_id, {"kind": "path", "name": name})
                else:
                    raise ValueError(f"unsupported JS builtin asset: {name}")
            else:
                raise ValueError(f"unsupported JS asset kind: {kind}")
    return target_cases, payload


def integer_sequence(value: Any, length: int | None = None) -> bool:
    return (
        isinstance(value, list)
        and (length is None or len(value) == length)
        and all(type(item) is int for item in value)
    )


def compatible_case(
    case: dict[str, Any],
    operation_index: dict[tuple[str, str], dict[str, Any]],
) -> str | None:
    """Return a stable diagnostic hint, or ``None`` when no hint applies."""

    for step in case.get("steps", []):
        key = (step["surface"], step["operation"])
        if key not in operation_index:
            return "unknown-manifest-operation"
        surface, operation = key
        if surface == "PIL.Image.Image":
            if operation in IMAGE_PROPERTIES:
                pass
            elif operation not in IMAGE_METHODS:
                return f"image-method-not-exported:{operation}"
        elif surface == "PIL.ImageDraw.ImageDraw":
            if operation not in DRAW_METHODS:
                return f"draw-method-not-exported:{operation}"
        elif surface == "PIL.ImageChops":
            if operation not in CHOPS_METHODS:
                return f"chops-method-not-exported:{operation}"
        elif surface == "PIL.ImageOps":
            if operation not in OPS_METHODS:
                return f"imageops-method-not-exported:{operation}"
        elif surface == "PIL.Image" and operation in {"new", *IMAGE_FUNCTIONS}:
            pass
        elif surface == "PIL.ImageDraw" and operation == "Draw":
            pass
        else:
            return f"surface-not-exported:{surface}.{operation}"

        descriptors = list(step.get("arguments", {}).values())
        if step.get("receiver") is not None:
            descriptors.append(step["receiver"])
        for descriptor in descriptors:
            if descriptor["kind"] in {"binding", "bindings"}:
                continue
            if descriptor["kind"] == "asset":
                # Assets are transported by ``js_asset_payload`` and are part
                # of the same public input workflow.  They are not pending.
                continue
            if descriptor["kind"] != "literal" or not plain_json(descriptor.get("value")):
                return "host-value-not-representable-in-js"

        arguments = step.get("arguments", {})
        if surface == "PIL.Image" and operation == "new":
            mode = literal_value(arguments.get("mode", {}))
            size = literal_value(arguments.get("size", {}))
            color = literal_value(arguments.get("color", {"kind": "literal", "value": 0}))
            if not isinstance(mode, str) or not integer_sequence(size, 2):
                return "image-new-shape-not-supported"
            if not (type(color) in {int, float} or integer_sequence(color)):
                return "image-new-color-not-supported"
        if surface == "PIL.Image.Image" and operation in {"putpixel", "putdata"}:
            for name in ("value", "data"):
                if name not in arguments:
                    continue
                descriptor = arguments[name]
                if descriptor["kind"] == "binding":
                    return f"{operation}-binding-not-supported"
                value = literal_value(descriptor)
                if operation == "putdata" and not isinstance(value, list):
                    return "putdata-shape-not-supported"
        if surface == "PIL.ImageDraw.ImageDraw" and operation == "line":
            xy = literal_value(arguments.get("xy", {}))
            if not integer_sequence(xy, 4):
                return "draw-line-only-flat-four-coordinate-js-api"
            if "joint" in arguments:
                return "draw-line-joint-not-exported"
        if surface == "PIL.Image.Image" and operation == "filter":
            return "image-filter-object-not-exported"
        if surface == "PIL.Image.Image" and operation == "point" and "mode" in arguments:
            mode = literal_value(arguments["mode"])
            if mode != "F":
                return "image-point-output-mode-not-exported"
        if surface == "PIL.Image.Image" and operation == "tobytes":
            if set(arguments) - {"encoder_name", "args"}:
                return "encoded-tobytes-not-exported"
            if "encoder_name" in arguments:
                encoder = literal_value(arguments["encoder_name"])
                if encoder not in {None, "raw"}:
                    return "encoded-tobytes-not-exported"
        if surface == "PIL.Image.Image" and operation in {"info", "format", "getim"}:
            return f"image-metadata-not-exported:{operation}"
    return None


def operation_payload(
    cases: list[dict[str, Any]],
    operation_index: dict[tuple[str, str], dict[str, Any]],
) -> dict[str, dict[str, Any]]:
    payload: dict[str, dict[str, Any]] = {}
    for case in cases:
        for step in case["steps"]:
            key = f"{step['surface']}::{step['operation']}"
            opdef = operation_index[(step["surface"], step["operation"])]
            result = opdef["source"]["result"]
            observations = result.get("observations", [])
            comparison = observations[0].get("comparison", {"kind": "exact"}) if observations else {"kind": "exact"}
            payload[key] = {
                "kind": opdef["kind"],
                "shape": result["shape"],
                "comparison": comparison,
            }
    return payload


def run_host(
    cases: list[dict[str, Any]],
    operation_index: dict[tuple[str, str], dict[str, Any]],
    timeout_seconds: int,
    *,
    runner: Path,
    host_name: str,
    assets: dict[str, dict[str, Any]],
) -> tuple[
    dict[str, Any],
    dict[str, dict[str, Any]],
    dict[str, Any] | None,
    dict[str, list[dict[str, Any]]] | None,
]:
    payload = json.dumps(
        js_json_value(
            {
                "cases": cases,
                "operations": operation_payload(cases, operation_index),
                "assets": assets,
            }
        ),
        allow_nan=False,
        separators=(",", ":"),
    )
    process = subprocess.Popen(
        ["node", str(runner)],
        cwd=ROOT,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    try:
        stdout, stderr = process.communicate(input=payload, timeout=timeout_seconds)
    except subprocess.TimeoutExpired as exc:
        process.kill()
        stdout, stderr = process.communicate()
        detail = (stderr or stdout).strip().replace("\n", " ")[-800:]
        raise RuntimeError(f"{host_name} WASM adapter timed out: {detail}") from exc
    if process.returncode != 0:
        details = []
        if stderr.strip():
            details.append(f"stderr: {stderr.strip().replace(chr(10), ' ')[-1200:]}")
        if stdout.strip():
            details.append(f"stdout: {stdout.strip().replace(chr(10), ' ')[-1200:]}")
        detail = " | ".join(details) or "no adapter diagnostics"
        raise RuntimeError(f"{host_name} WASM adapter exited {process.returncode}: {detail}")
    try:
        result = json.loads(stdout)
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"{host_name} WASM adapter emitted malformed JSON") from exc
    if not isinstance(result, dict) or not {"identity", "results"}.issubset(result):
        raise RuntimeError(f"{host_name} WASM adapter emitted an invalid handshake envelope")
    unexpected = set(result) - {"identity", "results", "capabilities", "execution"}
    if unexpected:
        raise RuntimeError(
            f"{host_name} WASM adapter emitted unexpected envelope fields: "
            f"{sorted(unexpected)}"
        )
    results = result["results"]
    if not isinstance(results, list):
        raise RuntimeError(f"{host_name} WASM adapter emitted a non-array result set")
    by_id = {item["case_id"]: item for item in results}
    expected = {case["case_id"] for case in cases}
    if len(by_id) != len(cases) or set(by_id) != expected:
        raise RuntimeError(
            f"{host_name} WASM adapter result IDs/count do not match selected cases"
        )
    capabilities = result.get("capabilities")
    if capabilities is not None and not isinstance(capabilities, dict):
        raise RuntimeError(f"{host_name} WASM adapter capabilities must be an object")
    execution = result.get("execution")
    if execution is not None:
        if not isinstance(execution, dict):
            raise RuntimeError(f"{host_name} WASM adapter execution evidence must be an object")
        for case_id, receipts in execution.items():
            if not isinstance(case_id, str) or not isinstance(receipts, list):
                raise RuntimeError(
                    f"{host_name} WASM adapter execution evidence has invalid case receipts"
                )
    return result["identity"], by_id, capabilities, execution


def run_node(
    cases: list[dict[str, Any]],
    operation_index: dict[tuple[str, str], dict[str, Any]],
    timeout_seconds: int,
    assets: dict[str, dict[str, Any]],
) -> tuple[
    dict[str, Any],
    dict[str, dict[str, Any]],
    dict[str, Any] | None,
    dict[str, list[dict[str, Any]]] | None,
]:
    return run_host(
        cases,
        operation_index,
        timeout_seconds,
        runner=NODE_RUNNER,
        host_name="Node",
        assets=assets,
    )


def run_browser(
    cases: list[dict[str, Any]],
    operation_index: dict[tuple[str, str], dict[str, Any]],
    timeout_seconds: int,
    assets: dict[str, dict[str, Any]],
) -> tuple[
    dict[str, Any],
    dict[str, dict[str, Any]],
    dict[str, Any] | None,
    dict[str, list[dict[str, Any]]] | None,
]:
    return run_host(
        cases,
        operation_index,
        timeout_seconds,
        runner=BROWSER_RUNNER,
        host_name="browser",
        assets=assets,
    )


def write_result(path: Path, result: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def execution_evidence_document(
    cases: list[dict[str, Any]],
    identity: dict[str, Any] | None,
    execution: dict[str, list[dict[str, Any]]],
) -> dict[str, Any]:
    """Summarize WASM receipts collected beside the public parity results."""

    case_ids = sorted(case["case_id"] for case in cases)
    actual_backend_counts: dict[str, int] = {}
    fallback_reason_counts: dict[str, int] = {}
    completed_receipts = 0
    terminal_complete_receipts = 0
    receipt_cases = 0
    not_recorded_cases = 0
    terminal_incomplete_cases = 0
    for case_id in case_ids:
        receipts = execution.get(case_id, [])
        completed = [
            receipt
            for receipt in receipts
            if isinstance(receipt, dict) and receipt.get("status") == "completed"
        ]
        has_receipt = any(
            isinstance(receipt, dict)
            and receipt.get("status") not in {"not_recorded", "not_applicable"}
            for receipt in receipts
        )
        if not has_receipt:
            not_recorded_cases += 1
        else:
            receipt_cases += 1
            completed_receipts += len(completed)
        terminal = [
            receipt
            for receipt in receipts
            if isinstance(receipt, dict) and receipt_terminal_complete(receipt)
        ]
        terminal_complete_receipts += len(terminal)
        if has_receipt and not terminal:
            terminal_incomplete_cases += 1
        for receipt in terminal:
            backend = receipt.get("actual_backend")
            if isinstance(backend, str):
                actual_backend_counts[backend] = (
                    actual_backend_counts.get(backend, 0) + 1
                )
            else:
                # Operation-only telemetry is still useful, but it does not
                # prove which backend produced pixels. Count it explicitly
                # instead of dropping it or guessing CPU/SIMD.
                actual_backend_counts["unattributed"] = (
                    actual_backend_counts.get("unattributed", 0) + 1
                )
            reason = receipt.get("fallback_reason")
            if isinstance(reason, str) and reason:
                fallback_reason_counts[reason] = (
                    fallback_reason_counts.get(reason, 0) + 1
                )

    return {
        "schema": "migration-parity/pipeline-execution-evidence@1",
        "status": "measured" if identity is not None else "not_measured",
        "reason": (
            "The shared Node/browser WASM workflow collected completed receipts "
            "for workflow calls and observations; cases without a receipt did "
            "not materialize a target image pipeline."
            if identity is not None
            else "The WASM facade did not expose pipeline execution telemetry."
        ),
        "identity": identity,
        "scope": {
            "kind": "public-parity-corpus",
            "selected": len(case_ids),
            "case_ids_sha256": case_digest(cases),
        },
        "summary": {
            "selected": len(case_ids),
            "receipt_cases": receipt_cases,
            "not_recorded_cases": not_recorded_cases,
            "completed_receipts": completed_receipts,
            "terminal_complete_receipts": terminal_complete_receipts,
            "terminal_incomplete_cases": terminal_incomplete_cases,
            "actual_backend_counts": dict(sorted(actual_backend_counts.items())),
            "fallback_reason_counts": dict(sorted(fallback_reason_counts.items())),
        },
        "cases": {case_id: execution.get(case_id, []) for case_id in case_ids},
    }


def run(args: argparse.Namespace) -> int:
    output = (
        args.output
        if args.output is not None
        else DEFAULT_BROWSER_OUTPUT if args.host == "browser" else DEFAULT_OUTPUT
    )
    manifest_path = args.manifest.resolve()
    manifest = load_manifest(manifest_path)
    requested = set(args.case_id or [])
    cases, _case_inputs = load_cases(
        manifest,
        case_ids=requested or None,
        surface=args.surface,
    )
    if args.limit is not None:
        cases = cases[: args.limit]
    if not cases:
        raise ValueError("no active parity cases selected")
    if args.chunk_size <= 0:
        raise ValueError("JS/WASM chunk size must be positive")
    operation_index = build_operation_index(manifest)
    diagnostic_hints: collections.Counter[str] = collections.Counter()
    for case in cases:
        reason = compatible_case(case, operation_index)
        if reason is not None:
            diagnostic_hints[reason] += 1

    comparisons: list[dict[str, Any]] = []
    infrastructure_errors: list[dict[str, Any]] = []
    source_identity: dict[str, Any] | None = None
    js_identity: dict[str, Any] | None = None
    host_capabilities: dict[str, Any] | None = None
    target_execution: dict[str, list[dict[str, Any]]] = {}
    execution_identity: dict[str, Any] | None = None
    target_profile = (
        "browser-wasm-core" if args.host == "browser" else "javascript-wasm-core"
    )
    run_target = run_browser if args.host == "browser" else run_node

    # The oracle is independent of the selected WASM host.  Run it once for
    # the selected corpus and reuse its keyed results while target adapter
    # chunks are retried/bisected.  Re-running Pillow for every target chunk
    # made an incremental run look like a stalled browser run and obscured the
    # fact that all selected inputs were being attempted.
    source_results: dict[str, dict[str, Any]] = {}
    try:
        source_identity, source_results = run_side_subprocess(
            "source", manifest_path, cases, args.timeout
        )
    except RuntimeError as exc:
        infrastructure_errors.append(
            {
                "scope": "runner",
                "id": None,
                "kind": "source_failure",
                "message": f"Pillow oracle failed for the selected corpus: {exc}",
            }
        )

    def process_batch(batch: list[dict[str, Any]], start: int, root: int) -> None:
        """Run a batch, bisecting adapter failures to preserve case evidence."""

        nonlocal source_identity, js_identity, host_capabilities
        nonlocal target_execution, execution_identity
        if not batch:
            return
        case_ids = [case["case_id"] for case in batch]
        try:
            target_batch, target_assets = js_asset_payload(batch)
            (
                batch_js_identity,
                js_results,
                batch_capabilities,
                batch_execution,
            ) = run_target(
                target_batch,
                operation_index,
                args.timeout,
                target_assets,
            )
        except RuntimeError as exc:
            if len(batch) > 1:
                midpoint = len(batch) // 2
                process_batch(batch[:midpoint], start, root)
                process_batch(batch[midpoint:], start + midpoint, root)
                return
            infrastructure_errors.append(
                {
                    "scope": "runner",
                    "id": None,
                    "kind": "adapter_failure",
                    "message": (
                        f"JS/WASM chunk {root // args.chunk_size + 1} "
                        f"({len(batch)} case; index={start}; case={case_ids[0]!r}): {exc}"
                    ),
                }
            )
            return
        if js_identity is None:
            js_identity = batch_js_identity
        if host_capabilities is None and batch_capabilities is not None:
            host_capabilities = batch_capabilities
        if execution_identity is None and batch_execution is not None:
            execution_identity = batch_js_identity
        if batch_execution is not None:
            for case_id in case_ids:
                receipts = batch_execution.get(case_id)
                if isinstance(receipts, list):
                    target_execution[case_id] = receipts
        for case in batch:
            outcome, diffs = compare_case(
                case,
                source_results[case["case_id"]],
                js_results[case["case_id"]],
                operation_index,
            )
            comparisons.append(
                {
                    "case_id": case["case_id"],
                    "target_profile": target_profile,
                    "requirements": case.get("covers", []),
                    "source": source_results[case["case_id"]],
                    "target": js_results[case["case_id"]],
                    "outcome": outcome,
                    "diffs": diffs,
                }
            )

    if source_results:
        # Keep RNG-consuming workflows in their original relative order, but
        # restart the host for each complete public case.  This preserves the
        # random sequence within a workflow while preventing an explicitly
        # unsupported strict-backend operation in one case from changing the
        # stream observed by a later case.
        stateful_cases = [case for case in cases if uses_process_global_state(case)]
        ordinary_cases = [case for case in cases if not uses_process_global_state(case)]
        for chunk_start in range(0, len(ordinary_cases), args.chunk_size):
            process_batch(
                ordinary_cases[chunk_start : chunk_start + args.chunk_size],
                chunk_start,
                chunk_start,
            )
        if stateful_cases:
            for case in stateful_cases:
                process_batch([case], 0, 0)

        # Batches above are intentionally scheduled by execution class, but
        # the result contract remains in manifest order.
        case_order = {case["case_id"]: index for index, case in enumerate(cases)}
        comparisons.sort(key=lambda comparison: case_order[comparison["case_id"]])
    passed = sum(1 for comparison in comparisons if comparison["outcome"] == "pass")
    failed = sum(1 for comparison in comparisons if comparison["outcome"] == "fail")
    chunk_failures = len(cases) - len(comparisons)
    if chunk_failures:
        diagnostic_hints["js-adapter-chunk-failure"] += chunk_failures

    summary = {
        "selected": len(cases),
        "executed": len(comparisons),
        "passed": passed,
        "failed": failed,
        "not_run": len(cases) - len(comparisons),
        "infrastructure_errors": len(infrastructure_errors),
    }
    result = {
        "schema": "migration-parity/js-wasm-parity-result@1",
        "status": "completed" if not infrastructure_errors else "infrastructure_failed",
        "started_at": now(),
        "finished_at": now(),
        "identity": {
            "manifest": str(manifest_path.relative_to(ROOT)),
            "source": source_identity,
            "target": js_identity,
        },
        "scope": {
            "kind": "public-parity-corpus",
            "selected": len(cases),
            "executed": len(comparisons),
            "pending": summary["not_run"],
            "pending_definition": "summary.not_run: selected cases without a completed target comparison",
            "case_ids_sha256": case_digest(cases),
            "filter": sorted(requested) if requested else None,
            "diagnostics": {
                "kind": "static-adapter-hints",
                "does_not_filter_or_change_execution": True,
                "not_pending": True,
                "hints": dict(
                    sorted(
                        (reason, count)
                        for reason, count in diagnostic_hints.items()
                        if reason != "js-adapter-chunk-failure"
                    )
                ),
            },
        },
        "summary": summary,
        "comparisons": comparisons,
        "infrastructure_errors": infrastructure_errors,
        "shader_coverage": {
            "status": "not_measured",
            "reason": (
                "The browser WASM package is currently built without the core GPU "
                "feature and the browser adapter reports capability separately; "
                "no WGSL dispatch is claimed."
                if args.host == "browser"
                else "The Node WASM package does not expose a GPU adapter or WGSL "
                "instrumentation; shader coverage remains a separate GPU/WGSL lane."
            ),
        },
    }
    result["capabilities"] = host_capabilities or {
        "webgpu": {
            "api": "not_measured",
            "adapter": "not_measured",
            "device": "not_measured",
            "shader_dispatch": "not_measured",
            "reason": "no WASM workflow batch completed",
        }
    }
    result["execution_evidence"] = execution_evidence_document(
        cases,
        execution_identity,
        target_execution,
    )
    write_result(output.resolve(), result)
    print(json.dumps(summary, sort_keys=True))
    if infrastructure_errors or failed:
        return 1
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--output", type=Path)
    parser.add_argument(
        "--host",
        choices=("node", "browser"),
        default="node",
        help="WASM host to execute: Node or a real browser page",
    )
    parser.add_argument("--case-id", action="append")
    parser.add_argument("--surface")
    parser.add_argument("--limit", type=int)
    parser.add_argument("--timeout", type=int, default=3600)
    parser.add_argument(
        "--chunk-size",
        type=int,
        default=DEFAULT_CHUNK_SIZE,
        help="number of public workflows sent to one host process/page",
    )
    return run(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
