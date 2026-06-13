#!/usr/bin/env python3
"""Shared fixture execution engine — imported by both generator and tests.

Provides `execute(op, img, img2, params)` that runs any operation defined
in ops_registry.py. Returns the result (Image, bytes, or value).
"""
import hashlib
from pathlib import Path
import PIL.Image, PIL.ImageFilter, PIL.ImageChops, PIL.ImageOps
import PIL.ImageEnhance, PIL.ImageColor, PIL.ImagePalette, PIL.ImageFont
import PIL.ImageStat, PIL.ImageSequence, PIL.ImageDraw

from .ops_registry import REGISTRY

ROOT = Path(__file__).parent.parent.parent
REF_PATH = ROOT / "tests" / "test_reference.png"


# ── Reference image loading ────────────────────────────────
_ref_rgb = None

def _get_reference():
    """Load reference image, cached once per process."""
    global _ref_rgb
    if _ref_rgb is None:
        _ref_rgb = PIL.Image.open(REF_PATH).resize((100, 100), PIL.Image.LANCZOS)
    return _ref_rgb.copy()


def make_input(mode):
    """Create a PIL test image in the given mode."""
    ref = _get_reference()
    if mode == "RGB": return ref.copy()
    if mode == "RGBA": return ref.convert("RGBA")
    if mode == "L": return ref.convert("L")
    if mode == "LA": return ref.convert("LA")
    if mode == "1": return ref.convert("1", dither=PIL.Image.NONE)
    if mode == "P": return ref.convert("P")
    if mode == "CMYK": return ref.convert("CMYK")
    if mode == "YCbCr": return ref.convert("YCbCr")
    if mode == "HSV": return ref.convert("HSV")
    if mode == "I": return ref.convert("I")
    if mode == "F": return ref.convert("F")
    return ref.copy()


def _serialize_value(val):
    """Convert PIL return value to JSON-serializable form."""
    if val is None:
        return None
    if isinstance(val, (int, float, str, bool)):
        return val
    if isinstance(val, bytes):
        return "b'" + val.hex() + "'"
    if isinstance(val, tuple):
        return [_serialize_value(v) for v in val]
    if isinstance(val, list):
        return [_serialize_value(v) for v in val]
    if isinstance(val, dict):
        return {str(k): _serialize_value(v) for k, v in val.items()}
    if hasattr(val, '__iter__') and not isinstance(val, (str, bytes)):
        try:
            return [_serialize_value(v) for v in list(val)[:1000]]
        except Exception:
            pass
    return str(val)


# ── Fixture execution ─────────────────────────────────────

def execute(op_name, img, img2=None, params_override=None):
    """Execute an operation against img (and optionally img2).

    Uses REGISTRY to determine how to execute the operation.
    params_override merges with defaults from registry.
    """
    spec = REGISTRY[op_name]
    params = dict(spec.get("params", {}))
    if params_override:
        params.update(params_override)
    typ = spec["type"]

    if typ == "image":
        method = spec["method"]
        return _exec_image(img, method, params)

    elif typ == "filter":
        return _exec_filter(img, spec, params)

    elif typ == "dual":
        return _exec_dual(op_name, img, img2, spec, params)

    elif typ == "draw":
        return _exec_draw(img, spec, params)

    elif typ == "enhance":
        return _exec_enhance(img, spec, params)

    elif typ == "module":
        return _exec_module(op_name, img, img2, spec, params)

    elif typ == "value":
        return _exec_value(img, spec, params)

    raise ValueError(f"Unknown operation type: {typ}")


def _exec_image(img, method, params):
    """Call an Image instance method: img.method(**params)."""
    # Special handling for convert
    if method == "convert" and params.get("mode") == "__CONVERT_TO__":
        target = "RGB" if img.mode != "RGB" else "L"
        params["mode"] = target
    # Special handling for putdata (needs correct tuple/list format)
    if method == "putdata":
        n_px = img.size[0] * img.size[1]
        n_b = len(img.getbands())
        if n_b > 1:
            params["data"] = [(128,) * n_b] * n_px
        else:
            params["data"] = [128] * n_px
    # Special handling for putpixel (color depends on mode)
    if method == "putpixel":
        n_b = len(img.getbands())
        v = params.get("value", [255])
        if n_b == 1:
            params["value"] = 255 if isinstance(v, list) else v
        elif n_b == 2:
            params["value"] = (255, 255)
        elif n_b == 3:
            params["value"] = (255, 255, 255)
        else:
            params["value"] = (255, 255, 255, 255)
    return getattr(img, method)(**params)


def _exec_filter(img, spec, params):
    """Apply a PIL filter: create filter object, then img.filter(obj)."""
    name = spec["name"]
    filter_class = getattr(PIL.ImageFilter, name)
    if name in ("BLUR", "CONTOUR", "DETAIL", "EDGE_ENHANCE", "EDGE_ENHANCE_MORE",
                "EMBOSS", "FIND_EDGES", "SHARPEN", "SMOOTH", "SMOOTH_MORE"):
        return img.filter(filter_class)
    # Parametric filter
    constructor_params = {k: v for k, v in params.items() if k in spec.get("params", {})}
    return img.filter(filter_class(**constructor_params))


def _exec_dual(op_name, img, img2, spec, params):
    """Two-image operation: ImageChops or Image.blend/composite/merge."""
    # Input prep (e.g. logical ops convert to mode 1)
    if spec.get("prep"):
        prep_code = spec["prep"]
        if "convert" in prep_code:
            img = img.convert("1", dither=PIL.Image.NONE)
            img2 = img2.convert("1", dither=PIL.Image.NONE) if img2 else None

    mod = op_name.rsplit(".", 1)[0]
    func_name = op_name.rsplit(".", 1)[1]
    if spec.get("function"):
        func_name = spec["function"].rsplit(".", 1)[1]
        mod = spec["function"].rsplit(".", 1)[0]

    # Map to PIL module
    module_map = {
        "ImageChops": PIL.ImageChops,
        "Image": PIL.Image,
        "ImageModule": PIL.Image,
    }
    pil_mod = module_map.get(mod, PIL.ImageChops)
    pil_func = getattr(pil_mod, func_name)

    if func_name in ("blend", "composite", "merge"):
        return pil_func(img, img2, **params)
    elif func_name == "composite" and mod == "Image":
        mask = PIL.Image.new("L", img.size, 128)
        return pil_func(img, img2, mask)
    else:
        return pil_func(img, img2)


def _exec_draw(img, spec, params):
    """Draw on image, return modified image."""
    draw = PIL.ImageDraw.Draw(img)
    draw_method = spec["draw"]
    # RGB modes use colored fill
    if img.mode in ("RGB", "RGBA") and "fill" in params and isinstance(params["fill"], int):
        params["fill"] = (0, 255, 0)
    if img.mode in ("RGB", "RGBA") and "outline" in params and isinstance(params["outline"], int):
        params["outline"] = (0, 255, 0)
    # bitmap needs a mask image
    if spec["draw"] == "bitmap":
        bmp = make_input("1") if img.mode != "1" else img.convert("1")
        draw.bitmap(tuple(params["xy"]), bmp, fill=params.get("fill"))
        return img
    getattr(draw, draw_method)(**params)
    return img


def _exec_enhance(img, spec, params):
    """Enhance operation: create enhancer, enhance, return result."""
    enhancer_class = getattr(PIL.ImageEnhance, spec["name"])
    return enhancer_class(img).enhance(params.get("factor", 1.5))


def _exec_module(op_name, img, img2, spec, params):
    """Module-level function: Image.new, Image.open, etc."""
    func_name = spec.get("function", op_name)
    mod_name, fn_name = func_name.rsplit(".", 1)
    pil_mod = getattr(PIL, mod_name) if hasattr(PIL, mod_name) else PIL.Image
    pil_func = getattr(pil_mod, fn_name)
    if fn_name == "new":
        return pil_func(params.get("mode", "RGB"), tuple(params.get("size", [100, 100])),
                        params.get("color", 0))
    elif fn_name == "effect_noise":
        return pil_func(tuple(params["size"]), params["sigma"])
    elif fn_name == "eval":
        return PIL.Image.eval(img, lambda x: min(255, x + 10))
    elif fn_name == "frombytes":
        return img  # already created
    elif fn_name == "open":
        return img  # already loaded
    elif fn_name == "merge":
        bands = img.split()
        return PIL.Image.merge(img.mode, bands)
    elif fn_name == "alpha_composite":
        fg = make_input(img.mode)
        fg.putalpha(128)
        try:
            img.alpha_composite(fg)
        except Exception:
            pass
        return img
    return img


def _exec_value(img, spec, params):
    """Return a value (non-image): property, function call, etc."""
    if "property" in spec:
        prop = spec["property"]
        if prop in ("mode", "size", "width", "height", "format", "info"):
            return getattr(img, prop)
        elif prop in ("getexif", "getim", "getpalette", "getxmp",
                      "get_flattened_data", "get_child_images",
                      "apply_transparency", "is_animated", "n_frames",
                      "has_transparency_data"):
            try:
                return getattr(img, prop)()
            except Exception:
                return None
        elif prop == "show":
            return None
        elif prop == "palette":
            return None
        return None
    if "method" in spec:
        method = spec["method"]
        if method == "load":
            return str(getattr(img, method)())
        if method in ("tobytes", "tobitmap", "close", "verify"):
            return getattr(img, method)()
        if method == "split":
            return getattr(img, method)()[0]
        if method == "getbands": return getattr(img, method)()
        if method == "getbbox": return getattr(img, method)()
        if method == "getextrema": return getattr(img, method)()
        if method == "histogram": return getattr(img, method)()
        if method == "getpixel":
            return getattr(img, method)(tuple(params["xy"]))
        if method == "getcolors":
            return getattr(img, method)(params.get("maxcolors", 256))
        if method == "getdata":
            band = params.get("band", -1)
            return getattr(img, method)(band)
        if method == "getprojection": return getattr(img, method)()
        if method == "entropy": return getattr(img, method)()
        if method == "seek": getattr(img, method)(params["frame"]); return None
        if method == "tell": return getattr(img, method)()
        return getattr(img, method)()
    if "function" in spec:
        func_name = spec["function"]
        mod_name, fn_name = func_name.rsplit(".", 1)
        if mod_name == "ImageColor":
            color = params.get("color", "red")
            mode = params.get("mode", "RGB")
            return getattr(PIL.ImageColor, fn_name)(color, mode) if fn_name == "getcolor" else getattr(PIL.ImageColor, fn_name)(color)
        elif mod_name == "ImagePalette":
            palette = img.getpalette() or img.palette
            if fn_name == "getdata":
                try:
                    return PIL.ImagePalette.ImagePalette(mode="RGB").getdata()
                except Exception:
                    return []
            elif fn_name == "copy":
                try:
                    p = PIL.ImagePalette.ImagePalette(mode="RGB")
                    return p.copy().tobytes()
                except Exception:
                    return bytes()
            elif fn_name == "save":
                return None
            elif fn_name == "tobytes":
                try:
                    return PIL.ImagePalette.ImagePalette(mode="RGB").tobytes()
                except Exception:
                    return bytes()
            elif fn_name == "getcolor":
                return PIL.ImagePalette.ImagePalette(mode="RGB").getcolor(tuple(params["color"]))
        elif mod_name == "ImageFont":
            if fn_name == "load_default":
                try:
                    font = PIL.ImageFont.load_default()
                    return str(type(font).__name__)
                except Exception:
                    return None
            elif fn_name == "load_default_imagefont":
                try:
                    font = PIL.ImageFont.ImageFont()
                    return str(type(font).__name__)
                except Exception:
                    return None
            elif fn_name == "load":
                return None
            elif fn_name == "load_path":
                return None
            elif fn_name == "truetype":
                return None
            elif fn_name == "FreeTypeFont":
                return None
            elif fn_name == "ImageFont":
                return None
        elif mod_name == "ImageStat":
            s = PIL.ImageStat.Stat(img)
            to_list = lambda v: v if isinstance(v, list) else [v]
            result = {
                'count': to_list(s.count), 'sum': to_list(s.sum),
                'mean': to_list(s.mean), 'median': to_list(s.median),
                'rms': to_list(s.rms), 'var': to_list(s.var),
                'stddev': to_list(s.stddev),
                'extrema': [[e[0], e[1]] for e in (s.extrema if isinstance(s.extrema, list) else [s.extrema])]
            }
            return result
        elif mod_name == "ImageSequence":
            return None
    return None


def generate_fixture(op_name, mode):
    """Generate a single fixture: create input, run op, return fixture dict."""
    spec = REGISTRY[op_name]
    img = make_input(mode)
    img2 = make_input(mode) if spec["type"] in ("dual",) else None

    result = execute(op_name, img, img2)

    fixture = {
        "op": op_name,
        "mode": mode,
        "params": spec.get("params", {}),
        "targets": ["cpu"],
        "inputMode": mode,
        "inputSize": list(img.size),
        "inputBytes": img.tobytes().hex(),
        "inputBytesRgb": _get_reference().tobytes().hex(),
    }

    if hasattr(result, 'tobytes'):
        fixture["expectedHash"] = hashlib.sha256(result.tobytes()).hexdigest()
    elif isinstance(result, bytes):
        fixture["expectedHash"] = hashlib.sha256(result).hexdigest()
    elif isinstance(result, (int, float, str, bool, list, tuple, dict, type(None))):
        fixture["expectedValue"] = _serialize_value(result)
    else:
        fixture["expectedValue"] = str(result) if result is not None else None

    return fixture


def generate_all(output_dir=None):
    """Generate all fixtures defined in the registry."""
    import json, sys
    if output_dir is None:
        output_dir = ROOT / "tests" / "fixtures"
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    count = 0
    for op_name, spec in REGISTRY.items():
        for mode in spec.get("modes", ["L", "RGB"]):
            try:
                fixture = generate_fixture(op_name, mode)
                fname = op_name.replace(".", "_") + "_" + mode + ".json"
                with open(output_dir / fname, "w") as f:
                    json.dump(fixture, f, indent=2)
                count += 1
            except Exception as e:
                print(f"  SKIP {op_name} x {mode}: {e}", file=sys.stderr)
    print(f"Generated {count} fixtures in {output_dir}")


if __name__ == "__main__":
    generate_all()
