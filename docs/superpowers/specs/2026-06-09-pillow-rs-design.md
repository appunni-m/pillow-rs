# pillow-rs: Pillow Drop-In Replacement in Rust

**Status:** Spec  
**Date:** 2026-06-09  
**Import name:** `RSPIL` (mirrors Pillow's `PIL`)  
**Package names:** `pillow-rs` (PyPI), `pillow-rs` (npm)

## Overview

A full-featured reimplementation of the Pillow image processing library in Rust, targeting both Python (via PyO3) and the browser (via WASM + wasm-bindgen). The goal is a drop-in replacement: `from RSPIL import Image` works identically to `from PIL import Image`, and `import { Image } from 'pillow-rs'` mirrors the same API in JavaScript.

Reference project: [Puhu](https://github.com/bgunebakan/puhu) (Rust-based Pillow subset for Python, evaluated for algorithmic patterns but not architectural structure).

## Architecture

### Workspace Tri-Crate

```
pillow-rs/
├── pillow-rs-core/       # Pure Rust, zero binding dependencies
├── pillow-rs-py/         # PyO3 bindings → Python wheel
├── pillow-rs-js/         # wasm-bindgen → npm package
├── manifest.yaml          # API surface definition (single source of truth)
├── tests/                 # Single Python test suite
├── coverage/              # Auto-generated coverage reports
└── scripts/               # Stub generation, coverage computation
```

**Core rule:** `pillow-rs-core` operates on `DynamicImage` and Rust primitives only. It never touches Python objects, JS objects, file paths, or network. All I/O and type conversion lives in the binding crates.

### Component Tree

```
pillow-rs-core/src/
├── lib.rs               # Re-exports
├── image.rs             # RsImage struct (DynamicImage wrapper)
├── lazy.rs              # LazyImage — deferred load (adopted from Puhu)
├── error.rs             # PilError enum (thiserror)
├── color.rs             # Color parsing, mode enums
├── ops/
│   ├── resize.rs        # resize() with all Pillow resample filters
│   ├── crop.rs          # crop() with negative-coord clipping
│   ├── rotate.rs        # rotate() arbitrary angles + expand
│   ├── transpose.rs     # transpose() FLIP_LEFT_RIGHT, ROTATE_90, etc.
│   ├── convert.rs       # convert() RGB↔L↔RGBA↔1↔P with matrix
│   ├── paste.rs         # paste() with mask alpha blending
│   ├── split.rs         # split() into band images
│   ├── filter.rs        # ImageFilter operations
│   └── enhance.rs       # ImageEnhance (brightness, contrast, etc.)
├── formats/
│   ├── png.rs
│   ├── jpeg.rs
│   ├── gif.rs
│   ├── webp.rs
│   └── ...
├── draw/                # ImageDraw equivalent
├── font/                # ImageFont equivalent
└── effects/             # Additional Pillow modules
```

## Coverage Matrix

Coverage is measured across six axes, computed dynamically after each test run.

### The Six Axes

| Axis | Weight | Description |
|------|--------|-------------|
| Signature | 10% | Function exists with correct params, types, defaults |
| Parameter variants | 20% | Each meaningful combination of optional params |
| Modes | 35% | Works for each image mode (L, LA, RGB, RGBA, 1, P, CMYK) |
| Edge cases | 15% | Boundary conditions, error paths |
| Formats | 10% | Read/write support per format (I/O ops, 0 if N/A) |
| Behavioral parity | 10% | Pixel-identical output vs Pillow within tolerance |

The mode×variant matrix is the dominant dimension. A function with 4 parameter variants and 7 supported modes has 28 cells — coverage is reported as a two-dimensional grid.

### Manifest-Driven

`manifest.yaml` is the single source of truth. Every function declares its signature, parameter variants, supported modes, edge cases, and target formats. Tests use pytest markers to declare which cells they cover:

```python
@pytest.mark.covers("Image.resize", mode="RGB", variant="lanczos")
def test_resize_rgb_lanczos():
    ...
```

A post-test script (`scripts/compute_coverage.py`) reads `pytest --json-report` output and cross-references with `manifest.yaml` to produce `coverage/report.json`.

### Coverage Formula

```
FUNCTION_COVERAGE = (signature × 0.10) + (params × 0.20) + (modes × 0.35)
                  + (edges × 0.15) + (formats × 0.10) + (parity × 0.10)

MODULE_COVERAGE   = mean(function_scores)
OVERALL_COVERAGE  = weighted_mean(module_scores, by function_count)
```

## Data Flow

```
Python / JavaScript call
        │
        ▼
┌───────────────────┐
│ Binding layer     │  Converts host types → Rust primitives
│ (PyO3/wasm-bindgen)│   PyImage.resize(tuple) → RsImage.resize((u32,u32))
└────────┬──────────┘
         │
         ▼
┌───────────────────┐
│ pillow-rs-core    │  Pure Rust, operates on DynamicImage
│ RsImage::resize() │  Delegates to image crate / custom impl
└────────┬──────────┘
         │
         ▼
┌───────────────────┐
│ Returns RsImage   │  Binding layer wraps back → PyImage / JsImage
└───────────────────┘
```

## Architecture-Specific Optimizations

| Feature | x86_64 (Linux) | aarch64 (macOS/ARM) | wasm32 (Browser) |
|---------|----------------|---------------------|------------------|
| Threading | rayon (all cores) | rayon (all cores) | N/A (single-threaded) |
| SIMD | SSE2 + AVX2 | NEON | wasm32 simd128 |
| Allocator | jemalloc | system default | wee_alloc |
| GPU (future) | wgpu → Vulkan | wgpu → Metal | wgpu → WebGPU |
| Binary size | unconstrained | unconstrained | wasm-opt -Oz + brotli |

Priority operations for optimization (ranked by real-world impact):
1. Resize/filter — `fast_image_resize` for SIMD-accelerated resampling
2. Color conversions — SIMD-vectorized chunk processing with rayon
3. Paste/blend — pixel-level alpha compositing (Puhu's benchmarked hot path)
4. Encode/decode — zune-jpeg for faster JPEG decoding

## Error Handling

Core defines a `PilError` enum (thiserror) mapping 1:1 to Pillow's exception hierarchy:

| Core Error | Python Exception | JS Error |
|-----------|-----------------|----------|
| `IOError` | `OSError` | `IOError` |
| `UnidentifiedImageError` | `PIL.UnidentifiedImageError` | `UnidentifiedImageError` |
| `ValueError` | `ValueError` (builtin) | `ValueError` |
| `TypeError` | `TypeError` (builtin) | `TypeError` |
| `NotImplementedError` | `NotImplementedError` (builtin) | `NotImplementedError` |
| `ImageError` | custom subclass of Exception | `ImageError` |
| `UnknownFormat` | custom | `UnknownFormat` |

## Build Pipeline

```
Git push → CI:
  1. cargo test (core)
  2. cargo clippy --all-targets -- -D warnings
  3. maturin build --release (Python wheels) × all platforms
  4. wasm-pack build --target web (npm package)
  5. wasm-opt -Oz (browser binary optimization)
  6. pytest tests/ --json-report (Python test suite against built wheel)
  7. scripts/compute_coverage.py → coverage/report.json
  8. Publish: PyPI + npm
```

## Implementation Phases

### Phase 1: Stub Everything
- Write `manifest.yaml` covering all Pillow modules, functions, signatures, modes
- `scripts/generate_stubs.py` creates stub functions in core with `unimplemented!()` bodies
- Binding crates expose every stub
- Tests verify signatures exist and raise correct errors for unimplemented
- Coverage report shows 0% with full API surface enumerated

### Phase 2: Core Image Operations
- `Image.open` / `Image.save` (PNG, JPEG)
- `Image.resize` (all 4 resample filters × all modes)
- `Image.crop` (all coord variants)
- `Image.rotate` (90/180/270 + arbitrary angles)
- `Image.transpose` (all 7 methods)
- `Image.convert` (L, LA, RGB, RGBA, 1, P with dither and matrix)
- `Image.paste` (image + color + mask variants)
- `Image.split` / `Image.getbands`
- `Image.copy`

### Phase 3: Extended Operations
- ImageFilter (BLUR, SHARPEN, CONTOUR, etc.)
- ImageEnhance (Brightness, Contrast, Color, Sharpness)
- ImageDraw (line, rectangle, ellipse, text)
- Additional formats (GIF, WEBP, TIFF, BMP, ICO)

### Phase 4: Full Coverage
- Remaining Pillow modules (ImageFont, ImageMath, ImageStat, EXIF, etc.)
- All edge cases
- Behavioral parity across all mode×variant cells

## Performance Benchmarking

Adopted and extended from Puhu's benchmark approach in `BENCHMARKS.md`.

### Benchmark Harness

- **Tool:** `hyperfine` for CLI benchmarks, `pytest-benchmark` for in-Python micro-benchmarks
- **Configuration:** 30 runs per test, 3 warmup runs, mean ± σ reported
- **Comparison baseline:** Pillow (latest stable) vs pillow-rs on identical inputs
- **Matrix:** each operation benchmarked across image sizes (small 100×100, medium 800×600, large 4000×3000) and modes (RGB, RGBA, L)

### Benchmark Suite

| Category | Tests |
|----------|-------|
| **I/O** | `open_jpeg`, `open_png`, `save_jpeg`, `save_png`, `open_save_roundtrip` |
| **Resize** | `resize_nearest`, `resize_bilinear`, `resize_bicubic`, `resize_lanczos` at 3 sizes |
| **Crop** | `crop_small`, `crop_large`, `crop_boundary` |
| **Rotate** | `rotate_90`, `rotate_180`, `rotate_270` |
| **Convert** | `rgb_to_l`, `rgb_to_rgba`, `rgba_to_rgb`, `rgb_to_1` |
| **Paste** | `paste_image`, `paste_color`, `paste_with_mask` |
| **Filter** | `filter_blur`, `filter_sharpen`, `filter_contour` |
| **Pipeline** | `open→resize→crop→rotate→save` composite workflow |
| **WASM-specific** | `decode_jpeg` (bytes→pixels), `encode_png` (pixels→bytes), `resize_wasm` (sequential path) |

### Automated Comparison

CI runs the full benchmark suite on every release tag. Results are published alongside coverage reports. A `scripts/compare_benchmarks.py` script generates:

```
┌──────────────┬──────────┬──────────┬──────────────┐
│ Test         │ Pillow   │ pillow-rs│ Speedup      │
├──────────────┼──────────┼──────────┼──────────────┤
│ resize_4k    │ 198.4ms  │ 105.1ms  │ 1.89× faster │
│ convert_rgb  │  12.3ms  │   5.2ms  │ 2.37× faster │
│ paste_mask   │ 208.4ms  │ 123.5ms  │ 1.69× faster │
└──────────────┴──────────┴──────────┴──────────────┘
```

Regressions >5% from previous release trigger CI warnings.

### Python Benchmarks (pytest-benchmark)

```python
# tests/benchmarks/test_bench_resize.py
def test_bench_resize_rgb_large(benchmark, rgb_large_image):
    result = benchmark(lambda: rgb_large_image.resize((800, 600), Resampling.LANCZOS))
    assert result.size == (800, 600)
```

These run as part of `pytest -m benchmark` and feed into the same comparison pipeline.

## Key Design Decisions

1. **Public API names match Pillow exactly.** Internal Rust types can be named freely; users never see them.
2. **Lazy loading** from Puhu: `LazyImage::Path`, `LazyImage::Bytes`, `LazyImage::Loaded` — image is decoded only on first operation.
3. **Core is synchronous.** WASM async (fetch) is handled in JS glue code before calling into Rust.
4. **Single test suite** runs against both Pillow and pillow-rs by swapping the import.
5. **Coverage is computed, not manually tracked.** The manifest + test markers + CI pipeline produce living coverage reports.
