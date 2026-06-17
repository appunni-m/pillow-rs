#!/usr/bin/env python3
"""Generate expected output fixtures by running PIL against input specs.

Reads each input JSON from tests/fixtures/input/jsons/, executes the operation
via PIL, and writes the expected results to tests/fixtures/outputs/jsons/.
Reference images are saved as PNGs in tests/fixtures/outputs/images/.

Usage:
    python scripts/generate_fixtures.py                          # process all suites
    python scripts/generate_fixtures.py --suite 1                # process suite 1 only
    python scripts/generate_fixtures.py --fixtures-dir tests/fixtures_2 --suite 1
"""

import argparse
import json
import sys
from pathlib import Path

import PIL.Image
import PIL.ImageDraw
import PIL.ImageFilter
import PIL.ImageChops
import PIL.ImageOps
import PIL.ImageEnhance
import PIL.ImageColor
import PIL.ImagePalette
import PIL.ImageFont
import PIL.ImageStat
import PIL.ImageSequence

ROOT = Path(__file__).parent.parent
sys.path.insert(0, str(ROOT / "tests"))

# Set up headless QApplication for Qt operations (toqpixmap needs it)
try:
    from PySide6.QtWidgets import QApplication
    _qt_app = QApplication.instance()
    if _qt_app is None:
        _qt_app = QApplication([])
except ImportError:
    pass

from engine import CALL_STYLE, get_call_style, create_input

# These are set in main() after parsing CLI args
FIXTURES_DIR = ROOT / "tests" / "fixtures"
INPUT_DIR = FIXTURES_DIR / "input" / "jsons"
OUTPUT_JSONS_DIR = FIXTURES_DIR / "outputs" / "jsons"
OUTPUT_IMAGES_DIR = FIXTURES_DIR / "outputs" / "images"
OUTPUT_RAWS_DIR = FIXTURES_DIR / "outputs" / "raws"
TARGET_SUITE = None  # None = all suites, 0-9 = specific suite


class PilBackend:
    """Adapter so engine code accesses PIL modules same way as pillow_rs."""
    Image = PIL.Image
    ImageFilter = PIL.ImageFilter
    ImageChops = PIL.ImageChops
    ImageOps = PIL.ImageOps
    ImageEnhance = PIL.ImageEnhance
    ImageDraw = PIL.ImageDraw
    ImageColor = PIL.ImageColor
    ImagePalette = PIL.ImagePalette
    ImageFont = PIL.ImageFont
    ImageStat = PIL.ImageStat
    ImageSequence = PIL.ImageSequence


pil = PilBackend()
def _pilify(v):
    """Recursively convert lists to tuples for PIL API compatibility.
    RSPIL PyO3 bindings handle this automatically; PIL does not."""
    if isinstance(v, list):
        return tuple(_pilify(x) for x in v)
    if isinstance(v, dict):
        return {k: _pilify(val) for k, val in v.items()}
    return v



def generate_one(input_path):
    """Run one input fixture through PIL, produce output JSON + reference files."""
    inp = json.loads(input_path.read_text())
    op = inp["operation"]
    call_style = get_call_style(op["module"], op["target"])

    out = {
        "format_version": 2,
        "suite": inp.get("suite", 0),
        "operation": op,
        "cases": [],
    }
    stem = input_path.stem

    for case in inp["cases"]:
        cid = case["id"]
        mode = case.get("mode")
        img = create_input(pil, mode, _pilify(case.get("input")))
        img2 = create_input(pil, mode, _pilify(case.get("input2")))
        params = _pilify(dict(case.get("params", {})))
        # Seed srand() for deterministic effect_noise output.
        # PIL's effect_noise uses C rand() with global state; without a fixed
        # seed the output varies per process. Pillow-rs uses a deterministic
        # PRNG seeded with 1, so match that here.
        if op["module"] == "ImageModule" and op["target"] == "effect_noise":
            import ctypes
            libc = ctypes.CDLL('libc.so.6')
            libc.srand(1)
        if op["module"] == "Image" and op["target"] == "effect_spread":
            import ctypes
            libc = ctypes.CDLL('libc.so.6')
            libc.srand(42)
        try:
            result = CALL_STYLE[call_style](pil, img, img2, op["target"], params)
        except Exception as e:
            out["cases"].append({
                "id": cid,
                "assert": {
                    "method": "error",
                    "exception": type(e).__name__,
                    "message_contains": str(e).split("(")[0].strip().split(":")[0].strip()[:100],
                },
            })
            continue

        # ── Determine result type and produce assertion ──
        # Convert Qt QImage/QPixmap to raw bytes (from toqimage/toqpixmap)
        qt_classes = []
        try:
            from PySide6.QtGui import QImage, QPixmap
            qt_classes = [QImage, QPixmap]
        except ImportError:
            pass
        if qt_classes and any(isinstance(result, cls) for cls in qt_classes):
            # QPixmap → QImage first
            if isinstance(result, QPixmap):
                result = result.toImage()
            # Extract raw bytes from QImage
            ptr = result.bits()
            if hasattr(ptr, 'setsize'):
                ptr.setsize(result.sizeInBytes())
            result = bytes(ptr)
        if isinstance(result, bytes):
            # Raw bytes → save as .bin file (e.g. toqimage, toqpixmap)
            ref = f"raws/{stem}_{cid}.bin"
            bin_path = OUTPUT_RAWS_DIR / f"{stem}_{cid}.bin"
            bin_path.parent.mkdir(parents=True, exist_ok=True)
            bin_path.write_bytes(result)
            out["cases"].append({
                "id": cid,
                "assert": {"method": "image", "reference": ref},
            })
        elif hasattr(result, 'tobytes') or hasattr(result, 'save'):
            # Exif objects have tobytes() but are not images — convert to dict
            if type(result).__name__ == 'Exif':
                val = dict(result)
                out["cases"].append({
                    "id": cid,
                    "assert": {"method": "exact", "value": {str(k): v for k, v in val.items()}},
                })
                continue
            # Single image result → save as PNG
            ref = f"images/{stem}_{cid}.png"
            img_path = OUTPUT_IMAGES_DIR / f"{stem}_{cid}.png"
            img_path.parent.mkdir(parents=True, exist_ok=True)
            if result.mode in ("P", "PA", "HSV", "YCbCr", "F", "I", "CMYK"):
                # P-mode PNGs lose palette on reload; these modes cannot be saved as PNG.
                # Save raw bytes for binary comparison instead.
                ref = f"raws/{stem}_{cid}.bin"
                bin_path = OUTPUT_RAWS_DIR / f"{stem}_{cid}.bin"
                bin_path.parent.mkdir(parents=True, exist_ok=True)
                bin_path.write_bytes(result.tobytes())
                out["cases"].append({
                    "id": cid,
                    "assert": {"method": "image", "reference": ref},
                })
                continue
            result.save(str(img_path))
            out["cases"].append({
                "id": cid,
                "assert": {"method": "image", "reference": ref},
            })

        elif (isinstance(result, (list, tuple))
              and len(result) > 0
              and hasattr(result[0], 'tobytes')):
            # List of images (e.g. split) → save each as PNG
            refs = []
            for j, band in enumerate(result):
                ref = f"images/{stem}_{cid}_{j}.png"
                img_path = OUTPUT_IMAGES_DIR / f"{stem}_{cid}_{j}.png"
                img_path.parent.mkdir(parents=True, exist_ok=True)
                if band.mode == "CMYK":
                    band = band.convert("RGB")
                band.save(str(img_path))
                refs.append(ref)
            out["cases"].append({
                "id": cid,
                "assert": {"method": "image_list", "references": refs},
            })

        elif isinstance(result, (int, float, str, bool, list, dict, type(None))):
            # Scalar / structured value result
            if isinstance(result, float):
                out["cases"].append({
                    "id": cid,
                    "assert": {"method": "float", "value": result, "tolerance": 1e-12},
                })
            else:
                out["cases"].append({
                    "id": cid,
                    "assert": {"method": "exact", "value": result},
                })

        else:
            # Unknown type — stringify
            out["cases"].append({
                "id": cid,
                "assert": {"method": "string", "value": repr(result)},
            })

    return out


def main():
    """Generate output fixtures for all input fixtures (filtered by --suite)."""
    global FIXTURES_DIR, INPUT_DIR, OUTPUT_JSONS_DIR, OUTPUT_IMAGES_DIR, OUTPUT_RAWS_DIR, TARGET_SUITE

    parser = argparse.ArgumentParser(
        description="Generate expected output fixtures by running PIL against input specs"
    )
    parser.add_argument(
        "--fixtures-dir",
        default="tests/fixtures",
        help="Fixtures directory relative to repo root (default: tests/fixtures)",
    )
    parser.add_argument(
        "--suite",
        type=int,
        default=None,
        help="Suite number 0-9 to generate (default: all suites). 0 = main fixtures.",
    )
    args = parser.parse_args()

    FIXTURES_DIR = ROOT / args.fixtures_dir
    TARGET_SUITE = args.suite
    INPUT_DIR = FIXTURES_DIR / "input" / "jsons"
    OUTPUT_JSONS_DIR = FIXTURES_DIR / "outputs" / "jsons"
    OUTPUT_IMAGES_DIR = FIXTURES_DIR / "outputs" / "images"
    OUTPUT_RAWS_DIR = FIXTURES_DIR / "outputs" / "raws"

    # Register extra reference image dirs for the target fixtures directory
    import engine
    extra_images = FIXTURES_DIR / "input" / "images"
    if extra_images.exists():
        engine.EXTRA_REFERENCE_DIRS = [str(extra_images)]

    OUTPUT_JSONS_DIR.mkdir(parents=True, exist_ok=True)
    OUTPUT_IMAGES_DIR.mkdir(parents=True, exist_ok=True)
    OUTPUT_RAWS_DIR.mkdir(parents=True, exist_ok=True)

    input_files = sorted(INPUT_DIR.glob("*.json"))
    if not input_files:
        print("No input fixtures found in", INPUT_DIR)
        return

    generated = 0
    skipped = 0
    for input_path in input_files:
        # Filter by suite if --suite specified
        if TARGET_SUITE is not None:
            try:
                inp_preview = json.loads(input_path.read_text())
                file_suite = inp_preview.get("suite", 0)
                if file_suite != TARGET_SUITE:
                    skipped += 1
                    continue
            except json.JSONDecodeError:
                print(f"  SKIP {input_path.stem}: invalid JSON", file=sys.stderr)
                continue

        try:
            out = generate_one(input_path)
            output_path = OUTPUT_JSONS_DIR / input_path.name
            output_path.write_text(json.dumps(out, indent=2))
            print(f"  OK  {input_path.stem} ({len(out['cases'])} cases)")
            generated += 1
        except Exception as e:
            print(f"  FAIL {input_path.stem}: {e}", file=sys.stderr)

    suite_msg = f" (suite {TARGET_SUITE})" if TARGET_SUITE is not None else " (all suites)"
    print(f"\nGenerated {generated} output fixtures{suite_msg}")
    if skipped > 0:
        print(f"Skipped {skipped} fixtures from other suites")


if __name__ == "__main__":
    main()
