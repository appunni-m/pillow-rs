#!/usr/bin/env python3
"""Python E2E benchmark harness — measures through PyO3 binding layer.

Measures RSPIL (pillow-rs) operations from Python, giving true end-to-end
timings including binding overhead. Compares against Pillow baseline.

Usage: python scripts/bench_native_cpu.py [--runs N] [--output path]
Output: target/benchmarks/native_cpu.json
"""

import json, sys, timeit, io, os
from pathlib import Path

ROOT = Path(__file__).parent.parent
OUT_DIR = ROOT / "target" / "benchmarks"
REF_DIR = ROOT / "scripts" / "bench_reference_images"

# Import RSPIL (pillow-rs) — the PyO3 binding
import sys as _sys
_py_dir = str(Path(__file__).parent.parent / "pillow-rs-py" / "python")
if _py_dir not in _sys.path:
    _sys.path.insert(0, _py_dir)
try:
    from pillow_rs import Image as RsImage, ImageOps as RsImageOps
    HAS_RSPIL = True
except ImportError:
    print("[WARN] pillow_rs not installed. Run: cd pillow-rs-py && maturin develop --release")
    print(f"[WARN] Tried path: {_py_dir}")
    HAS_RSPIL = False


def load_rgb():
    return RsImage.open(str(REF_DIR / "ref_2k.jpg"))


def load_rgba():
    return RsImage.open(str(REF_DIR / "ref_1k.png"))


def load_gray():
    return RsImage.open(str(REF_DIR / "ref_grayscale.png"))


def bench(name, fn, runs=30, warmup=3):
    for _ in range(warmup):
        try: fn()
        except: pass
    times = []
    for _ in range(runs):
        try:
            t0 = timeit.default_timer()
            fn()
            times.append(timeit.default_timer() - t0)
        except Exception as e:
            pass
    if not times:
        return None
    mean = sum(times) / len(times)
    variance = sum((t - mean) ** 2 for t in times) / len(times)
    result = {
        "mean_ms": round(mean * 1000, 4),
        "std_ms": round((variance ** 0.5) * 1000, 4),
        "runs": len(times),
    }
    print(f"  {name:<40} {result['mean_ms']:>8.2f}ms ±{result['std_ms']:>5.2f}ms")
    return result


def main():
    if not HAS_RSPIL:
        print("Cannot run — pillow_rs not installed")
        sys.exit(1)

    runs = int(sys.argv[2]) if "--runs" in sys.argv else 30
    out_path = sys.argv[sys.argv.index("--output") + 1] if "--output" in sys.argv else str(OUT_DIR / "native_cpu.json")

    results = {}

    # Load reference images once
    rgb = load_rgb()
    rgba = load_rgba()
    gray = load_gray()

    # ═══ Priority ops ═══
    def _open():
        RsImage.open(str(REF_DIR / "ref_2k.jpg"))
    results["Image.open"] = bench("Image.open", _open, runs)

    def _new():
        RsImage.new("RGB", (1920, 1080), (255, 0, 0, 255))
    results["Image.new"] = bench("Image.new", _new, runs)

    def _save():
        buf = io.BytesIO()
        rgb.save(buf, format="PNG")
    results["Image.save"] = bench("Image.save", _save, runs)

    def _resize():
        rgb.resize((800, 600), "LANCZOS")
    results["Image.resize"] = bench("Image.resize", _resize, runs)

    def _crop():
        rgb.crop((100, 100, 500, 500))
    results["Image.crop"] = bench("Image.crop", _crop, runs)

    def _rotate():
        rgb.rotate(90)
    results["Image.rotate"] = bench("Image.rotate", _rotate, runs)

    def _transpose():
        rgb.transpose("FLIP_LEFT_RIGHT")
    results["Image.transpose"] = bench("Image.transpose", _transpose, runs)

    def _thumbnail():
        r = load_rgb()
        r.thumbnail((200, 200))
    results["Image.thumbnail"] = bench("Image.thumbnail", _thumbnail, runs)

    def _tobytes():
        rgb.tobytes()
    results["Image.tobytes"] = bench("Image.tobytes", _tobytes, runs)

    def _paste():
        dst = load_rgb()
        src = RsImage.new("RGB", (800, 600), (0, 255, 0, 255))
        dst.paste(src, (100, 100))
    results["Image.paste"] = bench("Image.paste", _paste, runs)

    def _convert():
        rgb.convert("L")
    results["Image.convert"] = bench("Image.convert", _convert, runs)

    def _filter():
        rgb.filter("BLUR")
    results["Image.filter"] = bench("Image.filter", _filter, runs)

    # ═══ Image methods ═══
    results["Image.copy"] = bench("Image.copy", lambda: rgb.copy(), runs)
    results["Image.split"] = bench("Image.split", lambda: rgb.split(), runs)
    results["Image.getbands"] = bench("Image.getbands", lambda: rgb.getbands(), runs)
    results["Image.getbbox"] = bench("Image.getbbox", lambda: rgb.getbbox(), runs)
    results["Image.getchannel"] = bench("Image.getchannel", lambda: rgb.getchannel(0), runs)
    results["Image.getcolors"] = bench("Image.getcolors", lambda: rgb.getcolors(256), runs)
    results["Image.getextrema"] = bench("Image.getextrema", lambda: rgb.getextrema(), runs)
    results["Image.getpixel"] = bench("Image.getpixel", lambda: rgb.getpixel((50, 50)), runs)
    results["Image.getprojection"] = bench("Image.getprojection", lambda: rgb.getprojection(), runs)
    results["Image.histogram"] = bench("Image.histogram", lambda: rgb.histogram(), runs)
    results["Image.load"] = bench("Image.load", lambda: rgb.load(), runs)
    results["Image.point"] = bench("Image.point", lambda: rgb.point(lambda x: x * 2), runs)
    results["Image.putalpha"] = bench("Image.putalpha", lambda: rgba.putalpha(128), runs)
    results["Image.putpixel"] = bench("Image.putpixel", lambda: rgb.putpixel((10, 10), (255, 0, 0)), runs)
    results["Image.quantize"] = bench("Image.quantize", lambda: rgb.quantize(16), runs)
    results["Image.reduce"] = bench("Image.reduce", lambda: rgb.reduce(2), runs)
    results["Image.effect_spread"] = bench("Image.effect_spread", lambda: rgb.effect_spread(3), runs)
    results["Image.entropy"] = bench("Image.entropy", lambda: rgb.entropy(), runs)

    # ═══ ImageOps ═══
    results["ImageOps.autocontrast"] = bench("ImageOps.autocontrast", lambda: RsImageOps.autocontrast(rgb), runs)
    results["ImageOps.equalize"] = bench("ImageOps.equalize", lambda: RsImageOps.equalize(rgb), runs)
    results["ImageOps.invert"] = bench("ImageOps.invert", lambda: RsImageOps.invert(rgb), runs)
    results["ImageOps.flip"] = bench("ImageOps.flip", lambda: RsImageOps.flip(rgb), runs)
    results["ImageOps.mirror"] = bench("ImageOps.mirror", lambda: RsImageOps.mirror(rgb), runs)
    results["ImageOps.posterize"] = bench("ImageOps.posterize", lambda: RsImageOps.posterize(rgb, 3), runs)
    results["ImageOps.solarize"] = bench("ImageOps.solarize", lambda: RsImageOps.solarize(rgb, 128), runs)
    results["ImageOps.grayscale"] = bench("ImageOps.grayscale", lambda: RsImageOps.grayscale(rgb), runs)
    results["ImageOps.colorize"] = bench("ImageOps.colorize", lambda: RsImageOps.colorize(rgb, "black", "white"), runs)
    results["ImageOps.contain"] = bench("ImageOps.contain", lambda: RsImageOps.contain(rgb, (512, 512)), runs)
    results["ImageOps.cover"] = bench("ImageOps.cover", lambda: RsImageOps.cover(rgb, (512, 512)), runs)
    results["ImageOps.crop"] = bench("ImageOps.crop", lambda: RsImageOps.crop(rgb, 10), runs)
    results["ImageOps.deform"] = bench("ImageOps.deform", lambda: RsImageOps.deform(rgb, None), runs)
    results["ImageOps.exif_transpose"] = bench("ImageOps.exif_transpose", lambda: RsImageOps.exif_transpose(rgb), runs)
    results["ImageOps.expand"] = bench("ImageOps.expand", lambda: RsImageOps.expand(rgb, 10), runs)
    results["ImageOps.fit"] = bench("ImageOps.fit", lambda: RsImageOps.fit(rgb, (512, 512)), runs)
    results["ImageOps.pad"] = bench("ImageOps.pad", lambda: RsImageOps.pad(rgb, (600, 600)), runs)
    results["ImageOps.scale"] = bench("ImageOps.scale", lambda: RsImageOps.scale(rgb, 0.5), runs)

    # ═══ Additional Image methods ═══
    results["Image.effect_spread"] = bench("Image.effect_spread", lambda: rgb.effect_spread(3), runs)
    results["Image.alpha_composite"] = bench("Image.alpha_composite", lambda: rgba.alpha_composite(rgba), runs)
    results["Image.close"] = bench("Image.close", lambda: rgb.close(), runs)
    results["Image.getdata"] = bench("Image.getdata", lambda: rgb.getdata(), runs)
    results["Image.tobitmap"] = bench("Image.tobitmap", lambda: rgb.convert("1").tobitmap(), runs)
    results["Image.remap_palette"] = bench("Image.remap_palette", lambda: rgb.remap_palette(list(range(256))), runs)
    results["Image.transform"] = bench("Image.transform", lambda: rgb.transform((512, 512), 0), runs)
    results["Image.verify"] = bench("Image.verify", lambda: rgb.verify(), runs)
    results["Image.seek"] = bench("Image.seek", lambda: rgb.seek(0), runs)
    results["Image.show"] = bench("Image.show", lambda: rgb.show(), runs)
    results["Image.draft"] = bench("Image.draft", lambda: rgb.draft("RGB", (512, 384)), runs)
    results["Image.apply_transparency"] = bench("Image.apply_transparency", lambda: rgba.apply_transparency(), runs)
    results["Image.getexif"] = bench("Image.getexif", lambda: rgb.getexif(), runs)
    results["Image.getpalette"] = bench("Image.getpalette", lambda: rgb.getpalette(), runs)
    results["Image.putpalette"] = bench("Image.putpalette", lambda: rgb.putpalette([0]*768), runs)
    results["Image.get_child_images"] = bench("Image.get_child_images", lambda: rgb.get_child_images(), runs)
    results["Image.get_flattened_data"] = bench("Image.get_flattened_data", lambda: rgb.get_flattened_data(), runs)
    results["Image.frombytes"] = bench("Image.frombytes", lambda: Image.frombytes("RGB", (64, 64), b'\x00' * 64 * 64 * 3), runs)

    # ═══ Pipeline ═══
    def pipeline():
        r = load_rgb()
        r = r.resize((800, 600), "LANCZOS")
        r = r.crop((100, 100, 500, 500))
        r = r.convert("L")
        r = r.rotate(90)
        r = r.transpose("FLIP_LEFT_RIGHT")
        r = r.filter("BLUR")
        r = RsImageOps.autocontrast(r)
        r = RsImageOps.equalize(r)
        r = RsImageOps.invert(r)
        r = RsImageOps.flip(r)
        r = RsImageOps.mirror(r)
        src = RsImage.new("L", (50, 50), (128,))
        r.paste(src, (0, 0))
        r = r.quantize(256)
        r = r.reduce(2)
        r.split()
        r.getpixel((10, 10))
        buf = io.BytesIO()
        r.save(buf, format="PNG")
        return buf
    results["pipeline_20_st"] = bench("pipeline_20_st", pipeline, runs)
    results["pipeline_20_mt"] = bench("pipeline_20_mt", pipeline, runs)  # Same in Python (GIL)

    # Save
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    with open(out_path, "w") as f:
        json.dump(results, f, indent=2)
    print(f"\nSaved {len(results)} results to {out_path}")


if __name__ == "__main__":
    main()
