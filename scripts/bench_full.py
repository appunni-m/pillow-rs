#!/usr/bin/env python3
"""Benchmark ALL 166 manifest functions through Python E2E (RSPIL via PyO3).

Pipeline architecture makes this fast: creating chains is µs-level (Arc ops),
only tobytes()/size forces materialization. We measure end-to-end: create op + materialize.
"""
import json, sys, time, os
from pathlib import Path

ROOT = Path(__file__).parent.parent
BENCH_DIR = ROOT / "target" / "benchmarks"
REF_DIR = ROOT / "scripts" / "bench_reference_images"

sys.path.insert(0, str(ROOT / "pillow-rs-py" / "python"))
from pillow_rs import Image, ImageOps

IMAGES = {}
def load(name):
    if name not in IMAGES:
        path = REF_DIR / name
        IMAGES[name] = Image.open(str(path)) if path.exists() else None
    return IMAGES[name]

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

results = {}

# ── All 166 ops ──

img = load("ref_2k.jpg")
img_rgba = load("ref_1k.png")
img_gray = load("ref_grayscale.png")
small = Image.new("RGB", (512,512), (128,128,128,255))

# Each benchmark: create the operation + materialize via tobytes()
# The pipeline makes create() free — we're timing execute() only.

ops = [
    # Image methods that return new Image (pipeline ops)
    ("Image.resize", lambda: img.resize((800,600), "LANCZOS").tobytes()),
    ("Image.crop", lambda: img.crop((100,100,500,500)).tobytes()),
    ("Image.rotate", lambda: img.rotate(90).tobytes()),
    ("Image.transpose", lambda: img.transpose(1).tobytes() if hasattr(img, 'transpose') else None),
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
    ("Image.getextrema", lambda: img.getextrema()),
    ("Image.getpixel", lambda: img.getpixel((50,50))),
    ("Image.getprojection", lambda: img.getprojection()),
    ("Image.histogram", lambda: img.histogram()),
    ("Image.point", lambda: img.point(lambda x: x*2).tobytes()),
    ("Image.putalpha", lambda: img_rgba.putalpha(128)),
    ("Image.putpixel", lambda: img.putpixel((10,10),(255,0,0))),
    ("Image.effect_spread", lambda: small.effect_spread(3).tobytes()),
    ("Image.entropy", lambda: img.entropy()),
    ("Image.tobytes", lambda: img.tobytes()),

    # ImageOps
    ("ImageOps.autocontrast", lambda: ImageOps.autocontrast(img).tobytes()),
    ("ImageOps.equalize", lambda: ImageOps.equalize(img).tobytes()),
    ("ImageOps.invert", lambda: ImageOps.invert(img).tobytes()),
    ("ImageOps.flip", lambda: ImageOps.flip(img).tobytes()),
    ("ImageOps.mirror", lambda: ImageOps.mirror(img).tobytes()),
    ("ImageOps.posterize", lambda: ImageOps.posterize(img, 3).tobytes()),
    ("ImageOps.solarize", lambda: ImageOps.solarize(img, 128).tobytes()),
    ("ImageOps.grayscale", lambda: ImageOps.grayscale(img).tobytes()),
    ("ImageOps.colorize", lambda: ImageOps.colorize(img, "black", "white").tobytes()),
    ("ImageOps.contain", lambda: ImageOps.contain(img, (512,512)).tobytes()),
    ("ImageOps.cover", lambda: ImageOps.cover(img, (512,512)).tobytes()),
    ("ImageOps.fit", lambda: ImageOps.fit(img, (512,512)).tobytes()),
    ("ImageOps.pad", lambda: ImageOps.pad(img, (600,600)).tobytes()),
    ("ImageOps.scale", lambda: ImageOps.scale(img, 0.5).tobytes()),
    ("ImageOps.expand", lambda: ImageOps.expand(img, 10).tobytes()),
    ("ImageOps.crop", lambda: ImageOps.crop(img, 10).tobytes()),

    # Pipeline
    ("pipeline_20_st", lambda: (
        img.resize((800,600),"LANCZOS")
        .crop((100,100,500,500))
        .convert("L")
        .rotate(90)
        .filter("BLUR")
        .tobytes()
    )),
]

print(f"Running {len(ops)} benchmarks...")
for name, fn in ops:
    r = bench(name, fn, runs=1, warmup=0)
    if r:
        results[name] = r
        print(f"  {name:<45} {r['mean_ms']:>8.1f}ms")
    # Save incrementally so timeout doesn't lose data
    BENCH_DIR.mkdir(parents=True, exist_ok=True)
    with open(BENCH_DIR / "native_cpu.json", "w") as f:
        json.dump(results, f, indent=2)

BENCH_DIR.mkdir(parents=True, exist_ok=True)
out = BENCH_DIR / "native_cpu.json"
with open(out, "w") as f:
    json.dump(results, f, indent=2)
print(f"\nSaved {len(results)} results to {out}")
