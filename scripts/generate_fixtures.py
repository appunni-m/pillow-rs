#!/usr/bin/env python3
"""Generate expected output fixtures by running PIL against input specs.

Reads each input JSON from tests/fixtures/input/jsons/, executes the operation
via PIL, and writes the expected results to tests/fixtures/outputs/jsons/.
Reference images are saved as PNGs in tests/fixtures/outputs/images/.

Usage:
    python scripts/generate_fixtures.py                          # process default fixture directory
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
import PIL

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

from engine import (
    CALL_STYLE,
    _resolve_font_path,
    _typed_value,
    create_input,
    get_call_style,
)

# These are set in main() after parsing CLI args
FIXTURES_DIR = ROOT / "tests" / "fixtures"
INPUT_DIR = FIXTURES_DIR / "input" / "jsons"
OUTPUT_JSONS_DIR = FIXTURES_DIR / "outputs" / "jsons"
OUTPUT_IMAGES_DIR = FIXTURES_DIR / "outputs" / "images"
OUTPUT_RAWS_DIR = FIXTURES_DIR / "outputs" / "raws"
TARGET_SUITE = None  # None = all suites, 0-9 = specific suite
PILLOW_VERSION = "12.2.0"
FREETYPE_VERSION = "2.14.3"


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


def _seed_c_rng(seed):
    """Seed the C RNG used by Pillow's stochastic image operations."""
    import ctypes

    libc = ctypes.CDLL(None)
    libc.srand.argtypes = [ctypes.c_uint]
    libc.srand(seed)


def _pilify(v):
    """Recursively convert lists to tuples for PIL API compatibility.
    RSPIL PyO3 bindings handle this automatically; PIL does not."""
    if isinstance(v, list):
        return tuple(_pilify(x) for x in v)
    if isinstance(v, dict):
        return {k: _pilify(val) for k, val in v.items()}
    return v



def _artifact_references(assertion):
    method = assertion.get("method")
    if method == "image":
        yield assertion["reference"]
    elif method == "image_list":
        for item in assertion["items"]:
            yield from _artifact_references(item)
    elif method == "tuple":
        for item in assertion["items"]:
            yield from _artifact_references(item)


RAW_IMAGE_MODES = {"P", "PA", "HSV", "YCbCr", "F", "I", "CMYK"}


def _write_image_assertion(image, stem, case_id, suffix=""):
    """Write one exact image oracle without converting its mode."""
    artifact_stem = f"{stem}_{case_id}{suffix}"
    if image.mode in RAW_IMAGE_MODES:
        reference = f"raws/{artifact_stem}.bin"
        path = OUTPUT_RAWS_DIR / f"{artifact_stem}.bin"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(image.tobytes())
        assertion = {
            "method": "image",
            "reference": reference,
            "raw_kind": "image",
            "mode": image.mode,
            "size": list(image.size),
        }
        if image.mode in ("P", "PA"):
            assertion["palette"] = image.getpalette()
        return assertion

    reference = f"images/{artifact_stem}.png"
    path = OUTPUT_IMAGES_DIR / f"{artifact_stem}.png"
    path.parent.mkdir(parents=True, exist_ok=True)
    image.save(str(path))
    return {"method": "image", "reference": reference}


def generate_one(input_path):
    """Run one input fixture through PIL, produce output JSON + reference files."""
    inp = json.loads(input_path.read_text())
    op = inp["operation"]
    call_style = get_call_style(op["module"], op["target"], op.get("class"))

    out = {
        "format_version": 2,
        "pillow_version": PIL.__version__,
        "freetype_version": PIL.ImageFont.core.freetype2_version,
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
        if "font" in params:
            font_path = Path(_resolve_font_path(params["font"]))
            if not font_path.is_file():
                raise FileNotFoundError(
                    f"fixture font is missing for {cid}: {font_path}"
                )
            params["font"] = str(font_path)
        if op["module"] == "ImagePalette" and mode:
            params["_fixture_mode"] = mode
        if call_style == "file_open" and mode:
            params["_fixture_mode"] = mode
        # Decode/Encode: thread asset fields through params
        if op["module"] == "Decode":
            params["asset"] = case["asset"]
        elif op["module"] == "Encode":
            params["source_asset"] = case["source_asset"]
            params["source_format"] = case.get("source_format", op["target"])
        # Seed srand() for deterministic effect_noise output.
        # PIL's effect_noise uses C rand() with global state; without a fixed
        # seed the output varies per process. Pillow-rs uses a deterministic
        # PRNG seeded with 1, so match that here.
        if op["module"] == "ImageModule" and op["target"] == "effect_noise":
            _seed_c_rng(1)
        if op["module"] == "Image" and op["target"] == "effect_spread":
            _seed_c_rng(42)
        try:
            result = CALL_STYLE[call_style](pil, img, img2, op["target"], params)
        except Exception as e:
            out["cases"].append({
                "id": cid,
                "assert": {
                    "method": "error",
                    "exception": type(e).__name__,
                    "message": str(e),
                },
            })
            continue

        # ── Determine result type and produce assertion ──
        # Convert Qt QImage/QPixmap to raw bytes (from toqimage/toqpixmap)
        qt_classes = []
        qt_result_type = None
        try:
            from PySide6.QtGui import QImage, QPixmap
            qt_classes = [QImage, QPixmap]
        except ImportError:
            pass
        if qt_classes and any(isinstance(result, cls) for cls in qt_classes):
            qt_result_type = type(result).__name__
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
                "assert": {
                    "method": "image",
                    "reference": ref,
                    "raw_kind": "qt_image" if qt_result_type else "bytes",
                    "result_type": qt_result_type or type(result).__name__,
                },
            })
        elif hasattr(result, 'tobytes') or hasattr(result, 'save'):
            # Exif objects have tobytes() but are not images — convert to dict
            if type(result).__name__ == 'Exif':
                val = dict(result)
                out["cases"].append({
                    "id": cid,
                    "assert": {"method": "typed", "value": _typed_value(val)},
                })
                continue
            out["cases"].append({
                "id": cid,
                "assert": _write_image_assertion(result, stem, cid),
            })

        elif (isinstance(result, tuple)
              and len(result) > 0
              and any(hasattr(r, 'tobytes') for r in result)
              and not all(hasattr(r, 'tobytes') for r in result)):
            # Mixed-type tuple (e.g. (Image, offset) from getmask2) → tuple assertion
            items = []
            for i, r in enumerate(result):
                if hasattr(r, 'tobytes') or hasattr(r, 'save'):
                    items.append(_write_image_assertion(r, stem, cid, f"_{i}"))
                else:
                    items.append({"method": "typed", "value": _typed_value(r)})
            out["cases"].append({
                "id": cid,
                "assert": {"method": "tuple", "items": items},
            })

        elif (isinstance(result, (list, tuple))
              and len(result) > 0
              and all(hasattr(r, 'tobytes') for r in result)):
            # List of images (e.g. split or ImageSequence.Iterator).
            items = []
            for j, band in enumerate(result):
                items.append(_write_image_assertion(band, stem, cid, f"_{j}"))
            out["cases"].append({
                "id": cid,
                "assert": {
                    "method": "image_list",
                    "container_type": type(result).__name__,
                    "items": items,
                },
            })

        elif isinstance(result, (int, float, str, bool, list, tuple, dict, type(None))):
            out["cases"].append({
                "id": cid,
                "assert": {"method": "typed", "value": _typed_value(result)},
            })

        else:
            raise TypeError(
                f"{cid} returned unsupported fixture type "
                f"{type(result).__name__}; add a semantic call-style probe"
            )

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

    if PIL.__version__ != PILLOW_VERSION:
        raise SystemExit(
            f"expected Pillow {PILLOW_VERSION}, got {PIL.__version__}"
        )
    if PIL.ImageFont.core.freetype2_version != FREETYPE_VERSION:
        raise SystemExit(
            f"expected FreeType {FREETYPE_VERSION}, "
            f"got {PIL.ImageFont.core.freetype2_version}"
        )
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
        engine.ASSETS_DIR = extra_images  # For Decode/Encode asset resolution

    OUTPUT_JSONS_DIR.mkdir(parents=True, exist_ok=True)
    OUTPUT_IMAGES_DIR.mkdir(parents=True, exist_ok=True)
    OUTPUT_RAWS_DIR.mkdir(parents=True, exist_ok=True)
    for output_dir in [OUTPUT_JSONS_DIR, OUTPUT_IMAGES_DIR, OUTPUT_RAWS_DIR]:
        for path in output_dir.rglob("*"):
            if path.is_file():
                path.chmod(0o644)

    input_files = sorted(INPUT_DIR.glob("*.json"))
    if not input_files:
        print("No input fixtures found in", INPUT_DIR)
        return

    generated = 0
    generated_jsons = set()
    generated_artifacts = set()
    failed = 0
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
                print(f"  FAIL {input_path.stem}: invalid JSON", file=sys.stderr)
                failed += 1
                continue

        try:
            out = generate_one(input_path)
            output_path = OUTPUT_JSONS_DIR / input_path.name
            output_path.write_text(json.dumps(out, indent=2))
            generated_jsons.add(input_path.name)
            for case in out["cases"]:
                generated_artifacts.update(
                    _artifact_references(case["assert"])
                )
            print(f"  OK  {input_path.stem} ({len(out['cases'])} cases)")
            generated += 1
        except Exception as e:
            print(f"  FAIL {input_path.stem}: {e}", file=sys.stderr)
            failed += 1

    suite_msg = f" (suite {TARGET_SUITE})" if TARGET_SUITE is not None else " (all suites)"
    print(f"\nGenerated {generated} output fixtures{suite_msg}")
    if skipped > 0:
        print(f"Skipped {skipped} fixtures from other suites")
    if failed > 0:
        raise SystemExit(f"failed to generate {failed} fixture files")

    # A complete suite generation defines the entire output set. Remove stale
    # JSON and binary/image artifacts only after every input generated cleanly.
    if skipped == 0:
        for path in OUTPUT_JSONS_DIR.glob("*.json"):
            if path.name not in generated_jsons:
                path.unlink()
        for output_dir in (OUTPUT_IMAGES_DIR, OUTPUT_RAWS_DIR):
            for path in output_dir.rglob("*"):
                if (
                    path.is_file()
                    and path.relative_to(FIXTURES_DIR / "outputs").as_posix()
                    not in generated_artifacts
                ):
                    path.unlink()

    # Make output files read-only after generation (fixtures are immutable)
    for d in [OUTPUT_JSONS_DIR, OUTPUT_IMAGES_DIR, OUTPUT_RAWS_DIR]:
        if d.exists():
            for f in d.rglob("*"):
                if f.is_file():
                    f.chmod(0o444)
    print("Output fixtures locked (read-only)")


if __name__ == "__main__":
    main()
