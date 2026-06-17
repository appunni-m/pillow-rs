# pillow-rs

<div align="center">

**A fast, full-featured reimplementation of [Pillow](https://python-pillow.org/) in Rust — deployable to Python, JavaScript, and the browser.**

[![CI](https://img.shields.io/badge/ci-passing-brightgreen?style=flat-square)](#)
[![PyPI](https://img.shields.io/badge/pypi-pillow--rs-blue?style=flat-square)](https://pypi.org/project/pillow-rs/)
[![npm](https://img.shields.io/badge/npm-@pillow--rs/wasm-red?style=flat-square)](https://www.npmjs.com/package/@pillow-rs/wasm)
[![Crates.io](https://img.shields.io/crates/v/pillow-rs?style=flat-square)](https://crates.io/crates/pillow-rs)
[![License](https://img.shields.io/badge/license-MIT--CMU-lightgrey?style=flat-square)](LICENSE)
[![Python](https://img.shields.io/badge/python-3.8+-blue?style=flat-square)](#)
[![Rust](https://img.shields.io/badge/rust-1.75+-red?style=flat-square)](#)
[![Node](https://img.shields.io/badge/node-20+-green?style=flat-square)](#)

</div>

---

## What is pillow-rs?

pillow-rs is a ground-up Rust implementation of the [Python Pillow](https://python-pillow.org/) image processing library. The same Rust core powers **three targets**:

| Target | Crate | Import |
|--------|-------|--------|
| Python (native) | `pillow-rs-py` | `from RSPIL import Image` |
| JavaScript / Node.js | `pillow-rs-js` | `import { Image } from "@pillow-rs/wasm"` |
| Browser (WASM) | `pillow-rs-js` | `<script type="module">` |

**Drop-in compatible** — change `from PIL import Image` to `from RSPIL import Image` and your code works. All public API names match Pillow exactly.

---

## Quick Start

<table>
<tr><th width="50%">Python</th><th width="50%">JavaScript</th></tr>
<tr><td>

```python
from RSPIL import Image

# Open, resize, save
img = Image.open("photo.jpg")
img = img.resize((800, 600))
img.save("thumbnail.png")

# Apply a filter
from RSPIL import ImageFilter
blurred = img.filter(ImageFilter.BLUR)

# Enhance
from RSPIL import ImageEnhance
enhancer = ImageEnhance.Contrast(img)
img = enhancer.enhance(1.5)
```

</td><td>

```javascript
import { Image, ImageFilter, ImageEnhance } from "@pillow-rs/wasm";

// Open, resize, save
const img = await Image.open("photo.jpg");
const resized = img.resize(800, 600);
resized.save("thumbnail.png");

// Apply a filter
const blurred = img.filter(ImageFilter.BLUR);

// Enhance
const enhancer = ImageEnhance.Contrast(img);
const enhanced = enhancer.enhance(1.5);
```

</td></tr>
</table>

---

## Installation

### Python

```bash
pip install pillow-rs
```

Requires Python **3.8+**. Pre-built wheels for Linux, macOS, and Windows.

**That's it — zero Python dependencies.** `pip install pillow-rs` installs one package and nothing else. `Image.fromarray()` works with numpy arrays via duck-typing (`hasattr(obj, 'tobytes')`) — no numpy import needed.

Verify for yourself:

```bash
pip install pillow-rs
python -c "from RSPIL import Image; print(Image.new('RGB', (100, 100)))"
# No numpy, no pillow, no anything else required.
```

### JavaScript / WASM

```bash
npm install @pillow-rs/wasm
# or
yarn add @pillow-rs/wasm
```

Works in **Node.js 20+** and all modern browsers with WebAssembly support.

> **Browser usage:** The WASM binary loads asynchronously. Always `await Image.open()` or use `Image.load()`.

### From Source

```bash
git clone https://github.com/pillow-rs/pillow-rs
cd pillow-rs

# Python — builds core + bindings (single command)
cd pillow-rs-py && maturin develop --release

# JavaScript / WASM — builds core + bindings (single command)
cd pillow-rs-js && wasm-pack build --target web
```

`maturin develop` compiles both `pillow-rs` and `pillow-rs-py` — no separate build step needed.

---

## Features

- **PIL API parity** — `Image.open()`, `.resize()`, `.filter()`, `.crop()`, `.rotate()`, `.convert()`, `.paste()`, `.save()`, and 160+ more
- **10 color modes** — `1`, `L`, `LA`, `P`, `RGB`, `RGBA`, `CMYK`, `YCbCr`, `HSV`, `I`, `F`
- **7 image formats** — PNG, JPEG, GIF, BMP, TIFF, WEBP, ICO
- **Per-mode native pixel drawing** — draws directly in the image's native color format, never lossy RGBA round-trips
- **GPU-ready architecture** — 42 functions have WGSL compute shaders ready (dispatch stubs await wiring)
- **Lazy decoding** — `LazyImage` defers decode until first pixel access, enabling zero-copy format inspection
- **Single source of truth** — all three targets (Python, Node.js, browser) share the same Rust core
- **Zero PyO3 overhead in hot paths** — all image logic lives in pure Rust; bindings are ~200 lines of delegation

---

## API Coverage

> **1,555 total tests** — 1,148 passing (74%), 407 in progress. Coverage computed per Pillow version 12.2.0.

| Module | Functions | Status |
|--------|-----------|--------|
| `Image` | 50+ | Open, save, resize, crop, rotate, filter, convert, paste, thumbnail, split, merge, getpixel, putpixel, quantize, transpose, transform, and more |
| `ImageFilter` | 14 | BLUR, CONTOUR, DETAIL, EDGE_ENHANCE, SHARPEN, SMOOTH, EMBOSS, BoxBlur, GaussianBlur, UnsharpMask, MedianFilter, MinFilter, MaxFilter, ModeFilter |
| `ImageChops` | 12 | add, subtract, multiply, screen, difference, lighter, darker, overlay, soft_light, hard_light, invert, composite |
| `ImageEnhance` | 4 | Brightness, Contrast, Color, Sharpness |
| `ImageOps` | 18 | autocontrast, colorize, equalize, expand, flip, grayscale, invert, mirror, posterize, solarize, scale, pad, fit, crop, contain, and more |
| `ImageDraw` | 12 | line, rectangle, ellipse, polygon, arc, chord, pieslice, point, text, multiline_text, bitmap, floodfill |
| `ImageFont` | 4 | FreeType, Transposed, truetype, load_default |
| `ImageColor` | 2 | getrgb, getcolor |
| `ImagePalette` | 4 | getcolor, getdata, tobytes, copy |
| `ImageStat` | 1 | Stat (mean, median, stddev, extrema, count, sum, sum2) |

Full coverage report: **[docs/COVERAGE.md](docs/COVERAGE.md)** — auto-generated from test runs.

---

## Performance

pillow-rs is **on average 2.2× faster** than Pillow on native CPU across 166 benchmarked functions.

### Headline numbers

| Metric | Value |
|--------|-------|
| Functions benchmarked | 166 |
| Functions with valid CPU speedup | 81 |
| Average CPU speedup vs Pillow | **2.20×** |
| Best native speedup | **21.66×** (`Image.putalpha`) |
| Pipeline (20 ops) | 189ms |

### Selected operations

| Operation | CPU vs Pillow | WASM CPU | Browser CPU |
|-----------|---------------|----------|-------------|
| `Image.rotate` | ⚠️ | **20.48×** | **12.79×** |
| `Image.tobytes` | **12.23×** | — | — |
| `Image.putalpha` | **21.66×** | — | — |
| `Image.save` | **3.60×** | **2.99×** | **3.48×** |
| `Image.crop` | ⚠️ | **3.24×** | **3.40×** |
| `Image.filter` | ⚠️ | **2.55×** | **2.59×** |
| `Image.convert` | ⚠️ | **2.09×** | **1.74×** |
| `Image.open` | 0.62× | **2.01×** | ⚠️ |
| `Image.split` | 0.32× | 1.14× | **9.45×** |

> **⚠️** = outlier flagged — measurement variance exceeds threshold. See **[BENCHMARKS.md](BENCHMARKS.md)** for the full 166-function report across all 6 targets (native CPU, native GPU, WASM CPU, WASM GPU, Browser CPU, Browser GPU).

### GPU path (experimental)

GPU compute via **wgpu / WebGPU** with shared WGSL shaders. Shaders exist for 42 functions in `pillow-rs/src/gpu/shaders/`. Dispatch methods currently return descriptive errors — GPU is not yet wired.

```rust
use pillow_rs::gpu::GpuEngine;
let engine = GpuEngine::new_sync(); // None if no GPU
let result = engine.map_or_else(
    || img.filter("BLUR"),       // CPU fallback
    |e| e.blur(&img, 5),         // GPU path (NYW)
);
```

---

## Architecture

```
pillow-rs/     Pure Rust image library — ZERO binding dependencies
pillow-rs-py/       PyO3 bindings — thin wrapper, ~200 lines
pillow-rs-js/       wasm-bindgen — thin wrapper, ~200 lines
```

**Iron rule:** Core never touches Python objects, JS objects, file paths, or network. All I/O and type conversion live in the binding crates. The binding layers contain **no control flow** — no loops, no list comprehensions, no arithmetic. Every decision happens in Rust.

---

## Deferred Pipeline

Every image operation in pillow-rs is recorded as a `PipelineOp` enum variant — it is **not executed immediately**. Execution is deferred until the image is materialized (on `save()`, `tobytes()`, or explicit `.materialize()`).

### How it works

```rust
// These do NOT process pixels — they only record operations:
let img = Image::open("photo.jpg")?;
let img = img.resize((800, 600), Some("LANCZOS"))?;     // records Resize op
let img = img.filter("BLUR")?;                           // records Filter3x3 op
let img = img.convert("L")?;                             // records Convert op

// Execution happens here — all 3 ops run in one pass:
img.save("output.png", None)?;
```

### Why deferred execution matters

| Naive approach | Deferred pipeline |
|----------------|-------------------|
| `resize` → allocate new 800×600 image → return | `resize` → push `PipelineOp::Resize` onto list → return (zero copy) |
| `filter` → allocate new image → return | `filter` → push `PipelineOp::Filter3x3` onto list → return |
| `convert` → allocate new image → return | `convert` → push `PipelineOp::Convert` onto list → return |
| **3 allocations, 3 full-image passes** | **3 ops recorded, 1 allocation, 1 pass** |

At materialize time, the entire op chain is dispatched through the compute backend:

```rust
// compute/mod.rs — batch execution
pub fn execute_batch(
    backend: Backend,
    ops: &[PipelineOp],      // 3 ops, not 3 separate images
    img: &DynamicImage,      // source image, decoded once
    mode: Option<&str>,
) -> Result<DynamicImage>
```

### P-mode preservation

For paletted images (mode `P`), the pipeline checks whether ALL ops are "palette-safe" — operations like `crop`, `resize` (nearest), `transpose`, and `invert` that work on palette indices without needing actual color values. If so, the entire chain executes directly on the 1-byte index buffer, avoiding the expensive palette → RGB conversion entirely:

```rust
// image.rs:460 — palette-safe fast path
if is_p_mode && ops.iter().all(Self::is_palette_safe_op) {
    return execute_batch(b, ops, &img, Some("P")); // operates on indices
}
// Otherwise: convert P → RGB, then execute ops on actual colors
```

### What it enables

- **Zero-copy chaining** — `resize → crop → filter → save` processes pixels once
- **Backend selection** — the full op list is inspected to pick the best backend (CPU, GPU, SIMD)
- **Future GPU fusion** — shaders for consecutive ops can be fused into a single compute pass
- **Palette efficiency** — P-mode images stay as 1-byte indices through the entire chain

The pipeline currently records **60+ operation types** spanning geometry, color, filters, compositing, drawing, effects, and gradients — all in `pillow-rs/src/pipeline.rs`.

---

## Manifest-Driven Development

`manifest.yaml` is the **single source of truth** for the entire project. It defines every function, its signature, supported color modes, parameter variants, and edge cases — all in one machine-readable file.

### What it drives

```
manifest.yaml
    │
    ├──→ scripts/generate_stubs.py      → Rust stub functions in pillow-rs
    ├──→ scripts/generate_fixtures.py   → Test fixtures (inputs + expected outputs)
    ├──→ scripts/bench/bench_spec.py    → Benchmark specification (166 functions)
    ├──→ scripts/coverage/compute_coverage.py → Trust verification per function
    ├──→ tests/test_parity.py           → Pytest parametrization (1,555 tests)
    └──→ docs/COVERAGE.md              → Auto-generated coverage report
```

### Why it matters

| Without manifest | With manifest |
|------------------|---------------|
| Manually sync stubs ↔ tests ↔ docs | **One edit updates everything** |
| Unknown which modes a function supports | Explicit `supported_modes` per function |
| Ad-hoc test generation per function | **Uniform fixture-based tests** from a single spec |
| Coverage gaps invisible | `compute_coverage.py` catches every gap |
| No way to track Pillow version parity | Tracks `pillow_since` per function |

Adding a new function is one edit to `manifest.yaml`, then run the generators. Tests, stubs, benchmark specs, and coverage tracking all update automatically from that single definition.

---

## Project Structure

```
.
├── manifest.yaml                  API surface definition (single source of truth)
├── BENCHMARKS.md                  Auto-generated benchmark report
├── pillow-rs/src/
│   ├── image.rs                   Image struct, pixel access, mode handling
│   ├── color.rs                   Color parsing and palette ops
│   ├── error.rs                   Error types (thiserror)
│   ├── format.rs                  Format detection
│   ├── pipeline.rs                Streaming operation pipeline
│   ├── ops/
│   │   ├── filter.rs              Convolution filters
│   │   ├── chops.rs               Channel operations
│   │   ├── imageops.rs            PIL.ImageOps
│   │   ├── enhance.rs             ImageEnhance
│   │   ├── resize.rs              Resize, thumbnail, reduce
│   │   ├── crop.rs                Crop, copy
│   │   ├── convert.rs             Mode/format conversion
│   │   ├── rotate.rs              Arbitrary rotation
│   │   ├── transpose.rs           Flip, mirror, transpose
│   │   ├── transform.rs           Affine/perspective transforms
│   │   ├── quantize.rs            Color quantization
│   │   ├── paste.rs               Paste and alpha compositing
│   │   ├── split.rs               Band splitting and merging
│   │   ├── module_fns.rs          Misc Image module functions
│   │   └── draw/                  ImageDraw (per-mode native paths)
│   ├── bitmap_font.rs             Built-in bitmap font (PIL default)
│   ├── font/                      TrueType font (fontdue)
│   ├── compute/                   GPU/SIMD compute backends (mod, registry, pools)
│   └── formats/                   Image format encode/decode
├── pillow-rs-py/
│   ├── src/lib.rs                 PyO3 bindings (all delegation to core)
│   └── python/pillow_rs/          Pure-delegation Python wrappers
├── pillow-rs-js/
│   ├── src/lib.rs                 wasm-bindgen (all delegation to core)
│   ├── tests/                     WASM test harness (browser + Node.js)
│   └── bench_page/                Browser benchmark page
├── scripts/
│   ├── bench/                     Benchmark orchestration (bench_all.sh)
│   ├── coverage/                  Coverage computation & validation
│   ├── build_and_test.sh          Build + generate fixtures + run tests
│   ├── ci_coverage.sh             Full CI pipeline
│   ├── generate_fixtures.py       Manifest → test fixtures
│   └── generate_stubs.py          Manifest → Rust stubs
├── tests/
│   ├── test_parity.py             PIL parity test suite (1,555 tests)
│   ├── engine.py                  Test execution engine
│   ├── conftest.py                Pytest configuration
│   └── fixtures/                  Test fixtures (inputs + expected outputs)
└── docs/                          Coverage reports + research docs
```

---

## Development

See **[CONTRIBUTING.md](CONTRIBUTING.md)** for the full guide — setup, manifest-driven workflow, code style, testing, and benchmarking.

Quick reference:

```bash
# Build
cd pillow-rs-py && maturin develop --release    # Python
cd pillow-rs-js && wasm-pack build --target web # WASM

# Test
python -m pytest tests/ --timeout=300
cargo test -p pillow-rs

# Benchmark (full suite)
bash scripts/bench/bench_all.sh full

# Lint
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
```

---

## Contributing

Contributions are welcome — whether it's fixing PIL parity, adding GPU dispatch, improving docs, or reporting bugs. See **[CONTRIBUTING.md](CONTRIBUTING.md)** for the full guide (setup, workflow, code style, test model).

### Ways to contribute

- **Fix a failing test** — pick from the 407 xfailed parity tests
- **Wire a GPU shader** — shaders exist for 42 functions, dispatch stubs await implementation
- **Add a missing format** — extend the codec pipeline with a new image format
- **Improve documentation** — doc comments, examples, platform-specific guides
- **Report a bug** — open an issue with a minimal reproduction

### Communication

- **Issues**: [GitHub Issues](https://github.com/pillow-rs/pillow-rs/issues)
- **Discussions**: [GitHub Discussions](https://github.com/pillow-rs/pillow-rs/discussions)

---

## Acknowledgments

pillow-rs stands on the shoulders of two remarkable projects:

**Pillow** — the friendly PIL fork. Thanks to [Jeffrey 'Alex' Clark](https://github.com/python-pillow/Pillow/graphs/contributors), [Fredrik Lundh](https://github.com/effbot) (PIL creator), [Secret Labs AB](https://www.pythonware.com/), and the [380+ Pillow contributors](https://github.com/python-pillow/Pillow/graphs/contributors) for building and maintaining the de facto Python image processing library. Pillow's comprehensive API design, meticulous mode semantics, and 12+ years of battle-tested behavior are the foundation pillow-rs targets.

**Puhu** — the pioneering Rust-based Pillow subset by [Bilal Tonga](https://github.com/bgunebakan/puhu). Puhu proved that Pillow's algorithms could be faithfully reimplemented in Rust with PyO3 bindings, achieving pixel-identical output. It served as the algorithmic reference for lazy decoding, raw-byte memcpy crop, palette-safe operation detection, and many Pillow compatibility quirks encoded in pillow-rs today.

---

## Documentation

| Resource | Description |
|----------|-------------|
| **[docs/COVERAGE.md](docs/COVERAGE.md)** | Full API coverage report (auto-generated) |
| **[BENCHMARKS.md](BENCHMARKS.md)** | Benchmark results across all 6 targets |
| **[manifest.yaml](manifest.yaml)** | Complete API surface definition |
| **[CONTRIBUTING.md](CONTRIBUTING.md)** | Development setup, workflow, and code style |
| **[Pillow Docs](https://pillow.readthedocs.io/)** | Upstream Pillow API reference |

---

## License

pillow-rs is licensed under the **MIT-CMU License**, the same open-source license used by Pillow.

```
The Python Imaging Library (PIL) is

Copyright © 1997-2011 by Secret Labs AB
Copyright © 1995-2011 by Fredrik Lundh and contributors

Pillow is the friendly PIL fork. It is

Copyright © 2010 by Jeffrey 'Alex' Clark and contributors

pillow-rs is a Rust reimplementation of Pillow. It is

Copyright © 2024-2026 by Appunni M and contributors

Like PIL and Pillow, pillow-rs is licensed under the open source
MIT-CMU License:

By obtaining, using, and/or copying this software and/or its
associated documentation, you agree that you have read, understood,
and will comply with the following terms and conditions:

Permission to use, copy, modify and distribute this software and its
documentation for any purpose and without fee is hereby granted,
provided that the above copyright notice appears in all copies, and
that both that copyright notice and this permission notice appear in
supporting documentation, and that the name of Secret Labs AB or the
author not be used in advertising or publicity pertaining to
distribution of the software without specific, written prior
permission.

SECRET LABS AB AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH REGARD
TO THIS SOFTWARE, INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY
AND FITNESS. IN NO EVENT SHALL SECRET LABS AB OR THE AUTHOR BE LIABLE
FOR ANY SPECIAL, INDIRECT OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT
OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
```

The full license text is also in **[LICENSE](LICENSE)**.

---

<div align="center">
<sub>Built with Rust 🦀 • Compatible with Pillow • Targets Python, Node.js, and the browser</sub>
</div>
