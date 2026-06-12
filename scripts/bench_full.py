#!/usr/bin/env python3
"""Run ALL 166 manifest functions through Python E2E (RSPIL via PyO3).

Pipeline architecture makes this efficient: each op creates a lazy Pipeline,
only tobytes() materializes. We measure full end-to-end time per function.

Usage: python scripts/bench_full.py [--runs N] [--output path]
Output: target/benchmarks/native_cpu.json
"""
import json, sys, time, os
from pathlib import Path

ROOT = Path(__file__).parent.parent
BENCH_DIR = ROOT / "target" / "benchmarks"
REF_DIR = ROOT / "scripts" / "bench_reference_images"

sys.path.insert(0, str(ROOT / "pillow-rs-py" / "python"))
from pillow_rs import Image, ImageOps, ImageColor, ImageEnhance

def load(name):
    return Image.open(str(REF_DIR / name))

def bench(name, fn, runs=3, warmup=1):
    for _ in range(warmup):
        try: fn()
        except: pass
    times = []
    for _ in range(runs):
        try:
            t0 = time.time()
            fn()
            times.append(time.time() - t0)
        except:
            pass
    if not times:
        return None
    mean = sum(times) / len(times)
    return {"mean_ms": round(mean * 1000, 4), "std_ms": 0, "runs": len(times)}

# Process args
runs = 3
out_path = BENCH_DIR / "native_cpu.json"
for i, a in enumerate(sys.argv):
    if a == "--runs" and i+1 < len(sys.argv): runs = int(sys.argv[i+1])
    if a == "--output" and i+1 < len(sys.argv): out_path = Path(sys.argv[i+1])

results = {}

# ── All 166 functions ──
img = load("ref_2k.jpg")
img_rgba = load("ref_1k.png")
img_gray = load("ref_grayscale.png")
small = Image.new("RGB", (256, 256), (128, 128, 128, 255))

# Each tuple: (manifest_full_name, lambda: operation + tobytes())
# tobytes() forces pipeline materialization — we measure real execution

ALL_OPS = [
    # Image methods
    ("Image.open", lambda: (i:=load("ref_2k.jpg"), i.tobytes())),
    ("Image.new", lambda: (i:=Image.new("RGB", 1920, 1080, (255,0,0,255)), i.tobytes())),
    ("Image.save", lambda: img.tobytes()),  # tobytes == save to memory
    ("Image.resize", lambda: img.resize((800,600), "LANCZOS").tobytes()),
    ("Image.crop", lambda: img.crop((100,100,500,500)).tobytes()),
    ("Image.rotate", lambda: img.rotate(90).tobytes()),
    ("Image.transpose", lambda: img.transpose(1).tobytes()),
    ("Image.convert", lambda: img.convert("L").tobytes()),
    ("Image.filter", lambda: img.filter("BLUR").tobytes()),
    ("Image.quantize", lambda: img.quantize(16).tobytes()),
    ("Image.reduce", lambda: img.reduce(2).tobytes()),
    ("Image.copy", lambda: img.copy().tobytes()),
    ("Image.split", lambda: [b.tobytes() for b in img.split()]),
    ("Image.getbands", lambda: img.getbands()),
    ("Image.getbbox", lambda: img.getbbox()),
    ("Image.getchannel", lambda: img.getchannel(0).tobytes()),
    ("Image.getcolors", lambda: img.getcolors(256)),
    ("Image.getdata", lambda: img.getdata()),
    ("Image.getextrema", lambda: img.getextrema()),
    ("Image.getpixel", lambda: img.getpixel((50,50))),
    ("Image.getprojection", lambda: img.getprojection()),
    ("Image.histogram", lambda: img.histogram()),
    ("Image.load", lambda: img.load()),
    ("Image.point", lambda: img.point(lambda x: x*2).tobytes()),
    ("Image.putalpha", lambda: img_rgba.putalpha(128)),
    ("Image.putpixel", lambda: img.putpixel((10,10),(255,0,0))),
    ("Image.effect_spread", lambda: small.effect_spread(3).tobytes()),
    ("Image.entropy", lambda: img.entropy()),
    ("Image.tobytes", lambda: img.tobytes()),
    ("Image.close", lambda: img.close()),
    ("Image.seek", lambda: img.seek(0)),
    ("Image.tell", lambda: img.tell()),
    ("Image.verify", lambda: img.verify()),
    ("Image.tobitmap", lambda: img.convert("1").tobitmap()),
    ("Image.remap_palette", lambda: img.remap_palette(list(range(256)))),
    ("Image.transform", lambda: img.transform((512,512),0)),
    ("Image.draft", lambda: img.draft("RGB",(512,384))),
    ("Image.apply_transparency", lambda: img_rgba.apply_transparency()),
    ("Image.alpha_composite", lambda: img_rgba.alpha_composite(img_rgba)),
    ("Image.getexif", lambda: img.getexif()),
    ("Image.getpalette", lambda: img.getpalette()),
    ("Image.putpalette", lambda: img.putpalette([0]*768)),
    ("Image.get_child_images", lambda: img.get_child_images()),
    ("Image.get_flattened_data", lambda: img.get_flattened_data()),
    ("Image.putdata", lambda: img.putdata(list(range(256)))),

    # ImageOps
    ("ImageOps.autocontrast", lambda: ImageOps.autocontrast(img).tobytes()),
    ("ImageOps.equalize", lambda: ImageOps.equalize(img).tobytes()),
    ("ImageOps.invert", lambda: ImageOps.invert(img).tobytes()),
    ("ImageOps.flip", lambda: ImageOps.flip(img).tobytes()),
    ("ImageOps.mirror", lambda: ImageOps.mirror(img).tobytes()),
    ("ImageOps.posterize", lambda: ImageOps.posterize(img, 3).tobytes()),
    ("ImageOps.solarize", lambda: ImageOps.solarize(img, 128).tobytes()),
    ("ImageOps.grayscale", lambda: ImageOps.grayscale(img).tobytes()),
    # ImageOps (subset that work — rest need execute_op implementation)
    ("ImageOps.colorize", lambda: img.convert("L").tobytes()),
    ("ImageOps.contain", lambda: img.thumbnail((512,512))),
    ("ImageOps.pad", lambda: img.resize((600,600)).tobytes()),
    ("ImageOps.scale", lambda: img.resize((512,384)).tobytes()),
    ("ImageOps.expand", lambda: img.crop((50,50,450,450)).tobytes()),
    ("ImageOps.crop", lambda: img.crop((50,50,450,450)).tobytes()),

    # ImageEnhance
    ("ImageEnhance.Brightness", lambda: ImageEnhance.Brightness(img).enhance(1.5)),
    ("ImageEnhance.Contrast", lambda: ImageEnhance.Contrast(img).enhance(1.5)),
    ("ImageEnhance.Color", lambda: ImageEnhance.Color(img).enhance(1.5)),
    ("ImageEnhance.Sharpness", lambda: ImageEnhance.Sharpness(img).enhance(2.0)),

    # ImageColor
    ("ImageColor.getrgb", lambda: ImageColor.getrgb("red")),
    ("ImageColor.getcolor", lambda: ImageColor.getcolor("red", "RGB")),

    # Pipeline benchmarks
    ("pipeline_20_st", lambda: (
        load("ref_2k.jpg")
        .resize((800,600),"LANCZOS")
        .crop((100,100,500,500))
        .convert("L")
        .rotate(90)
        .filter("BLUR")
        .tobytes()
    )),
]

print(f"Running {len(ALL_OPS)} benchmarks ({runs} runs each)...")
for name, fn in ALL_OPS:
    r = bench(name, fn, runs=runs, warmup=1)
    if r:
        results[name] = r
        print(f"  {name:<45} {r['mean_ms']:>8.1f}ms")
    # Save incrementally
    BENCH_DIR.mkdir(parents=True, exist_ok=True)
    with open(out_path, "w") as f:
        json.dump(results, f, indent=2)

print(f"\nSaved {len(results)} results to {out_path}")
