# pillow-rs

A full-featured reimplementation of [Pillow](https://python-pillow.org/) in Rust, targeting **Python** (via PyO3) and **WebAssembly/JavaScript** (via wasm-bindgen). Drop-in replacement: `from RSPIL import Image` works identically to `from PIL import Image`.

## Architecture

```
pillow-rs-core/   ← Pure Rust image library (ZERO binding dependencies)
pillow-rs-py/     ← PyO3 bindings (thin wrapper, ~200 lines)
pillow-rs-js/     ← wasm-bindgen (thin wrapper, ~200 lines)
```

## Quick Start

### Prerequisites

- Rust 1.75+
- Python 3.9+ with Pillow installed
- Node.js 20+ (for WASM benchmarks)
- Optional: `numpy` (`pip install numpy`) for ImageModule.fromarray baseline

```bash
# Install system dependencies (Ubuntu/Debian)
sudo apt-get install -y fonts-dejavu-core

# Install Python dependencies
pip install pillow numpy pyyaml pytest
```

### Build

```bash
# Python (native)
cd pillow-rs-py && maturin develop --release

# WASM (browser/Node.js)
cd pillow-rs-js && wasm-pack build --target web

# Core tests
cargo test -p pillow-rs-core
```

### Run tests

```bash
# Run all PIL parity tests
python -m pytest tests/ --json-report --json-report-file=/tmp/report.json

# Compute coverage
python scripts/compute_coverage.py manifest.yaml /tmp/report.json

# Run Rust tests (includes 29 GPU validation tests)
cargo test -p pillow-rs-core
```

## Benchmarking

### Single command

```bash
bash scripts/bench_all.sh full
```

This runs **all benchmarks** (native CPU, WASM CPU, pipeline) and regenerates `BENCHMARKS.md`.

### What gets benchmarked

| Target | Harness | Description |
|--------|---------|-------------|
| CPU | `cargo bench` (criterion) | Native Rust with rayon multicore |
| WASM CPU | Node.js + `performance.now()` | WASM in Node.js runtime |
| Browser CPU | Puppeteer + headless Chrome | WASM in browser (WebGPU disabled) |
| Browser GPU | Puppeteer + headless Chrome | WASM in browser (WebGPU enabled) |
| GPU | wgpu compute shaders | Native GPU via Vulkan/Metal/DX12 |
| WASM GPU | Node.js + experimental WebGPU | WASM with WebGPU compute |

GPU targets are flagged as **NYW** (not yet wired) — shaders exist but dispatch methods are stubs.

### Incremental benchmarking

Only changed functions are re-benchmarked (SHA-256 cache keys from source files):

```bash
bash scripts/bench_all.sh incremental   # only changed code
bash scripts/bench_all.sh --group priority  # just Tier 1 ops
bash scripts/bench_all.sh --only resize,crop # specific functions
```

### Output format

`BENCHMARKS.md` shows speedup ratios vs Pillow CPU (>1.0× = pillow-rs faster):

- **Pipeline**: 20-op chained pipeline with ST/MT comparison
- **Priority**: 12 most-used operations (puhu parity)
- **All Functions**: 166 functions grouped by module (Image, ImageFilter, ImageChops, etc.)

`—` = no data yet, `NYW` = GPU not yet wired, `⚠️` = possible outlier

### Key benchmarks (latest)

| Metric | Value |
|--------|-------|
| Functions benchmarked | 166 |
| Pillow baselines | 165 |
| Native CPU speedups | 47 |
| Average CPU speedup vs Pillow | 3.36× |
| Pipeline (20 ops, ST) | 290ms |
| Pipeline (20 ops, MT) | 154ms (1.88× MT speedup) |
| Pipeline (Pillow) | 59ms |

## Performance

### Rayon parallelization

pixel-parallel operations use [rayon](https://crates.io/crates/rayon) on native targets (CPU multicore). WASM targets use sequential fallback.

| Operation | Before | After | Improvement |
|-----------|--------|-------|-------------|
| Filter (3×3 convolution) | 153ms | 77ms | **49%** |
| Chops (channel ops) | 128ms | 94ms | **27%** |
| Enhance (color saturation) | 79ms | 73ms | **8%** |
| Pipeline (MT) | 220ms | 154ms | **30%** |

### GPU path (future)

GPU compute via wgpu/WebGPU with shared WGSL shaders. Shaders exist in `pillow-rs-core/src/gpu/shaders/`. Dispatch methods are stubs returning descriptive errors. Callers select GPU path via flag:

```rust
use pillow_rs_core::gpu::GpuEngine;
let engine = GpuEngine::new_sync(); // None if no GPU
let result = engine.map_or_else(
    || img.filter("BLUR"),       // CPU fallback
    |e| e.blur(&img, 5),         // GPU path
);
```

## Project Structure

```
.
├── manifest.yaml              ← Single source of truth (API surface)
├── BENCHMARKS.md              ← Auto-generated benchmark report
├── pillow-rs-core/
│   ├── src/
│   │   ├── image.rs           ← Image struct
│   │   ├── lazy.rs            ← LazyImage (deferred decode)
│   │   ├── ops/               ← Operation modules
│   │   │   ├── filter.rs      ← Convolution (rayon)
│   │   │   ├── chops.rs       ← Channel ops (rayon)
│   │   │   ├── imageops.rs    ← PIL.ImageOps (rayon)
│   │   │   ├── enhance.rs     ← ImageEnhance (rayon)
│   │   │   ├── crop.rs        ← Puhu raw-byte memcpy pattern
│   │   │   ├── resize.rs      ← Delegates to image crate
│   │   │   └── ...
│   │   └── gpu/
│   │       ├── mod.rs         ← GpuEngine (flag-controlled)
│   │       └── shaders/       ← 5 WGSL shader families
│   └── benches/
│       ├── native_cpu.rs      ← Criterion benchmarks (50 functions)
│       └── bench_utils.rs     ← Shared helpers
├── pillow-rs-py/              ← PyO3 bindings
├── pillow-rs-js/              ← wasm-bindgen
│   └── bench_page/            ← Browser benchmark page + shared harness
├── scripts/
│   ├── bench_all.sh           ← **Single benchmark command**
│   ├── bench_spec.py          ← 43-function benchmark spec (groups)
│   ├── bench_manifest.py      ← Manifest parser → function list
│   ├── bench_pillow_baseline.py ← Pillow reference timings (165 ops)
│   ├── bench_aggregate.py     ← JSONs → BENCHMARKS.md
│   ├── bench_cache.py         ← SHA-256 incremental cache
│   ├── bench_wasm_cpu.mjs     ← Node.js WASM harness
│   ├── bench_browser.mjs      ← Puppeteer browser driver
│   └── bench_reference_images/ ← Synthetic benchmark images
└── tests/                     ← PIL parity test suite
```

## Development

### Manifest-driven development

All work starts from `manifest.yaml`. To add a function:
1. Add entry to `manifest.yaml`
2. Run `scripts/generate_stubs.py`
3. Implement in `pillow-rs-core/src/ops/`
4. Add binding delegation in `pillow-rs-py/src/lib.rs`
5. Write PIL parity tests in `tests/`
6. Add test→function mapping in `scripts/coverage_map.json`

### Code quality

```bash
# Lint
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check

# Before commit
bash scripts/bench_all.sh full  # Regenerate BENCHMARKS.md
python -m pytest tests/         # Verify PIL parity
```

### Naming rules

- Public API names match Pillow exactly (`Image.open()`, `Image.resize()`, etc.)
- Import name is `RSPIL` (mirrors `PIL`)
- Internal Rust types use any naming convention
- GPU is explicitly flag-controlled — never auto-detected

### Key patterns (from puhu reference)

- `par_chunks()` / `par_chunks_mut()` for pixel-parallel ops on native targets
- `#[cfg(not(target_arch = "wasm32"))]` guard for rayon
- Raw byte memcpy (`extend_from_slice`) for crop
- `Vec::set_len()` pre-allocation for channel splitting
- `as_rgb8()` / `as_rgba8()` fast-path dispatch to avoid DynamicImage overhead
