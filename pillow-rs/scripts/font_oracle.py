#!/usr/bin/env python3
"""Execute input-only font cases against pinned Pillow at test runtime."""

from __future__ import annotations

import json
import inspect
import sys
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[1]
WORKSPACE_ROOT = REPO_ROOT.parent
FIXTURE_ROOT = REPO_ROOT / "tests" / "fixtures" / "font"
REPO_ORACLE_VENV = WORKSPACE_ROOT / ".oracle-venv"
EXPECTED_PILLOW_VERSION = "12.2.0"


def load_pillow() -> tuple[Any, Any, Any, Any, Any]:
    import PIL
    import os

    if PIL.__version__ != EXPECTED_PILLOW_VERSION:
        raise RuntimeError(
            f"font oracle must use Pillow {EXPECTED_PILLOW_VERSION}; got {PIL.__version__}"
        )

    venv_root = Path(os.environ.get("VIRTUAL_ENV", "")).resolve()
    if venv_root != REPO_ORACLE_VENV:
        raise RuntimeError(
            f"font oracle must run from repo-local oracle venv: VIRTUAL_ENV={venv_root}"
        )

    python_executable = Path(sys.executable)
    if python_executable != REPO_ORACLE_VENV / "bin" / "python":
        raise RuntimeError(
            f"font oracle must run from {REPO_ORACLE_VENV / 'bin' / 'python'}; got {python_executable}"
        )
    pillow_path = Path(PIL.__file__).resolve() if PIL.__file__ else Path()
    if not str(pillow_path).startswith(str(REPO_ORACLE_VENV.resolve())):
        raise RuntimeError(
            f"font oracle must use Pillow from repo-local .oracle-venv site-packages; got {PIL.__file__}"
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

    core_file = Path(getattr(_imagingft, "__file__", "")).resolve()
    if not core_file.is_file() or core_file.suffix not in {".so", ".dylib", ".dll", ".pyd"}:
        raise RuntimeError(
            f"font oracle runtime mismatch: PIL._imagingft must be a native extension; got {core_file}"
        )

    assert_python_calls_into_c_layer(ImageFont)
    return PIL, _imagingft, Image, ImageDraw, ImageFont


def assert_python_calls_into_c_layer(ImageFont: Any) -> None:
    """Prove that the public Python methods we test are thin wrappers over C calls."""
    font_methods = {
        "FreeTypeFont.getmask2": ("self.font.render",),
        "FreeTypeFont.getmask": ("self.getmask2(",),
        "FreeTypeFont.getbbox": ("self.font.getsize",),
        "FreeTypeFont.getlength": ("self.font.getlength",),
        "FreeTypeFont.getname": ("self.font.family", "self.font.style"),
        "FreeTypeFont.get_variation_axes": ("self.font.getvaraxes",),
    }
    transposed_methods = {
        "TransposedFont.getmask": ("self.font.getmask(", "im.transpose("),
        "TransposedFont.getbbox": ("self.font.getbbox(",),
        "TransposedFont.getlength": ("self.font.getlength(",),
    }

    for target, expected in font_methods.items():
        name = target.split(".", 1)[1]
        source = inspect.getsource(getattr(ImageFont.FreeTypeFont, name))
        if not any(needle in source for needle in expected):
            raise RuntimeError(
                f"font oracle invariant broken: {target} is not"
                f" delegated to PIL._imagingft C layer"
            )

    for target, expected in transposed_methods.items():
        cls_name, name = target.split(".", 1)
        source = inspect.getsource(getattr(getattr(ImageFont, cls_name), name))
        if not any(needle in source for needle in expected):
            raise RuntimeError(
                f"font oracle invariant broken: {target} is not"
                f" delegating correctly into Pillow font core path"
            )


def ensure_c_font_path(font: Any) -> None:
    core_font = getattr(font, "font", None)
    if core_font is None:
        raise RuntimeError("font oracle requires an ImageFont object with a native core font")

    if core_font.__class__.__name__ != "Font" or core_font.__class__.__module__ != "builtins":
        raise RuntimeError(
            "font oracle requires Pillow's native C Font object backend"
        )


def load_bitmap_imagefont(case: dict[str, Any], ImageFont: Any) -> Any:
    inputs = case["inputs"]
    params = inputs["params"]
    font = inputs["assets"]["font"]

    if font["kind"] == "pilfont_default":
        return ImageFont.load_default_imagefont()
    if font["kind"] == "pilfont_ref":
        filename = FIXTURE_ROOT / font["id"]
        if params.get("loader") == "load_path":
            return ImageFont.load_path(str(filename))
        return ImageFont.load(str(filename))

    raise ValueError(f"unsupported bitmap ImageFont fixture font kind: {font['kind']}")


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
        kwargs: dict[str, Any] = {"layout_engine": ImageFont.Layout.BASIC}
        if "index" in params:
            kwargs["index"] = params["index"]
        if "encoding" in params:
            kwargs["encoding"] = params["encoding"]
        if "layout_engine" in params:
            kwargs["layout_engine"] = layout_engine(params["layout_engine"], ImageFont)
        loaded = ImageFont.truetype(
            FIXTURE_ROOT / font["id"],
            size,
            **kwargs,
        )
        ensure_c_font_path(loaded)
        return loaded

    raise ValueError(f"unsupported font fixture font kind: {font['kind']}")


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


def layout_engine(value: str | None, ImageFont: Any) -> Any:
    if value is None:
        return None
    if value in ImageFont.Layout.__members__:
        return ImageFont.Layout[value]
    return value


def has_variations(font: Any) -> bool:
    try:
        font.get_variation_axes()
    except OSError:
        return False
    return True


def font_descriptor(font: Any) -> dict[str, Any]:
    return {
        "type": "font",
        "size": float(font.size),
        "name": list(font.getname()),
        "metrics": list(font.getmetrics()),
        "has_variations": has_variations(font),
    }


def native_face_attrs_value(font: Any) -> dict[str, Any]:
    return {
        "type": "native_face_attrs",
        "value": {
            "family": font.font.family,
            "style": font.font.style,
            "ascent": font.font.ascent,
            "descent": font.font.descent,
            "height": font.font.height,
            "x_ppem": font.font.x_ppem,
            "y_ppem": font.font.y_ppem,
            "glyphs": font.font.glyphs,
        },
    }


def bytes_hex(value: bytes) -> str:
    return value.replace(b"\x00", b"").hex()


def variation_axes_value(font: Any) -> dict[str, Any]:
    return {
        "type": "variation_axes",
        "value": [
            {
                "minimum": axis["minimum"],
                "default": axis["default"],
                "maximum": axis["maximum"],
                "name_hex": bytes_hex(axis["name"]),
            }
            for axis in font.get_variation_axes()
        ],
    }


def native_variation_axes_value(font: Any) -> dict[str, Any]:
    return {
        "type": "variation_axes",
        "value": [
            {
                "minimum": axis["minimum"],
                "default": axis["default"],
                "maximum": axis["maximum"],
                "name_hex": axis["name"].hex(),
            }
            for axis in font.font.getvaraxes()
        ],
    }


def variation_names_value(font: Any) -> dict[str, Any]:
    return {
        "type": "variation_names",
        "value": [bytes_hex(name) for name in font.get_variation_names()],
    }


def native_variation_names_value(font: Any) -> dict[str, Any]:
    return {
        "type": "variation_names",
        "value": [name.hex() for name in font.font.getvarnames()],
    }


def text_bbox_value(font: Any, text: str) -> dict[str, Any]:
    left, top, right, bottom = font.getbbox(text)
    return {
        "type": "size",
        "value": [max(0, right - left), max(0, bottom - top)],
    }


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


def text_kwargs(params: dict[str, Any], include_stroke: bool = False) -> dict[str, Any]:
    keys = ["mode", "direction", "features", "language", "anchor"]
    if include_stroke:
        keys.extend(["stroke_width", "ink", "start"])
    return {key: params[key] for key in keys if key in params}


def getmask2_call_args(params: dict[str, Any]) -> tuple[list[Any], dict[str, Any]]:
    args = list(params.get("args", []))
    kwargs = text_kwargs(params, include_stroke=True)
    kwargs.update(params.get("kwargs", {}))
    if "mode" not in kwargs and not args:
        kwargs["mode"] = "L"
    return args, kwargs


def bitmap_extra_call_args(params: dict[str, Any]) -> tuple[list[Any], dict[str, Any]]:
    """Return Pillow-compatible ignored bitmap ImageFont variadic arguments."""
    return list(params.get("args", [])), dict(params.get("kwargs", {}))


def text_value(params: dict[str, Any]) -> str | bytes | None:
    if "text_bytes_hex" in params:
        return bytes.fromhex(params["text_bytes_hex"]) * params.get("text_repeat", 1)
    text = params.get("text")
    if isinstance(text, str):
        return text * params.get("text_repeat", 1)
    return text


def execute(case: dict[str, Any], Image: Any, ImageDraw: Any, ImageFont: Any) -> dict[str, Any]:
    operation = case["operation"].removeprefix("font.")
    params = case["inputs"]["params"]

    if operation in {"load", "load_path", "load_default_imagefont"}:
        font = load_bitmap_imagefont(case, ImageFont)
        return {"type": "pilfont", "mode": font.getmask("").mode}
    if operation in {
        "ImageFont.info",
        "ImageFont.getbbox",
        "ImageFont.getlength",
        "ImageFont.getmask",
        "TransposedFont.getbbox",
        "TransposedFont.getlength",
        "TransposedFont.getmask",
    }:
        font = load_bitmap_imagefont(case, ImageFont)
        text = text_value(params)
        if operation == "ImageFont.info":
            return {"type": "info", "value": [line.hex() for line in font.info]}
        if operation.startswith("TransposedFont."):
            font = ImageFont.TransposedFont(
                font, orientation(params.get("orientation"), Image)
            )
        args, kwargs = bitmap_extra_call_args(params)
        if operation == "ImageFont.getbbox":
            return {"type": "bbox", "value": list(font.getbbox(text, *args, **kwargs))}
        if operation == "TransposedFont.getbbox":
            return {"type": "bbox", "value": list(font.getbbox(text, *args, **kwargs))}
        if operation == "ImageFont.getlength":
            return {"type": "length", "value": font.getlength(text, *args, **kwargs)}
        if operation == "TransposedFont.getlength":
            return {"type": "length", "value": font.getlength(text, *args, **kwargs)}
        mask = font.getmask(text, params.get("mode", ""), *args, **kwargs)
        return image_value(mask.size, mask.mode, bytes(mask))

    font = load_font(case, ImageFont)
    text = text_value(params)

    if operation in {"load_default", "truetype"}:
        return font_descriptor(font)
    if operation == "font_size":
        return {"type": "size", "value": float(font.size)}
    if operation == "text_bbox":
        return text_bbox_value(font, text)
    if operation == "getname":
        return {"type": "name", "value": list(font.getname())}
    if operation == "getmetrics":
        return {"type": "metrics", "value": list(font.getmetrics())}
    if operation == "native_face_attrs":
        return native_face_attrs_value(font)
    if operation == "native_getlength_26dot6":
        return {"type": "native_length_26dot6", "value": font.font.getlength(text)}
    if operation == "native_getsize":
        size, offset = font.font.getsize(text)
        return {
            "type": "native_size",
            "size": list(size),
            "offset": list(offset),
        }
    if operation == "native_getvaraxes":
        return native_variation_axes_value(font)
    if operation == "native_getvarnames":
        return native_variation_names_value(font)
    if operation == "native_setvarname":
        font.font.setvarname(params["instance_index"])
        return {
            "type": "font_after_variation",
            "name": list(font.getname()),
            "length": font.getlength(text),
        }
    if operation == "native_setvaraxes":
        font.font.setvaraxes(params["axes"])
        return {
            "type": "font_after_variation",
            "name": list(font.getname()),
            "length": font.getlength(text),
        }
    if operation == "getlength":
        return {"type": "length", "value": font.getlength(text, **text_kwargs(params))}
    if operation == "has_variations":
        return {"type": "bool", "value": has_variations(font)}
    if operation == "get_variation_axes":
        return variation_axes_value(font)
    if operation == "get_variation_names":
        return variation_names_value(font)
    if operation == "set_variation_by_name":
        name = (
            bytes.fromhex(params["name_bytes_hex"])
            if "name_bytes_hex" in params
            else params["name"]
        )
        repeat_count = params.get("repeat_count", 1)
        for _ in range(repeat_count):
            font.set_variation_by_name(name)
        return {
            "type": "font_after_variation",
            "name": list(font.getname()),
            "length": font.getlength(text),
        }
    if operation == "set_variation_by_axes":
        font.set_variation_by_axes(params["axes"])
        return {
            "type": "font_after_variation",
            "name": list(font.getname()),
            "length": font.getlength(text),
        }
    if operation == "font_variant":
        kwargs: dict[str, Any] = {"size": params.get("variant_size")}
        variant_font = case["inputs"].get("assets", {}).get("variant_font")
        if variant_font is not None:
            kwargs["font"] = FIXTURE_ROOT / variant_font["id"]
        if "variant_index" in params:
            kwargs["index"] = params["variant_index"]
        if "variant_encoding" in params:
            kwargs["encoding"] = params["variant_encoding"]
        if "variant_layout_engine" in params:
            kwargs["layout_engine"] = layout_engine(
                params["variant_layout_engine"], ImageFont
            )
        variant = font.font_variant(**kwargs)
        return font_descriptor(variant)
    if operation == "getbbox":
        return {
            "type": "bbox",
            "value": list(font.getbbox(text, **text_kwargs(params, include_stroke=True))),
        }
    if operation == "getbbox_binary":
        return {"type": "bbox", "value": binary_bbox(font, text)}
    if operation == "getmask":
        kwargs = text_kwargs(params, include_stroke=True)
        if "mode" not in kwargs:
            kwargs["mode"] = "L"
        mask = font.getmask(text, **kwargs)
        return image_value(mask.size, "L", bytes(mask))
    if operation == "getmask2":
        args, kwargs = getmask2_call_args(params)
        mask, offset = font.getmask2(text, *args, **kwargs)
        return image_with_offset_value(mask.size, "L", bytes(mask), offset)
    if operation == "getmask2_with_start":
        kwargs = text_kwargs(params, include_stroke=True)
        kwargs["mode"] = kwargs.get("mode", "L")
        kwargs["start"] = tuple(params["start"])
        mask, offset = font.getmask2(text, **kwargs)
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
    raise NotImplementedError(f"unsupported font operation: {operation}")


def outcome(case: dict[str, Any], modules: tuple[Any, Any, Any, Any]) -> dict[str, Any]:
    _, _, Image, ImageDraw, ImageFont = modules
    try:
        return {"status": "ok", "value": execute(case, Image, ImageDraw, ImageFont)}
    except Exception as error:  # noqa: BLE001
        return {
            "status": "error",
            "error": {"kind": type(error).__name__, "message": str(error)},
        }


def public_class_signatures(cls: Any, prefix: str = "") -> dict[str, list[str]]:
    signatures = {}
    for name, value in cls.__dict__.items():
        if name.startswith("_") or not callable(value):
            continue
        signatures[f"{prefix}{name}"] = [
            parameter.name
            for parameter in inspect.signature(value).parameters.values()
            if parameter.name != "self"
        ]
    return signatures


def public_signatures(ImageFont: Any) -> dict[str, list[str]]:
    signatures: dict[str, list[str]] = {
        "load": ["filename"],
        "load_path": ["filename"],
        "load_default_imagefont": [],
        "load_default": ["size"],
        "truetype": ["font", "size", "index", "encoding", "layout_engine"],
        "ImageFont.info": [],
    }
    signatures.update(public_class_signatures(ImageFont.ImageFont, "ImageFont."))
    signatures.update(public_class_signatures(ImageFont.FreeTypeFont))
    signatures.update(public_class_signatures(ImageFont.TransposedFont, "TransposedFont."))
    return signatures


def public_operations(ImageFont: Any) -> list[str]:
    return sorted(public_signatures(ImageFont))


def public_surface(ImageFont: Any) -> dict[str, Any]:
    module_public_names = sorted(name for name in dir(ImageFont) if not name.startswith("_"))
    module_functions = {
        name: str(inspect.signature(getattr(ImageFont, name)))
        for name in [
            "load",
            "load_path",
            "load_default_imagefont",
            "load_default",
            "truetype",
        ]
    }
    classes = {}
    for class_name in ["ImageFont", "FreeTypeFont", "TransposedFont"]:
        cls = getattr(ImageFont, class_name)
        classes[class_name] = {
            name: str(inspect.signature(value))
            for name, value in cls.__dict__.items()
            if not name.startswith("_") and callable(value)
        }
    return {
        "module_public_names": module_public_names,
        "module_functions": module_functions,
        "classes": classes,
        "layout": list(ImageFont.Layout.__members__.keys()),
    }


def main() -> None:
    PIL, _imagingft, Image, ImageDraw, ImageFont = load_pillow()

    if not hasattr(_imagingft, "getfont"):
        raise RuntimeError(
            "font oracle requires PIL._imagingft C layer (_imagingft.getfont missing)"
        )

    if len(sys.argv) == 2 and sys.argv[1] == "--public-methods":
        json.dump(public_operations(ImageFont), sys.stdout, separators=(",", ":"), sort_keys=True)
        return

    if len(sys.argv) == 2 and sys.argv[1] == "--public-signatures":
        json.dump(public_signatures(ImageFont), sys.stdout, separators=(",", ":"), sort_keys=True)
        return

    if len(sys.argv) == 2 and sys.argv[1] == "--public-surface":
        json.dump(public_surface(ImageFont), sys.stdout, separators=(",", ":"), sort_keys=True)
        return

    cases = json.load(sys.stdin)
    results = {
        case["case_id"]: outcome(case, (PIL, _imagingft, Image, ImageDraw, ImageFont))
        for case in cases
    }
    json.dump(results, sys.stdout, separators=(",", ":"), sort_keys=True)


if __name__ == "__main__":
    main()
