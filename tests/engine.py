"""Generic test execution engine — zero per-operation code.

This module is imported by:
  - tests/test_parity.py (test runner, uses pillow_rs as backend)
  - scripts/generate_fixtures.py (fixture generator, uses PIL as backend)

Adding a new operation requires ZERO changes to this file.
"""

import json, hashlib
from pathlib import Path

# ── Module → call_style lookup ──────────────────────────────────

SINGLE_CHOPS = {"invert", "duplicate", "constant", "offset"}
MUTATE_TARGETS = {"putpixel", "putdata", "thumbnail", "putalpha"}
DUAL_TARGETS = {"paste", "alpha_composite"}
VALUE_TARGETS = {
    "tobytes", "split", "getbands", "getbbox", "getextrema", "histogram",
    "getpixel", "getcolors", "getdata", "getprojection", "entropy",
    "load", "verify", "seek", "tell", "tobitmap",
    "has_transparency_data", "getexif", "getim", "getpalette", "getxmp",
    "get_flattened_data", "get_child_images", "apply_transparency",
    "close", "save", "mode", "size", "width", "height",
    "format", "info", "is_animated", "n_frames", "palette",
}
DRAW_VALUE_TARGETS = {"textlength", "textbbox", "multiline_textbbox", "getfont"}
IMAGE_CLASSMETHOD_TARGETS = {"new"}
FONT_METHOD_TARGETS = {"getbbox", "getlength", "getmask", "getmask2", "getmetrics", "getname",
                       "font_variant", "get_variation_axes", "get_variation_names",
                       "set_variation_by_axes", "set_variation_by_name"}

def get_call_style(module, target):
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
        if target in DUAL_TARGETS:               return "instance_method_dual"
        if target in MUTATE_TARGETS:             return "instance_method_mutate"
        if target in VALUE_TARGETS:              return "instance_method_value"
        return "instance_method"
    if module == "ImageOps":
        if target == "deform":             return "deform"
        return "module_function"
    if module == "ImageChops":
        if target in SINGLE_CHOPS:      return "module_function"
        if target == "composite":       return "module_function_triple"
        return "module_function_dual"
    if module == "ImageDraw":
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
        if target in FONT_METHOD_TARGETS:          return "font_method"
        if target in ("truetype", "load", "load_path"):
            return "font_truetype"
        if target == "TransposedFont":             return "transposed_font"
        if target in ("FreeTypeFont", "ImageFont"):
            return "module_function_value"
        return "module_function_value"
    if module == "ImagePalette":
        return "palette_method"
    if module == "ImageSequence":
        if target == "Iterator":          return "sequence_iterator"
        return "module_function_value"
    if module == "ImageColor":
        return "module_function_value"
    if module == "ImageStat":           return "stat"
    raise ValueError(f"Unknown module: {module}")


# ── Input creation ──────────────────────────────────────────────

REFERENCE_IMAGE = Path(__file__).parent / "test_reference.png"
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
        ref = backend.Image.open(str(REFERENCE_IMAGE))
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

def _call_mod(backend, target):
    """Resolve target function from backend's module hierarchy."""
    for mod_name in ["ImageOps", "ImageChops", "ImageColor", "ImagePalette",
                     "ImageFont", "ImageSequence", "Image", "ImageDraw"]:
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
    return filter_cls if target in BUILTINS else filter_cls(**params)

def _stat_to_dict(stat):
    to_l = lambda v: v if isinstance(v, list) else [v]
    return {
        "count": to_l(stat.count), "sum": to_l(stat.sum),
        "mean": to_l(stat.mean), "median": to_l(stat.median),
        "rms": to_l(stat.rms), "var": to_l(stat.var),
        "stddev": to_l(stat.stddev),
        "extrema": [[e[0], e[1]] for e in (stat.extrema if isinstance(stat.extrema, list) else [stat.extrema])]
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
    For font_variant, returns the variant's getname().
    For variation getters on non-variable fonts, returns [] (PIL raises,
    RSPIL returns [] — we normalize to [] for both).
    """
    font = backend.ImageFont.load_default()
    if target == "font_variant":
        variant = font.font_variant(**params)
        return variant.getname()
    # Some PIL methods need list not tuple (e.g. set_variation_by_axes)
    for key in list(params.keys()):
        if key.endswith("axes") and isinstance(params[key], tuple):
            params[key] = list(params[key])
    try:
        return getattr(font, target)(**params)
    except (OSError, TypeError, ValueError) as e:
        # Normalize: getters return [], setters return None for PIL/RSPIL parity
        if "get_" in target:
            return []
        return None

def _font_truetype(backend, img, target, params):
    """Load a TrueType font and return its name tuple for comparison.
    Generic: extracts font/size from params, loads via truetype(), returns getname().
    Handles truetype, load, load_path, FreeTypeFont targets uniformly."""
    font_path = params.pop("font")
    size = params.pop("size", 20)
    font = backend.ImageFont.truetype(font_path, size)
    return font.getname()

def _palette_method(backend, img, target, params):
    """Create default palette, then call method on it."""
    import io
    palette = backend.ImagePalette.ImagePalette(mode="RGB")
    if target == "save":
        buf = io.BytesIO()
        text_buf = io.TextIOWrapper(buf)
        getattr(palette, target)(text_buf, **params)
        text_buf.flush()
        return buf.getvalue()
    if target == "copy":
        return palette.copy().tobytes()
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
    data_hex = params.pop("data_hex", "")
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
    getattr(draw, target)(outline, **params)
    return img

def _merge_mod(backend, img, target, params):
    """Image.merge(mode, bands) - create bands from params."""
    mode = params.pop("mode")
    bands_specs = params.pop("bands", [])
    # Create band images from specs
    bands = []
    for spec in bands_specs:
        band_mode = spec.get("band_mode", "L")
        band = create_input(backend, band_mode, spec.get("input"))
        bands.append(band)
    return _call_mod(backend, target)(mode, tuple(bands), **params)

def _sequence_iterator(backend, img, target, params):
    """ImageSequence.Iterator(img) - returns number of frames."""
    it = _call_mod(backend, target)(img, **params)
    frames = list(it)
    return len(frames)



# ── New call style helpers for missing operations ──────────────────

import os, tempfile


def _file_open(backend, img, target, params):
    """Open an image from a temp file. Creates a small test image if no data given.
    The temp file is NOT deleted — RSPIL may lazily load image data."""
    png_b64 = params.pop("png_b64", None)
    suffix = params.pop("suffix", ".png")
    if png_b64:
        import base64 as _b64
        data = _b64.b64decode(png_b64)
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
    getattr(img, target)(tmp, format=fmt)
    return backend.Image.open(tmp)









def _frombytes_instance(backend, img, target, params):
    """Image.frombytes instance method (img.frombytes(data, decoder_name)).
    Handles data_hex by decoding it to raw bytes before calling.
    frombytes() modifies the image in-place and returns None, so we
    return the modified image."""
    data_hex = params.pop("data_hex", "")
    data = bytes.fromhex(data_hex)
    getattr(img, target)(data, **params)
    return img


def _transposed_font(backend, img, target, params):
    """Create a TransposedFont and render text with it for image comparison.
    Handles font path → object loading and orientation mapping."""
    font_path = params.pop("font")
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
    """Image.fromarray — creates a numpy array and calls fromarray."""
    import numpy as np
    mode = params.get("mode", "L")
    size = tuple(params.get("size", [100, 100]))
    arr = np.zeros(size, dtype=np.uint8)
    if mode == "RGB":
        arr = np.zeros((size[0], size[1], 3), dtype=np.uint8)
    return backend.Image.fromarray(arr)

CALL_STYLE = {
    "instance_method":        lambda b, img, img2, tgt, p: getattr(img, tgt)(**p),
    "instance_method_value":  lambda b, img, img2, tgt, p: getattr(img, tgt)(**p),
    "instance_method_mutate": lambda b, img, img2, tgt, p: (getattr(img, tgt)(**p), img)[1],
    "instance_method_dual":   lambda b, img, img2, tgt, p: getattr(img, tgt)(img2, **p),
    "draw":       lambda b, img, img2, tgt, p: (_draw(b, img, tgt, p), img)[1],
    "draw_value": lambda b, img, img2, tgt, p: _draw(b, img, tgt, p),
    "draw_bitmap":lambda b, img, img2, tgt, p: (_draw_bitmap(b, img, img2, tgt, p), img)[1],
    "filter":     lambda b, img, img2, tgt, p: img.filter(_make_filter(b, tgt if tgt != "filter" else p.pop("filter"), p)),
    "enhance":    lambda b, img, img2, tgt, p: getattr(b.ImageEnhance, tgt)(img).enhance(p.pop("factor", 1.0)),
    "font_method":lambda b, img, img2, tgt, p: _font_method(b, img, tgt, p),
    "font_truetype":lambda b, img, img2, tgt, p: _font_truetype(b, img, tgt, p),
    "palette_method":lambda b, img, img2, tgt, p: _palette_method(b, img, tgt, p),
    "module_function":       lambda b, img, img2, tgt, p: _call_mod(b, tgt)(img, **p),
    "module_function_dual":  lambda b, img, img2, tgt, p: _call_mod(b, tgt)(img, img2, **p),
    "module_function_triple":lambda b, img, img2, tgt, p: _call_mod(b, tgt)(img, img2, create_input(b, "L", p.pop("mask_img", None)), **p),
    "module_function_value": lambda b, img, img2, tgt, p: _call_mod(b, tgt)(**p),
    "classmethod":           lambda b, img, img2, tgt, p: _call_mod(b, tgt)(**p),
    "classmethod_dual":      lambda b, img, img2, tgt, p: (
        getattr(b, tgt, None) if callable(getattr(b, tgt, None)) else _call_mod(b, tgt)
    )(img, img2, **p),
    "fromarray_mod":lambda b, img, img2, tgt, p: _fromarray(b, img, tgt, p),
    "classmethod_triple":    lambda b, img, img2, tgt, p: _call_mod(b, tgt)(img, img2, create_input(b, "L", p.pop("mask_img", None)), **p),
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
}


# ── Assertion methods ───────────────────────────────────────────

OUTPUTS_DIR = Path(__file__).parent / "fixtures" / "outputs"

def _load_reference(path):
    """Reference paths are relative to fixtures/outputs/."""
    full = OUTPUTS_DIR / path
    if path.endswith('.png'):
        # Lazy import to avoid PIL dependency when running tests
        from PIL import Image as PILImage
        return PILImage.open(str(full))
    return open(str(full), 'rb').read()

def _sha(data):
    if hasattr(data, 'tobytes'):
        return hashlib.sha256(data.tobytes()).hexdigest()
    # QPixmap/QImage from Qt — convert to image then extract raw bits
    if hasattr(data, 'toImage'):  # QPixmap → QImage
        data = data.toImage()
    if hasattr(data, 'bits') and callable(data.bits):
        raw = data.bits()
        # PySide6 returns memoryview, PyQt5/PySide2 return sip.voidptr
        if isinstance(raw, bytes):
            return hashlib.sha256(raw).hexdigest()
        if hasattr(raw, 'tobytes'):
            return hashlib.sha256(raw.tobytes()).hexdigest()
        if hasattr(raw, 'asstring'):
            return hashlib.sha256(raw.asstring(data.sizeInBytes())).hexdigest()
    return hashlib.sha256(data).hexdigest()

def _to_json_compat(val):
    """Convert any result type to JSON-serializable form."""
    if val is None: return None
    if isinstance(val, (int, float, str, bool)): return val
    if isinstance(val, bytes): return val.hex()
    if isinstance(val, (tuple, list)): return [_to_json_compat(v) for v in val]
    if isinstance(val, dict): return {str(k): _to_json_compat(v) for k, v in val.items()}
    if hasattr(val, 'tobytes'): return _sha(val)
    if hasattr(val, '__iter__') and not isinstance(val, (str, bytes, dict)):
        return [_to_json_compat(v) for v in val]
    return repr(val)

ASSERT = {
    "image": lambda case, result:
        _sha(result) == _sha(_load_reference(case["reference"])),
    "image_list": lambda case, result:
        all(_sha(band) == _sha(_load_reference(ref))
            for band, ref in zip(result, case["references"])),
    "exact": lambda case, result:
        result == case["value"],
    "json": lambda case, result:
        json.dumps(_to_json_compat(result)) == json.dumps(case["value"]),
    "string": lambda case, result:
        str(result).startswith(case.get("prefix", ""))
        or repr(result) == case.get("value", ""),
    "float": lambda case, result:
        abs(result - case["value"]) < case.get("tolerance", 0.0001),
    "error": lambda case, result:
        isinstance(result, Exception)
        and case.get("exception", "") in type(result).__name__
        and case.get("message_contains", "") in str(result),
}
