#!/usr/bin/env python3
"""Execute input-only imagingft cases against pinned Pillow at test runtime."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[1]
WORKSPACE_ROOT = REPO_ROOT.parent
FIXTURE_ROOT = REPO_ROOT / "tests" / "fixtures" / "imagingft"
REPO_ORACLE_VENV = WORKSPACE_ROOT / ".oracle-venv"


def load_pillow() -> tuple[Any, Any, Any, Any, Any]:
    import PIL
    import os

    venv_root = Path(os.environ.get("VIRTUAL_ENV", "")).resolve()
    if venv_root != REPO_ORACLE_VENV:
        raise RuntimeError(
            f"imagingft oracle must run from repo-local oracle venv: VIRTUAL_ENV={venv_root}"
        )

    python_executable = Path(sys.executable)
    if python_executable != REPO_ORACLE_VENV / "bin" / "python":
        raise RuntimeError(
            f"imagingft oracle must run from {REPO_ORACLE_VENV / 'bin' / 'python'}; got {python_executable}"
        )
    pillow_path = Path(PIL.__file__).resolve() if PIL.__file__ else Path()
    if not str(pillow_path).startswith(str(REPO_ORACLE_VENV.resolve())):
        raise RuntimeError(
            f"imagingft oracle must use Pillow from repo-local .oracle-venv site-packages; got {PIL.__file__}"
        )

    from PIL import Image, ImageDraw, ImageFont
    import PIL._imagingft as _imagingft  # type: ignore

    core = ImageFont.core
    if not getattr(core, "getfont", None):
        raise RuntimeError(
            "Pillow oracle runtime mismatch: PIL.ImageFont.core must provide getfont"
        )
    if core.__name__ not in {"_imagingft", "PIL._imagingft"}:
        raise RuntimeError(
            "Pillow oracle runtime mismatch: PIL.ImageFont.core is not _imagingft"
        )

    return PIL, _imagingft, Image, ImageDraw, ImageFont


def ensure_c_font_path(font: Any) -> None:
    core_font = getattr(font, "font", None)
    if core_font is None:
        raise RuntimeError("imagingft oracle requires an ImageFont object with a native core font")

    if core_font.__class__.__name__ != "Font" or core_font.__class__.__module__ != "builtins":
        raise RuntimeError(
            "imagingft oracle requires Pillow's native C Font object backend"
        )


def load_font(case: dict[str, Any], ImageFont: Any) -> Any:
    inputs = case["inputs"]
    params = inputs["params"]
    font = inputs["assets"]["font"]
    size = params.get("size", 10.0)

    if font["kind"] == "load_default":
        value = ImageFont.load_default(size)
        ensure_c_font_path(value)
        return value

    if font["kind"] == "ref":
        loaded = ImageFont.truetype(
            FIXTURE_ROOT / font["id"],
            size,
            layout_engine=ImageFont.Layout.BASIC,
        )
        ensure_c_font_path(loaded)
        return loaded

    raise ValueError(f"unsupported imagingft fixture font kind: {font['kind']}")


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


def orientation(value: str | None, Image: Any) -> Any:
    if value is None:
        return None
    if value in Image.Transpose.__members__:
        return Image.Transpose[value]
    return value


def has_variations(font: Any) -> bool:
    try:
        font.get_variation_axes()
    except OSError:
        return False
    return True


def binary_bbox(font: Any, text: str) -> list[int]:
    size, offset = font.font.getsize(text, "1", None, None, None, None)
    return [offset[0], offset[1], offset[0] + size[0], offset[1] + size[1]]


def binary_rgba(font: Any, text: str, fill: list[int], ImageFont: Any) -> dict[str, Any]:
    mask = font.getmask(text, mode="1")
    rgba = bytearray(len(mask) * 4)
    for index, coverage in enumerate(bytes(mask)):
        if coverage:
            offset = index * 4
            rgba[offset : offset + 3] = bytes(fill[:3])
            rgba[offset + 3] = coverage
    return image_value(mask.size, "RGBA", bytes(rgba))


def execute(case: dict[str, Any], Image: Any, ImageDraw: Any, ImageFont: Any) -> dict[str, Any]:
    operation = case["operation"].removeprefix("imagingft.")
    params = case["inputs"]["params"]
    font = load_font(case, ImageFont)
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
        transposed = ImageFont.TransposedFont(font, orientation(params.get("orientation"), Image))
        mask = transposed.getmask(text, mode="L")
        return image_value(mask.size, "L", bytes(mask))
    if operation == "transposed_bbox":
        transposed = ImageFont.TransposedFont(font, orientation(params.get("orientation"), Image))
        return {"type": "bbox", "value": list(transposed.getbbox(text))}
    if operation == "validate_transposed_length":
        transposed = ImageFont.TransposedFont(font, orientation(params.get("orientation"), Image))
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
        return binary_rgba(font, text, params["fill"], ImageFont)
    raise NotImplementedError(f"unsupported imagingft operation: {operation}")


def outcome(case: dict[str, Any], modules: tuple[Any, Any, Any, Any]) -> dict[str, Any]:
    _, _, Image, ImageDraw, ImageFont = modules
    try:
        return {"status": "ok", "value": execute(case, Image, ImageDraw, ImageFont)}
    except Exception as error:  # noqa: BLE001
        return {
            "status": "error",
            "error": {"kind": type(error).__name__, "message": str(error)},
        }


def main() -> None:
    PIL, _imagingft, Image, ImageDraw, ImageFont = load_pillow()

    if not hasattr(_imagingft, "getfont"):
        raise RuntimeError(
            "imagingft oracle requires PIL._imagingft C layer (_imagingft.getfont missing)"
        )

    cases = json.load(sys.stdin)
    results = {
        case["case_id"]: outcome(case, (PIL, _imagingft, Image, ImageDraw, ImageFont))
        for case in cases
    }
    json.dump(results, sys.stdout, separators=(",", ":"), sort_keys=True)


if __name__ == "__main__":
    main()
