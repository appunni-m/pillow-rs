# Benchmarking Infrastructure — Design Spec

> 2026-06-10 | pillow-rs | 6 targets

## Purpose

Single `BENCHMARKS.md` generated every build (pre-commit), showing all `manifest.yaml` functions ordered by importance with speedup ratios vs Pillow CPU across 6 runtime targets.

## Architecture

```
                    ┌──────────────────────────────┐
                    │     scripts/bench_all.sh      │  ← pre-commit hook
                    │  (orchestrator + aggregator)  │
                    └──────────┬───────────────────┘
           ┌─────────┬─────────┼─────────┬─────────┬──────────┐
           ▼         ▼         ▼         ▼         ▼          ▼
      ┌────────┐┌────────┐┌────────┐┌────────┐ ┌────────┐┌────────┐
      │Native  ││Native  ││WASM    ││WASM    │ │Browser ││Browser │
      │CPU     ││GPU     ││CPU     ││GPU     │ │CPU     ││GPU     │
      │(cargo) ││(wgpu)  ││(Node)  ││(Node   │ │(chrome ││(chrome │
      │        ││        ││        ││+WebGPU)│ │headless││+WebGPU)│
      └────┬───┘└───┬────┘└───┬────┘└───┬────┘ └───┬────┘└───┬────┘
           │        │         │         │          │         │
           ▼        ▼         ▼         ▼          ▼         ▼
      ┌──────────────────────────────────────────────────────────┐
      │              pillow-rs (shared ops)                 │
      │  open, resize, crop, rotate, transpose, thumbnail, ...   │
      └──────────────────────────────────────────────────────────┘
```

## 6 Runtime Targets

| # | Target | Harness | GPU |
|---|--------|---------|-----|
| 1 | CPU | Native Rust criterion benchmark binary | No |
| 2 | GPU | Native Rust + wgpu compute shaders | wgpu (Vulkan/Metal/DX12) |
| 3 | WASM CPU | Node.js loads `.wasm`, `performance.now()` timing | No |
| 4 | WASM GPU | Node.js + `--experimental-webgpu`, WebGPU compute | WebGPU experimental |
| 5 | WASM Browser CPU | Puppeteer + headless Chrome, `--disable-webgpu` | No |
| 6 | WASM Browser GPU | Puppeteer + headless Chrome, WebGPU enabled | WebGPU in-browser |

- All speedup ratios are **vs Pillow CPU** (Python Pillow on same machine).
- Pillow baseline is measured once per reference image set and cached in `target/benchmarks/pillow_baseline.json`.

## Importance Ordering

- **Tier 1 (Priority):** The 12 puhu benchmark operations, sorted per puhu's `BENCHMARKS.md` table order: `open_save`, `resize`, `crop`, `rotate`, `transpose`, `thumbnail`, `to_bytes`, `new`, `paste`, `paste_mask`, `paste_color`, `pipeline`.
- **Remaining functions:** Alphabetical within their module (Image, ImageChops, ImageColor, ImageDraw, ImageFilter, ImageEnhance, ImageFont, ImageModule, ImageOps, ImagePalette, ImageStat, ImageSequence).
- **Stubs:** Excluded from benchmark doc entirely.

## Function Classification

| Category | Examples | GPU? | Doc display |
|----------|----------|------|-------------|
| Pixel-parallel | filters, resize, convert, point, quantize, reduce, ops (invert, autocontrast, etc.) | GPU path | Show both CPU + GPU speedup |
| Geometry | crop, rotate, transpose, transform, paste | Partial GPU | Bench both; GPU may be N/A or marginal |
| Metadata/I/O | getbands, getextrema, getbbox, seek, tell, verify | No | Show "—" for GPU columns |
| Drawing | arc, line, text, ellipse, polygon | No | Show "—" for GPU columns |
| Stubs | toqimage, shape, effect_mandelbrot | No | Excluded entirely |

## Reference Images

Standard public-domain benchmark images, packaged in the repo or downloaded on first run:
- `ref_2k.jpg` — 2048×1536 RGB (main workhorse)
- `ref_1k.png` — 1024×1024 RGBA (transparency ops)
- `ref_grayscale.png` — 1024×1024 L (single-channel ops)

Each function's benchmark selects the appropriate reference image + mode based on `manifest.yaml`'s `supported_modes`.

## BENCHMARKS.md Format

```markdown
# pillow-rs Benchmarks

> Auto-generated: YYYY-MM-DD HH:MM:SS | commit <sha> | N functions | 6 targets

## Summary
| Metric | Value |
|--------|-------|
| Functions benchmarked | N |
| Average CPU speedup vs Pillow | 1.47× |
| ... |

## Priority Operations (Tier 1)
| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
|----------|-----|-----|----------|----------|-------------|-------------|
| open | 1.83× | — | 1.21× | — | 1.15× | — |
| resize | 1.12× | 3.45× | 0.88× | 2.10× | 0.82× | 1.95× |
| ... | ... | ... | ... | ... | ... | ... |

## All Functions
### Image
| Function | CPU | GPU | WASM CPU | WASM GPU | Browser CPU | Browser GPU |
|----------|-----|-----|----------|----------|-------------|-------------|
| ... | ... | ... | ... | ... | ... | ... |

### ImageFilter
...
```

- `—` means GPU not applicable.
- Column widths fixed per function set.
- "Change delta" footer section showing which functions moved since last commit.

## Incremental Caching

To make pre-commit viable, only re-benchmark functions whose source changed.

### Cache Key
```
cache_key = sha256(
    src/ops/<module>.rs   ← per-function source file
  + src/image.rs          ← shared struct (affects all)
  + Cargo.toml deps       ← dep version bumps
  + reference image hash  ← new sample image = full invalidate
)
```

### Cache Storage
```
target/benchmarks/
├── cache.json              ← per-function: {key, timings for all 6 targets}
├── pillow_baseline.json    ← Pillow reference timings
├── native_cpu.json         ← raw criterion output
├── native_gpu.json
├── wasm_cpu.json
├── wasm_gpu.json
├── browser_cpu.json
└── browser_gpu.json
```

### Expected Impact
| Change | Functions re-benched | Approx. time |
|--------|---------------------|--------------|
| Fix in single `resize.rs` | 1 function, 6 targets | ~10s |
| Change in `image.rs` (shared) | All 110 functions | ~5min |
| New function added | 1 function, 6 targets | ~10s |
| No Rust changes | 0 functions | ~1s |

## File Structure

```
pillow-rs/
├── benches/
│   ├── native_cpu.rs          # criterion benchmarks (all functions)
│   └── native_gpu.rs          # wgpu-accelerated benchmarks
├── src/gpu/
│   ├── mod.rs                 # GPU compute dispatcher
│   └── shaders/
│       ├── blur.wgsl
│       ├── color_ops.wgsl
│       ├── resample.wgsl
│       ├── blend.wgsl
│       └── convolve.wgsl

scripts/
├── bench_all.sh               # orchestrator
├── bench_cache.py             # cache key compute + stale detection
├── bench_aggregate.py         # merge JSONs → BENCHMARKS.md
├── bench_wasm_cpu.mjs         # Node.js WASM-CPU harness
├── bench_wasm_gpu.mjs         # Node.js WASM-GPU harness
├── bench_browser.mjs          # Puppeteer (both browser targets)
├── bench_pillow_baseline.py   # Generate Pillow reference timings

pillow-rs-js/
├── bench_page/
│   ├── index.html
│   └── bench_runner.js

BENCHMARKS.md                  # ← OUTPUT (auto-generated, committed)
```

## Shared WGSL Shader Pool

Same WGSL shaders for both native wgpu and WebGPU targets. Stored in `pillow-rs/src/gpu/shaders/`. One shader per operation family.

## Pre-Commit Hook

```bash
#!/bin/bash
# .git/hooks/pre-commit
set -e
# 1. Compute which ops changed
python3 scripts/bench_cache.py --check
# 2. Run stale targets only
bash scripts/bench_all.sh --incremental
# 3. Generate BENCHMARKS.md
python3 scripts/bench_aggregate.py
# 4. Stage if changed
git add BENCHMARKS.md
```

## Implementation Order

1. Pillow baseline script (`bench_pillow_baseline.py`) + source reference images
2. Native CPU benchmarks (`benches/native_cpu.rs` + criterion)
3. Cache system (`bench_cache.py`)
4. Aggregator + doc generator (`bench_aggregate.py`)
5. Orchestrator (`bench_all.sh`) — wires steps 1-4 end-to-end
6. Pre-commit hook wiring
7. WASM CPU harness (`bench_wasm_cpu.mjs`)
8. Browser harness (`bench_browser.mjs` + `bench_page/`)
9. Native GPU benchmarks + WGSL shaders
10. WASM GPU (Node.js experimental WebGPU)
11. Browser GPU (WebGPU enabled Chrome)
