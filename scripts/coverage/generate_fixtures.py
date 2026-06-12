#!/usr/bin/env python3
"""Generate WASM test fixtures from PIL reference outputs.

For each (operation, mode) with any target, runs PIL operation,
captures output (image bytes OR return value), and writes a JSON fixture.

JS/WASM tests and Python fixture tests load the same fixtures and
compare outputs (hash for images, value for non-image returns).

Usage: python scripts/coverage/generate_fixtures.py [--target wasm|wasm_gpu]
"""
import sys, json, hashlib, yaml
from pathlib import Path
from io import BytesIO

ROOT = Path(__file__).parent.parent.parent
MANIFEST_PATH = ROOT / "manifest.yaml"
FIXTURES_DIR = ROOT / "tests" / "fixtures"

import PIL.Image as PILImage
import PIL.ImageDraw as PILImageDraw
import PIL.ImageOps as PILImageOps
import PIL.ImageChops as PILImageChops
import PIL.ImageFilter as PILFilter
import PIL.ImageEnhance as PILImageEnhance
import PIL.ImageColor as PILImageColor
import PIL.ImagePalette as PILImagePalette
import PIL.ImageFont as PILImageFont
import PIL.ImageStat as PILImageStat
import PIL.ImageSequence as PILImageSequence


_REFERENCE_RGB = None

def _get_reference():
    """Load complex reference image (gradients, shapes, text)."""
    global _REFERENCE_RGB
    if _REFERENCE_RGB is None:
        ref_path = ROOT / "tests" / "test_reference.png"
        if ref_path.exists():
            _REFERENCE_RGB = PILImage.open(ref_path).resize((100, 100), PILImage.LANCZOS)
        else:
            _REFERENCE_RGB = PILImage.new("RGB", (100, 100), (128, 128, 128))
    return _REFERENCE_RGB.copy()


def _make_image(mode, size=(100, 100)):
    """Create PIL image from complex reference for realistic pixel variety."""
    ref = _get_reference()
    if mode == "RGB": return ref
    if mode == "RGBA": return ref.convert("RGBA")
    if mode == "L": return ref.convert("L")
    if mode == "LA": return ref.convert("LA")
    if mode == "1": return ref.convert("1")
    if mode == "P": return ref.convert("P")
    if mode == "CMYK": return ref.convert("CMYK")
    if mode == "YCbCr": return ref.convert("YCbCr")
    if mode == "HSV": return ref.convert("HSV")
    if mode == "I": return ref.convert("I")
    if mode == "F": return ref.convert("F")
    return ref


# ── Non-image return ops (return values, not images) ───────────────
# These ops return primitives (int, float, str, tuple, list) that
# should be stored as JSON values, not hashed bytes.

_NON_IMAGE_OPS = {
    "entropy", "getbbox", "getextrema", "histogram", "getpixel",
    "getcolors", "getdata", "getprojection", "getbands",
    "close", "load", "verify", "seek", "tell",
    "format", "mode", "size", "width", "height", "info",
    "palette", "is_animated", "n_frames", "has_transparency_data",
    "getim",  # returns PyCapsule / low-level imaging object
}

_VALUE_OPS = _NON_IMAGE_OPS  # alias


def _serialize_value(val):
    """Convert PIL return value to JSON-serializable form."""
    if val is None:
        return None
    if isinstance(val, (int, float, str, bool)):
        return val
    if isinstance(val, tuple):
        return [_serialize_value(v) for v in val]
    if isinstance(val, list):
        return [_serialize_value(v) for v in val]
    if isinstance(val, dict):
        return {str(k): _serialize_value(v) for k, v in val.items()}
    if hasattr(val, '__iter__') and not isinstance(val, (str, bytes)):
        # Lazy sequences (e.g., getdata's ImagingCore)
        try:
            return [_serialize_value(v) for v in list(val)[:1000]]  # cap at 1000
        except Exception:
            pass
    # Fallback: convert to string
    return str(val)


def run_pil(op_name, mode):
    """Run a PIL operation and return (status, data, params, input_bytes, input_size, input_bytes_rgb) tuple."""
    ref_rgb = _get_reference()
    input_size = list(ref_rgb.size)
    input_bytes_rgb = ref_rgb.tobytes()
    img = _make_image(mode)
    input_bytes = img.tobytes()  # bytes in target mode
    module, func = op_name.rsplit(".", 1)

    try:
        if module == "Image":
            result, params = _run_image_op(img, func, mode)
        elif module == "ImageOps":
            result, params = _run_imageops(img, func, mode)
        elif module == "ImageChops":
            result, params = _run_chops(img, func, mode)
        elif module == "ImageFilter":
            result, params = _run_filter(img, func)
        elif module == "ImageEnhance":
            result, params = getattr(PILImageEnhance, func)(img).enhance(1.5), {"factor": 1.5}
        elif module == "ImageModule":
            result, params = _run_module_func(img, func, mode)
        elif module == "ImageDraw":
            result, params = _run_draw(img, func, mode)
        elif module == "ImageColor":
            result, params = _run_color(func, mode)
        elif module == "ImagePalette":
            result, params = _run_palette(img, func, mode)
        elif module == "ImageFont":
            result, params = _run_font(func, mode)
        elif module == "ImageStat":
            result, params = _run_stat(img, func, mode)
        elif module == "ImageSequence":
            result, params = _run_sequence(img, func, mode)
        else:
            return None

        # Determine fixture type: image (hash) or value (JSON)
        if func in _VALUE_OPS or module in ("ImageColor", "ImageStat"):
            return ('value', _serialize_value(result), params, input_bytes, input_size, input_bytes_rgb)
        elif result is None:
            return ('value', None, params, input_bytes, input_size, input_bytes_rgb)
        elif hasattr(result, 'tobytes'):
            return ('success', result.tobytes(), params, input_bytes, input_size, input_bytes_rgb)
        elif isinstance(result, bytes):
            return ('success', result, params, input_bytes, input_size, input_bytes_rgb)
        elif hasattr(result, 'save'):
            buf = BytesIO()
            result.save(buf, format="PNG")
            return ('success', buf.getvalue(), params, input_bytes, input_size, input_bytes_rgb)
        elif isinstance(result, (int, float, str, bool, list, tuple, dict)):
            return ('value', _serialize_value(result), params, input_bytes, input_size, input_bytes_rgb)
        else:
            return None
    except Exception as e:
        return ('error', f"{type(e).__name__}: {str(e)[:100]}", {},
                input_bytes, input_size, input_bytes_rgb)


def _run_image_op(img, func, mode):
    """Dispatch Image instance method operations. Returns (result, params)."""
    if func in ("resize",): return img.resize((50, 50)), {"size": [50, 50]}
    if func in ("crop",): return img.crop((25, 25, 75, 75)), {"box": [25, 25, 75, 75]}
    if func in ("rotate",): return img.rotate(90), {"angle": 90}
    if func in ("transpose",): return img.transpose(PILImage.FLIP_LEFT_RIGHT), {"method": "FLIP_LEFT_RIGHT"}
    if func in ("filter",): return img.filter(PILFilter.BLUR), {"type": "BLUR"}
    if func in ("convert",):
        target = "RGB" if img.mode != "RGB" else "L"
        return img.convert(target), {"mode": target}
    if func in ("thumbnail",):
        img.thumbnail((50, 50))
        return img, {"size": [50, 50]}
    if func in ("copy", "tobytes", "load", "close", "verify"):
        return getattr(img, func)(), {}
    if func in ("split",):
        bands = img.split()
        return bands[0], {"band": 0}
    if func in ("getbands",):
        return img.getbands(), {}
    if func in ("getbbox",):
        return img.getbbox(), {}
    if func in ("getextrema",):
        return img.getextrema(), {}
    if func in ("histogram",):
        mask = None
        return img.histogram(mask), {}
    if func in ("getpixel",):
        return img.getpixel((50, 50)), {"xy": [50, 50]}
    if func in ("getcolors",):
        return img.getcolors(maxcolors=256), {"maxcolors": 256}
    if func in ("getdata",):
        data = list(img.getdata())
        return data[:100], {"count": 100, "total": len(data)}  # sample
    if func in ("getprojection",):
        return img.getprojection(), {}
    if func in ("entropy",):
        mask = None
        return img.entropy(mask), {}
    if func in ("seek",):
        try:
            img.seek(0)
            return None, {"frame": 0}
        except Exception:
            return None, {}
    if func in ("tell",):
        try:
            return img.tell(), {}
        except Exception:
            return 0, {}
    if func in ("paste",):
        paste_img = _make_image(mode, (10, 10))
        img.paste(paste_img, (0, 0))
        return img, {"size": [10, 10], "position": [0, 0]}
    if func in ("alpha_composite",):
        fg = _make_image("RGBA", (10, 10))
        try:
            img.alpha_composite(fg)
        except Exception:
            pass
        return img, {"fgSize": [10, 10]}
    if func in ("point",):
        lut = bytes([min(255, i + 50) for i in range(256)])
        return img.point(lut), {"lutSize": 256, "offset": 50}
    if func in ("putalpha",):
        try:
            img.putalpha(128)
        except Exception:
            pass
        return img, {"alpha": 128}
    if func in ("putdata",):
        n = img.size[0] * img.size[1]
        data = [128] * n
        try:
            img.putdata(data)
        except Exception:
            pass
        return img, {"count": n}
    if func in ("quantize",): return img.quantize(16), {"colors": 16}
    if func in ("reduce",): return img.reduce(2), {"factor": 2}
    if func in ("effect_spread",): return img.effect_spread(2), {"distance": 2}
    if func in ("transform",):
        return img.transform((50, 50), PILImage.AFFINE, (1, 0, 0, 0, 1, 0)), {"size": [50, 50], "method": "AFFINE"}
    if func in ("getchannel",):
        bands = img.getbands()
        if bands:
            return img.getchannel(bands[0]), {"channel": 0}
        return img.getchannel(0), {"channel": 0}
    if func in ("putpixel",):
        n_bands = len(img.getbands())
        if n_bands >= 4:
            img.putpixel((0, 0), (255, 0, 0, 255))
        else:
            img.putpixel((0, 0), (255, 0, 0))
        return img, {}
    if func in ("apply_transparency",):
        try:
            return img.apply_transparency(), {}
        except Exception:
            return img, {}
    if func in ("getpalette",):
        try:
            pal = img.getpalette()
            return pal[:32] if pal else None, {}
        except Exception:
            return None, {}
    if func in ("putpalette",):
        try:
            img.putpalette([0, 0, 0, 255, 255, 255] * 128)
        except Exception:
            pass
        return img, {}
    if func in ("remap_palette",):
        try:
            return img.remap_palette([0, 1, 2, 3], [128, 128, 128, 128]), {}
        except Exception:
            return img, {}
    if func in ("tobitmap",):
        try:
            return img.tobitmap(), {}
        except Exception:
            return img, {}
    if func in ("draft",):
        try:
            img.draft(mode, (50, 50))
        except Exception:
            pass
        return img, {"mode": mode, "size": [50, 50]}
    if func in ("effect_noise",):
        return PILImage.effect_noise(img.size, 10), {"sigma": 10}
    if func in ("format", "mode", "size", "width", "height", "info",
                "palette", "is_animated", "n_frames", "has_transparency_data"):
        val = getattr(img, func, None)
        if callable(val):
            val = val()
        return val, {}
    if func in ("getexif", "getim", "getxmp", "get_child_images",
                "get_flattened_data", "show"):
        try:
            val = getattr(img, func, None)
            if callable(val):
                val = val()
            return val, {}
        except Exception:
            return None, {}
    if func in ("save",):
        buf = BytesIO()
        try:
            img.save(buf, format="PNG")
            return buf.getvalue(), {"format": "PNG"}
        except Exception:
            return None, {}
    return img, {}


def _run_imageops(img, func, mode):
    """Dispatch ImageOps functions. Returns (result, params)."""
    if func in ("autocontrast", "equalize", "invert", "flip", "mirror",
                "grayscale"):
        return getattr(PILImageOps, func)(img), {"func": func}
    if func in ("posterize",):
        return getattr(PILImageOps, func)(img, 4), {"func": func, "bits": 4}
    if func in ("solarize",):
        return getattr(PILImageOps, func)(img, 128), {"func": func, "threshold": 128}
    if func in ("contain", "cover", "fit", "pad", "scale"):
        return getattr(PILImageOps, func)(img, (25, 25)), {"size": [25, 25], "func": func}
    if func in ("expand",):
        return getattr(PILImageOps, func)(img, 5), {"border": 5, "func": func}
    if func in ("crop",):
        return getattr(PILImageOps, func)(img, 5), {"border": 5, "func": func}
    if func in ("colorize",):
        return getattr(PILImageOps, func)(img, "black", "white"), {"func": func, "black": "black", "white": "white"}
    if func in ("exif_transpose",):
        return img, {}
    if func in ("deform",): return img, {}
    return img, {}


def _run_chops(img, func, mode):
    """Dispatch ImageChops functions."""
    img2 = _make_image(mode, img.size)
    dual = ("add", "subtract", "multiply", "screen", "darker", "lighter", "difference",
            "add_modulo", "subtract_modulo",
            "hard_light", "soft_light", "overlay", "logical_and", "logical_or", "logical_xor")
    if func in dual:
        return getattr(PILImageChops, func)(img, img2), {"func": func}
    if func in ("blend",):
        return getattr(PILImageChops, func)(img, img2, 0.5), {"func": func, "alpha": 0.5}
    if func in ("composite",):
        return getattr(PILImageChops, func)(img, img2, img2), {"func": func}
    if func in ("invert", "constant", "duplicate", "offset"):
        if func == "offset":
            return getattr(PILImageChops, func)(img, 5, 5), {"func": func, "x": 5, "y": 5}
        if func == "constant":
            return getattr(PILImageChops, func)(img, 128), {"func": func, "value": 128}
        return getattr(PILImageChops, func)(img), {"func": func}
    return img, {}


def _run_filter(img, func):
    """Dispatch ImageFilter operations."""
    PARAMETRIC = {
        "BoxBlur": lambda c: c(1),
        "GaussianBlur": lambda c: c(1),
        "Kernel": lambda c: c((3, 3), [1] * 9, 9, 0),
        "RankFilter": lambda c: c(3, 1),
        "Color3DLUT": lambda c: c(17, "RGB"),
        "UnsharpMask": lambda c: c(2, 150, 0),
        "MaxFilter": lambda c: c(3),
        "MinFilter": lambda c: c(3),
        "MedianFilter": lambda c: c(3),
        "ModeFilter": lambda c: c(3),
    }
    filt_cls = getattr(PILFilter, func, None)
    if filt_cls is None:
        return img.filter(PILFilter.BLUR), {"type": "BLUR"}
    if func in PARAMETRIC:
        filt = PARAMETRIC[func](filt_cls)
    else:
        filt = filt_cls  # Singleton: BLUR, CONTOUR, DETAIL, etc.
    return img.filter(filt), {"type": func}


def _run_module_func(img, func, mode):
    """Dispatch ImageModule functions."""
    if func == "merge":
        bands = img.split()
        try:
            return PILImage.merge(mode, bands), {"func": "merge", "mode": mode}
        except Exception:
            return img, {}
    if func == "effect_noise":
        return PILImage.effect_noise(img.size, 10), {"func": "effect_noise", "sigma": 10}
    if func in ("blend",):
        img2 = _make_image(mode, img.size)
        return PILImage.blend(img, img2, 0.5), {"func": "blend", "alpha": 0.5}
    if func in ("composite",):
        img2 = _make_image(mode, img.size)
        mask = PILImage.new("L", img.size, 128)
        return PILImage.composite(img, img2, mask), {"func": "composite"}
    if func in ("eval",):
        return PILImage.eval(img, lambda x: min(255, x + 10)), {"func": "eval", "offset": 10}
    if func in ("alpha_composite",):
        fg = _make_image("RGBA", (10, 10))
        img2 = img.copy()
        try:
            img2.alpha_composite(fg)
        except Exception:
            pass
        return img2, {}
    if func in ("new", "open", "fromarray", "frombytes"):
        return img, {}
    return img, {}


def _run_draw(img, func, mode):
    """Dispatch ImageDraw operations."""
    draw = PILImageDraw.Draw(img)
    fill = 200 if mode in ("L",) else (0, 255, 0)
    if func in ("line",):
        draw.line([(10, 10), (40, 40)], fill=fill)
    elif func in ("rectangle",):
        draw.rectangle([10, 10, 40, 40], outline=fill)
    elif func in ("ellipse",):
        draw.ellipse([10, 10, 40, 40], outline=fill)
    elif func in ("polygon",):
        draw.polygon([(10, 10), (40, 10), (25, 40)], outline=fill)
    elif func in ("arc", "chord", "pieslice"):
        getattr(draw, func)([10, 10, 40, 40], 0, 180, fill=fill)
    elif func in ("circle",):
        draw.circle((25, 25), 15, fill=fill)
    elif func in ("point",):
        draw.point((25, 25), fill=fill)
    elif func in ("text",):
        draw.text((5, 5), "Test", fill=fill)
    elif func in ("rounded_rectangle",):
        draw.rounded_rectangle([10, 10, 40, 40], radius=5, outline=fill)
    elif func in ("bitmap",):
        bitmap = _make_image("L", (10, 10))
        draw.bitmap((5, 5), bitmap, fill=fill)
    elif func in ("textbbox", "multiline_textbbox", "textlength"):
        return img, {}
    elif func in ("getfont",):
        return img, {}
    elif func in ("multiline_text",):
        draw.multiline_text((5, 5), "Line1\nLine2", fill=fill)
    elif func in ("multiline_size",):
        return img, {}
    elif func in ("regular_polygon",):
        try:
            draw.regular_polygon((25, 25, 15), 5, fill=fill)
        except Exception:
            pass
    elif func in ("textsize", "ImageDraw.textsize"):
        return img, {}
    elif func in ("fill",):
        draw.rectangle([0, 0, img.size[0], img.size[1]], fill=fill)
    return img, {}


def _run_color(func, mode):
    """Dispatch ImageColor operations."""
    if func == "getrgb":
        val = PILImageColor.getrgb("red")
        return list(val), {"color": "red"}
    if func == "getcolor":
        val = PILImageColor.getcolor("red", "RGB")
        return list(val) if isinstance(val, tuple) else val, {"color": "red", "mode": "RGB"}
    return None, {}


def _run_palette(img, func, mode):
    """Dispatch ImagePalette operations."""
    try:
        pal = img.getpalette()
        if pal is None:
            pal_data = [0, 0, 0, 255, 255, 255]
        else:
            pal_data = list(pal)
        palette = PILImagePalette.ImagePalette(mode="RGB")
        if func == "copy":
            return palette.copy(), {}
        if func == "getcolor":
            rgba = palette.getcolor((255, 0, 0))
            return list(rgba) if isinstance(rgba, tuple) else rgba, {"color": "red"}
        if func == "getdata":
            try:
                data = palette.getdata()
                return list(data) if data else [], {}
            except Exception:
                return [], {}
        if func == "save":
            return None, {}
        if func == "tobytes":
            try:
                return palette.tobytes(), {}
            except Exception:
                return bytes(), {}
    except Exception:
        pass
    return None, {}


def _run_font(func, mode):
    """Dispatch ImageFont operations."""
    if func == "load_default":
        try:
            font = PILImageFont.load_default()
            return str(type(font).__name__), {"size": 0}
        except Exception:
            return None, {}
    if func == "load_default_imagefont":
        try:
            font = PILImageFont.load_default_imagefont()
            return str(type(font).__name__), {}
        except Exception:
            return None, {}
    if func == "load":
        return None, {}
    if func == "truetype":
        return None, {}
    if func == "load_path":
        return None, {}
    if func in ("FreeTypeFont", "ImageFont"):
        return None, {}
    return None, {}


def _run_stat(img, func, mode):
    """Dispatch ImageStat operations."""
    if func == "Stat":
        stat = PILImageStat.Stat(img)
        result = {
            "count": list(stat.count) if hasattr(stat, 'count') else [],
            "sum": list(stat.sum) if hasattr(stat, 'sum') else [],
            "mean": list(stat.mean) if hasattr(stat, 'mean') else [],
            "median": list(stat.median) if hasattr(stat, 'median') else [],
            "rms": list(stat.rms) if hasattr(stat, 'rms') else [],
            "var": list(stat.var) if hasattr(stat, 'var') else [],
            "stddev": list(stat.stddev) if hasattr(stat, 'stddev') else [],
            "extrema": list(stat.extrema) if hasattr(stat, 'extrema') else [],
        }
        return result, {}
    return None, {}


def _run_sequence(img, func, mode):
    """Dispatch ImageSequence operations."""
    if func == "Iterator":
        frames = list(PILImageSequence.Iterator(img))
        return len(frames), {"frame_count": len(frames)}
    if func == "all_frames":
        frames = list(PILImageSequence.all_frames(img))
        return len(frames), {}
    return None, {}


def main():
    target_filter = sys.argv[2] if len(sys.argv) > 2 and sys.argv[1] == "--target" else None

    with open(MANIFEST_PATH) as f:
        manifest = yaml.safe_load(f)

    FIXTURES_DIR.mkdir(parents=True, exist_ok=True)
    index = {"operations": {}}
    count = 0

    for mod_name, mod_def in manifest.get("modules", {}).items():
        for section in ["class_methods", "methods", "functions", "classes", "properties"]:
            for item in mod_def.get(section, []):
                if not isinstance(item, dict):
                    continue
                # Properties have no status field, always included
                if section != "properties" and item.get("status") != "implemented":
                    continue
                op_name = f"{mod_name}.{item['name']}"
                modes = [str(m) for m in item.get("supported_modes", item.get("modes", []))]
                targets = item.get("supported_targets", ["cpu"])
                if not modes:
                    modes = ["L", "RGB"]  # default: test grayscale + color

                for mode in modes:
                    result = run_pil(op_name, mode)
                    if result is None:
                        continue

                    status, data, params, input_bytes, input_size, input_bytes_rgb = result
                    key = f"{op_name.replace('.', '_')}_{mode}"
                    if status == 'success':
                        h = hashlib.sha256(data).hexdigest()
                        fixture = {
                            "op": op_name,
                            "mode": mode,
                            "targets": targets,
                            "params": params,
                            "expectedHash": h,
                            "inputMode": mode,
                            "inputSize": input_size,
                            "inputBytes": input_bytes.hex() if input_bytes else "",
                            "inputBytesRgb": input_bytes_rgb.hex() if input_bytes_rgb else "",
                        }
                    elif status == 'value':
                        fixture = {
                            "op": op_name,
                            "mode": mode,
                            "targets": targets,
                            "params": params,
                            "expectedValue": data,
                            "inputMode": mode,
                            "inputSize": input_size,
                            "inputBytes": input_bytes.hex() if input_bytes else "",
                            "inputBytesRgb": input_bytes_rgb.hex() if input_bytes_rgb else "",
                        }
                    else:  # error
                        fixture = {
                            "op": op_name,
                            "mode": mode,
                            "targets": targets,
                            "params": params,
                            "expectedError": data,
                            "inputMode": mode,
                            "inputSize": input_size,
                            "inputBytes": input_bytes.hex() if input_bytes else "",
                            "inputBytesRgb": input_bytes_rgb.hex() if input_bytes_rgb else "",
                        }
                    index["operations"][key] = fixture
                    with open(FIXTURES_DIR / f"{key}.json", "w") as f_out:
                        json.dump(fixture, f_out, indent=2)
                    count += 1

    with open(FIXTURES_DIR / "index.json", "w") as f:
        json.dump(index, f, indent=2)

    print(f"Generated {count} fixtures in {FIXTURES_DIR}")


if __name__ == "__main__":
    main()
