#!/usr/bin/env python3
"""Execute input-only font probes and report observations, never parity passes.

Public errors are recorded as raised observations. Harness failures abort the
coverage command. Rust-only operations use the test-api example driver rather
than a similarly named Python method. No expected values live in this corpus.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "pillow-rs" / "tests" / "fixtures"
FONT_NATIVE_ROOT = FIXTURE_ROOT / "inputs" / "font-native"
ASSETS = FIXTURE_ROOT / "assets"
RUST_OPERATIONS = {"text_bbox", "getbbox_binary", "render_text_binary"}
LOAD_OPERATIONS = {"truetype", "constructor", "load", "load_path", "load_default", "load_default_imagefont"}
LAYOUT_KEYS = {"mode", "direction", "features", "language"}
MASK_KEYS = LAYOUT_KEYS | {"stroke_width", "anchor", "ink", "start"}
OPERATION_KEYS = {
    **dict.fromkeys(LOAD_OPERATIONS, set()),
    "ImageFont.info": set(),
    **dict.fromkeys([f"{prefix}.{method}" for prefix in ("ImageFont", "TransposedFont")
                    for method in ("getbbox", "getlength", "getmask")], {"args", "kwargs", "mode", "orientation"}),
    "draw_text": {"canvas_height", "canvas_width", "fill", "mode", "xy"},
    "render_text_binary": {"fill", "spacing"},
    "getbbox_binary": set(), "text_bbox": set(), "unsupported_magic": set(),
    "getbbox": LAYOUT_KEYS | {"stroke_width", "anchor"}, "getlength": LAYOUT_KEYS,
    **dict.fromkeys(("getmask", "getmask2", "getmask2_with_start"), MASK_KEYS | {"args", "kwargs"}),
    **dict.fromkeys(("getmetrics", "getname", "font_size", "has_variations", "get_variation_axes",
                    "native_getvaraxes", "get_variation_names", "native_getvarnames",
                    "native_getlength_26dot6", "native_getsize", "native_face_attrs"), set()),
    "font_variant": {"variant_size", "variant_index", "variant_encoding", "variant_layout_engine"},
    **dict.fromkeys(("set_variation_by_axes", "native_setvaraxes"), {"axes"}),
    "set_variation_by_name": {"name", "name_bytes_hex", "repeat_count"},
    "native_setvarname": {"instance_index"},
    "native_render": MASK_KEYS | {"stroke_filled"},
    **dict.fromkeys(("get_transposed_mask", "transposed_bbox", "validate_transposed_length"), {"orientation"}),
}


class ApiRaised(Exception):
    """An observed error from an API call, without any claim of correctness."""

    def __init__(self, error_type: str, message: str, stage: str):
        super().__init__(message)
        self.observation = {"type": error_type, "message": message, "stage": stage}


def call(method, *args, **kwargs):
    # Resolve methods and validate harness inputs before this boundary. Missing
    # methods, imports, process failures and coding errors must never pass here.
    try:
        return method(*args, **kwargs)
    except (ValueError, TypeError, OSError, KeyError, SystemError, OverflowError, SyntaxError) as error:
        raise ApiRaised(type(error).__name__, str(error), method.__name__) from error


def asset_path(asset: dict[str, Any], *, assets_root: Path = ASSETS) -> Path | None:
    asset_id = asset.get("id", "")
    for prefix, directory in (("input/fonts/", "fonts"), ("input/pilfont/", "pilfont")):
        if asset_id.startswith(prefix):
            return assets_root / "font" / directory / asset_id.removeprefix(prefix)
    return None


def required_asset(asset):
    path = asset_path(asset)
    if path is None:
        raise ValueError(f"unrecognized font asset: {asset!r}")
    return str(path)


def load_font(params: dict[str, Any], assets: dict[str, Any]) -> Any:
    from PIL import ImageFont

    asset = assets["font"]
    size = params.get("size", 20)
    if asset.get("kind") == "load_default":
        return call(ImageFont.load_default, size)
    if asset.get("kind") == "pilfont_default":
        return call(ImageFont.load_default_imagefont)
    path = required_asset(asset)
    if asset.get("kind") == "pilfont_ref":
        loader = params.get("loader", "load")
        if loader not in {"load", "load_path"}:
            raise ValueError(f"unrecognized bitmap font loader: {loader}")
        return call(getattr(ImageFont, loader), path)
    return call(ImageFont.truetype, path, size, index=params.get("index", 0),
                encoding=params.get("encoding", ""), layout_engine=params.get("layout_engine"))


def case_text(params: dict[str, Any]) -> str | bytes:
    text = bytes.fromhex(params["text_bytes_hex"]) if "text_bytes_hex" in params else params.get("text", "Hello")
    return text * params.get("text_repeat", 1) if isinstance(text, (str, bytes)) else text


def options(params, keys):
    result = {key: params[key] for key in keys if key in params}
    # A JSON array represents the native tuple, not a deliberate Python list.
    if isinstance(result.get("start"), list):
        result["start"] = tuple(result["start"])
    result.update(params.get("kwargs", {}))
    return result


def observe(value):
    """Stable, complete observations; large buffers retain size and SHA-256."""
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if isinstance(value, bytes):
        return {"bytes": len(value), "sha256": hashlib.sha256(value).hexdigest()}
    if isinstance(value, (tuple, list)):
        return [observe(item) for item in value]
    if isinstance(value, dict):
        return {str(key): observe(item) for key, item in value.items()}
    if hasattr(value, "tobytes_unpacked"):
        return {"size": observe(value.size), "mode": value.mode, "pixels": observe(bytes(value.tobytes_unpacked()))}
    if hasattr(value, "tobytes"):
        return {"size": observe(value.size), "mode": value.mode, "pixels": observe(value.tobytes())}
    if hasattr(value, "getname"):
        return {"size": value.size, "name": observe(call(value.getname))}
    if hasattr(value, "info"):
        return observe(value.info)
    raise TypeError(f"no observation adapter for {type(value).__name__}")


def run_rust_case(operation, params, assets):
    driver = Path(os.environ.get("MIGRATION_FONT_NATIVE_DRIVER", ROOT / "target/debug/examples/font_native_coverage"))
    if not driver.is_file():
        raise FileNotFoundError("build the font-native driver with make migration-parity-font-native-build")
    asset = assets["font"]
    source = "load_default" if asset.get("kind") == "load_default" else required_asset(asset)
    text = case_text(params)
    text_kind = "bytes" if isinstance(text, bytes) else "utf8"
    result = subprocess.run(
        [str(driver), operation, source, str(params.get("size", 20)), text_kind,
         ",".join(map(str, params.get("fill", [255, 255, 255, 255]))), str(params.get("spacing", 0))],
        input=text if isinstance(text, bytes) else text.encode("utf-8"), capture_output=True, check=True,
    )
    status, separator, value = result.stdout.decode("utf-8").rstrip("\n").partition("\t")
    if not separator or status not in {"returned", "raised"}:
        raise RuntimeError("invalid font-native driver response")
    if status == "raised":
        raise ApiRaised("PilError", value, operation)
    return value


def run_case(case: dict[str, Any]):
    from pillow_rs import _core
    from PIL import Image, ImageDraw, ImageFont

    operation = case["operation"].removeprefix("font.")
    params = case["inputs"].get("params", {})
    assets = case["inputs"]["assets"]
    if operation not in OPERATION_KEYS:
        raise ValueError(f"unknown font operation: {operation}")
    unknown = set(params) - OPERATION_KEYS[operation] - {
        "size", "index", "encoding", "layout_engine", "loader", "text", "text_bytes_hex", "text_repeat"}
    if unknown:
        raise ValueError(f"unhandled parameters for {operation}: {sorted(unknown)}")
    if set(assets) - {"font", "variant_font"}:
        raise ValueError(f"unhandled assets: {sorted(assets)}")
    if operation in RUST_OPERATIONS:
        return run_rust_case(operation, params, assets)
    text = case_text(params)
    font = load_font(params, assets)
    if operation in LOAD_OPERATIONS:
        return observe(font)
    if operation.startswith(("ImageFont.", "TransposedFont.")):
        method = operation.split(".")[1]
        if operation.startswith("TransposedFont."):
            font = call(ImageFont.TransposedFont, font, orientation=params.get("orientation"))
        if method == "info":
            return observe(font.info)
        args = params.get("args", [])
        # Bitmap getmask's mode precedes its deliberately ignored extra args.
        if method == "getmask":
            return observe(call(font.getmask, text, params.get("mode", ""), *args, **params.get("kwargs", {})))
        return observe(call(getattr(font, method), text, *args, **params.get("kwargs", {})))
    if operation == "draw_text":
        canvas = call(Image.new, params.get("mode", "RGBA"), (params["canvas_width"], params["canvas_height"]))
        draw = call(ImageDraw.Draw, canvas)
        call(draw.text, tuple(params["xy"]), text, font=font, fill=tuple(params["fill"]))
        return observe(canvas)
    if operation == "unsupported_magic":
        return observe(call(ImageFont.truetype, str(ASSETS / "font/pilfont/courb08.png"), 20))
    if operation in {"getbbox", "getlength", "getmask", "getmask2", "getmask2_with_start"}:
        method = "getmask2" if operation == "getmask2_with_start" else operation
        keys = (MASK_KEYS if "getmask" in operation else
                LAYOUT_KEYS | {"stroke_width", "anchor"} if operation == "getbbox" else LAYOUT_KEYS)
        return observe(call(getattr(font, method), text, *params.get("args", []), **options(params, keys)))
    if operation in {"getmetrics", "getname", "get_variation_axes", "get_variation_names"}:
        return observe(call(getattr(font, operation)))
    if operation == "font_size":
        return font.size
    if operation == "font_variant":
        overrides = {key.removeprefix("variant_"): value for key, value in params.items() if key.startswith("variant_")}
        if "variant_font" in assets:
            overrides["font"] = required_asset(assets["variant_font"])
        return observe(call(font.font_variant, **overrides))
    if operation == "set_variation_by_name":
        name = bytes.fromhex(params["name_bytes_hex"]) if "name_bytes_hex" in params else params.get("name", "Bold")
        for _ in range(params.get("repeat_count", 1)):
            call(font.set_variation_by_name, name)
        return observe(call(font.getbbox, text))
    if operation in {"set_variation_by_axes", "native_setvaraxes", "native_setvarname"}:
        if operation == "set_variation_by_axes":
            call(font.set_variation_by_axes, params.get("axes", [100.0]))
        elif operation == "native_setvaraxes":
            call(font._rust_font.setvaraxes, params.get("axes", [100.0]))
        else:
            call(font._rust_font.setvarname, params.get("instance_index", 0))
        return observe(call(font.getbbox, text))
    rust = font._rust_font
    if operation in {"has_variations", "native_getvaraxes", "native_getvarnames"}:
        return observe(call(getattr(rust, operation.removeprefix("native_"))))
    if operation == "native_getlength_26dot6":
        return observe(call(rust.getlength, text))
    if operation == "native_getsize":
        return observe(call(rust.getsize, text))
    if operation == "native_render":
        return observe(call(rust.render_with_options, text, **options(params, MASK_KEYS | {"stroke_filled"})))
    if operation == "native_face_attrs":
        return {name: observe(getattr(rust, name)) for name in
                ("family", "style", "ascent", "descent", "height", "x_ppem", "y_ppem", "glyphs", "font_format", "is_scalable")}
    orientation = call(_core.transposed_font_orientation, params.get("orientation"))
    if operation == "get_transposed_mask":
        return observe(call(rust.get_transposed_mask_image, text, orientation))
    if operation == "transposed_bbox":
        return observe(call(_core.transposed_font_bbox, call(font.getbbox, text), orientation))
    if operation == "validate_transposed_length":
        return observe(call(_core.validate_transposed_font_length, orientation))
    raise ValueError(f"unhandled font operation: {operation}")


def require_target():
    import PIL
    from pillow_rs import _core

    if Path(PIL.__file__).resolve() != ROOT / "pillow-rs-py/python/PIL/__init__.py":
        raise RuntimeError("font coverage requires the checkout PIL; use make migration-parity-font-native-coverage")
    if not Path(_core.__file__).resolve().is_relative_to(ROOT / "pillow-rs-py/python/pillow_rs"):
        raise RuntimeError("font coverage requires the checkout extension")


def run_native_cases() -> dict[str, Any]:
    require_target()
    from run_migration_parity import configure_target_backend

    configure_target_backend()
    results, seen = [], set()
    for path in sorted(FONT_NATIVE_ROOT.glob("*.json")):
        for case in json.loads(path.read_text(encoding="utf-8"))["cases"]:
            case_id = case["case_id"]
            if case_id in seen:
                raise ValueError(f"duplicate font case ID: {case_id}")
            seen.add(case_id)
            record = {"case_id": case_id, "input": str(path.relative_to(FIXTURE_ROOT)), "operation": case["operation"]}
            try:
                record.update(status="returned", observation=run_case(case))
            except ApiRaised as error:
                record.update(status="raised", observation=error.observation)
            except Exception as error:
                record.update(status="failed", error={"type": type(error).__name__, "message": str(error)})
            results.append(record)
    counts = {status: sum(item["status"] == status for item in results) for status in ("returned", "raised", "failed")}
    return {"coverage_only": True, "parity_verified": False, "total": len(results), **counts, "cases": results}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = run_native_cases()
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({key: value for key, value in result.items() if key != "cases"}, sort_keys=True))
    for case in result["cases"]:
        if case["status"] == "failed":
            print(json.dumps(case, sort_keys=True))
    return 1 if result["failed"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
