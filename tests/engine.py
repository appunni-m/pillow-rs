"""Generic test execution engine — zero per-operation code.

This module is imported by:
  - tests/test_parity.py (test runner, uses pillow_rs as backend)
  - scripts/generate_fixtures.py (fixture generator, uses PIL as backend)

Adding a new operation requires ZERO changes to this file.
"""

from pathlib import Path

# ── Module → call_style lookup ──────────────────────────────────

SINGLE_CHOPS = {"invert", "duplicate", "constant", "offset"}
MUTATE_TARGETS = {
    "apply_transparency",
    "draft",
    "putalpha",
    "putdata",
    "putpalette",
    "putpixel",
    "thumbnail",
}
DUAL_MUTATE_TARGETS = {"paste", "alpha_composite"}
VALUE_TARGETS = {
    "tobytes", "split", "getbands", "getbbox", "getextrema", "histogram",
    "getpixel", "getcolors", "getdata", "getprojection", "entropy",
    "load", "verify", "seek", "tell", "tobitmap",
    "has_transparency_data", "getexif", "getim", "getpalette", "getxmp",
    "get_flattened_data", "get_child_images", "apply_transparency",
    "close", "save", "mode", "size", "width", "height",
    "format", "info", "is_animated", "n_frames", "palette",
}
PROPERTY_TARGETS = {"format", "height", "info", "mode", "size", "width"}
DRAW_VALUE_TARGETS = {"textlength", "textbbox", "multiline_textbbox", "getfont"}
IMAGE_CLASSMETHOD_TARGETS = {"new"}
FONT_METHOD_TARGETS = {"getbbox", "getlength", "getmask", "getmask2", "getmetrics", "getname",
                       "font_variant", "get_variation_axes", "get_variation_names",
                       "set_variation_by_axes", "set_variation_by_name"}

# Call styles that consume a case's ``input`` or ``input2`` image. A top-level
# fixture mode is coverage only when the operation actually reads that image.
IMAGE_INPUT_CALL_STYLES = frozenset({
    "instance_method",
    "instance_method_value",
    "instance_property",
    "instance_method_sequence",
    "pixel_access",
    "result_descriptor",
    "terminal_image_method",
    "seek",
    "instance_method_mutate",
    "instance_method_dual_mutate",
    "frombytes_instance",
    "draw",
    "draw_value",
    "draw_getfont",
    "draw_bitmap",
    "filter",
    "enhance",
    "module_function",
    "single_chops",
    "module_function_dual",
    "module_function_triple",
    "classmethod_dual",
    "classmethod_triple",
    "draw_shape",
    "sequence_iterator",
    "deform",
    "eval",
    "stat",
    "file_save",
})

# These call styles intentionally use the case mode without an input image.
CASE_MODE_CALL_STYLES = IMAGE_INPUT_CALL_STYLES | {
    "file_open",
    "palette_method",
}

def get_call_style(module, target, owner=None):
    """Return the call_style string for any (module, target) pair.

    Pure data lookup. Never needs new entries for new operations
    — new operations in an existing module resolve automatically.
    """
    if module == "Image":
        if target in IMAGE_CLASSMETHOD_TARGETS:  return "classmethod"
        if target in ("save",):                  return "file_save"
        if target in ("filter",):                return "filter"
        if target in ("open",):                  return "file_open"
        if target in ("toqimage", "toqpixmap"):  return "instance_method_value"
        if target in ("frombytes",):             return "frombytes_instance"
        if target == "getdata":                  return "instance_method_sequence"
        if target == "load":                     return "pixel_access"
        if target == "getim":                    return "result_descriptor"
        if target in ("close", "verify"):         return "terminal_image_method"
        if target == "seek":                     return "seek"
        if target in DUAL_MUTATE_TARGETS:        return "instance_method_dual_mutate"
        if target in MUTATE_TARGETS:             return "instance_method_mutate"
        if target in PROPERTY_TARGETS:           return "instance_property"
        if target in VALUE_TARGETS:              return "instance_method_value"
        return "instance_method"
    if module == "ImageOps":
        if target == "deform":             return "deform"
        return "module_function"
    if module == "ImageChops":
        if target in SINGLE_CHOPS:      return "single_chops"
        if target == "composite":       return "module_function_triple"
        return "module_function_dual"
    if module == "ImageDraw":
        if target == "getfont":           return "draw_getfont"
        if target in DRAW_VALUE_TARGETS: return "draw_value"
        if target == "bitmap":           return "draw_bitmap"
        if target == "shape":            return "draw_shape"
        return "draw"
    if module == "ImageFilter":         return "filter"
    if module == "ImageEnhance":        return "enhance"
    if module == "ImageModule":
        if target == "fromarray":         return "fromarray_mod"
        if target == "eval":               return "eval"
        if target == "open":               return "file_open"
        if target in ("blend", "alpha_composite"):
            return "classmethod_dual"
        if target == "composite":          return "classmethod_triple"
        if target == "frombytes":          return "frombytes_mod"
        if target == "frombuffer":         return "frombuffer_mod"
        if target == "merge":              return "merge_mod"
        return "classmethod"
    if module == "ImageFont":
        if owner == "ImageFont":                  return "font_base_method"
        if owner == "TransposedFont":             return "transposed_font_method"
        if target in FONT_METHOD_TARGETS:          return "font_method"
        if target in ("truetype", "load", "load_path"):
            return "font_truetype"
        if target == "TransposedFont":             return "transposed_font"
        if target in ("FreeTypeFont",):
            return "font_constructor"
        if target in ("load_default", "load_default_imagefont"):
            return "font_constructor"
        if target in ("ImageFont",):
            return "font_base_descriptor"
        return "module_function_value"
    if module == "ImagePalette":
        return "palette_method"
    if module == "ImageSequence":
        if target == "Iterator":          return "sequence_iterator"
        return "module_function_value"
    if module == "ImageColor":
        return "module_function_value"
    if module == "ImageStat":           return "stat"
    if module == "Decode":              return "decode"
    if module == "Encode":              return "encode"
    raise ValueError(f"Unknown module: {module}")


# ── Input creation ──────────────────────────────────────────────

REFERENCE_IMAGE = Path(__file__).parent / "test_reference.png"
FONT_FIXTURE_DIR = (
    Path(__file__).parent.parent
    / "pillow-rs-freetype"
    / "tests"
    / "fixtures"
    / "input"
    / "fonts"
)
# Additional directories to search for named reference images.
# Populated at test time by test_parity.py for fixtures_2 support.
EXTRA_REFERENCE_DIRS = []

# Base directory for format asset images (e.g., webp/*.webp, png/*.png).
# Overridden at test/generation time by test_parity.py and generate_fixtures.py
# to point to the correct fixture directory (fixtures vs fixtures_2).
ASSETS_DIR = Path(__file__).parent / "fixtures" / "input" / "images"

def _find_reference_image(name):
    """Resolve a named reference image, checking extra dirs first."""
    if name:
        for directory in EXTRA_REFERENCE_DIRS:
            candidate = Path(directory) / f"{name}.png"
            if candidate.is_file():
                return candidate
        raise FileNotFoundError(f"reference image is missing: {name}.png")
    if REFERENCE_IMAGE.is_file():
        return REFERENCE_IMAGE
    raise FileNotFoundError(f"default reference image is missing: {REFERENCE_IMAGE}")


def _resolve_font_path(font):
    """Resolve legacy host font paths to stable checked-in fixture paths."""
    if not isinstance(font, (str, Path)):
        return font
    path = Path(font)
    fixture_path = FONT_FIXTURE_DIR / path.name
    if str(path).startswith("/usr/share/fonts/") and fixture_path.is_file():
        return fixture_path.relative_to(Path(__file__).parent.parent).as_posix()
    if path.is_file():
        return str(path)
    if fixture_path.is_file():
        return fixture_path.relative_to(Path(__file__).parent.parent).as_posix()
    return str(path)


def _pilify(v):
    """Recursively convert lists to tuples for PIL API compatibility.
    RSPIL PyO3 bindings handle this automatically; PIL does not.
    However, some RSPIL bindings also need tuples (e.g. Image.new size)."""
    if isinstance(v, list):
        return tuple(_pilify(x) for x in v)
    if isinstance(v, dict):
        return {k: _pilify(val) for k, val in v.items()}
    return v




def create_input(backend, mode, spec):
    """Create an image from a declarative input spec.

    Works identically for both PIL and RSPIL backends because both
    provide Image.open, Image.new, Image.frombytes, .resize, .convert.

    Args:
        backend: Module with PIL-identical API (PIL or pillow_rs)
        mode: Image mode string (e.g. 'L', 'RGB') — from case-level field
        spec: Input specification dict, or None for no-input operations
    """
    if spec is None:
        return None

    source = spec["source"]
    size = tuple(spec["size"])

    if source == "reference_rgb":
        ref_path = _find_reference_image(spec.get("reference", ""))
        ref = backend.Image.open(str(ref_path))
        if ref.size != size:
            ref = ref.resize(size, backend.Image.LANCZOS)
        return ref.convert(mode)
    elif source == "constant":
        color = spec.get("color", 0)
        return backend.Image.new(mode, size, color)
    elif source == "bytes":
        raw = bytes.fromhex(spec["bytes"])
        return backend.Image.frombytes(mode, size, raw)
    else:
        raise ValueError(f"Unknown input source: {source}")


# ── Call style implementations ──────────────────────────────────

def _draw(backend, img, target, params):
    draw = backend.ImageDraw.Draw(img)
    return getattr(draw, target)(**params)

def _draw_bitmap(backend, img, img2, target, params):
    """Draw a bitmap onto img. img2 is the bitmap source."""
    draw = backend.ImageDraw.Draw(img)
    # img2 IS the bitmap — pass it directly
    bitmap = img2
    if bitmap is None:
        # If no input2, convert img to mode '1' as the bitmap
        bitmap = img.convert("1")
    xy = tuple(params.pop("xy", [5, 5]))
    fill = params.pop("fill", 200)
    return draw.bitmap(xy, bitmap, fill=fill, **params)

def _call_mod(backend, target, prefer_chops=False):
    """Resolve target function from backend's module hierarchy.

    When prefer_chops=True, ImageChops is checked before ImageOps for operations
    like 'invert' that exist in both but have different mode support.
    """
    search = ["ImageOps", "ImageChops", "ImageColor", "ImagePalette",
              "ImageFont", "ImageSequence", "Image", "ImageDraw"]
    if prefer_chops:
        search = ["ImageChops", "ImageOps", "ImageColor", "ImagePalette",
                  "ImageFont", "ImageSequence", "Image", "ImageDraw"]
    for mod_name in search:
        mod = getattr(backend, mod_name, None)
        if mod and hasattr(mod, target):
            return getattr(mod, target)
    if hasattr(backend, "ImageFilter"):
        f = getattr(backend.ImageFilter, target, None)
        if f: return f
    if hasattr(backend, "ImageEnhance"):
        e = getattr(backend.ImageEnhance, target, None)
        if e: return e
    if hasattr(backend, "ImageFont"):
        f = getattr(backend.ImageFont, target, None)
        if f: return f
    raise ValueError(f"Cannot resolve target function: {target}")

def _make_filter(backend, target, params):
    filter_cls = getattr(backend.ImageFilter, target)
    BUILTINS = {"BLUR", "CONTOUR", "DETAIL", "EDGE_ENHANCE", "EDGE_ENHANCE_MORE",
                "EMBOSS", "FIND_EDGES", "SHARPEN", "SMOOTH", "SMOOTH_MORE"}
    if target == "Color3DLUT" and "_table_pattern" in params:
        pattern = params.pop("_table_pattern")
        raw_size = params["size"]
        size = (raw_size, raw_size, raw_size) if isinstance(raw_size, int) else tuple(raw_size)
        channels = params.get("channels", 3)
        if pattern == "identity":
            table = []
            for z in range(size[2]):
                for y in range(size[1]):
                    for x in range(size[0]):
                        values = [
                            x / (size[0] - 1),
                            y / (size[1] - 1),
                            z / (size[2] - 1),
                            (x + 2 * y + 3 * z) / (6 * (size[0] - 1)),
                        ]
                        table.extend(values[:channels])
            params["table"] = table
        else:
            raise ValueError(f"unknown Color3DLUT table pattern: {pattern}")
    return filter_cls if target in BUILTINS else filter_cls(**params)

def _stat_to_dict(stat):
    return {
        "type": type(stat).__name__,
        "count": stat.count,
        "sum": stat.sum,
        "mean": stat.mean,
        "median": stat.median,
        "rms": stat.rms,
        "var": stat.var,
        "stddev": stat.stddev,
        "extrema": stat.extrema,
    }


def _eval_image(backend, img, target, params):
    """Image.eval(image, function) — applies a named function."""
    func_name = params.pop("function", "identity")
    funcs = {
        "add_10": lambda x: min(255, x + 10),
        "identity": lambda x: x,
        "invert": lambda x: 255 - x,
    }
    return _call_mod(backend, target)(img, funcs[func_name])


def _font_method(backend, img, target, params):
    """Load default font, then call method on it.
    
    Uses method dispatch: calls getattr(font, target)(**params).
    Opaque font and mask results are represented by exact type and stable
    observable data rather than converted to a friendlier result type.
    """
    font = backend.ImageFont.load_default()
    if target == "font_variant":
        variant = font.font_variant(**params)
        return _font_descriptor(variant)
    # Some PIL methods need list not tuple (e.g. set_variation_by_axes)
    for key in list(params.keys()):
        if key.endswith("axes") and isinstance(params[key], tuple):
            params[key] = list(params[key])
    result = getattr(font, target)(**params)
    if target == "getmask":
        return _mask_descriptor(result)
    if target == "getmask2":
        mask, offset = result
        return {
            "type": type(result).__name__,
            "mask": _mask_descriptor(mask),
            "offset": offset,
        }
    return result

def _font_truetype(backend, img, target, params):
    """Call the requested font loader and describe its result exactly."""
    font_path = _resolve_font_path(params.pop("font"))
    size = params.pop("size", 20)
    if target == "truetype":
        font = backend.ImageFont.truetype(font_path, size, **params)
    else:
        font = getattr(backend.ImageFont, target)(font_path, **params)
    return _font_descriptor(font)


def _mask_descriptor(mask):
    """Describe a Pillow/RSPIL mask without normalizing its concrete type."""
    pixels = bytes(mask) if type(mask).__name__ == "ImagingCore" else mask.tobytes()
    return {
        "type": type(mask).__name__,
        "mode": getattr(mask, "mode", None),
        "size": list(getattr(mask, "size", ())),
        "pixels_hex": pixels.hex(),
    }


def _font_descriptor(font, text="A"):
    """Describe a font object's type and stable mask behavior."""
    descriptor = {"type": type(font).__name__}
    try:
        mask = font.getmask(text)
    except Exception as error:
        descriptor["getmask"] = {
            "exception": type(error).__name__,
            "message": str(error),
        }
    else:
        descriptor["getmask"] = _mask_descriptor(mask)
    return descriptor


def _font_constructor(backend, img, target, params):
    """Create a font and describe its concrete type and mask behavior."""
    if "font" in params:
        params["font"] = _resolve_font_path(params["font"])
    font = _call_mod(backend, target)(**params)
    return _font_descriptor(font)


def _font_base_descriptor(backend, img, target, params):
    """Describe the base ImageFont object's observable mask behavior exactly."""
    font = _call_mod(backend, target)(**params)
    return _font_descriptor(font)


def _font_base_method(backend, img, target, params):
    """Call one method on the concrete Pillow-compatible base ImageFont."""
    font = backend.ImageFont.ImageFont()
    result = getattr(font, target)(**params)
    if target == "getmask":
        return _mask_descriptor(result)
    return result


def _transposed_font_method(backend, img, target, params):
    """Call a TransposedFont method on a version-pinned TrueType font."""
    font_path = _resolve_font_path(params.pop("font"))
    size = params.pop("size", 20)
    orientation_name = params.pop("orientation", None)
    font = backend.ImageFont.truetype(font_path, size)
    orientation = (
        getattr(backend.Image, orientation_name)
        if orientation_name is not None
        else None
    )
    transposed = backend.ImageFont.TransposedFont(font, orientation=orientation)
    result = getattr(transposed, target)(**params)
    if target == "getmask":
        return _mask_descriptor(result)
    return result


def _draw_getfont(backend, img, target, params):
    """Get ImageDraw's font and describe its exact observable behavior."""
    draw = backend.ImageDraw.Draw(img)
    font = getattr(draw, target)(**params)
    return _font_descriptor(font)


def _palette_method(backend, img, target, params):
    """Create default palette, then call method on it."""
    import io
    palette_mode = params.pop("_fixture_mode", "RGB")
    palette = backend.ImagePalette.ImagePalette(mode=palette_mode)
    if target == "save":
        buf = io.BytesIO()
        text_buf = io.TextIOWrapper(buf)
        result = getattr(palette, target)(text_buf, **params)
        text_buf.flush()
        return result, buf.getvalue()
    if target == "copy":
        copied = palette.copy()
        return {
            "type": type(copied).__name__,
            "mode": copied.mode,
            "palette": copied.palette,
        }
    return getattr(palette, target)(**params)

class _SimpleDeformer:
    """Minimal deformer for ImageOps.deform fixture."""
    def getmesh(self, img):
        w, h = img.size
        return [((0, 0, w, h), (0, 0, 0, h, w, h, w, 0))]

def _deform_op(backend, img, target, params):
    """ImageOps.deform(img, deformer) with a simple deformer."""
    return _call_mod(backend, target)(img, _SimpleDeformer(), **params)

def _frombytes_mod(backend, img, target, params):
    """Image.frombytes(mode, size, data=data_hex decoded)."""
    mode = params.pop("mode")
    size = tuple(params.pop("size"))
    pattern = params.pop("data_pattern", None)
    data_hex = params.pop("data_hex", "")
    if pattern == "ramp":
        if mode == "1":
            byte_count = ((size[0] + 7) // 8) * size[1]
        else:
            channels = {
                "L": 1,
                "P": 1,
                "LA": 2,
                "RGB": 3,
                "RGBA": 4,
                "CMYK": 4,
            }[mode]
            byte_count = size[0] * size[1] * channels
        data = bytes((index * 37 + 11) % 256 for index in range(byte_count))
    else:
        data = bytes.fromhex(data_hex)
    return _call_mod(backend, target)(mode, size, data, **params)

def _frombuffer_mod(backend, img, target, params):
    """Image.frombuffer(mode, size, data=hex decoded)."""
    mode = params.pop("mode")
    size = tuple(params.pop("size"))
    data_hex = params.pop("data_hex", "")
    data = bytes.fromhex(data_hex)
    return _call_mod(backend, target)(mode, size, data, **params)

def _draw_shape(backend, img, target, params):
    """Draw a shape using ImageDraw.Outline()."""
    draw = backend.ImageDraw.Draw(img)
    outline = backend.ImageDraw.Outline()
    points = params.pop("points", [])
    if points:
        x0, y0 = points[0]
        outline.move(x0, y0)
        for x, y in points[1:]:
            outline.line(x, y)
    result = getattr(draw, target)(outline, **params)
    return result, img

def _merge_mod(backend, img, target, params):
    """Image.merge(mode, bands) - create bands from params."""
    mode = params.pop("mode")
    band_values = params.pop("band_values", None)
    if band_values is not None:
        size = tuple(params.pop("size"))
        bands = [
            backend.Image.new("L", size, value)
            for value in band_values
        ]
        return _call_mod(backend, target)(mode, tuple(bands), **params)
    bands_specs = params.pop("bands", [])
    # Create band images from specs
    bands = []
    for spec in bands_specs:
        band_mode = spec.get("band_mode", "L")
        band = create_input(backend, band_mode, spec.get("input"))
        bands.append(band)
    return _call_mod(backend, target)(mode, tuple(bands), **params)

def _sequence_iterator(backend, img, target, params):
    """ImageSequence.Iterator(img) - return every frame for exact comparison."""
    it = _call_mod(backend, target)(img, **params)
    return list(it)



# ── New call style helpers for missing operations ──────────────────

import os, tempfile

# Pillow 12.2.0-encoded 3x2 images. File-open fixtures use these identical
# bytes for both implementations, so encoder behavior cannot mask decoder
# differences. TIFF is used for modes PNG cannot represent.
OPEN_FIXTURE_B64 = {
    "1": "iVBORw0KGgoAAAANSUhEUgAAAAMAAAACAQAAAAC1D1u3AAAADElEQVR4nGNwYDgAAAGEAQEKf5BQAAAAAElFTkSuQmCC",
    "L": "iVBORw0KGgoAAAANSUhEUgAAAAMAAAACCAAAAAC4HznGAAAAEElEQVR4nGNkcHBgOPBfEAAHHwJSXRpuggAAAABJRU5ErkJggg==",
    "LA": "iVBORw0KGgoAAAANSUhEUgAAAAMAAAACCAQAAAA3fa6RAAAAFklEQVR4nGNk+O9w0OEAwwGH/wyCigAomQUz31OBlAAAAABJRU5ErkJggg==",
    "P": "R0lGODdhAwACAIIAAAAA/wED/gIG/QMJ/AQM+wUP+gAAAAAAACwAAAAAAwACAAAICQABBBAwgECBgAA7",
    "RGB": "iVBORw0KGgoAAAANSUhEUgAAAAMAAAACCAIAAAASFvFNAAAAHElEQVR4nGNkZGJWN7CUk5NjSc4plZOTc3NzAwAaxANmrbycrQAAAABJRU5ErkJggg==",
    "RGBA": "iVBORw0KGgoAAAANSUhEUgAAAAMAAAACCAYAAACddGYaAAAAH0lEQVR4nGNkZGJmUTewdJKTk5NjSc4prQMx3Nzc3AAwdASsz35Y5AAAAABJRU5ErkJggg==",
    "CMYK": "SUkqAAgAAAAKAAABBAABAAAAAwAAAAEBBAABAAAAAgAAAAIBAwAEAAAAhgAAAAMBAwABAAAAAQAAAAYBAwABAAAABQAAABEBBAABAAAAjgAAABUBAwABAAAABAAAABYBBAABAAAAAgAAABcBBAABAAAAGAAAABwBAwABAAAAAQAAAAAAAAAIAAgACAAIAAECAwQoMjxGRlBaZGRueIKCjJagyNLc5g==",
    "I": "SUkqAAgAAAAKAAABBAABAAAAAwAAAAEBBAABAAAAAgAAAAIBAwABAAAAIAAAAAMBAwABAAAAAQAAAAYBAwABAAAAAQAAABEBBAABAAAAhgAAABYBBAABAAAAAgAAABcBBAABAAAAGAAAABwBAwABAAAAAQAAAFMBAwABAAAAAgAAAAAAAAAAAAAAAQAAAAABAAD/////AAABAACA//8=",
    "F": "SUkqAAgAAAAKAAABBAABAAAAAwAAAAEBBAABAAAAAgAAAAIBAwABAAAAIAAAAAMBAwABAAAAAQAAAAYBAwABAAAAAQAAABEBBAABAAAAhgAAABYBBAABAAAAAgAAABcBBAABAAAAGAAAABwBAwABAAAAAQAAAFMBAwABAAAAAwAAAAAAAAAAAAAAAAAAPwAAoL8AAHBAAAB/QwAAAD4=",
}
OPEN_FIXTURE_SUFFIX = {
    "CMYK": ".tif",
    "F": ".tif",
    "I": ".tif",
    "P": ".gif",
}


def _file_open(backend, img, target, params):
    """Open identical encoded bytes through Pillow or pillow-rs.

    ``file_b64`` keeps this an open/decode test rather than allowing each
    backend's encoder to manufacture a different input file.
    """
    file_b64 = params.pop("file_b64", params.pop("png_b64", None))
    fixture_mode = params.pop("_fixture_mode", None)
    requested_suffix = params.pop("suffix", None)
    if file_b64 is None and fixture_mode is not None:
        file_b64 = OPEN_FIXTURE_B64.get(fixture_mode)
        if file_b64 is None:
            raise ValueError(f"no exact open fixture for mode {fixture_mode}")
    suffix = (
        OPEN_FIXTURE_SUFFIX.get(fixture_mode, ".png")
        if fixture_mode is not None
        else requested_suffix or ".png"
    )
    if file_b64:
        import base64 as _b64
        data = _b64.b64decode(file_b64)
        fd, tmp = tempfile.mkstemp(suffix=suffix)
        os.write(fd, data)
        os.close(fd)
    else:
        tmp_img = backend.Image.new("RGB", (10, 10), (128, 64, 200))
        fd, tmp = tempfile.mkstemp(suffix=suffix)
        os.close(fd)
        tmp_img.save(tmp)
    return _call_mod(backend, target)(tmp, **params)


def _file_save(backend, img, target, params):
    """Save image to a temp file, re-open, and return the image.
    The temp file is NOT deleted — RSPIL may lazily load image data."""
    suffix = params.pop("suffix", ".png")
    fmt = params.pop("format", None)
    fd, tmp = tempfile.mkstemp(suffix=suffix)
    os.close(fd)
    result = getattr(img, target)(tmp, format=fmt, **params)
    return result, backend.Image.open(tmp)


def _decode_asset(backend, img, img2, target, params):
    """Decode an image asset file from ASSETS_DIR/target/asset_name.

    Used for Decode module parity tests — reads the asset path from params,
    resolves it relative to ASSETS_DIR, opens with the backend's Image.open,
    and returns the decoded Image. img/img2 are unused (no input creation needed).
    """
    asset_name = params.pop("asset")
    asset_path = ASSETS_DIR / target / asset_name
    if not asset_path.exists():
        raise FileNotFoundError(f"Decode asset not found: {asset_path}")
    return _call_mod(backend, "open")(str(asset_path))


def _encode_roundtrip(backend, img, img2, target, params):
    """Encode a source image to the target format and decode back.

    Used for Encode module parity tests. Reads source_asset and source_format
    from params, opens the source image, re-encodes to the target format
    with the given params, then re-opens the encoded output and returns
    the decoded Image for comparison against the PIL-generated reference.
    Uses a temp file for the encoded output to support backends that
    do not accept BytesIO (e.g., pillow_rs).
    """
    source_asset = params.pop("source_asset")
    source_format = params.pop("source_format", target)
    source_path = ASSETS_DIR / source_format / source_asset
    source_img = _call_mod(backend, "open")(str(source_path))

    # Handle resize-before-encode (used by enc_1x1)
    if "size" in params:
        size = tuple(params.pop("size"))
        source_img = source_img.resize(size, backend.Image.LANCZOS)

    fd, tmp = tempfile.mkstemp(suffix="." + target)
    os.close(fd)
    source_img.save(tmp, format=target.upper(), **params)
    # NOTE: temp file is NOT deleted — RSPIL may lazily load image data.
    return _call_mod(backend, "open")(tmp)


def _frombytes_instance(backend, img, target, params):
    """Image.frombytes instance method (img.frombytes(data, decoder_name)).
    Handles data_hex by decoding it to raw bytes before calling.
    frombytes() modifies the image in-place and returns None, so we
    return the modified image."""
    data_hex = params.pop("data_hex", "")
    data = bytes.fromhex(data_hex)
    result = getattr(img, target)(data, **params)
    return result, img


def _transposed_font(backend, img, target, params):
    """Create a TransposedFont and render text with it for image comparison.
    Handles font path → object loading and orientation mapping."""
    font_path = _resolve_font_path(params.pop("font"))
    size = params.pop("size", 20)
    orientation_name = params.pop("orientation", None)
    text = params.pop("text", "Hello")
    font = backend.ImageFont.truetype(font_path, size)
    orientation = getattr(backend.Image, orientation_name, None) if orientation_name else None
    tf = backend.ImageFont.TransposedFont(font, orientation=orientation)
    # Render text on a canvas for pixel comparison
    canvas = backend.Image.new("RGB", (150, 50), (255, 255, 255))
    draw = backend.ImageDraw.Draw(canvas)
    draw.text((10, 10), text, font=tf, fill=(0, 0, 0))
    return canvas


def _fromarray(backend, img, target, params):
    """Image.fromarray — use a deterministic zero-filled array-interface object."""

    class FixtureArray:
        def __init__(self, shape):
            self.shape = shape
            self._data = bytes(_shape_product(shape))
            stride = 1
            reversed_strides = []
            for dimension in reversed(shape):
                reversed_strides.append(stride)
                stride *= dimension
            self.__array_interface__ = {
                "shape": shape,
                "strides": tuple(reversed(reversed_strides)),
                "typestr": "|u1",
                "version": 3,
                "data": self._data,
            }

        def tobytes(self):
            return self._data

    def _shape_product(shape):
        total = 1
        for dimension in shape:
            total *= dimension
        return total

    mode = params.get("mode", "L")
    size = tuple(params.get("size", [100, 100]))
    channels = {"LA": 2, "RGB": 3, "RGBA": 4}.get(mode)
    shape = (
        (size[1], size[0])
        if channels is None
        else (size[1], size[0], channels)
    )
    arr = FixtureArray(shape)
    return backend.Image.fromarray(arr, mode=mode)


def _instance_method_sequence(backend, img, img2, target, params):
    """Exercise and describe a returned sequence instead of the source image."""
    result = getattr(img, target)(**params)
    return {
        "type": type(result).__name__,
        "values": list(result),
    }


def _pixel_access(backend, img, img2, target, params):
    """Exercise Image.load() read and write semantics through PixelAccess."""
    result = getattr(img, target)(**params)
    width, height = img.size
    first_xy = (0, 0)
    last_xy = (width - 1, height - 1)
    first = result[first_xy]
    last = result[last_xy]
    result[first_xy] = first
    return {
        "type": type(result).__name__,
        "first": _typed_value(first),
        "last": _typed_value(last),
        "write_roundtrip": _typed_value(result[first_xy]),
    }


def _result_descriptor(backend, img, img2, target, params):
    """Describe opaque results without accepting an arbitrary placeholder."""
    result = getattr(img, target)(**params)
    representation = repr(result)
    address_marker = " at 0x"
    if address_marker in representation:
        representation = representation.split(address_marker, 1)[0] + ">"
    return {
        "type": type(result).__name__,
        "repr": representation,
    }


def _method_and_image(backend, img, img2, target, params):
    """Preserve both a mutator's return value and its resulting pixels."""
    result = getattr(img, target)(**params)
    return result, img


def _dual_method_and_image(backend, img, img2, target, params):
    """Preserve return value and pixels for two-image mutators."""
    result = getattr(img, target)(img2, **params)
    return result, img


def _terminal_image_method(backend, img, img2, target, params):
    """Exercise close/verify and describe whether pixels remain accessible."""
    result = getattr(img, target)(**params)
    try:
        img.tobytes()
    except Exception as error:
        state = {
            "accessible": False,
            "exception": type(error).__name__,
            "message": str(error),
        }
    else:
        state = {"accessible": True}
    return result, state


def _seek(backend, img, img2, target, params):
    """Exercise seek's return value and resulting frame position."""
    result = getattr(img, target)(**params)
    return result, img.tell()


CALL_STYLE = {
    "instance_method":        lambda b, img, img2, tgt, p: getattr(img, tgt)(**p),
    "instance_method_value":  lambda b, img, img2, tgt, p: getattr(img, tgt)(**p),
    "instance_property":      lambda b, img, img2, tgt, p: getattr(img, tgt),
    "instance_method_sequence": _instance_method_sequence,
    "pixel_access": _pixel_access,
    "result_descriptor": _result_descriptor,
    "terminal_image_method": _terminal_image_method,
    "seek": _seek,
    "instance_method_mutate": _method_and_image,
    "instance_method_dual_mutate": _dual_method_and_image,
    "draw":       lambda b, img, img2, tgt, p: (_draw(b, img, tgt, p), img),
    "draw_value": lambda b, img, img2, tgt, p: _draw(b, img, tgt, p),
    "draw_getfont":lambda b, img, img2, tgt, p: _draw_getfont(b, img, tgt, p),
    "draw_bitmap":lambda b, img, img2, tgt, p: (_draw_bitmap(b, img, img2, tgt, p), img),
    "filter":     lambda b, img, img2, tgt, p: img.filter(_make_filter(b, tgt if tgt != "filter" else p.pop("filter"), p)),
    "enhance":    lambda b, img, img2, tgt, p: getattr(b.ImageEnhance, tgt)(img).enhance(p.pop("factor", 1.0)),
    "font_method":lambda b, img, img2, tgt, p: _font_method(b, img, tgt, p),
    "font_truetype":lambda b, img, img2, tgt, p: _font_truetype(b, img, tgt, p),
    "font_constructor": lambda b, img, img2, tgt, p: _font_constructor(b, img, tgt, p),
    "font_base_descriptor": lambda b, img, img2, tgt, p: _font_base_descriptor(
        b, img, tgt, p
    ),
    "font_base_method": lambda b, img, img2, tgt, p: _font_base_method(
        b, img, tgt, p
    ),
    "transposed_font_method": lambda b, img, img2, tgt, p: _transposed_font_method(
        b, img, tgt, p
    ),
    "palette_method":lambda b, img, img2, tgt, p: _palette_method(b, img, tgt, p),
    "module_function":       lambda b, img, img2, tgt, p: _call_mod(b, tgt)(img, **p),
    "single_chops":          lambda b, img, img2, tgt, p: _call_mod(b, tgt, prefer_chops=True)(img, **p),
    "module_function_dual":  lambda b, img, img2, tgt, p: _call_mod(b, tgt)(img, img2, **p),
    "module_function_triple":lambda b, img, img2, tgt, p: _call_mod(b, tgt)(img, img2, create_input(b, "L", p.pop("mask_img", None)), **p),
    "module_function_value": lambda b, img, img2, tgt, p: _call_mod(b, tgt)(**p),
    "classmethod":           lambda b, img, img2, tgt, p: _call_mod(b, tgt)(**p),
    "classmethod_dual":      lambda b, img, img2, tgt, p: (
        getattr(b, tgt, None) if callable(getattr(b, tgt, None)) else _call_mod(b, tgt)
    )(img, img2, **p),
    "fromarray_mod":lambda b, img, img2, tgt, p: _fromarray(b, img, tgt, p),
    "classmethod_triple":    lambda b, img, img2, tgt, p: (
        b.Image.composite if tgt == "composite" else _call_mod(b, tgt)
    )(img, img2, create_input(b, "L", p.pop("mask_img", None)), **p),
    "draw_shape":            lambda b, img, img2, tgt, p: _draw_shape(b, img, tgt, p),
    "frombytes_mod":         lambda b, img, img2, tgt, p: _frombytes_mod(b, img, tgt, p),
    "frombytes_instance":    lambda b, img, img2, tgt, p: _frombytes_instance(b, img, tgt, p),
    "transposed_font":       lambda b, img, img2, tgt, p: _transposed_font(b, img, tgt, p),
    "frombuffer_mod":        lambda b, img, img2, tgt, p: _frombuffer_mod(b, img, tgt, p),
    "merge_mod":             lambda b, img, img2, tgt, p: _merge_mod(b, img, tgt, p),
    "sequence_iterator":     lambda b, img, img2, tgt, p: _sequence_iterator(b, img, tgt, p),
    "deform":                lambda b, img, img2, tgt, p: _deform_op(b, img, tgt, p),
    "eval":                   lambda b, img, img2, tgt, p: _eval_image(b, img, tgt, p),
    "stat": lambda b, img, img2, tgt, p: _stat_to_dict(getattr(b.ImageStat, tgt)(img)),
    "file_open": lambda b, img, img2, tgt, p: _file_open(b, img, tgt, p),
    "file_save": lambda b, img, img2, tgt, p: _file_save(b, img, tgt, p),
    "decode": lambda b, img, img2, tgt, p: _decode_asset(b, img, img2, tgt, p),
    "encode": lambda b, img, img2, tgt, p: _encode_roundtrip(b, img, img2, tgt, p),
}


# ── Assertion methods ───────────────────────────────────────────

OUTPUTS_DIR = Path(__file__).parent / "fixtures" / "outputs"
# May be overridden at test time by test_parity.py for fixtures_2 support.
# _load_reference reads OUTPUTS_DIR at call time, so overriding works correctly.

def _load_reference(path):
    """Reference paths are relative to fixtures/outputs/."""
    full = OUTPUTS_DIR / path
    if path.endswith('.png'):
        # Lazy import to avoid PIL dependency when running tests
        from PIL import Image as PILImage
        return PILImage.open(str(full))
    return open(str(full), 'rb').read()

def _bytes(data):
    if isinstance(data, (bytes, bytearray, memoryview)):
        return bytes(data)
    if hasattr(data, 'tobytes'):
        return data.tobytes()
    if hasattr(data, 'toImage'):
        data = data.toImage()
    if hasattr(data, 'bits') and callable(data.bits):
        raw = data.bits()
        if isinstance(raw, bytes):
            return raw
        if hasattr(raw, 'tobytes'):
            return raw.tobytes()
        if hasattr(raw, 'asstring'):
            return raw.asstring(data.sizeInBytes())
    raise TypeError(f"result does not expose exact bytes: {type(data).__name__}")


def _typed_value(value):
    """Encode a value with container and scalar types preserved exactly."""
    if value is None:
        return {"type": "NoneType"}
    if type(value) is bool:
        return {"type": "bool", "value": value}
    if type(value) is int:
        return {"type": "int", "value": value}
    if type(value) is float:
        return {"type": "float", "hex": value.hex()}
    if type(value) is str:
        return {"type": "str", "value": value}
    if type(value) is bytes:
        return {"type": "bytes", "hex": value.hex()}
    if type(value) is bytearray:
        return {"type": "bytearray", "hex": value.hex()}
    if type(value) is memoryview:
        return {"type": "memoryview", "hex": value.tobytes().hex()}
    if type(value) is tuple:
        return {
            "type": "tuple",
            "items": [_typed_value(item) for item in value],
        }
    if type(value) is list:
        return {
            "type": "list",
            "items": [_typed_value(item) for item in value],
        }
    if type(value) is dict:
        return {
            "type": "dict",
            "items": [
                [_typed_value(key), _typed_value(item)]
                for key, item in value.items()
            ],
        }
    raise TypeError(f"unsupported exact fixture value: {type(value).__name__}")


def _canonical_typed_bytes(value):
    """Encode an exact Python value without expanding it into fixture JSON.

    The format is intentionally small and language-neutral. Every node starts
    with a one-byte type tag. Variable-width payloads use an unsigned
    eight-byte big-endian length, and containers recursively encode members in
    iteration order. Integer and float text uses Python's exact decimal and
    hexadecimal representations.
    """
    encoded = bytearray(b"PTV1")

    def write_length(length):
        encoded.extend(length.to_bytes(8, "big", signed=False))

    def write_payload(tag, payload):
        encoded.extend(tag)
        write_length(len(payload))
        encoded.extend(payload)

    def encode(item):
        if item is None:
            encoded.extend(b"N")
        elif type(item) is bool:
            encoded.extend(b"B1" if item else b"B0")
        elif type(item) is int:
            write_payload(b"I", str(item).encode("ascii"))
        elif type(item) is float:
            write_payload(b"F", item.hex().encode("ascii"))
        elif type(item) is str:
            write_payload(b"S", item.encode("utf-8"))
        elif type(item) is bytes:
            write_payload(b"Y", item)
        elif type(item) is bytearray:
            write_payload(b"A", bytes(item))
        elif type(item) is memoryview:
            write_payload(b"M", item.tobytes())
        elif type(item) is tuple:
            encoded.extend(b"T")
            write_length(len(item))
            for child in item:
                encode(child)
        elif type(item) is list:
            encoded.extend(b"L")
            write_length(len(item))
            for child in item:
                encode(child)
        elif type(item) is dict:
            encoded.extend(b"D")
            write_length(len(item))
            for key, child in item.items():
                encode(key)
                encode(child)
        else:
            raise TypeError(
                f"unsupported exact fixture value: {type(item).__name__}"
            )

    encode(value)
    return bytes(encoded)


def _assert_image(case, result):
    reference = _load_reference(case["reference"])
    try:
        if isinstance(reference, bytes):
            raw_kind = case.get("raw_kind")
            if raw_kind == "bytes":
                return (
                    type(result).__name__ == case.get("result_type")
                    and bytes(result) == reference
                )
            if raw_kind == "qt_image":
                return (
                    type(result).__name__ == case.get("result_type")
                    and _bytes(result) == reference
                )
            if raw_kind != "image":
                return False
            if (
                getattr(result, "mode", None) != case.get("mode")
                or tuple(getattr(result, "size", ())) != tuple(case.get("size", ()))
                or _bytes(result) != reference
            ):
                return False
            if "palette" in case:
                return result.getpalette() == case["palette"]
            return True
        matches = (
            getattr(result, "mode", None) == reference.mode
            and tuple(getattr(result, "size", ())) == tuple(reference.size)
            and _bytes(result) == reference.tobytes()
        )
        if not matches:
            return False
        if reference.mode in ("P", "PA"):
            return result.getpalette() == reference.getpalette()
        return True
    except (AttributeError, TypeError, ValueError):
        return False


def _assert_image_list(case, result):
    items = case.get("items")
    if items is None:
        items = [
            {"method": "image", "reference": reference}
            for reference in case["references"]
        ]
    return (
        type(result).__name__ == case.get("container_type")
        and len(result) == len(items)
        and all(
            _assert_image(item, band)
            for band, item in zip(result, items)
        )
    )


def _assert_string(case, result):
    return "value" in case and repr(result) == case["value"]


def _assert_float(case, result):
    return type(result) is float and result == case["value"]


def _assert_error(case, result):
    return (
        isinstance(result, Exception)
        and type(result).__name__ == case.get("exception")
        and str(result) == case.get("message")
    )


def _assert_exact(case, result):
    expected = case["value"]
    if type(result) is not type(expected):
        return False
    if isinstance(expected, list):
        return len(result) == len(expected) and all(
            _assert_exact({"value": expected_item}, result_item)
            for expected_item, result_item in zip(expected, result)
        )
    if isinstance(expected, dict):
        return result.keys() == expected.keys() and all(
            _assert_exact({"value": expected[key]}, result[key])
            for key in expected
        )
    return result == expected


def _assert_typed(case, result):
    try:
        return _typed_value(result) == case["value"]
    except TypeError:
        return False


def _assert_typed_binary(case, result):
    try:
        return _canonical_typed_bytes(result) == _load_reference(case["reference"])
    except (OSError, TypeError, ValueError):
        return False


ASSERT = {
    "image": _assert_image,
    "image_list": _assert_image_list,
    "exact": _assert_exact,
    "typed": _assert_typed,
    "typed_binary": _assert_typed_binary,
    "string": _assert_string,
    "float": _assert_float,
    "error": _assert_error,
}


def _assert_tuple(case, result):
    items = case["items"]
    return (
        type(result) is tuple
        and len(result) == len(items)
        and all(
            ASSERT[item["method"]](item, value)
            for item, value in zip(items, result)
        )
    )


ASSERT["tuple"] = _assert_tuple
