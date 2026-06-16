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

def get_call_style(module, target):
    """Return the call_style string for any (module, target) pair.

    Pure data lookup. Never needs new entries for new operations
    — new operations in an existing module resolve automatically.
    """
    if module == "Image":
        if target in DUAL_TARGETS:      return "instance_method_dual"
        if target in MUTATE_TARGETS:    return "instance_method_mutate"
        if target in VALUE_TARGETS:     return "instance_method_value"
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
        if target == "eval":               return "eval"
        if target in ("blend", "alpha_composite"):
            return "classmethod_dual"
        if target == "composite":          return "classmethod_triple"
        if target == "frombytes":          return "frombytes_mod"
        if target == "frombuffer":         return "frombuffer_mod"
        if target == "merge":              return "merge_mod"
        return "classmethod"
    if module == "ImageFont":
        if target in ("getbbox", "getlength", "getmask", "getmask2", "getmetrics", "getname", "font_variant"):
            return "font_method"
        if target == "truetype":
            return "font_truetype"
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
    """Load default font, then call method on it."""
    font = backend.ImageFont.load_default()
    if target == "font_variant":
        variant = font.font_variant(**params)
        return variant.getname()
    return getattr(font, target)(**params)

def _font_truetype(backend, img, target, params):
    """Load a TrueType font and return its name tuple for comparison."""
    font = _call_mod(backend, target)(**params)
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

CALL_STYLE = {
    "instance_method":        lambda b, img, img2, tgt, p: getattr(img, tgt)(**p),
    "instance_method_value":  lambda b, img, img2, tgt, p: getattr(img, tgt)(**p),
    "instance_method_mutate": lambda b, img, img2, tgt, p: (getattr(img, tgt)(**p), img)[1],
    "instance_method_dual":   lambda b, img, img2, tgt, p: getattr(img, tgt)(img2, **p),
    "draw":       lambda b, img, img2, tgt, p: (_draw(b, img, tgt, p), img)[1],
    "draw_value": lambda b, img, img2, tgt, p: _draw(b, img, tgt, p),
    "draw_bitmap":lambda b, img, img2, tgt, p: (_draw_bitmap(b, img, img2, tgt, p), img)[1],
    "filter":     lambda b, img, img2, tgt, p: img.filter(_make_filter(b, tgt, p)),
    "enhance":    lambda b, img, img2, tgt, p: getattr(b.ImageEnhance, tgt)(img).enhance(p.pop("factor", 1.0)),
    "font_method":lambda b, img, img2, tgt, p: _font_method(b, img, tgt, p),
    "font_truetype":lambda b, img, img2, tgt, p: _font_truetype(b, img, tgt, p),
    "palette_method":lambda b, img, img2, tgt, p: _palette_method(b, img, tgt, p),
    "module_function":       lambda b, img, img2, tgt, p: _call_mod(b, tgt)(img, **p),
    "module_function_dual":  lambda b, img, img2, tgt, p: _call_mod(b, tgt)(img, img2, **p),
    "module_function_triple":lambda b, img, img2, tgt, p: _call_mod(b, tgt)(img, img2, create_input(b, "L", p.pop("mask_img", None)), **p),
    "module_function_value": lambda b, img, img2, tgt, p: _call_mod(b, tgt)(**p),
    "classmethod":           lambda b, img, img2, tgt, p: _call_mod(b, tgt)(**p),
    "classmethod_dual":      lambda b, img, img2, tgt, p: _call_mod(b, tgt)(img, img2, **p),
    "classmethod_triple":    lambda b, img, img2, tgt, p: _call_mod(b, tgt)(img, img2, create_input(b, "L", p.pop("mask_img", None)), **p),
    "draw_shape":            lambda b, img, img2, tgt, p: _draw_shape(b, img, tgt, p),
    "frombytes_mod":         lambda b, img, img2, tgt, p: _frombytes_mod(b, img, tgt, p),
    "frombuffer_mod":        lambda b, img, img2, tgt, p: _frombuffer_mod(b, img, tgt, p),
    "merge_mod":             lambda b, img, img2, tgt, p: _merge_mod(b, img, tgt, p),
    "sequence_iterator":     lambda b, img, img2, tgt, p: _sequence_iterator(b, img, tgt, p),
    "deform":                lambda b, img, img2, tgt, p: _deform_op(b, img, tgt, p),
    "eval":                   lambda b, img, img2, tgt, p: _eval_image(b, img, tgt, p),
    "stat": lambda b, img, img2, tgt, p: _stat_to_dict(getattr(b.ImageStat, tgt)(img)),
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
