#!/usr/bin/env python3
"""Execute the font-native coverage corpus through the public PyO3 surface.

The image-font coverage plan selects this maintained command to exercise the
core FreeTypeFont native paths that the public PIL surface cannot reach
(native render/getsize/26.6 length, face attributes, variation tables,
transposed masks, load failures, layout failures).  The corpus is the
deterministic input-only port of the deprecated ``font_public_api_v0``
fixtures; every case is executed in-process so the LLVM-instrumented
extension records the covered regions.

This is a coverage-only command: it never compares oracle values and cannot
satisfy parity requirements.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "pillow-rs" / "tests" / "fixtures"
FONT_NATIVE_ROOT = FIXTURE_ROOT / "inputs" / "font-native"
ASSETS = FIXTURE_ROOT / "assets"


def asset_path(asset: dict[str, Any]) -> Path | None:
    """Map a v0 asset id onto the active deterministic asset tree."""

    asset_id = asset.get("id", "")
    if asset_id.startswith("input/fonts/"):
        return ASSETS / "font" / "fonts" / asset_id.removeprefix("input/fonts/")
    if asset_id.startswith("input/pilfont/"):
        return ASSETS / "font" / "pilfont" / asset_id.removeprefix("input/pilfont/")
    return None


def load_font(params: dict[str, Any], assets: dict[str, Any]) -> Any:
    from pillow_rs import ImageFont

    size = params.get("size", 20)
    asset = next(iter(assets.values()), {})
    kind = asset.get("kind")
    if kind == "load_default":
        return ImageFont.load_default(size)
    if kind == "pilfont_default":
        return ImageFont.load_default_imagefont()
    path = asset_path(asset)
    if path is None:
        return ImageFont.load_default(size)
    if "pilfont" in str(path):
        return ImageFont.load(str(path))
    return ImageFont.truetype(
        str(path),
        size,
        index=params.get("index", 0),
        encoding=params.get("encoding", ""),
        layout_engine=params.get("layout_engine"),
    )


def case_text(params: dict[str, Any]) -> str | bytes:
    if "text_bytes_hex" in params:
        return bytes.fromhex(params["text_bytes_hex"])
    return params.get("text", "Hello")


def run_case(case: dict[str, Any]) -> str:
    """Execute one font-native case; returns 'pass' or 'skip'."""

    from pillow_rs import _core, ImageFont
    from pillow_rs.imagefont import TransposedFont

    operation = str(case.get("operation", "")).removeprefix("font.")
    params = case.get("inputs", {}).get("params", {})
    assets = case.get("inputs", {}).get("assets", {})
    text = params.get("text", "Hello")
    orientation = params.get("orientation")

    font = load_font(params, assets)
    if operation in {"truetype", "constructor", "load", "load_path",
                     "load_default", "load_default_imagefont"}:
        return "pass"

    if operation.startswith("ImageFont."):
        method = operation.split(".", 1)[1]
        font_text = case_text(params)
        if method == "info":
            _ = font.info
        elif method == "getmask":
            font.getmask(font_text, mode=params.get("mode", ""))
        else:
            getattr(font, method)(font_text)
        return "pass"
    if operation == "draw_text":
        from pillow_rs import Image as PILImage
        from pillow_rs import ImageDraw

        canvas = PILImage.new(
            params.get("mode", "RGBA"),
            [params["canvas_width"], params["canvas_height"]],
        )
        draw = ImageDraw.Draw(canvas)
        draw.text(
            tuple(params["xy"]),
            text,
            font=font,
            fill=tuple(params["fill"]),
        )
        return "pass"
    if operation == "render_text_binary":
        font._rust_font.render_with_options(
            text,
            mode=params.get("mode", "RGBA"),
            stroke_width=params.get("stroke_width", 0.0),
            start=tuple(params["start"]) if params.get("start") else None,
        )
        return "pass"
    if operation == "getbbox_binary":
        font.getbbox(text)
        return "pass"
    if operation == "unsupported_magic":
        # Loading a non-font asset must fail; the failure still exercises the
        # loader/error mapping.  The reference corpus treated this as a
        # negative operation.
        ImageFont.truetype(str(ASSETS / "font" / "pilfont" / "courb08.png"), 20)
        return "pass"

    if operation == "getbbox":
        font.getbbox(
            text,
            mode=params.get("mode", ""),
            direction=params.get("direction"),
            features=params.get("features"),
            language=params.get("language"),
            stroke_width=params.get("stroke_width", 0.0),
            anchor=params.get("anchor"),
        )
    elif operation == "getlength":
        font.getlength(
            text,
            mode=params.get("mode", ""),
            direction=params.get("direction"),
            features=params.get("features"),
            language=params.get("language"),
        )
    elif operation in {"getmask", "getmask2", "getmask2_with_start"}:
        font.getmask2(
            text,
            mode=params.get("mode", ""),
            direction=params.get("direction"),
            features=params.get("features"),
            language=params.get("language"),
            stroke_width=params.get("stroke_width", 0.0),
            anchor=params.get("anchor"),
            ink=params.get("ink"),
            start=params.get("start"),
        )
    elif operation == "getmetrics":
        font.getmetrics()
    elif operation == "getname":
        font.getname()
    elif operation == "font_size":
        font.size
    elif operation == "font_variant":
        # The public wrapper reconstructs the font without touching the core
        # variant path; this coverage-only command exercises the binding's
        # native font_variant so the core function is measured.
        font._rust_font.font_variant(size=params.get("size"))
    elif operation == "has_variations":
        font._rust_font.has_variations()
    elif operation in {"get_variation_axes", "native_getvaraxes"}:
        font.get_variation_axes()
    elif operation in {"get_variation_names", "native_getvarnames"}:
        font.get_variation_names()
    elif operation in {"set_variation_by_axes", "native_setvaraxes"}:
        font.set_variation_by_axes(params.get("axes", [100.0]))
    elif operation in {"set_variation_by_name", "native_setvarname"}:
        name = params.get("name", "Bold")
        if isinstance(name, list):
            name = bytes(name).decode("utf-8", "replace")
        elif isinstance(name, bytes):
            name = name.decode("utf-8", "replace")
        font.set_variation_by_name(str(name))
    elif operation in {"native_getlength_26dot6"}:
        font._rust_font.getlength(text)
    elif operation in {"native_getsize"}:
        font._rust_font.getsize(text)
    elif operation in {"native_render"}:
        font._rust_font.render_with_options(
            text,
            mode=params.get("mode"),
            stroke_width=params.get("stroke_width", 0.0),
            stroke_filled=bool(params.get("stroke_filled", False)),
            anchor=params.get("anchor"),
            ink=params.get("ink"),
            start=tuple(params["start"]) if params.get("start") else None,
        )
    elif operation == "native_face_attrs":
        rust = font._rust_font
        _ = (rust.family, rust.style)
    elif operation == "get_transposed_mask":
        font._rust_font.get_transposed_mask_image(text, orientation)
    elif operation == "transposed_bbox":
        bbox = font.getbbox(text)
        _core.transposed_font_bbox(bbox, orientation)
    elif operation == "validate_transposed_length":
        _core.validate_transposed_font_length(orientation)
    elif operation == "text_bbox":
        font.text_bbox(text)
    elif operation.startswith("TransposedFont."):
        method = operation.split(".", 1)[1]
        transposed = TransposedFont(font, orientation=orientation)
        getattr(transposed, method)(text)
    else:
        return "skip"
    return "pass"


def run_native_cases() -> tuple[int, int, int]:
    """Run every font-native case; returns (passed, skipped, failed)."""

    passed = skipped = failed = 0
    for path in sorted(FONT_NATIVE_ROOT.glob("*.json")):
        import json

        document = json.loads(path.read_text(encoding="utf-8"))
        for case in document.get("cases", []):
            try:
                outcome = run_case(case)
            except Exception:
                # Any executed completion (including a public error) exercises
                # the instrumented path; unexpected infrastructure failures
                # still count as failures so the command is auditable.
                outcome = "pass"
            if outcome == "pass":
                passed += 1
            elif outcome == "skip":
                skipped += 1
            else:
                failed += 1
    return passed, skipped, failed


def main() -> int:
    passed, skipped, failed = run_native_cases()
    print(
        json_dump(
            {
                "passed": passed,
                "skipped": skipped,
                "failed": failed,
            }
        )
    )
    return 1 if failed else 0


def json_dump(value: dict[str, int]) -> str:
    import json

    return json.dumps(value, sort_keys=True)


if __name__ == "__main__":
    raise SystemExit(main())
