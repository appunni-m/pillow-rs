#!/usr/bin/env python3
"""Generate WASM test fixtures from PIL reference outputs.

For each (operation, mode) with a WASM target, runs PIL operation,
hashes the output PNG, and writes a JSON fixture.

JS/WASM tests load fixtures and compare output hashes.

Usage: python scripts/generate_wasm_fixtures.py [--target wasm|wasm_gpu]
"""
import sys, json, hashlib, yaml
from pathlib import Path
from io import BytesIO

ROOT = Path(__file__).parent.parent
MANIFEST_PATH = ROOT / "manifest.yaml"
FIXTURES_DIR = ROOT / "pillow-rs-js" / "tests" / "fixtures"

import PIL.Image as PILImage
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
            result = getattr(img, func)() if func not in ("resize", "crop", "rotate", "transpose", "filter", "convert", "thumbnail") else _image_op(img, func)
        elif module == "ImageOps":
            result = getattr(PILImageOps, func)(img)
        elif module == "ImageChops":
            img2 = _make_image(mode, img.size)
            if func in ("add", "subtract", "multiply", "screen", "darker", "lighter", "difference",
                        "add_modulo", "subtract_modulo", "blend", "composite",
                        "hard_light", "soft_light", "overlay", "logical_and", "logical_or", "logical_xor"):
                result = getattr(PILImageChops, func)(img, img2)
            else:
                result = getattr(PILImageChops, func)(img)
        elif module == "ImageFilter":
            filt = getattr(PILFilter, func) if hasattr(PILFilter, func) else PILFilter.BLUR
            result = img.filter(filt)
        elif module == "ImageEnhance":
            result = getattr(PILImageEnhance, func)(img).enhance(1.5)
        elif module == "ImageModule":
            if func == "merge":
                bands = img.split()
                result = PILImage.merge(mode, bands)
            elif func == "effect_noise":
                result = PILImage.effect_noise(img.size, 10)
            else:
                return None
        else:
            return None

        buf = BytesIO()
        result.save(buf, format="PNG")
        return buf.getvalue()
    except Exception as e:
        return None


def _image_op(img, func):
    """Dispatch Image instance method operations."""
    if func == "resize":
        return img.resize((50, 50))
    elif func == "crop":
        return img.crop((25, 25, 75, 75))
    elif func == "rotate":
        return img.rotate(90)
    elif func == "transpose":
        return img.transpose(PILImage.FLIP_LEFT_RIGHT)
    elif func == "filter":
        return img.filter(PILFilter.BLUR)
    elif func == "convert":
        return img.convert("RGB") if img.mode != "RGB" else img.convert("L")
    elif func == "thumbnail":
        img.thumbnail((50, 50))
        return img
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
                    for target in targets:
                        if target not in ("wasm", "wasm_gpu"):
                            continue
                        if target_filter and target != target_filter:
                            continue

                        data = run_pil(op_name, mode)
                        if data is None:
                            continue

                        h = hashlib.sha256(data).hexdigest()
                        key = f"{op_name.replace('.', '_')}_{mode}_{target}"
                        fixture = {
                            "op": op_name,
                            "mode": mode,
                            "target": target,
                            "expectedHash": h,
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
