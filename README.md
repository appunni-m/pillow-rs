# pillow-rs

<div align="center">

**A fast, full-featured reimplementation of [Pillow](https://python-pillow.org/) in Rust — deployable to Python, JavaScript, and the browser.**

[![CI](https://img.shields.io/badge/ci-passing-brightgreen?style=flat-square)](#)
[![PyPI](https://img.shields.io/badge/pypi-pillow--rs-blue?style=flat-square)](https://pypi.org/project/pillow-rs/)
[![npm](https://img.shields.io/badge/npm-@pillow--rs/wasm-red?style=flat-square)](https://www.npmjs.com/package/@pillow-rs/wasm)
[![Crates.io](https://img.shields.io/crates/v/pillow-rs-core?style=flat-square)](https://crates.io/crates/pillow-rs-core)
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

Requires Python **3.8+**. Pre-built wheels available for Linux, macOS, and Windows on x86_64 and arm64.

**Zero required dependencies.** `Image.fromarray()` works with numpy arrays (and any object with `.tobytes()` or `.__array_interface__`) via duck-typing — no numpy install needed.

### JavaScript / WASM

```bash
npm install @pillow-rs/wasm
# or
yarn add @pillow-rs/wasm
```

Works in **Node.js 20+** and all modern browsers with WebAssembly support.

> **Browser usage:** The WASM binary loads asynchronously. Always `await Image.open()` or use `Image.load()`.

### From Source (development)

```bash
# Clone
git clone https://github.com/pillow-rs/pillow-rs
cd pillow-rs

# Install system deps (Ubuntu/Debian)
sudo apt-get install -y fonts-dejavu-core

# Python
cd pillow-rs-py && maturin develop --release

# JavaScript / WASM
cd pillow-rs-js && wasm-pack build --target web
```

---

## Features

- **PIL API parity** — `Image.open()`, `.resize()`, `.filter()`, `.crop()`, `.rotate()`, `.convert()`, `.paste()`, `.save()`, and 160+ more
- **10 color modes** — `1`, `L`, `LA`, `P`, `RGB`, `RGBA`, `CMYK`, `YCbCr`, `HSV`, `I`, `F`
- **7 image formats** — PNG, JPEG, GIF, BMP, TIFF, WEBP, ICO
- **Per-mode native pixel drawing** — draws directly in the image's native color format, never lossy RGBA round-trips
- **Multicore acceleration** — pixel-parallel operations use [rayon](https://crates.io/crates/rayon) on native targets
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
| Pipeline (20 ops, single-threaded) | 189ms |
| Rayon multicore improvement | up to **49% faster** on filter ops |

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

### Rayon multicore acceleration

Pixel-parallel operations use [rayon](https://crates.io/crates/rayon) on native targets. WASM falls back to sequential.

| Operation | Before | After | Improvement |
|-----------|--------|-------|-------------|
| Filter (3×3 convolution) | 153ms | 77ms | **49%** |
| Chops (channel ops) | 128ms | 94ms | **27%** |
| Enhance (color) | 79ms | 73ms | **8%** |
| Pipeline (MT, 20 ops) | 220ms | 154ms | **30%** |

### GPU path (experimental)

GPU compute via **wgpu / WebGPU** with shared WGSL shaders. Shaders exist for 42 functions in `pillow-rs-core/src/gpu/shaders/`. Dispatch methods currently return descriptive errors — GPU is not yet wired.

```rust
use pillow_rs_core::gpu::GpuEngine;
let engine = GpuEngine::new_sync(); // None if no GPU
let result = engine.map_or_else(
    || img.filter("BLUR"),       // CPU fallback
    |e| e.blur(&img, 5),         // GPU path (NYW)
);
```

---

## Architecture

```
pillow-rs-core/     Pure Rust image library — ZERO binding dependencies
pillow-rs-py/       PyO3 bindings — thin wrapper, ~200 lines
pillow-rs-js/       wasm-bindgen — thin wrapper, ~200 lines
```

**Iron rule:** Core never touches Python objects, JS objects, file paths, or network. All I/O and type conversion live in the binding crates. The binding layers contain **no control flow** — no loops, no list comprehensions, no arithmetic. Every decision happens in Rust.

---

## Project Structure

```
.
├── manifest.yaml              API surface definition (single source of truth)
├── BENCHMARKS.md              Auto-generated full benchmark report
├── pillow-rs-core/
│   ├── src/
│   │   ├── image.rs           Image struct, pixel access, mode handling
│   │   ├── lazy.rs            LazyImage (deferred decode)
│   │   ├── ops/               Operation modules
│   │   │   ├── filter.rs      Convolution filters (rayon)
│   │   │   ├── chops.rs       Channel operations (rayon)
│   │   │   ├── imageops.rs    PIL.ImageOps (rayon)
│   │   │   ├── enhance.rs     ImageEnhance (rayon)
│   │   │   ├── resize.rs      Resize, thumbnail, reduce
│   │   │   ├── crop.rs        Crop, copy, paste
│   │   │   ├── convert.rs     Mode/format conversion
│   │   │   ├── draw/          ImageDraw (per-mode native paths)
│   │   │   └── ...
│   │   └── gpu/
│   │       ├── mod.rs         GpuEngine (flag-controlled)
│   │       └── shaders/       WGSL compute shaders (5 families)
│   └── benches/
│       ├── native_cpu.rs      Criterion benchmarks (50+ functions)
│       └── bench_utils.rs     Shared benchmarking helpers
├── pillow-rs-py/              PyO3 bindings
│   └── python/pillow_rs/      Pure delegation Python wrappers
├── pillow-rs-js/              wasm-bindgen
│   └── bench_page/            Browser benchmark page + harness
├── scripts/
│   ├── bench/                 Benchmark orchestration (bench_all.sh)
│   ├── coverage/              Coverage computation & validation
│   └── generate_stubs.py      Manifest → Rust stub generator
└── tests/                     PIL parity test suite (1,555 tests)
```

---

## Development

### Manifest-driven workflow

All work starts from `manifest.yaml` — the single source of truth for the API surface. To add a function:

1. Define signature, modes, and variants in `manifest.yaml`
2. Run `python scripts/generate_stubs.py` to scaffold Rust stubs
3. Implement in `pillow-rs-core/src/ops/<module>.rs`
4. Add binding delegation in `pillow-rs-py/src/lib.rs`
5. Add Python wrapper in `pillow-rs-py/python/pillow_rs/`
6. Write PIL parity tests in `tests/`
7. Add test→function mapping in `scripts/coverage/coverage_map.json`
8. Run `python -m pytest tests/ --json-report --json-report-file=/tmp/report.json`
9. Validate: `python scripts/coverage/compute_coverage.py manifest.yaml /tmp/report.json`

### Code quality

```bash
# Lint (CI gate)
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check

# Run all tests
python -m pytest tests/ --json-report --json-report-file=/tmp/report.json

# Run core tests (includes GPU validation tests)
cargo test -p pillow-rs-core

# Full CI pipeline
bash scripts/ci_coverage.sh
```

**Quality standards:**
- No `unwrap()` or `expect()` outside `#[cfg(test)]`
- `thiserror` for error types in core library
- `&str` over `String`, `&[T]` over `Vec<T>` in function parameters
- `#[derive(Debug)]` on all public types
- All `pub` functions have `///` doc comments with `# Examples`

### Running benchmarks

```bash
# Full benchmark suite (all 166 functions, 6 targets)
bash scripts/bench/bench_all.sh full

# Incremental (only changed code since last run)
bash scripts/bench/bench_all.sh incremental

# Priority tier only (12 most-used ops)
bash scripts/bench/bench_all.sh --group priority

# Specific functions
bash scripts/bench/bench_all.sh --only resize,crop
```

Output goes to `BENCHMARKS.md`. Uses SHA-256 cache keys so unchanged functions skip re-benchmarking.

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

## Documentation

| Resource | Description |
|----------|-------------|
| **[docs/COVERAGE.md](docs/COVERAGE.md)** | Full API coverage report (auto-generated) |
| **[docs/COVERAGE_WASM.md](docs/COVERAGE_WASM.md)** | WASM-specific coverage |
| **[BENCHMARKS.md](BENCHMARKS.md)** | Benchmark results across all 6 targets |
| **[manifest.yaml](manifest.yaml)** | Complete API surface definition |
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
