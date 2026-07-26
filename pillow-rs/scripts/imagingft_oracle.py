#!/usr/bin/env python3
"""Execute input-only imagingft cases against pinned Pillow at test runtime."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import PIL
from PIL import Image, ImageDraw, ImageFont
import PIL._imagingft as _imagingft


PILLOW_VERSION = "12.2.0"
FREETYPE_VERSION = "2.14.3"
FIXTURE_ROOT = Path(__file__).resolve().parents[1] / "tests" / "fixtures" / "imagingft"


def image_value(size: tuple[int, int], mode: str, pixels: bytes) -> dict[str, Any]:
    return {
        "type": "image",
        "size": list(size),
        "mode": mode,
        "pixels_hex": pixels.hex(),
    }


def image_with_offset_value(
    size: tuple[int, int], mode: str, pixels: bytes, offset: tuple[int, int]
) -> dict[str, Any]:
    return {
        "type": "image_with_offset",
        "size": list(size),
        "mode": mode,
        "pixels_hex": pixels.hex(),
        "offset": list(offset),
    }


def orientation(value: str | None) -> Image.Transpose | str | None:
    if value is None:
        return None
    if value in Image.Transpose.__members__:
        return Image.Transpose[value]
    return value


def load_font(case: dict[str, Any]) -> ImageFont.FreeTypeFont:
    inputs = case["inputs"]
    params = inputs["params"]
    font = inputs["assets"]["font"]
    size = params.get("size", 10.0)
    if font["kind"] == "load_default":
        return ImageFont.load_default(size)
    if font["kind"] == "ref":
        return ImageFont.truetype(
            FIXTURE_ROOT / font["id"],
            size,
            layout_engine=ImageFont.Layout.BASIC,
        )
    raise ValueError(f"unsupported imagingft fixture font kind: {font['kind']}")


def has_variations(font: ImageFont.FreeTypeFont) -> bool:
    try:
        font.get_variation_axes()
    except OSError:
        return False
    return True


def binary_bbox(font: ImageFont.FreeTypeFont, text: str) -> list[int]:
    size, offset = font.font.getsize(text, "1", None, None, None, None)
    return [offset[0], offset[1], offset[0] + size[0], offset[1] + size[1]]


def binary_rgba(
    font: ImageFont.FreeTypeFont, text: str, fill: list[int]
) -> dict[str, Any]:
    mask = font.getmask(text, mode="1")
    rgba = bytearray(len(mask) * 4)
    for index, coverage in enumerate(bytes(mask)):
        if coverage:
            offset = index * 4
            rgba[offset : offset + 3] = bytes(fill[:3])
            rgba[offset + 3] = coverage
    return image_value(mask.size, "RGBA", bytes(rgba))


def execute(case: dict[str, Any]) -> dict[str, Any]:
    operation = case["operation"].removeprefix("imagingft.")
    params = case["inputs"]["params"]
    font = load_font(case)
    text = params.get("text")

    if operation == "getname":
        return {"type": "name", "value": list(font.getname())}
    if operation == "getmetrics":
        return {"type": "metrics", "value": list(font.getmetrics())}
    if operation == "getlength":
        return {"type": "length", "value": font.getlength(text)}
    if operation == "has_variations":
        return {"type": "bool", "value": has_variations(font)}
    if operation == "getbbox":
        return {"type": "bbox", "value": list(font.getbbox(text))}
    if operation == "getbbox_binary":
        return {"type": "bbox", "value": binary_bbox(font, text)}
    if operation == "getmask":
        mask = font.getmask(text, mode="L")
        return image_value(mask.size, "L", bytes(mask))
    if operation == "getmask2":
        mask, offset = font.getmask2(text, mode="L")
        return image_with_offset_value(mask.size, "L", bytes(mask), offset)
    if operation == "getmask2_with_start":
        mask, offset = font.getmask2(text, mode="L", start=tuple(params["start"]))
        return image_with_offset_value(mask.size, "L", bytes(mask), offset)
    if operation == "get_transposed_mask":
        transposed = ImageFont.TransposedFont(
            font, orientation(params.get("orientation"))
        )
        mask = transposed.getmask(text, mode="L")
        return image_value(mask.size, "L", bytes(mask))
    if operation == "transposed_bbox":
        transposed = ImageFont.TransposedFont(
            font, orientation(params.get("orientation"))
        )
        return {"type": "bbox", "value": list(transposed.getbbox(text))}
    if operation == "validate_transposed_length":
        transposed = ImageFont.TransposedFont(
            font, orientation(params.get("orientation"))
        )
        return {"type": "length", "value": transposed.getlength(text)}
    if operation == "draw_text":
        image = Image.new(
            params["mode"],
            (params["canvas_width"], params["canvas_height"]),
            (0, 0, 0, 0),
        )
        ImageDraw.Draw(image).text(
            tuple(params["xy"]), text, font=font, fill=tuple(params["fill"])
        )
        return image_value(image.size, image.mode, image.tobytes())
    if operation == "render_text_binary":
        return binary_rgba(font, text, params["fill"])
    raise NotImplementedError(f"unsupported imagingft operation: {operation}")


def outcome(case: dict[str, Any]) -> dict[str, Any]:
    try:
        return {"status": "ok", "value": execute(case)}
    except Exception as error:
        return {
            "status": "error",
            "error": {"kind": type(error).__name__, "message": str(error)},
        }


def main() -> None:
    if not hasattr(_imagingft, "getfont"):
        raise RuntimeError(
            "imagingft oracle requires PIL._imagingft C layer (_imagingft.getfont missing)"
        )
    if PIL.__version__ != PILLOW_VERSION:
        raise RuntimeError(f"expected Pillow {PILLOW_VERSION}, got {PIL.__version__}")
    if ImageFont.core.freetype2_version != FREETYPE_VERSION:
        raise RuntimeError(
            f"expected FreeType {FREETYPE_VERSION}, "
            f"got {ImageFont.core.freetype2_version}"
        )
    cases = json.load(sys.stdin)
    results = {case["case_id"]: outcome(case) for case in cases}
    json.dump(results, sys.stdout, separators=(",", ":"), sort_keys=True)


if __name__ == "__main__":
    main()
