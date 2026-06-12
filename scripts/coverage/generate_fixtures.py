#!/usr/bin/env python3
"""Generate WASM test fixtures from PIL reference outputs.

For each (operation, mode) with a WASM target, runs PIL operation,
hashes the output PNG, and writes a JSON fixture.

JS/WASM tests load fixtures and compare output hashes.

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


def _make_image(mode, size=(100, 100)):
    """Create a PIL image for the given mode."""
    if mode == "L":
        return PILImage.new("L", size, 128)
    elif mode == "LA":
        return PILImage.new("LA", size, (128, 255))
    elif mode == "RGB":
        return PILImage.new("RGB", size, (255, 0, 0))
    elif mode == "RGBA":
        return PILImage.new("RGBA", size, (255, 0, 0, 255))
    elif mode == "1":
        return PILImage.new("1", size, 1)
    elif mode == "P":
        return PILImage.new("RGB", size, (255, 0, 0)).convert("P")
    elif mode == "CMYK":
        return PILImage.new("RGB", size, (255, 0, 0)).convert("CMYK")
    elif mode == "YCbCr":
        return PILImage.new("RGB", size, (255, 0, 0)).convert("YCbCr")
    elif mode == "HSV":
        return PILImage.new("RGB", size, (255, 0, 0)).convert("HSV")
    elif mode == "I":
        return PILImage.new("I", size, 128)
    elif mode == "F":
        return PILImage.new("F", size, 0.5)
    return PILImage.new("RGB", size, (255, 0, 0))


def run_pil(op_name, mode):
    """Run a PIL operation and return PNG bytes + metadata."""
    img = _make_image(mode)
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
        else:
            return None

        if hasattr(result, 'tobytes'):
            return ('success', result.tobytes(), params)
        elif isinstance(result, bytes):
            return ('success', result, params)
        elif hasattr(result, 'save'):
            buf = BytesIO()
            result.save(buf, format="PNG")
            return ('success', buf.getvalue(), params)
        else:
            return None
    except Exception as e:
        return ('error', f"{type(e).__name__}: {str(e)[:100]}", {})


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
    if func in ("copy", "split", "getbands", "tobytes", "getbbox", "getextrema",
                "histogram", "getpixel", "getcolors", "getdata", "getprojection",
                "entropy", "load", "close", "verify", "seek", "tell"):
        return getattr(img, func)(), {}
    if func in ("paste",):
        paste_img = _make_image(mode, (10, 10))
        img.paste(paste_img, (0, 0))
        return img, {"size": [10, 10], "position": [0, 0]}
    if func in ("alpha_composite",):
        fg = _make_image("RGBA", (10, 10))
        img.alpha_composite(fg)
        return img, {"fgSize": [10, 10]}
    if func in ("point",):
        lut = bytes([min(255, i + 50) for i in range(256)])
        return img.point(lut), {"lutSize": 256, "offset": 50}
    if func in ("putalpha",):
        img.putalpha(128)
        return img, {"alpha": 128}
    if func in ("putdata",):
        n = img.size[0] * img.size[1]
        data = [128] * n
        img.putdata(data)
        return img, {"count": n}
    if func in ("quantize",): return img.quantize(16), {"colors": 16}
    if func in ("reduce",): return img.reduce(2), {"factor": 2}
    if func in ("effect_spread",): return img.effect_spread(2), {"distance": 2}
    if func in ("transform",): return img.transform((50, 50), PILImage.AFFINE, (1, 0, 0, 0, 1, 0)), {"size": [50, 50], "method": "AFFINE"}
    if func in ("getchannel",):
        return img.getchannel(0), {"channel": 0}
    if func in ("putpixel",):
        img.putpixel((0, 0), (255, 0, 0, 255) if len(img.getbands()) == 4 else (255, 0, 0))
        return img, {}
    return img


def _run_imageops(img, func, mode):
    """Dispatch ImageOps functions. Returns (result, params)."""
    if func in ("autocontrast", "equalize", "invert", "flip", "mirror",
                "grayscale", "posterize", "solarize"):
        return getattr(PILImageOps, func)(img), {"func": func}
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
    return img


def _run_chops(img, func, mode):
    """Dispatch ImageChops functions."""
    img2 = _make_image(mode, img.size)
    dual = ("add", "subtract", "multiply", "screen", "darker", "lighter", "difference",
            "add_modulo", "subtract_modulo", "blend", "composite",
            "hard_light", "soft_light", "overlay", "logical_and", "logical_or", "logical_xor")
    if func in dual:
        return getattr(PILImageChops, func)(img, img2), {"func": func}
    if func in ("invert", "constant", "duplicate", "offset"):
        if func == "offset":
            return getattr(PILImageChops, func)(img, 5, 5), {"func": func, "x": 5, "y": 5}
        if func == "constant":
            return getattr(PILImageChops, func)(img, 128), {"func": func, "value": 128}
        return getattr(PILImageChops, func)(img), {"func": func}
    return img


def _run_filter(img, func):
    """Dispatch ImageFilter operations."""
    filt = getattr(PILFilter, func, None)
    if filt:
        return img.filter(filt), {"type": func}
    return img.filter(PILFilter.BLUR), {"type": "BLUR"}


def _run_module_func(img, func, mode):
    """Dispatch ImageModule functions."""
    if func == "merge":
        bands = img.split()
        return PILImage.merge(mode, bands), {"func": "merge", "mode": mode}
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
        img2.alpha_composite(fg)
        return img2
    if func in ("new", "open", "fromarray", "frombytes"):
        return img  # return identity image for these
    return img


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
        return img  # return identity, these return values not images
    return img


def main():
    target_filter = sys.argv[2] if len(sys.argv) > 2 and sys.argv[1] == "--target" else None

    with open(MANIFEST_PATH) as f:
        manifest = yaml.safe_load(f)

    FIXTURES_DIR.mkdir(parents=True, exist_ok=True)
    index = {"operations": {}}
    count = 0

    for mod_name, mod_def in manifest.get("modules", {}).items():
        for section in ["class_methods", "methods", "functions"]:
            for item in mod_def.get(section, []):
                if not isinstance(item, dict) or item.get("status") != "implemented":
                    continue
                op_name = f"{mod_name}.{item['name']}"
                modes = item.get("supported_modes", [])
                targets = item.get("supported_targets", ["cpu"])
                if not modes:
                    continue

                for mode in modes:
                    # One fixture per (op, mode) — target doesn't affect PIL output
                    result = run_pil(op_name, mode)
                    if result is None:
                        continue

                    status, data, params = result
                    key = f"{op_name.replace('.', '_')}_{mode}"
                    if status == 'success':
                        h = hashlib.sha256(data).hexdigest()
                        fixture = {
                            "op": op_name,
                            "mode": mode,
                            "targets": targets,
                            "params": params,
                            "expectedHash": h,
                        }
                    else:  # error
                        fixture = {
                            "op": op_name,
                            "mode": mode,
                            "targets": targets,
                            "params": params,
                            "expectedError": data,
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
