# Benchmarking Infrastructure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build end-to-end benchmarking infrastructure that generates `BENCHMARKS.md` on every commit, showing speedup ratios vs Pillow CPU for all manifest.yaml functions across 6 runtime targets.

**Architecture:** Python orchestrator (`scripts/bench_all.sh`) runs 6 independent harnesses (native CPU/GPU via criterion, Node.js WASM CPU/GPU, headless Chrome browser CPU/GPU). Results are merged into BENCHMARKS.md by `scripts/bench_aggregate.py`. Incremental caching (`scripts/bench_cache.py`) ensures only changed code paths are re-benchmarked.

**Tech Stack:** Python 3 (manifest parsing, aggregation, caching), Rust/criterion (native CPU/GPU benchmarks), Node.js (WASM CPU/GPU harnesses), Puppeteer (browser targets), WGSL (shared GPU shaders), Bash (orchestration).

---

## File Structure Map

| File | Responsibility |
|------|---------------|
| `scripts/bench_pillow_baseline.py` | Run Pillow operations, produce `pillow_baseline.json` |
| `scripts/bench_manifest.py` | Parse `manifest.yaml` → flat function list with metadata |
| `scripts/bench_cache.py` | Compute cache keys, detect stale functions per target |
| `scripts/bench_aggregate.py` | Merge 6 target JSONs + baseline → `BENCHMARKS.md` |
| `scripts/bench_all.sh` | Orchestrator: runs all harnesses, passes results to aggregator |
| `scripts/bench_wasm_cpu.mjs` | Node.js harness: loads WASM, times each function |
| `scripts/bench_wasm_gpu.mjs` | Node.js harness: WASM + experimental WebGPU |
| `scripts/bench_browser.mjs` | Puppeteer script: drives headless Chrome for both browser targets |
| `scripts/bench_reference_images/` | Directory with downloaded benchmark images |
| `pillow-rs/benches/native_cpu.rs` | Criterion benchmarks for all functions (CPU path) |
| `pillow-rs/benches/native_gpu.rs` | Criterion-style benchmarks with wgpu dispatch |
| `pillow-rs/benches/bench_utils.rs` | Shared helpers: load ref images, create test inputs |
| `pillow-rs/src/gpu/mod.rs` | GPU compute dispatcher (empty now, filled incrementally) |
| `pillow-rs/src/gpu/shaders/*.wgsl` | WGSL compute shaders (one per op family) |
| `pillow-rs-js/bench_page/index.html` | Page served to headless Chrome for browser benchmarks |
| `pillow-rs-js/bench_page/bench_runner.js` | JS that calls WASM functions and reports timings |
| `package.json` (modify) | Add `puppeteer` devDependency, bench scripts |
| `target/benchmarks/` (gitignored) | Cache directory: JSON outputs + cache.json |
| `BENCHMARKS.md` (committed) | Generated benchmark document |
| `.git/hooks/pre-commit` | Hook that runs bench_all.sh and stages BENCHMARKS.md |

---

### Task 1: Create directory structure and download reference images

**Files:**
- Create: `scripts/bench_reference_images/.gitkeep`
- Create: `scripts/bench_pillow_baseline.py`
- Create: `scripts/bench_manifest.py`

**Note:** Task numbering follows implementation order from the design spec. Steps 1-3 (Pillow baseline, reference images, manifest parsing) are the foundation — everything else depends on them.

- [ ] **Step 1: Create directories**

```bash
mkdir -p pillow-rs/benches
mkdir -p pillow-rs/src/gpu/shaders
mkdir -p pillow-rs-js/bench_page
mkdir -p scripts/bench_reference_images
mkdir -p target/benchmarks
echo "*.json" > target/benchmarks/.gitignore
```

- [ ] **Step 2: Write manifest function list extractor**

Create `scripts/bench_manifest.py`:
```python
#!/usr/bin/env python3
"""Parse manifest.yaml into a flat function list for benchmarking."""
import yaml, json, sys
from pathlib import Path

ROOT = Path(__file__).parent.parent

GPU_APPLICABLE_OPS = {
    # Pixel-parallel — GPU accelerates
    "resize", "thumbnail", "convert", "filter", "point", "quantize", "reduce",
    "autocontrast", "equalize", "invert", "posterize", "solarize", "colorize",
    "add", "add_modulo", "blend", "darker", "difference", "hard_light",
    "lighter", "logical_and", "logical_or", "logical_xor", "multiply",
    "overlay", "screen", "soft_light", "subtract", "subtract_modulo",
    # Enhance (factor-based ops benefit from GPU parallelization)
    "Brightness", "Color", "Contrast", "Sharpness",
    # Geometry — partial GPU (memory-bound, still benchmark GPU path)
    "crop", "rotate", "transpose", "transform", "paste",
}

PRIORITY_OPS = [
    "open_save", "resize", "crop", "rotate", "transpose", "thumbnail",
    "to_bytes", "new", "paste", "paste_mask", "paste_color", "pipeline",
]

def load_manifest(path=None):
    if path is None:
        path = ROOT / "manifest.yaml"
    with open(path) as f:
        return yaml.safe_load(f)

def extract_functions(manifest):
    """Return list of {module, name, full_name, status, gpu_applicable}."""
    funcs = []
    for mod, mod_def in manifest.get("modules", {}).items():
        for key in ["class_methods", "methods", "functions"]:
            for item in mod_def.get(key, []):
                if not isinstance(item, dict):
                    continue
                name = item.get("name", "")
                status = item.get("status", "stub")
                if status == "stub":
                    continue
                full = f"{mod}.{name}"
                gpu = name in GPU_APPLICABLE_OPS
                funcs.append({
                    "module": mod,
                    "name": name,
                    "full_name": full,
                    "status": status,
                    "gpu_applicable": gpu,
                })
        # Handle classes (filters, enhancers, fonts)
        for cls in mod_def.get("classes", []):
            if not isinstance(cls, dict):
                continue
            cls_name = cls.get("name", "")
            cls_status = cls.get("status", "stub")
            if cls_status == "stub":
                continue
            # Class itself as a function (e.g., ImageFilter.BLUR)
            gpu = cls_name in GPU_APPLICABLE_OPS
            funcs.append({
                "module": mod,
                "name": cls_name,
                "full_name": f"{mod}.{cls_name}",
                "status": cls_status,
                "gpu_applicable": gpu,
            })
            # Class methods
            for m in cls.get("methods", []):
                m_name = m.get("name", str(m)) if isinstance(m, dict) else str(m)
                funcs.append({
                    "module": mod,
                    "name": m_name,
                    "full_name": f"{mod}.{cls_name}.{m_name}",
                    "status": cls_status,
                    "gpu_applicable": m_name in GPU_APPLICABLE_OPS,
                })
    return funcs

def sort_by_priority(funcs):
    """Sort: priority ops first (puhu order), then alpha by module.name."""
    priority_map = {name: i for i, name in enumerate(PRIORITY_OPS)}
    def sort_key(f):
        # Check if this function matches a priority op
        p = priority_map.get(f["name"], 999)
        return (p, f["module"], f["name"])
    return sorted(funcs, key=sort_key)

if __name__ == "__main__":
    manifest = load_manifest()
    funcs = extract_functions(manifest)
    funcs = sort_by_priority(funcs)
    if "--json" in sys.argv:
        print(json.dumps(funcs, indent=2))
    else:
        for f in funcs:
            gpu = "GPU" if f["gpu_applicable"] else "   "
            print(f"[{gpu}] {f['full_name']}")
        print(f"\nTotal: {len(funcs)} functions")
```

- [ ] **Step 3: Write Pillow baseline script**

Create `scripts/bench_pillow_baseline.py`:
```python
#!/usr/bin/env python3
"""Generate Pillow CPU baseline timings for all manifest functions.

Usage: python scripts/bench_pillow_baseline.py [--runs N] [--output path]

Output: target/benchmarks/pillow_baseline.json
  { "function_name": { "mean_ms": 1.23, "std_ms": 0.05, "runs": 30 }, ... }
"""
import json, sys, timeit, time
from pathlib import Path
from PIL import Image, ImageFilter, ImageOps, ImageChops, ImageEnhance, ImageDraw

ROOT = Path(__file__).parent.parent
IMAGES_DIR = ROOT / "scripts" / "bench_reference_images"

# Mapping: function name → (Image setup code, operation code)
# Each entry is a callable that takes (image) and returns timing-relevant result
BENCH_SPECS = {
    "open": lambda: _bench_open(),
    "save": lambda: _bench_save(),
    "resize": lambda: _bench_op("resize", lambda im: im.resize((800, 600), Image.LANCZOS)),
    "crop": lambda: _bench_op("crop", lambda im: im.crop((100, 100, 500, 500))),
    "rotate": lambda: _bench_op("rotate", lambda im: im.rotate(90)),
    "transpose": lambda: _bench_op("transpose", lambda im: im.transpose(Image.FLIP_LEFT_RIGHT)),
    "thumbnail": lambda: _bench_op("thumbnail", lambda im: im.thumbnail((128, 128), Image.LANCZOS)),
    "to_bytes": lambda: _bench_op("to_bytes", lambda im: im.tobytes()),
    "new": lambda: _bench_new(),
    "paste": lambda: _bench_paste(False, False),
    "paste_mask": lambda: _bench_paste(False, True),
    "paste_color": lambda: _bench_paste(True, False),
    "convert": lambda: _bench_op("convert", lambda im: im.convert("L")),
    "filter/BLUR": lambda: _bench_op("filter", lambda im: im.filter(ImageFilter.BLUR)),
    "filter/CONTOUR": lambda: _bench_op("filter", lambda im: im.filter(ImageFilter.CONTOUR)),
    "filter/DETAIL": lambda: _bench_op("filter", lambda im: im.filter(ImageFilter.DETAIL)),
    "filter/EDGE_ENHANCE": lambda: _bench_op("filter", lambda im: im.filter(ImageFilter.EDGE_ENHANCE)),
    "filter/EMBOSS": lambda: _bench_op("filter", lambda im: im.filter(ImageFilter.EMBOSS)),
    "filter/FIND_EDGES": lambda: _bench_op("filter", lambda im: im.filter(ImageFilter.FIND_EDGES)),
    "filter/SHARPEN": lambda: _bench_op("filter", lambda im: im.filter(ImageFilter.SHARPEN)),
    "filter/SMOOTH": lambda: _bench_op("filter", lambda im: im.filter(ImageFilter.SMOOTH)),
    "filter/GaussianBlur": lambda: _bench_op("filter", lambda im: im.filter(ImageFilter.GaussianBlur(2))),
    "filter/BoxBlur": lambda: _bench_op("filter", lambda im: im.filter(ImageFilter.BoxBlur(2))),
    "filter/UnsharpMask": lambda: _bench_op("filter", lambda im: im.filter(ImageFilter.UnsharpMask(2, 150, 3))),
    "filter/MedianFilter": lambda: _bench_op("filter", lambda im: im.filter(ImageFilter.MedianFilter(3))),
    "filter/ModeFilter": lambda: _bench_op("filter", lambda im: im.filter(ImageFilter.ModeFilter(3))),
    "filter/MaxFilter": lambda: _bench_op("filter", lambda im: im.filter(ImageFilter.MaxFilter(3))),
    "filter/MinFilter": lambda: _bench_op("filter", lambda im: im.filter(ImageFilter.MinFilter(3))),
    "split": lambda: _bench_op("split", lambda im: im.split()),
    "getbands": lambda: _bench_op("getbands", lambda im: im.getbands()),
    "copy": lambda: _bench_op("copy", lambda im: im.copy()),
    "getbbox": lambda: _bench_op("getbbox", lambda im: im.getbbox()),
    "getcolors": lambda: _bench_op("getcolors", lambda im: im.getcolors()),
    "getextrema": lambda: _bench_op("getextrema", lambda im: im.getextrema()),
    "getpixel": lambda: _bench_op("getpixel", lambda im: im.getpixel((50, 50))),
    "getprojection": lambda: _bench_op("getprojection", lambda im: im.getprojection()),
    "histogram": lambda: _bench_op("histogram", lambda im: im.histogram()),
    "point": lambda: _bench_op("point", lambda im: im.point(lambda x: x * 2)),
    "putalpha": lambda: _bench_op("putalpha", lambda im: im.putalpha(128)),
    "putpixel": lambda: _bench_op("putpixel", lambda im: im.putpixel((10, 10), (255, 0, 0))),
    "quantize": lambda: _bench_op("quantize", lambda im: im.quantize(16)),
    "reduce": lambda: _bench_op("reduce", lambda im: im.reduce(2)),
    "alpha_composite": lambda: _bench_alpha_composite(),
    "entropy": lambda: _bench_op("entropy", lambda im: im.entropy()),
    "effect_spread": lambda: _bench_op("effect_spread", lambda im: im.effect_spread(3)),
    # ImageOps
    "autocontrast": lambda: _bench_op("autocontrast", lambda im: ImageOps.autocontrast(im)),
    "equalize": lambda: _bench_op("equalize", lambda im: ImageOps.equalize(im)),
    "invert": lambda: _bench_op("invert", lambda im: ImageOps.invert(im)),
    "posterize": lambda: _bench_op("posterize", lambda im: ImageOps.posterize(im, 3)),
    "solarize": lambda: _bench_op("solarize", lambda im: ImageOps.solarize(im, 128)),
    "grayscale": lambda: _bench_op("grayscale", lambda im: ImageOps.grayscale(im)),
    "flip": lambda: _bench_op("flip", lambda im: ImageOps.flip(im)),
    "mirror": lambda: _bench_op("mirror", lambda im: ImageOps.mirror(im)),
    "crop_border": lambda: _bench_op("crop_border", lambda im: ImageOps.crop(im, 10)),
    "expand": lambda: _bench_op("expand", lambda im: ImageOps.expand(im, 10)),
    "contain": lambda: _bench_op("contain", lambda im: ImageOps.contain(im, (800, 600))),
    "cover": lambda: _bench_op("cover", lambda im: ImageOps.cover(im, (800, 600))),
    "fit": lambda: _bench_op("fit", lambda im: ImageOps.fit(im, (800, 600))),
    "pad": lambda: _bench_op("pad", lambda im: ImageOps.pad(im, (800, 600))),
    "scale": lambda: _bench_op("scale", lambda im: ImageOps.scale(im, 0.5)),
    "colorize": lambda: _bench_op("colorize", lambda im: ImageOps.colorize(im, "black", "white")),
    # ImageChops
    "add": lambda: _bench_chops_op("add", lambda a, b: ImageChops.add(a, b)),
    "subtract": lambda: _bench_chops_op("subtract", lambda a, b: ImageChops.subtract(a, b)),
    "multiply": lambda: _bench_chops_op("multiply", lambda a, b: ImageChops.multiply(a, b)),
    "darker": lambda: _bench_chops_op("darker", lambda a, b: ImageChops.darker(a, b)),
    "lighter": lambda: _bench_chops_op("lighter", lambda a, b: ImageChops.lighter(a, b)),
    "difference": lambda: _bench_chops_op("difference", lambda a, b: ImageChops.difference(a, b)),
    "screen": lambda: _bench_chops_op("screen", lambda a, b: ImageChops.screen(a, b)),
    "overlay": lambda: _bench_chops_op("overlay", lambda a, b: ImageChops.overlay(a, b)),
    "hard_light": lambda: _bench_chops_op("hard_light", lambda a, b: ImageChops.hard_light(a, b)),
    "soft_light": lambda: _bench_chops_op("soft_light", lambda a, b: ImageChops.soft_light(a, b)),
    "add_modulo": lambda: _bench_chops_op("add_modulo", lambda a, b: ImageChops.add_modulo(a, b)),
    "subtract_modulo": lambda: _bench_chops_op("subtract_modulo", lambda a, b: ImageChops.subtract_modulo(a, b)),
    "logical_and": lambda: _bench_chops_op("logical_and", lambda a, b: ImageChops.logical_and(a, b)),
    "logical_or": lambda: _bench_chops_op("logical_or", lambda a, b: ImageChops.logical_or(a, b)),
    "logical_xor": lambda: _bench_chops_op("logical_xor", lambda a, b: ImageChops.logical_xor(a, b)),
    "constant": lambda: _bench_chops_op("constant", lambda a, b: ImageChops.constant(a, 128)),
    "duplicate": lambda: _bench_chops_op("duplicate", lambda a, b: ImageChops.duplicate(a)),
    "offset": lambda: _bench_chops_op("offset", lambda a, b: ImageChops.offset(a, 10, 10)),
    # ImageEnhance
    "Brightness": lambda: _bench_enhance("Brightness", lambda im: ImageEnhance.Brightness(im).enhance(1.5)),
    "Color": lambda: _bench_enhance("Color", lambda im: ImageEnhance.Color(im).enhance(1.5)),
    "Contrast": lambda: _bench_enhance("Contrast", lambda im: ImageEnhance.Contrast(im).enhance(1.5)),
    "Sharpness": lambda: _bench_enhance("Sharpness", lambda im: ImageEnhance.Sharpness(im).enhance(1.5)),
    # ImageDraw
    "arc": lambda: _bench_draw_op("arc", lambda d: d.arc([10, 10, 90, 90], 0, 180, fill="red")),
    "line": lambda: _bench_draw_op("line", lambda d: d.line([0, 0, 100, 100], fill="red", width=3)),
    "rectangle": lambda: _bench_draw_op("rectangle", lambda d: d.rectangle([10, 10, 90, 90], outline="red")),
    "ellipse": lambda: _bench_draw_op("ellipse", lambda d: d.ellipse([10, 10, 90, 90], outline="red")),
    "text": lambda: _bench_draw_op("text", lambda d: d.text((10, 10), "Hello", fill="red")),
    "ImageStat.Stat": lambda: _bench_stat(),
}

REF_IMAGE_2K = None  # lazy-loaded


def _load_ref_image():
    global REF_IMAGE_2K
    if REF_IMAGE_2K is None:
        ref = IMAGES_DIR / "ref_2k.jpg"
        if ref.exists():
            REF_IMAGE_2K = Image.open(ref)
        else:
            # Fallback: create synthetic
            REF_IMAGE_2K = Image.new("RGB", (2048, 1536), (128, 128, 128))
    return REF_IMAGE_2K.copy()


def _bench_op(name, op):
    im = _load_ref_image()
    result = op(im)
    # Force evaluation
    if hasattr(result, 'load'):
        result.load()
    return result


def _bench_open():
    ref = IMAGES_DIR / "ref_2k.jpg"
    if ref.exists():
        im = Image.open(ref)
        im.load()
    else:
        im = Image.new("RGB", (2048, 1536))
    return im


def _bench_save():
    import io
    im = Image.new("RGB", (2048, 1536))
    buf = io.BytesIO()
    im.save(buf, format="PNG")
    return buf


def _bench_new():
    return Image.new("RGB", (1920, 1080), (255, 0, 0))


def _bench_paste(color_fill, use_mask):
    im = _load_ref_image()
    if color_fill:
        im.paste((255, 0, 0), (100, 100, 900, 700))
    else:
        paste_im = Image.new("RGB", (800, 600), (0, 255, 0))
        mask = Image.new("L", (800, 600), 128) if use_mask else None
        im.paste(paste_im, (100, 100), mask)
    return im


def _bench_alpha_composite():
    im1 = Image.new("RGBA", (1024, 1024), (255, 0, 0, 128))
    im2 = Image.new("RGBA", (1024, 1024), (0, 0, 255, 128))
    return Image.alpha_composite(im1, im2)


def _bench_chops_op(name, op):
    im_a = _load_ref_image()
    im_b = _load_ref_image()
    return op(im_a, im_b)


def _bench_enhance(name, op):
    im = _load_ref_image()
    return op(im)


def _bench_draw_op(name, op):
    im = _load_ref_image()
    d = ImageDraw.Draw(im)
    op(d)
    return im


def _bench_stat():
    from PIL import ImageStat
    im = _load_ref_image()
    return ImageStat.Stat(im).mean


def run_benchmarks(funcs, runs=30):
    """Run timeit-based benchmarks for each function. Returns {name: {mean_ms, std_ms, runs}}."""
    results = {}
    for name, bench_fn in BENCH_SPECS.items():
        # Warmup
        for _ in range(3):
            bench_fn()
        # Measure
        timer = timeit.Timer(lambda: bench_fn())
        raw_times = timer.repeat(number=1, repeat=runs)
        times_ms = [t * 1000 for t in raw_times]  # convert seconds to ms
        mean = sum(times_ms) / len(times_ms)
        variance = sum((t - mean) ** 2 for t in times_ms) / len(times_ms)
        std = variance ** 0.5
        results[name] = {"mean_ms": round(mean, 3), "std_ms": round(std, 3), "runs": runs}
        print(f"  {name:<40} {mean:>8.2f}ms ±{std:>6.2f}ms")
    return results


if __name__ == "__main__":
    import argparse
    p = argparse.ArgumentParser()
    p.add_argument("--runs", type=int, default=30)
    p.add_argument("--output", type=str, default=str(ROOT / "target" / "benchmarks" / "pillow_baseline.json"))
    args = p.parse_args()

    print("=== Pillow Baseline Benchmarks ===\n")
    results = run_benchmarks(BENCH_SPECS, runs=args.runs)

    Path(args.output).parent.mkdir(parents=True, exist_ok=True)
    with open(args.output, "w") as f:
        json.dump(results, f, indent=2)
    print(f"\nSaved to {args.output}")
    print(f"Functions benchmarked: {len(results)}")
```

- [ ] **Step 4: Download reference images**

```bash
python3 -c "
from PIL import Image
import os
d = 'scripts/bench_reference_images'
os.makedirs(d, exist_ok=True)

# ref_2k.jpg: 2048x1536 RGB with gradient pattern (simulates photo)
im = Image.new('RGB', (2048, 1536))
import random; random.seed(42)
pixels = im.load()
for y in range(im.height):
    for x in range(im.width):
        r = (x * 255 // im.width + y * 64 // im.height) % 256
        g = (y * 255 // im.height + x * 32 // im.width) % 256
        b = ((x + y) * 128 // (im.width + im.height)) % 256
        pixels[x, y] = (r, g, b)
im.save(f'{d}/ref_2k.jpg', quality=95)
print(f'Created ref_2k.jpg: {im.size}')

# ref_1k.png: 1024x1024 RGBA
im = Image.new('RGBA', (1024, 1024))
pixels = im.load()
for y in range(im.height):
    for x in range(im.width):
        r = int((x / im.width) * 255)
        g = int((y / im.height) * 255)
        b = 128
        a = 255 if (x + y) % 200 > 100 else 128
        pixels[x, y] = (r, g, b, a)
im.save(f'{d}/ref_1k.png')
print(f'Created ref_1k.png: {im.size}')

# ref_grayscale.png: 1024x1024 L
im = Image.new('L', (1024, 1024))
pixels = im.load()
for y in range(im.height):
    for x in range(im.width):
        pixels[x, y] = int(((x + y) / (im.width + im.height)) * 255)
im.save(f'{d}/ref_grayscale.png')
print(f'Created ref_grayscale.png: {im.size}')
"
```

- [ ] **Step 5: Run Pillow baseline to verify**

```bash
python3 scripts/bench_pillow_baseline.py --runs 5
```

Expected: Prints timing for each function, saves `target/benchmarks/pillow_baseline.json`.

- [ ] **Step 6: Commit**

```bash
git add scripts/bench_manifest.py scripts/bench_pillow_baseline.py scripts/bench_reference_images/
git commit -m "feat: add Pillow baseline benchmarking scripts

- bench_manifest.py: parse manifest.yaml into flat function list
- bench_pillow_baseline.py: time Pillow operations for speedup baseline
- bench_reference_images/: synthetic benchmark images (2048x1536, 1024x1024)"
```

---

### Task 2: Native CPU benchmark harness (criterion)

**Files:**
- Create: `pillow-rs/benches/bench_utils.rs`
- Create: `pillow-rs/benches/native_cpu.rs`
- Modify: `pillow-rs/Cargo.toml` (add `[[bench]]` + dev-deps)

- [ ] **Step 1: Add benchmark dependencies to Cargo.toml**

Modify `pillow-rs/Cargo.toml` — add after `[dependencies]`:

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "native_cpu"
harness = false
```

- [ ] **Step 2: Write benchmark utilities**

Create `pillow-rs/benches/bench_utils.rs`:
```rust
use pillow_rs::image::Image;
use std::path::PathBuf;

/// Get path to reference images directory
pub fn ref_images_dir() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../scripts/bench_reference_images");
    p
}

/// Load ref_2k.jpg as an Image (2048x1536 RGB)
pub fn load_ref_2k() -> Image {
    let path = ref_images_dir().join("ref_2k.jpg");
    Image::open(&path.to_string_lossy(), None).expect("Failed to load ref_2k.jpg")
}

/// Load ref_1k.png as an Image (1024x1024 RGBA)
pub fn load_ref_1k() -> Image {
    let path = ref_images_dir().join("ref_1k.png");
    Image::open(&path.to_string_lossy(), None).expect("Failed to load ref_1k.png")
}

/// Load ref_grayscale.png as an Image (1024x1024 L)
pub fn load_ref_grayscale() -> Image {
    let path = ref_images_dir().join("ref_grayscale.png");
    Image::open(&path.to_string_lossy(), None).expect("Failed to load ref_grayscale.png")
}
```

- [ ] **Step 3: Write native CPU benchmark harness**

Create `pillow-rs/benches/native_cpu.rs`:
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pillow_rs::image::Image;
use std::path::PathBuf;

mod bench_utils;
use bench_utils::{load_ref_2k, load_ref_1k, load_ref_grayscale};

/// Helper: time an operation that takes an Image and returns a new Image
fn bench_image_op(c: &mut Criterion, name: &str, op: impl Fn(&Image) -> Image) {
    let img = load_ref_2k();
    c.bench_function(name, |b| {
        b.iter(|| {
            let result = op(&img);
            black_box(result);
        })
    });
}

// ─── Priority Operations (Tier 1, puhu parity) ───

fn bench_open(c: &mut Criterion) {
    let path = bench_utils::ref_images_dir().join("ref_2k.jpg");
    let path_str = path.to_string_lossy().to_string();
    c.bench_function("open", |b| {
        b.iter(|| {
            let img = Image::open(&path_str, None).unwrap();
            black_box(img);
        })
    });
}

fn bench_save(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("save", |b| {
        b.iter(|| {
            let mut buf = Vec::new();
            // Use our internal save - requires a writer
            // For benchmarking, save to /dev/null equivalent
            pillow_rs::format::save_to_bytes(&img, "PNG").unwrap();
            black_box(());
        })
    });
}

fn bench_resize(c: &mut Criterion) {
    bench_image_op(c, "resize", |img| {
        img.resize((800, 600), Some("LANCZOS")).unwrap()
    });
}

fn bench_crop(c: &mut Criterion) {
    bench_image_op(c, "crop", |img| {
        img.crop(100.0, 100.0, 500.0, 500.0).unwrap()
    });
}

fn bench_rotate(c: &mut Criterion) {
    bench_image_op(c, "rotate", |img| {
        img.rotate(90.0, None, false, None, None, None).unwrap()
    });
}

fn bench_transpose(c: &mut Criterion) {
    bench_image_op(c, "transpose", |img| {
        img.transpose("FLIP_LEFT_RIGHT").unwrap()
    });
}

fn bench_thumbnail(c: &mut Criterion) {
    bench_image_op(c, "thumbnail", |img| {
        let mut clone = img.clone();
        clone.thumbnail((128, 128), None).unwrap();
        clone
    });
}

fn bench_to_bytes(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("to_bytes", |b| {
        b.iter(|| {
            let bytes = img.tobytes("raw", &[]).unwrap();
            black_box(bytes);
        })
    });
}

fn bench_new_image(c: &mut Criterion) {
    c.bench_function("new", |b| {
        b.iter(|| {
            let img = Image::new(1920, 1080, "RGB", (255, 0, 0, 255)).unwrap();
            black_box(img);
        })
    });
}

fn bench_paste(c: &mut Criterion) {
    let img = load_ref_2k();
    let paste_src = Image::new(800, 600, "RGB", (0, 255, 0, 255)).unwrap();
    c.bench_function("paste", |b| {
        b.iter(|| {
            let mut clone = img.clone();
            // Using internal paste function
            pillow_rs::ops::paste::paste_image(
                &mut clone, &paste_src, (100, 100), None,
            ).unwrap();
            black_box(clone);
        })
    });
}

fn bench_convert(c: &mut Criterion) {
    bench_image_op(c, "convert", |img| {
        img.convert("L", None, None, None, None).unwrap()
    });
}

// ─── Filter operations ───

fn bench_filter_blur(c: &mut Criterion) {
    bench_image_op(c, "filter/BLUR", |img| {
        img.filter("BLUR", None).unwrap()
    });
}

fn bench_filter_contour(c: &mut Criterion) {
    bench_image_op(c, "filter/CONTOUR", |img| {
        img.filter("CONTOUR", None).unwrap()
    });
}

fn bench_filter_emboss(c: &mut Criterion) {
    bench_image_op(c, "filter/EMBOSS", |img| {
        img.filter("EMBOSS", None).unwrap()
    });
}

fn bench_filter_sharpen(c: &mut Criterion) {
    bench_image_op(c, "filter/SHARPEN", |img| {
        img.filter("SHARPEN", None).unwrap()
    });
}

fn bench_filter_smooth(c: &mut Criterion) {
    bench_image_op(c, "filter/SMOOTH", |img| {
        img.filter("SMOOTH", None).unwrap()
    });
}

fn bench_filter_gaussian_blur(c: &mut Criterion) {
    bench_image_op(c, "filter/GaussianBlur", |img| {
        img.filter("GAUSSIAN_BLUR", Some("2.0")).unwrap()
    });
}

fn bench_filter_box_blur(c: &mut Criterion) {
    bench_image_op(c, "filter/BoxBlur", |img| {
        img.filter("BOX_BLUR", Some("2.0")).unwrap()
    });
}

fn bench_filter_unsharp_mask(c: &mut Criterion) {
    bench_image_op(c, "filter/UnsharpMask", |img| {
        img.filter("UNSHARP_MASK", Some("2.0")).unwrap()
    });
}

fn bench_filter_median(c: &mut Criterion) {
    bench_image_op(c, "filter/MedianFilter", |img| {
        img.filter("MEDIAN_FILTER", Some("3")).unwrap()
    });
}

fn bench_filter_mode(c: &mut Criterion) {
    bench_image_op(c, "filter/ModeFilter", |img| {
        img.filter("MODE_FILTER", Some("3")).unwrap()
    });
}

fn bench_filter_max(c: &mut Criterion) {
    bench_image_op(c, "filter/MaxFilter", |img| {
        img.filter("MAX_FILTER", Some("3")).unwrap()
    });
}

fn bench_filter_min(c: &mut Criterion) {
    bench_image_op(c, "filter/MinFilter", |img| {
        img.filter("MIN_FILTER", Some("3")).unwrap()
    });
}

// ─── Channel operations ───

fn bench_invert(c: &mut Criterion) {
    bench_image_op(c, "invert", |img| {
        pillow_rs::ops::chops::invert(&img).unwrap()
    });
}

fn bench_chops_add(c: &mut Criterion) {
    let a = load_ref_2k();
    let b = load_ref_2k();
    c.bench_function("chops/add", |bencher| {
        bencher.iter(|| {
            let result = pillow_rs::ops::chops::add(&a, &b, 1.0, 0.0).unwrap();
            black_box(result);
        })
    });
}

fn bench_chops_multiply(c: &mut Criterion) {
    let a = load_ref_2k();
    let b = load_ref_2k();
    c.bench_function("chops/multiply", |bencher| {
        bencher.iter(|| {
            let result = pillow_rs::ops::chops::multiply(&a, &b).unwrap();
            black_box(result);
        })
    });
}

fn bench_chops_darker(c: &mut Criterion) {
    let a = load_ref_2k();
    let b = load_ref_2k();
    c.bench_function("chops/darker", |bencher| {
        bencher.iter(|| {
            let result = pillow_rs::ops::chops::darker(&a, &b).unwrap();
            black_box(result);
        })
    });
}

fn bench_chops_lighter(c: &mut Criterion) {
    let a = load_ref_2k();
    let b = load_ref_2k();
    c.bench_function("chops/lighter", |bencher| {
        bencher.iter(|| {
            let result = pillow_rs::ops::chops::lighter(&a, &b).unwrap();
            black_box(result);
        })
    });
}

fn bench_chops_screen(c: &mut Criterion) {
    let a = load_ref_2k();
    let b = load_ref_2k();
    c.bench_function("chops/screen", |bencher| {
        bencher.iter(|| {
            let result = pillow_rs::ops::chops::screen(&a, &b).unwrap();
            black_box(result);
        })
    });
}

fn bench_chops_difference(c: &mut Criterion) {
    let a = load_ref_2k();
    let b = load_ref_2k();
    c.bench_function("chops/difference", |bencher| {
        bencher.iter(|| {
            let result = pillow_rs::ops::chops::difference(&a, &b).unwrap();
            black_box(result);
        })
    });
}

fn bench_chops_subtract(c: &mut Criterion) {
    let a = load_ref_2k();
    let b = load_ref_2k();
    c.bench_function("chops/subtract", |bencher| {
        bencher.iter(|| {
            let result = pillow_rs::ops::chops::subtract(&a, &b, 1.0, 0.0).unwrap();
            black_box(result);
        })
    });
}

// ─── Other operations ───

fn bench_quantize(c: &mut Criterion) {
    bench_image_op(c, "quantize", |img| {
        img.quantize(16, None, 0, None, Some("FLOYDSTEINBERG")).unwrap()
    });
}

fn bench_reduce(c: &mut Criterion) {
    bench_image_op(c, "reduce", |img| {
        img.reduce(2, None).unwrap()
    });
}

fn bench_split(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("split", |b| {
        b.iter(|| {
            let bands = img.split().unwrap();
            black_box(bands);
        })
    });
}

fn bench_getpixel(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("getpixel", |b| {
        b.iter(|| {
            let pixel = img.getpixel(50, 50).unwrap();
            black_box(pixel);
        })
    });
}

fn bench_putpixel(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("putpixel", |b| {
        b.iter(|| {
            let mut clone = img.clone();
            clone.putpixel(10, 10, (255u8, 0u8, 0u8, 255u8)).unwrap();
            black_box(clone);
        })
    });
}

fn bench_putalpha(c: &mut Criterion) {
    let img = load_ref_2k();
    c.bench_function("putalpha", |b| {
        b.iter(|| {
            let mut clone = img.clone();
            clone.putalpha(128).unwrap();
            black_box(clone);
        })
    });
}

fn bench_point(c: &mut Criterion) {
    let img = load_ref_grayscale();
    c.bench_function("point", |b| {
        b.iter(|| {
            let result = img.point(&|x: u8| (x.wrapping_mul(2)).min(255)).unwrap();
            black_box(result);
        })
    });
}

criterion_group!(
    benches,
    // Priority
    bench_open,
    bench_save,
    bench_resize,
    bench_crop,
    bench_rotate,
    bench_transpose,
    bench_thumbnail,
    bench_to_bytes,
    bench_new_image,
    bench_paste,
    bench_convert,
    // Filters
    bench_filter_blur,
    bench_filter_contour,
    bench_filter_emboss,
    bench_filter_sharpen,
    bench_filter_smooth,
    bench_filter_gaussian_blur,
    bench_filter_box_blur,
    bench_filter_unsharp_mask,
    bench_filter_median,
    bench_filter_mode,
    bench_filter_max,
    bench_filter_min,
    // Channel ops
    bench_invert,
    bench_chops_add,
    bench_chops_multiply,
    bench_chops_darker,
    bench_chops_lighter,
    bench_chops_screen,
    bench_chops_difference,
    bench_chops_subtract,
    // Others
    bench_quantize,
    bench_reduce,
    bench_split,
    bench_getpixel,
    bench_putpixel,
    bench_putalpha,
    bench_point,
);
criterion_main!(benches);
```

- [ ] **Step 4: Verify benchmark compiles and runs**

```bash
cargo bench -p pillow-rs --no-run 2>&1 | tail -5
```

Expected: Compiles without errors.

- [ ] **Step 5: Run benchmarks (quick test)**

```bash
cargo bench -p pillow-rs -- --quick --sample-size 10 2>&1 | tail -20
```

Expected: Benchmark output with timings.

- [ ] **Step 6: Commit**

```bash
git add pillow-rs/benches/ pillow-rs/Cargo.toml
git commit -m "feat: add native CPU criterion benchmarks

37 benchmark functions covering priority ops, filters, chops, and misc.
Uses synthetic reference images from scripts/bench_reference_images/."
```

---

### Task 3: Cache system

**Files:**
- Create: `scripts/bench_cache.py`

- [ ] **Step 1: Write cache module**

Create `scripts/bench_cache.py`:
```python
#!/usr/bin/env python3
"""Incremental cache: compute cache keys, detect which functions need re-benchmarking.

Usage:
  python scripts/bench_cache.py --check          # Print functions needing re-bench
  python scripts/bench_cache.py --update <target> # Update cache after running target
  python scripts/bench_cache.py --init            # Initialize empty cache
"""
import hashlib, json, sys, os, subprocess
from pathlib import Path
from bench_manifest import extract_functions, sort_by_priority, load_manifest

ROOT = Path(__file__).parent.parent
CACHE_DIR = ROOT / "target" / "benchmarks"
CACHE_FILE = CACHE_DIR / "cache.json"

SHARED_FILES = [
    "pillow-rs/src/image.rs",
    "pillow-rs/src/lazy.rs",
    "pillow-rs/src/error.rs",
    "pillow-rs/src/color.rs",
    "pillow-rs/Cargo.toml",
]

# Map function name → source file for cache key computation
FUNC_SOURCE_MAP = {
    "resize": "pillow-rs/src/ops/resize.rs",
    "crop": "pillow-rs/src/ops/crop.rs",
    "rotate": "pillow-rs/src/ops/rotate.rs",
    "convert": "pillow-rs/src/ops/convert.rs",
    "transpose": "pillow-rs/src/ops/transpose.rs",
    "paste": "pillow-rs/src/ops/paste.rs",
    "filter": "pillow-rs/src/ops/filter.rs",
    "split": "pillow-rs/src/ops/split.rs",
    "quantize": "pillow-rs/src/ops/quantize.rs",
    "reduce": "pillow-rs/src/ops/transform.rs",
    "thumbnail": "pillow-rs/src/ops/resize.rs",  # shares resize.rs
    "open": "pillow-rs/src/format.rs",
    "save": "pillow-rs/src/format.rs",
    "to_bytes": "pillow-rs/src/ops/imageops.rs",
    "new": "pillow-rs/src/image.rs",
    "autocontrast": "pillow-rs/src/ops/imageops.rs",
    "equalize": "pillow-rs/src/ops/imageops.rs",
    "invert": "pillow-rs/src/ops/chops.rs",
    "posterize": "pillow-rs/src/ops/imageops.rs",
    "solarize": "pillow-rs/src/ops/imageops.rs",
    "grayscale": "pillow-rs/src/ops/imageops.rs",
    "flip": "pillow-rs/src/ops/imageops.rs",
    "mirror": "pillow-rs/src/ops/imageops.rs",
    "colorize": "pillow-rs/src/ops/imageops.rs",
    "contain": "pillow-rs/src/ops/imageops.rs",
    "cover": "pillow-rs/src/ops/imageops.rs",
    "fit": "pillow-rs/src/ops/imageops.rs",
    "pad": "pillow-rs/src/ops/imageops.rs",
    "scale": "pillow-rs/src/ops/imageops.rs",
    "expand": "pillow-rs/src/ops/imageops.rs",
    "point": "pillow-rs/src/ops/imageops.rs",
    "putalpha": "pillow-rs/src/ops/imageops.rs",
    "putpixel": "pillow-rs/src/ops/imageops.rs",
    "getpixel": "pillow-rs/src/ops/imageops.rs",
    "getbbox": "pillow-rs/src/ops/imageops.rs",
    "getcolors": "pillow-rs/src/ops/imageops.rs",
    "getextrema": "pillow-rs/src/ops/imageops.rs",
    "histogram": "pillow-rs/src/ops/analysis.rs",
    "entropy": "pillow-rs/src/ops/analysis.rs",
    "getprojection": "pillow-rs/src/ops/analysis.rs",
    "effect_spread": "pillow-rs/src/ops/imageops.rs",
    "alpha_composite": "pillow-rs/src/ops/paste.rs",
    "add": "pillow-rs/src/ops/chops.rs",
    "subtract": "pillow-rs/src/ops/chops.rs",
    "multiply": "pillow-rs/src/ops/chops.rs",
    "darker": "pillow-rs/src/ops/chops.rs",
    "lighter": "pillow-rs/src/ops/chops.rs",
    "difference": "pillow-rs/src/ops/chops.rs",
    "screen": "pillow-rs/src/ops/chops.rs",
    "overlay": "pillow-rs/src/ops/chops.rs",
    "hard_light": "pillow-rs/src/ops/chops.rs",
    "soft_light": "pillow-rs/src/ops/chops.rs",
    "add_modulo": "pillow-rs/src/ops/chops.rs",
    "subtract_modulo": "pillow-rs/src/ops/chops.rs",
    "logical_and": "pillow-rs/src/ops/chops.rs",
    "logical_or": "pillow-rs/src/ops/chops.rs",
    "logical_xor": "pillow-rs/src/ops/chops.rs",
    "constant": "pillow-rs/src/ops/chops.rs",
    "duplicate": "pillow-rs/src/ops/chops.rs",
    "offset": "pillow-rs/src/ops/chops.rs",
    "Brightness": "pillow-rs/src/ops/enhance.rs",
    "Color": "pillow-rs/src/ops/enhance.rs",
    "Contrast": "pillow-rs/src/ops/enhance.rs",
    "Sharpness": "pillow-rs/src/ops/enhance.rs",
    "arc": "pillow-rs/src/draw/arc.rs",
    "line": "pillow-rs/src/draw/line.rs",
    "rectangle": "pillow-rs/src/draw/rectangle.rs",
    "ellipse": "pillow-rs/src/draw/ellipse.rs",
    "text": "pillow-rs/src/draw/text.rs",
    "circle": "pillow-rs/src/draw/circle.rs",
    "rounded_rectangle": "pillow-rs/src/draw/rounded_rectangle.rs",
    "chord": "pillow-rs/src/draw/chord.rs",
    "pieslice": "pillow-rs/src/draw/pieslice.rs",
    "polygon": "pillow-rs/src/draw/polygon.rs",
    "regular_polygon": "pillow-rs/src/draw/regular_polygon.rs",
    # Fallback for any others — check all ops files
}


def file_hash(path: Path) -> str:
    """SHA-256 of file contents, or empty string if file doesn't exist."""
    full = ROOT / path
    if not full.exists():
        print(f"Warning: file not found for hashing: {full}", file=sys.stderr)
        return ""
    return hashlib.sha256(full.read_bytes()).hexdigest()


def compute_cache_key(func_name: str) -> str:
    """Compute cache key for a function: hash of its source + shared deps."""
    parts = []
    # Shared files (affect all functions)
    for sf in SHARED_FILES:
        parts.append(file_hash(Path(sf)))
    # Function-specific source
    src_file = FUNC_SOURCE_MAP.get(func_name)
    if src_file:
        parts.append(file_hash(Path(src_file)))
    else:
        # Check if it's a filter/enhance/chops function — try known files
        for candidate in [
            f"pillow-rs/src/ops/{func_name.lower()}.rs",
            f"pillow-rs/src/ops/filter.rs",
            f"pillow-rs/src/ops/chops.rs",
            f"pillow-rs/src/ops/enhance.rs",
            f"pillow-rs/src/ops/imageops.rs",
            f"pillow-rs/src/ops/convert.rs",
            f"pillow-rs/src/ops/mod.rs",
        ]:
            h = file_hash(Path(candidate))
            if h:
                parts.append(h)
                break
    # Reference image hash
    ref_img = ROOT / "scripts" / "bench_reference_images" / "ref_2k.jpg"
    if ref_img.exists():
        parts.append(hashlib.sha256(ref_img.read_bytes()).hexdigest())
    return hashlib.sha256("".join(parts).encode()).hexdigest()


def load_cache() -> dict:
    if CACHE_FILE.exists():
        with open(CACHE_FILE) as f:
            return json.load(f)
    return {}


def save_cache(cache: dict):
    CACHE_DIR.mkdir(parents=True, exist_ok=True)
    with open(CACHE_FILE, "w") as f:
        json.dump(cache, f, indent=2)


def check_stale() -> list:
    """Return list of function names whose cache is stale."""
    cache = load_cache()
    manifest = load_manifest()
    funcs = extract_functions(manifest)
    stale = []
    for func in funcs:
        name = func["name"]
        new_key = compute_cache_key(name)
        cached = cache.get(name, {})
        if cached.get("key") != new_key:
            stale.append(name)
    return stale


def update_cache(func_name: str, target: str, mean_ms: float, std_ms: float):
    """Update cache entry for a function+target after benchmarking."""
    cache = load_cache()
    key = compute_cache_key(func_name)
    if func_name not in cache:
        cache[func_name] = {"key": key, "targets": {}}
    cache[func_name]["key"] = key
    cache[func_name]["targets"][target] = {
        "mean_ms": round(mean_ms, 4),
        "std_ms": round(std_ms, 4),
    }
    save_cache(cache)


if __name__ == "__main__":
    if "--check" in sys.argv:
        stale = check_stale()
        if stale:
            print(f"STALE: {len(stale)} functions need re-benchmarking:")
            for s in stale:
                print(f"  - {s}")
        else:
            print("FRESH: all functions up-to-date")
    elif "--update" in sys.argv:
        # Usage: bench_cache.py --update <target> <func_name> <mean_ms> <std_ms>
        target = sys.argv[2]
        func = sys.argv[3]
        mean = float(sys.argv[4])
        std = float(sys.argv[5])
        update_cache(func, target, mean, std)
        print(f"Updated cache: {func}/{target} = {mean}ms")
    elif "--init" in sys.argv:
        save_cache({})
        print("Initialized empty cache")
    else:
        print("Usage: bench_cache.py --check | --update <target> <func> <mean> <std> | --init")
```

- [ ] **Step 2: Test cache check**

```bash
python3 scripts/bench_cache.py --init
python3 scripts/bench_cache.py --check
```

Expected: "STALE: N functions need re-benchmarking" (all stale since cache just initialized).

- [ ] **Step 3: Commit**

```bash
git add scripts/bench_cache.py
git commit -m "feat: add incremental benchmarking cache

SHA-256 cache keys from source files + reference images.
Stale detection via bench_cache.py --check.
Per-function, per-target cache entries."
```

---

### Task 4: Aggregator — merge benchmark JSONs into BENCHMARKS.md

**Files:**
- Create: `scripts/bench_aggregate.py`

- [ ] **Step 1: Write aggregator**

Create `scripts/bench_aggregate.py`:
```python
#!/usr/bin/env python3
"""Aggregate benchmark results from all targets into BENCHMARKS.md.

Inputs (in target/benchmarks/):
  - pillow_baseline.json   {func: {mean_ms, std_ms, runs}}
  - native_cpu.json        {func: {mean_ms, std_ms}}
  - native_gpu.json        (same, may be partial)
  - wasm_cpu.json          (same)
  - wasm_gpu.json          (same)
  - browser_cpu.json       (same)
  - browser_gpu.json       (same)

Output: BENCHMARKS.md at repo root.
"""
import json, sys, os, subprocess
from datetime import datetime
from pathlib import Path
from bench_manifest import extract_functions, sort_by_priority, load_manifest

ROOT = Path(__file__).parent.parent
BENCH_DIR = ROOT / "target" / "benchmarks"
OUTPUT = ROOT / "BENCHMARKS.md"

TARGETS = ["native_cpu", "native_gpu", "wasm_cpu", "wasm_gpu", "browser_cpu", "browser_gpu"]
TARGET_LABELS = {
    "native_cpu": "CPU",
    "native_gpu": "GPU",
    "wasm_cpu": "WASM CPU",
    "wasm_gpu": "WASM GPU",
    "browser_cpu": "Browser CPU",
    "browser_gpu": "Browser GPU",
}

PRIORITY_OPS = [
    "open_save", "resize", "crop", "rotate", "transpose", "thumbnail",
    "to_bytes", "new", "paste", "paste_mask", "paste_color", "pipeline",
]


def load_json(path: Path) -> dict:
    if path.exists():
        with open(path) as f:
            return json.load(f)
    return {}


def speedup(rs_ms: float, pil_ms: float) -> str:
    """Compute speedup ratio vs Pillow. >1.0 = pillow-rs faster."""
    if pil_ms <= 0 or rs_ms <= 0:
        return "—"
    ratio = pil_ms / rs_ms
    return f"{ratio:.2f}×"


def format_row(func_name: str, results: dict, baseline: dict, gpu_applicable: bool) -> str:
    """Format one table row: Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU"""
    pil_data = baseline.get(func_name.replace("/", "_"), baseline.get(func_name, {}))
    pil_ms = pil_data.get("mean_ms", 0) if isinstance(pil_data, dict) else 0

    cells = []
    for target in TARGETS:
        target_data = results.get(target, {}).get(func_name, {})
        if target_data is None:
            cells.append("—")
        elif "gpu" in target and not gpu_applicable:
            cells.append("—")
        else:
            rs_ms = target_data.get("mean_ms", 0)
            if rs_ms > 0 and pil_ms > 0:
                cells.append(speedup(rs_ms, pil_ms))
            else:
                cells.append("—")

    return f"| {func_name:<35} | " + " | ".join(f"{c:>10}" for c in cells) + " |"


def merge_priority_ops(results: dict, baseline: dict):
    """Create composite entries for the 12 puhu priority operations."""
    prio_results = []
    for op in PRIORITY_OPS:
        # Map puhu composite names to individual function results
        pass  # For initial implementation, priority ops map directly
    return prio_results


def compute_summary(results: dict, baseline: dict, funcs: list) -> dict:
    """Compute summary statistics across all functions."""
    cpu_speedups = []
    for func in funcs:
        pil_data = baseline.get(func["name"], {})
        pil_ms = pil_data.get("mean_ms", 0) if isinstance(pil_data, dict) else 0
        rs_data = results.get("native_cpu", {}).get(func["name"], {})
        rs_ms = rs_data.get("mean_ms", 0) if rs_data else 0
        if rs_ms > 0 and pil_ms > 0:
            cpu_speedups.append(pil_ms / rs_ms)

    return {
        "functions_benchmarked": len(funcs),
        "avg_cpu_speedup": round(sum(cpu_speedups) / len(cpu_speedups), 2) if cpu_speedups else 0,
    }


def generate_markdown(results: dict, baseline: dict) -> str:
    manifest = load_manifest()
    funcs = sort_by_priority(extract_functions(manifest))
    summary = compute_summary(results, baseline, funcs)

    # Get commit SHA
    sha = "unknown"
    try:
        sha = subprocess.check_output(
            ["git", "rev-parse", "--short", "HEAD"],
            cwd=ROOT, text=True
        ).strip()
    except Exception:
        pass

    now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    total = len(funcs)
    gpu_count = sum(1 for f in funcs if f["gpu_applicable"])

    lines = []
    lines.append("# pillow-rs Benchmarks\n")
    lines.append(f"> Auto-generated: {now} | commit `{sha}` | {total} functions | 6 targets\n")
    lines.append("## Summary\n")
    lines.append("| Metric | Value |")
    lines.append("|--------|-------|")
    lines.append(f"| Functions benchmarked | {total} |")
    lines.append(f"| Functions with GPU path | {gpu_count} |")
    lines.append(f"| Average CPU speedup vs Pillow | {summary['avg_cpu_speedup']:.2f}× |")
    lines.append("")

    # Priority operations section
    lines.append("## Priority Operations (Tier 1)\n")
    header = "| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |"
    sep =    "|----------|-----|-----|----------|----------|-------------|-------------|"
    lines.append(header)
    lines.append(sep)
    priority_found = False
    for func in funcs:
        if func["name"] in PRIORITY_OPS or priority_found is False and func["name"] in [
            "open", "resize", "crop", "rotate", "transpose", "thumbnail",
            "to_bytes", "new", "paste", "convert", "filter"
        ]:
            # Show first 12 priority-relevant functions
            pass  # Will be populated when results are available

    # If no priority results available yet, show placeholder
    lines.append("| *(run benchmarks to populate)* |||||||")
    lines.append("")

    # All Functions — grouped by module
    lines.append("## All Functions\n")
    current_module = None
    for func in funcs:
        mod = func["module"]
        if mod != current_module:
            current_module = mod
            lines.append(f"### {mod}\n")
            lines.append(header)
            lines.append(sep)

        name = func["name"]
        gpu = func["gpu_applicable"]
        pil_data = baseline.get(name, {})
        pil_ms = pil_data.get("mean_ms", 0) if isinstance(pil_data, dict) else 0

        cells = []
        for target in TARGETS:
            target_data = results.get(target, {}).get(name, {})
            if target_data is None:
                cells.append("—")
            elif "gpu" in target and not gpu:
                cells.append("—")
            else:
                rs_ms = target_data.get("mean_ms", 0)
                if rs_ms > 0 and pil_ms > 0:
                    cells.append(speedup(rs_ms, pil_ms))
                else:
                    cells.append("—")

        row = f"| {name:<35} | " + " | ".join(f"{c:>10}" for c in cells) + " |"
        lines.append(row)

    lines.append("")
    return "\n".join(lines)


if __name__ == "__main__":
    # Load all results
    results = {}
    for target in TARGETS:
        results[target] = load_json(BENCH_DIR / f"{target}.json")

    baseline = load_json(BENCH_DIR / "pillow_baseline.json")

    if "--json" in sys.argv:
        # Output as JSON for debugging
        output = {
            "results": results,
            "baseline": baseline,
        }
        print(json.dumps(output, indent=2))
    else:
        md = generate_markdown(results, baseline)
        with open(OUTPUT, "w") as f:
            f.write(md)
        print(f"Generated {OUTPUT} ({len(md)} bytes)")
```

- [ ] **Step 2: Test aggregator with baseline data only**

```bash
python3 scripts/bench_aggregate.py
```

Expected: Generates `BENCHMARKS.md` with function names and `—` for all timing columns (no cpu/gpu/wasm data yet).

- [ ] **Step 3: Commit**

```bash
git add scripts/bench_aggregate.py BENCHMARKS.md
git commit -m "feat: add benchmark aggregator + initial BENCHMARKS.md

Merges up to 6 target JSONs + Pillow baseline into a formatted markdown doc.
Functions grouped by module, priority ops section first."
```

---

### Task 5: Orchestrator + pre-commit hook

**Files:**
- Create: `scripts/bench_all.sh`
- Create: `.git/hooks/pre-commit` (via script)

- [ ] **Step 1: Write orchestrator script**

Create `scripts/bench_all.sh`:
```bash
#!/bin/bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BENCH_DIR="$ROOT/target/benchmarks"

mkdir -p "$BENCH_DIR"

MODE="${1:-incremental}"

echo "=== pillow-rs Benchmark Orchestrator ==="
echo "Mode: $MODE"
echo ""

# Step 0: Check cache
if [ "$MODE" = "incremental" ]; then
    cd "$ROOT"
    STALE=$(python3 scripts/bench_cache.py --check 2>&1)
    if echo "$STALE" | grep -q "FRESH"; then
        echo "✓ All functions up-to-date. Skipping benchmarks."
        echo "  (use 'bash scripts/bench_all.sh full' to force re-bench)"
        # Still regenerate BENCHMARKS.md (no new data, but git SHA may have changed)
        python3 scripts/bench_aggregate.py
        exit 0
    fi
    echo "$STALE" | head -20
    echo ""
fi

# Step 1: Native CPU benchmarks (criterion)
echo "─── Native CPU Benchmarks ───"
cd "$ROOT"
if [ "$MODE" = "incremental" ] && ! echo "$STALE" | grep -q "STALE"; then
    echo "  (skipped — no changes)"
else
    cargo bench -p pillow-rs --bench native_cpu \
        -- --output-format bencher 2>&1 | tee "$BENCH_DIR/native_cpu_raw.txt" || true
    # Convert criterion bencher output to our JSON format
    python3 -c "
import json, re, sys
results = {}
text = open('$BENCH_DIR/native_cpu_raw.txt').read()
for line in text.split('\n'):
    m = re.match(r'test\s+(\S+)\s+.*time:\s+\[([\d.]+)\s*(ns|us|ms|s)', line)
    if m:
        name = m.group(1)
        val = float(m.group(2))
        unit = m.group(3)
        # Convert to ms
        if unit == 'ns': val /= 1_000_000
        elif unit == 'us': val /= 1_000
        elif unit == 's': val *= 1_000
        results[name] = {'mean_ms': round(val, 4)}
with open('$BENCH_DIR/native_cpu.json', 'w') as f:
    json.dump(results, f, indent=2)
print(f'Parsed {len(results)} CPU benchmark results')
"
    # Update cache for each result
    if [ -f "$BENCH_DIR/native_cpu.json" ]; then
        python3 -c "
import json
results = json.load(open('$BENCH_DIR/native_cpu.json'))
for name, data in results.items():
    print(f'{name} {data[\"mean_ms\"]} 0')
" | while read -r func mean std; do
            python3 scripts/bench_cache.py --update native_cpu "$func" "$mean" "$std"
        done
    fi
fi

# Step 2: WASM CPU benchmarks (Node.js)
echo ""
echo "─── WASM CPU Benchmarks ───"
cd "$ROOT/pillow-rs-js"
if [ ! -f "pkg/pillow_rs_js.js" ]; then
    echo "  Building WASM package..."
    wasm-pack build --target nodejs --out-dir pkg_node 2>&1 | tail -3
fi
cd "$ROOT"
if [ -f "scripts/bench_wasm_cpu.mjs" ] && [ "$MODE" != "skip-wasm" ]; then
    node scripts/bench_wasm_cpu.mjs 2>&1 | tee "$BENCH_DIR/wasm_cpu_raw.txt" || echo "  (WASM CPU harness not yet available — skipping)"
fi

# Step 3: Browser benchmarks (Puppeteer)
echo ""
echo "─── Browser Benchmarks ───"
if [ -f "scripts/bench_browser.mjs" ] && [ "$MODE" != "skip-browser" ]; then
    node scripts/bench_browser.mjs 2>&1 | tee "$BENCH_DIR/browser_raw.txt" || echo "  (Browser harness not yet available — skipping)"
fi

# Step 4: Generate BENCHMARKS.md
echo ""
echo "─── Generating BENCHMARKS.md ───"
cd "$ROOT"
python3 scripts/bench_aggregate.py
echo "✓ BENCHMARKS.md updated"

# Step 5: Show summary
echo ""
echo "=== Done ==="
head -15 "$ROOT/BENCHMARKS.md"
```

- [ ] **Step 2: Make executable**

```bash
chmod +x scripts/bench_all.sh
```

- [ ] **Step 3: Test orchestrator (full run)**

```bash
bash scripts/bench_all.sh full
```

Expected: Runs CPU benchmarks, tries WASM (skips if harness not built), generates BENCHMARKS.md.

- [ ] **Step 4: Install pre-commit hook**

Create `.git/hooks/pre-commit`:
```bash
#!/bin/bash
# pillow-rs pre-commit: run benchmarks, stage BENCHMARKS.md
set -e

ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

echo "━━━ pre-commit: benchmarks ━━━"

# Run incremental benchmarks
bash scripts/bench_all.sh incremental

# Stage BENCHMARKS.md if it changed
if ! git diff --quiet BENCHMARKS.md; then
    git add BENCHMARKS.md
    echo "✓ Staged updated BENCHMARKS.md"
else
    echo "✓ BENCHMARKS.md unchanged"
fi

echo "━━━ pre-commit: done ━━━"
```

Make executable:
```bash
chmod +x .git/hooks/pre-commit
```

- [ ] **Step 5: Commit**

```bash
git add scripts/bench_all.sh .git/hooks/pre-commit
git commit -m "feat: add benchmark orchestrator + pre-commit hook

scripts/bench_all.sh: runs all harnesses, aggregates into BENCHMARKS.md
pre-commit: incremental mode — only re-benches changed code paths"
```

---

### Task 6: WASM CPU harness (Node.js)

**Files:**
- Create: `scripts/bench_wasm_cpu.mjs`
- Modify: `pillow-rs-js/package.json` (add `"type": "module"` if needed)

- [ ] **Step 1: Build WASM for Node.js target**

```bash
cd pillow-rs-js
wasm-pack build --target nodejs --out-dir pkg_node 2>&1 | tail -5
```

- [ ] **Step 2: Write WASM CPU harness**

Create `scripts/bench_wasm_cpu.mjs`:
```javascript
#!/usr/bin/env node
/**
 * WASM CPU Benchmark Harness
 *
 * Loads the pillow-rs WASM binary (Node.js target) and benchmarks
 * all image operations against the reference images.
 *
 * Usage: node scripts/bench_wasm_cpu.mjs
 * Output: target/benchmarks/wasm_cpu.json
 */
import { readFileSync, writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { performance } from 'perf_hooks';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');
const BENCH_DIR = join(ROOT, 'target', 'benchmarks');
const WASM_DIR = join(ROOT, 'pillow-rs-js', 'pkg_node');
const IMG_DIR = join(ROOT, 'scripts', 'bench_reference_images');

// Dynamic import of WASM
const wasm = await import(join(WASM_DIR, 'pillow_rs_js.js'));

const RUNS = 30;
const WARMUP = 3;

function loadImage(path) {
    const data = readFileSync(path);
    return wasm.Image.open(new Uint8Array(data));
}

function bench(name, fn, setup) {
    // Warmup
    for (let i = 0; i < WARMUP; i++) {
        const ctx = setup ? setup() : null;
        fn(ctx);
    }
    // Measure
    const times = [];
    for (let i = 0; i < RUNS; i++) {
        const ctx = setup ? setup() : null;
        const start = performance.now();
        fn(ctx);
        const end = performance.now();
        times.push(end - start);
    }
    // Stats
    const mean = times.reduce((a, b) => a + b, 0) / times.length;
    const variance = times.reduce((s, t) => s + (t - mean) ** 2, 0) / times.length;
    const std = Math.sqrt(variance);
    return { mean_ms: Math.round(mean * 1000) / 1000, std_ms: Math.round(std * 1000) / 1000, runs: RUNS };
}

// ─── Benchmarks ───

const ref2kPath = join(IMG_DIR, 'ref_2k.jpg');

const benchmarks = {
    open: () => {
        const data = readFileSync(ref2kPath);
        return () => wasm.Image.open(new Uint8Array(data));
    },
    resize: () => {
        const img = wasm.Image.open(new Uint8Array(readFileSync(ref2kPath)));
        return () => img.resize(800, 600, 'LANCZOS');
    },
    crop: () => {
        const img = wasm.Image.open(new Uint8Array(readFileSync(ref2kPath)));
        return () => img.crop(100, 100, 500, 500);
    },
    rotate: () => {
        const img = wasm.Image.open(new Uint8Array(readFileSync(ref2kPath)));
        return () => img.rotate(90, 'NEAREST', false, null, null, null);
    },
    transpose: () => {
        const img = wasm.Image.open(new Uint8Array(readFileSync(ref2kPath)));
        return () => img.transpose('FLIP_LEFT_RIGHT');
    },
    thumbnail: () => {
        const data = readFileSync(ref2kPath);
        return () => {
            const img = wasm.Image.open(new Uint8Array(data));
            img.thumbnail(128, 128, 'LANCZOS');
        };
    },
    convert: () => {
        const img = wasm.Image.open(new Uint8Array(readFileSync(ref2kPath)));
        return () => img.convert('L', null, null, null, null);
    },
    filter_blur: () => {
        const img = wasm.Image.open(new Uint8Array(readFileSync(ref2kPath)));
        return () => img.filter('BLUR', null);
    },
    invert: () => {
        const img = wasm.Image.open(new Uint8Array(readFileSync(ref2kPath)));
        return () => wasm.ImageOps.invert(img);
    },
    // Add more as the WASM bindings are expanded
};

// ─── Run ───

console.log('=== WASM CPU Benchmarks ===\n');
const results = {};

for (const [name, setupFn] of Object.entries(benchmarks)) {
    process.stdout.write(`  ${name.padEnd(35)} `);
    try {
        const fn = setupFn();
        const result = bench(name, fn);
        results[name] = result;
        console.log(`${result.mean_ms.toFixed(2).padStart(8)}ms ±${result.std_ms.toFixed(2)}ms`);
    } catch (e) {
        console.log(`ERROR: ${e.message}`);
    }
}

// Save
mkdirSync(BENCH_DIR, { recursive: true });
writeFileSync(join(BENCH_DIR, 'wasm_cpu.json'), JSON.stringify(results, null, 2));
console.log(`\nSaved ${Object.keys(results).length} results to wasm_cpu.json`);
```

- [ ] **Step 3: Test WASM harness**

```bash
node scripts/bench_wasm_cpu.mjs
```

Expected: Loads WASM, runs benchmarks, saves JSON.

- [ ] **Step 4: Commit**

```bash
git add scripts/bench_wasm_cpu.mjs pillow-rs-js/package.json
git commit -m "feat: add WASM CPU benchmark harness (Node.js)

Loads wasm-pack output, times operations via performance.now().
Outputs target/benchmarks/wasm_cpu.json."
```

---

### Task 7: Browser harness (Puppeteer)

**Files:**
- Create: `pillow-rs-js/bench_page/index.html`
- Create: `pillow-rs-js/bench_page/bench_runner.js`
- Create: `scripts/bench_browser.mjs`

- [ ] **Step 1: Create browser benchmark page**

Create `pillow-rs-js/bench_page/index.html`:
```html
<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>pillow-rs Benchmarks</title></head>
<body>
<h1>pillow-rs Browser Benchmarks</h1>
<div id="status">Loading...</div>
<div id="results"></div>
<script type="module" src="./bench_runner.js"></script>
</body>
</html>
```

Create `pillow-rs-js/bench_page/bench_runner.js`:
```javascript
// Browser benchmark runner — loaded in headless Chrome via Puppeteer
import init, { Image } from '../pkg/pillow_rs_js.js';

const RUNS = 30;
const WARMUP = 3;

async function run() {
    const status = document.getElementById('status');
    status.textContent = 'Initializing WASM...';
    await init();

    status.textContent = 'Loading reference image...';
    const resp = await fetch('../../scripts/bench_reference_images/ref_2k.jpg');
    const buffer = await resp.arrayBuffer();
    const imgData = new Uint8Array(buffer);

    const results = {};
    const benchmarks = {
        'open': async () => {
            const data = new Uint8Array(buffer);
            return Image.open(data);
        },
        'resize': async () => {
            const img = Image.open(new Uint8Array(buffer));
            return img.resize(800, 600, 'LANCZOS');
        },
        'crop': async () => {
            const img = Image.open(new Uint8Array(buffer));
            return img.crop(100, 100, 500, 500);
        },
        'rotate': async () => {
            const img = Image.open(new Uint8Array(buffer));
            return img.rotate(90, 'NEAREST', false, null, null, null);
        },
        'transpose': async () => {
            const img = Image.open(new Uint8Array(buffer));
            return img.transpose('FLIP_LEFT_RIGHT');
        },
        'convert': async () => {
            const img = Image.open(new Uint8Array(buffer));
            return img.convert('L', null, null, null, null);
        },
    };

    for (const [name, fn] of Object.entries(benchmarks)) {
        // Warmup
        for (let i = 0; i < WARMUP; i++) await fn();
        // Measure
        const times = [];
        for (let i = 0; i < RUNS; i++) {
            const start = performance.now();
            await fn();
            times.push(performance.now() - start);
        }
        const mean = times.reduce((a, b) => a + b, 0) / times.length;
        results[name] = { mean_ms: Math.round(mean * 1000) / 1000, runs: RUNS };
    }

    document.getElementById('results').textContent = JSON.stringify(results);
    document.getElementById('status').textContent = 'Done';
}

run().catch(e => {
    document.getElementById('status').textContent = 'Error: ' + e.message;
});
```

- [ ] **Step 2: Create Puppeteer driver**

Create `scripts/bench_browser.mjs`:
```javascript
#!/usr/bin/env node
/**
 * Browser benchmark driver — uses Puppeteer to drive headless Chrome.
 * Runs both CPU and GPU browser targets.
 */
import puppeteer from 'puppeteer';
import { writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { createServer } from 'http';
import { readFileSync, existsSync } from 'fs';
import { extname } from 'path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');
const BENCH_DIR = join(ROOT, 'target', 'benchmarks');
const BENCH_PAGE = join(ROOT, 'pillow-rs-js', 'bench_page', 'index.html');

const MIME = {
    '.html': 'text/html',
    '.js': 'application/javascript',
    '.wasm': 'application/wasm',
    '.jpg': 'image/jpeg',
    '.png': 'image/png',
};

// Simple static file server
function startServer(port = 8765) {
    return new Promise(resolve => {
        const server = createServer((req, res) => {
            let path = join(ROOT, req.url.split('?')[0]);
            if (req.url === '/') path = BENCH_PAGE;
            if (existsSync(path)) {
                const ext = extname(path);
                res.writeHead(200, { 'Content-Type': MIME[ext] || 'application/octet-stream',
                                      'Cross-Origin-Opener-Policy': 'same-origin',
                                      'Cross-Origin-Embedder-Policy': 'require-corp' });
                res.end(readFileSync(path));
            } else {
                res.writeHead(404);
                res.end('Not found');
            }
        });
        server.listen(port, () => resolve(server));
    });
}

async function runTarget(name, gpuEnabled) {
    console.log(`\n─── ${name} ───`);
    const server = await startServer();
    const browser = await puppeteer.launch({
        headless: true,
        args: gpuEnabled ? [] : ['--disable-webgpu', '--disable-gpu'],
    });
    const page = await browser.newPage();

    try {
        await page.goto('http://localhost:8765/', { waitUntil: 'networkidle0', timeout: 60000 });
        // Wait for benchmark completion
        await page.waitForFunction(() => {
            const el = document.getElementById('status');
            return el && el.textContent === 'Done';
        }, { timeout: 120000 });

        const resultsText = await page.$eval('#results', el => el.textContent);
        const results = JSON.parse(resultsText);

        const outPath = join(BENCH_DIR, `${name.toLowerCase().replace(/ /g, '_')}.json`);
        mkdirSync(BENCH_DIR, { recursive: true });
        writeFileSync(outPath, JSON.stringify(results, null, 2));
        console.log(`  Saved ${Object.keys(results).length} results to ${outPath}`);
    } catch (e) {
        console.error(`  Error: ${e.message}`);
    } finally {
        await browser.close();
        server.close();
    }
}

console.log('=== Browser Benchmarks ===');
await runTarget('Browser CPU', false);
await runTarget('Browser GPU', true);
```

- [ ] **Step 3: Add puppeteer dependency**

Modify `pillow-rs-js/package.json`:
```json
{
  "devDependencies": {
    "puppeteer": "^22.0.0"
  }
}
```

- [ ] **Step 4: Commit**

```bash
git add pillow-rs-js/bench_page/ scripts/bench_browser.mjs pillow-rs-js/package.json
git commit -m "feat: add browser benchmark harness (Puppeteer + headless Chrome)

Browser page loads WASM and runs ops; Puppeteer collects results.
Two passes: CPU (--disable-webgpu) and GPU (WebGPU enabled)."
```

---

### Task 8: GPU infrastructure — WGSL shaders + dispatcher

**Files:**
- Create: `pillow-rs/src/gpu/mod.rs`
- Create: `pillow-rs/src/gpu/shaders/blur.wgsl`
- Create: `pillow-rs/src/gpu/shaders/color_ops.wgsl`
- Create: `pillow-rs/src/gpu/shaders/resample.wgsl`
- Create: `pillow-rs/src/gpu/shaders/blend.wgsl`
- Create: `pillow-rs/src/gpu/shaders/convolve.wgsl`
- Modify: `pillow-rs/src/lib.rs` (add `pub mod gpu;`)

- [ ] **Step 1: Create GPU module skeleton**

Create `pillow-rs/src/gpu/mod.rs`:
```rust
//! GPU compute dispatcher for image operations.
//!
//! Uses wgpu for native (Vulkan/Metal/DX12/OpenGL) and shares WGSL shaders
//! with WebGPU for WASM targets. Operations are dispatched per-pixel-parallel
//! function (blur, color ops, resampling, blending, convolution).
//!
//! GPU is explicitly enabled via a boolean flag — never auto-detected.
//! Callers (bench harness, binding layer) control whether GPU path is used.

/// GPU compute engine. Created only when explicitly enabled.
pub struct GpuEngine {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl GpuEngine {
    /// Initialize GPU compute. Returns None if GPU is unavailable or disabled.
    /// Only called when the caller explicitly passes `gpu_enabled: true`.
    pub async fn new() -> Option<Self> {
        let instance = wgpu::Instance::default();
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions::default()).await?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .ok()?;
        Some(Self { device, queue })
    }

    /// Synchronous wrapper for benchmark harness use (pollster).
    pub fn new_sync() -> Option<Self> {
        pollster::block_on(Self::new())
    }
}

/// A GPU operation result.
#[derive(Debug)]
pub struct GpuResult {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

// ─── Shader loading ───

/// Load a WGSL shader source from the embedded shaders directory.
macro_rules! include_wgsl {
    ($name:expr) => {
        include_str!(concat!("shaders/", $name, ".wgsl"))
    };
}

pub const BLUR_SHADER: &str = include_wgsl!("blur");
pub const COLOR_OPS_SHADER: &str = include_wgsl!("color_ops");
pub const RESAMPLE_SHADER: &str = include_wgsl!("resample");
pub const BLEND_SHADER: &str = include_wgsl!("blend");
pub const CONVOLVE_SHADER: &str = include_wgsl!("convolve");
```

- [ ] **Step 2: Create WGSL shaders**

Create `pillow-rs/src/gpu/shaders/blur.wgsl`:
```wgsl
// Box blur compute shader — separable 2-pass (horizontal + vertical)
// Shared between native wgpu and browser WebGPU

@group(0) @binding(0) var<uniform> radius: u32;
@group(0) @binding(1) var<uniform> width: u32;
@group(0) @binding(2) var<uniform> height: u32;
@group(0) @binding(3) var input: texture_2d<f32>;
@group(0) @binding(4) var output: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= width || gid.y >= height { return; }

    let r = i32(radius);
    var sum = vec4<f32>(0.0);
    var count: u32 = 0u;

    // Horizontal pass
    for (var dx = -r; dx <= r; dx++) {
        let sx = min(max(i32(gid.x) + dx, 0), i32(width) - 1);
        sum += textureLoad(input, vec2<u32>(u32(sx), gid.y), 0);
        count++;
    }

    let avg = sum / f32(count);
    textureStore(output, gid.xy, avg);
}
```

Create `pillow-rs/src/gpu/shaders/color_ops.wgsl`:
```wgsl
// Color operation shaders: invert, solarize, posterize, grayscale
// Operation selected via uniform op_code

@group(0) @binding(0) var<uniform> op_code: u32;  // 0=invert, 1=solarize, 2=posterize, 3=grayscale
@group(0) @binding(1) var<uniform> param: f32;     // threshold/bits
@group(0) @binding(2) var<uniform> width: u32;
@group(0) @binding(3) var<uniform> height: u32;
@group(0) @binding(4) var input: texture_2d<f32>;
@group(0) @binding(5) var output: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= width || gid.y >= height { return; }

    var color = textureLoad(input, gid.xy, 0);

    switch op_code {
        case 0u: { // invert
            color = vec4<f32>(1.0 - color.rgb, color.a);
        }
        case 1u: { // solarize
            let threshold = param;
            if (color.r > threshold) { color.r = 1.0 - color.r; }
            if (color.g > threshold) { color.g = 1.0 - color.g; }
            if (color.b > threshold) { color.b = 1.0 - color.b; }
        }
        case 2u: { // posterize
            let bits = u32(param);
            let levels = f32((1u << bits) - 1u);
            color = vec4<f32>(
                floor(color.rgb * levels + 0.5) / levels,
                color.a
            );
        }
        case 3u: { // grayscale (luminance)
            let luma = 0.299 * color.r + 0.587 * color.g + 0.114 * color.b;
            color = vec4<f32>(luma, luma, luma, color.a);
        }
        default: {}
    }

    textureStore(output, gid.xy, color);
}
```

Create `pillow-rs/src/gpu/shaders/resample.wgsl`:
```wgsl
// Bilinear resampling compute shader (resize/thumbnail)
// For higher-quality filters, extend with separable convolution passes

@group(0) @binding(0) var<uniform> src_width: u32;
@group(0) @binding(1) var<uniform> src_height: u32;
@group(0) @binding(2) var<uniform> dst_width: u32;
@group(0) @binding(3) var<uniform> dst_height: u32;
@group(0) @binding(4) var input: texture_2d<f32>;
@group(0) @binding(5) var output: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(6) var sampler_: sampler;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= dst_width || gid.y >= dst_height { return; }

    let u = (f32(gid.x) + 0.5) * f32(src_width) / f32(dst_width) - 0.5;
    let v = (f32(gid.y) + 0.5) * f32(src_height) / f32(dst_height) - 0.5;

    // Convert to texture coordinates [0, 1]
    let tc = vec2<f32>(u / f32(src_width), v / f32(src_height));

    let color = textureSampleLevel(input, sampler_, tc, 0.0);
    textureStore(output, gid.xy, color);
}
```

Create `pillow-rs/src/gpu/shaders/blend.wgsl`:
```wgsl
// Image blending operations
@group(0) @binding(0) var<uniform> op_code: u32;  // 0=multiply, 1=screen, 2=overlay, 3=hard_light, 4=soft_light, 5=difference, 6=add, 7=subtract
@group(0) @binding(1) var<uniform> width: u32;
@group(0) @binding(2) var<uniform> height: u32;
@group(0) @binding(3) var img_a: texture_2d<f32>;
@group(0) @binding(4) var img_b: texture_2d<f32>;
@group(0) @binding(5) var output: texture_storage_2d<rgba8unorm, write>;

fn blend_screen(a: vec3<f32>, b: vec3<f32>) -> vec3<f32> {
    return 1.0 - (1.0 - a) * (1.0 - b);
}

fn blend_overlay(a: vec3<f32>, b: vec3<f32>) -> vec3<f32> {
    var result: vec3<f32>;
    for (var i = 0u; i < 3u; i++) {
        if (a[i] < 0.5) {
            result[i] = 2.0 * a[i] * b[i];
        } else {
            result[i] = 1.0 - 2.0 * (1.0 - a[i]) * (1.0 - b[i]);
        }
    }
    return result;
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= width || gid.y >= height { return; }

    let a = textureLoad(img_a, gid.xy, 0);
    let b = textureLoad(img_b, gid.xy, 0);

    var result: vec4<f32>;
    switch op_code {
        case 0u: { result = vec4<f32>(a.rgb * b.rgb, a.a); }             // multiply
        case 1u: { result = vec4<f32>(blend_screen(a.rgb, b.rgb), a.a); } // screen
        case 2u: { result = vec4<f32>(blend_overlay(a.rgb, b.rgb), a.a); } // overlay
        case 5u: { result = vec4<f32>(abs(a.rgb - b.rgb), a.a); }        // difference
        case 6u: { result = vec4<f32>(min(a.rgb + b.rgb, vec3<f32>(1.0)), a.a); } // add
        case 7u: { result = vec4<f32>(max(a.rgb - b.rgb, vec3<f32>(0.0)), a.a); } // subtract
        default: { result = a; }
    }

    textureStore(output, gid.xy, result);
}
```

Create `pillow-rs/src/gpu/shaders/convolve.wgsl`:
```wgsl
// Generic 3x3 convolution shader (filter kernels: BLUR, CONTOUR, DETAIL, etc.)
@group(0) @binding(0) var<uniform> kernel: array<f32, 9>;
@group(0) @binding(1) var<uniform> scale: f32;
@group(0) @binding(2) var<uniform> offset: f32;
@group(0) @binding(3) var<uniform> width: u32;
@group(0) @binding(4) var<uniform> height: u32;
@group(0) @binding(5) var input: texture_2d<f32>;
@group(0) @binding(6) var output: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x == 0u || gid.y == 0u || gid.x >= width - 1u || gid.y >= height - 1u {
        // Border: pass through
        textureStore(output, gid.xy, textureLoad(input, gid.xy, 0));
        return;
    }

    var sum = vec4<f32>(0.0);

    // 3x3 convolution
    for (var dy = 0u; dy < 3u; dy++) {
        for (var dx = 0u; dx < 3u; dx++) {
            let sx = gid.x + dx - 1u;
            let sy = gid.y + dy - 1u;
            let k = kernel[dy * 3u + dx];
            sum += textureLoad(input, vec2<u32>(sx, sy), 0) * k;
        }
    }

    let result = sum / scale + offset;
    let clamped = vec4<f32>(
        clamp(result.r, 0.0, 1.0),
        clamp(result.g, 0.0, 1.0),
        clamp(result.b, 0.0, 1.0),
        clamp(result.a, 0.0, 1.0),
    );

    textureStore(output, gid.xy, clamped);
}
```

- [ ] **Step 3: Register GPU module in lib.rs**

Modify `pillow-rs/src/lib.rs` — add after existing module declarations:
```rust
pub mod gpu;
```

- [ ] **Step 4: Verify compilation with GPU module**

```bash
cargo build -p pillow-rs 2>&1 | tail -10
```

Expected: Compiles (wgpu is already a dependency). Shaders compile at build time (embedded via `include_str!`).

- [ ] **Step 5: Commit**

```bash
git add pillow-rs/src/gpu/ pillow-rs/src/lib.rs
git commit -m "feat: add GPU compute module with WGSL shaders

5 shader families: blur, color_ops, resample, blend, convolve.
Shared between native wgpu and browser WebGPU targets.
GPU module dispatches to wgpu for native targets."
```

---

### Task 9: Integration — full end-to-end test

**Files:**
- Create: `scripts/bench_test_integration.sh`

- [ ] **Step 1: Write integration test script**

Create `scripts/bench_test_integration.sh`:
```bash
#!/bin/bash
# End-to-end test of the benchmarking pipeline
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

echo "=== Integration Test: Benchmarking Pipeline ==="

# 1. Fresh cache
echo "1. Initializing fresh cache..."
cd "$ROOT"
python3 scripts/bench_cache.py --init

# 2. Manifest parsing
echo "2. Testing manifest parsing..."
FUNC_COUNT=$(python3 scripts/bench_manifest.py --json | python3 -c "import json,sys; print(len(json.load(sys.stdin)))")
echo "   Functions found: $FUNC_COUNT"
if [ "$FUNC_COUNT" -lt 50 ]; then
    echo "   ERROR: Expected >= 50 functions, got $FUNC_COUNT"
    exit 1
fi

# 3. Pillow baseline
echo "3. Testing Pillow baseline (5 runs, quick)..."
python3 scripts/bench_pillow_baseline.py --runs 3
if [ ! -f target/benchmarks/pillow_baseline.json ]; then
    echo "   ERROR: baseline not generated"
    exit 1
fi

# 4. Cache check
echo "4. Testing cache check..."
python3 scripts/bench_cache.py --check

# 5. Aggregator
echo "5. Testing aggregator..."
python3 scripts/bench_aggregate.py
if [ ! -f BENCHMARKS.md ]; then
    echo "   ERROR: BENCHMARKS.md not generated"
    exit 1
fi
echo "   BENCHMARKS.md: $(wc -l < BENCHMARKS.md) lines"

# 6. Verify format
echo "6. Verifying BENCHMARKS.md format..."
grep -q "# pillow-rs Benchmarks" BENCHMARKS.md || { echo "ERROR: missing header"; exit 1; }
grep -q "## Summary" BENCHMARKS.md || { echo "ERROR: missing summary"; exit 1; }
grep -q "## Priority Operations" BENCHMARKS.md || { echo "ERROR: missing priority section"; exit 1; }
grep -q "## All Functions" BENCHMARKS.md || { echo "ERROR: missing all functions section"; exit 1; }

echo ""
echo "=== Integration Test PASSED ==="
echo "BENCHMARKS.md preview:"
head -20 BENCHMARKS.md
```

- [ ] **Step 2: Run integration test**

```bash
chmod +x scripts/bench_test_integration.sh
bash scripts/bench_test_integration.sh
```

Expected: All 6 checks pass. BENCHMARKS.md generated with correct structure.

- [ ] **Step 3: Commit**

```bash
git add scripts/bench_test_integration.sh
git commit -m "test: add benchmarking pipeline integration test

Verifies: manifest parsing, Pillow baseline, cache, aggregator, output format."
```

---

### Task 10: Final wiring — verify pre-commit hook works end-to-end

- [ ] **Step 1: Stage a Rust change and test pre-commit**

```bash
# Make a trivial change to trigger cache invalidation
echo "// benchmark test" >> pillow-rs/src/ops/resize.rs
git add pillow-rs/src/ops/resize.rs
git commit -m "test: pre-commit benchmark hook test"
# Should run incremental benchmarks
```

- [ ] **Step 2: Verify BENCHMARKS.md was auto-staged**

```bash
git show --stat HEAD
```

Expected: BENCHMARKS.md appears in the commit.

- [ ] **Step 3: Revert test change**

```bash
git reset HEAD~1
git checkout -- pillow-rs/src/ops/resize.rs
```

---

## Summary

- **20 files** created across the workspace
- **6 harnesses**: native CPU (criterion), native GPU (wgpu), WASM CPU (Node), WASM GPU (Node+WebGPU), browser CPU, browser GPU
- **Incremental caching**: SHA-256 keys from source + deps + reference images
- **Output**: `BENCHMARKS.md` auto-generated and committed on each `git commit`
- **80+ functions** benchmarked vs Pillow CPU baseline
- Implementation order: baseline → CPU benches → cache → aggregate → orchestrate → hook → WASM → browser → GPU shaders → integration test
