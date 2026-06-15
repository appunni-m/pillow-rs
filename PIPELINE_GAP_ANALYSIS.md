# Pipeline Gap Analysis — Complete Work Document

## Current State

| Metric | Count |
|--------|-------|
| `PipelineOp` variants in `pipeline.rs` | 55 |
| GPU shaders in `pool_gpu/shaders/` | 73 `.wgsl` files |
| `OpId` variants (GPU-dispatchable) | 52 |
| `gpu_entry!` registrations | 48 |
| `cpu_only` registrations | 6 |
| `simd_entry!` registrations | **0** (never used) |
| Manifest functions total | ~140 implemented |
| Manifest functions NOT pipelined | 20+ |

---

## Category A: PipelineOp variants — NO GPU (CPU-only)

These 6 ops have `PipelineOp` variants registered as `OpEntry::cpu_only()`.
They cannot benefit from GPU/SIMD today. Each needs a WGSL shader + `OpId` variant.

### A1. Rotate
- **Manifest:** `Image.rotate(angle, resample, expand, center, translate, fillcolor)`
- **PipelineOp:** `Rotate { angle: f64, expand: bool, fill: Option<(u8,u8,u8,u8)> }`
- **Difficulty:** 🔴 HIGH
- **GPU benefit:** 🔴 HIGH — bilinear/bicubic sampling at arbitrary angle
- **Algorithm:** Inverse mapping — for each output pixel, compute source coordinate via rotation matrix, sample with interpolation. Expand mode changes output size.
- **Notes:** Need to implement bilinear/bicubic texture sampling in WGSL. Rotation matrix: `[cos(θ), -sin(θ); sin(θ), cos(θ)]`. Inverse transform per output pixel.
- **Registry:** `registry.rs:734` — `OpEntry::cpu_only(...)`

### A2. RemapPalette
- **Manifest:** `Image.remap_palette(dest_map, source_palette)`
- **PipelineOp:** `RemapPalette { dest_map: Vec<u8> }`
- **Difficulty:** 🟢 LOW
- **GPU benefit:** 🟡 MEDIUM — 256-byte LUT lookup per pixel
- **Algorithm:** For each pixel's index byte, look up new index from 256-entry dest_map. Write new index. Affects P-mode images only.
- **Notes:** Simple uniform buffer for 256 u32 LUT. Read index, lookup, write.
- **Registry:** `registry.rs:837` — `OpEntry::cpu_only(...)`

### A3. Pad
- **Manifest:** `ImageOps.pad(image, size, method, color, centering)`
- **PipelineOp:** `Pad { w, h, filter, color, centering }`
- **Difficulty:** 🟡 MEDIUM
- **GPU benefit:** 🟡 MEDIUM — resampling for content placement
- **Algorithm:** Compute source rect within padded output using centering. If source fits: copy with fill border. If source too large: resize to fit then pad. Uses resampling filter.
- **Notes:** Related to `Fit`/`Cover`/`Contain` — shares resampling logic. The `Fit` shader already exists as a model.
- **Registry:** `registry.rs:1192` — `OpEntry::cpu_only(...)`

### A4. Expand
- **Manifest:** `ImageOps.expand(image, border, fill)`
- **PipelineOp:** `Expand { border: u32, fill: (u8,u8,u8,u8) }`
- **Difficulty:** 🟢 LOW
- **GPU benefit:** 🟢 LOW — simple copy + fill
- **Algorithm:** Output = (w + 2*border) × (h + 2*border). Fill border pixels with fill color. Copy source to center region.
- **Notes:** Trivial GPU shader — if/else on pixel position.
- **Registry:** `registry.rs:1225` — `OpEntry::cpu_only(...)`

### A5. CropBorder
- **Manifest:** `ImageOps.crop(image, border)`  (note: same name as Image.crop but different semantics)
- **PipelineOp:** `CropBorder { border: u32 }`
- **Difficulty:** 🟢 LOW
- **GPU benefit:** 🟢 LOW — simple copy
- **Algorithm:** Crop `border` pixels from all 4 edges. If border*2 >= w or h, return 1×1 image.
- **Notes:** Trivial — just copy a subregion. Similar to `Crop` which already has GPU.
- **Registry:** `registry.rs:1235` — `OpEntry::cpu_only(...)`

### A6. Merge
- **Manifest:** `Image.merge(mode, bands)`
- **PipelineOp:** `Merge { mode: ColorMode, bands: Vec<Image> }`
- **Difficulty:** 🟢 LOW
- **GPU benefit:** 🟡 MEDIUM — band interleaving
- **Algorithm:** Take N single-channel images (bands), interleave them into output image. L mode: 1 band → grayscale. RGB: 3 bands → interleave R,G,B. RGBA: 4 bands.
- **Notes:** Simple — for each pixel, read from each band and write interleaved.
- **Registry:** `registry.rs:1662` — `OpEntry::cpu_only(...)`

---

## Category B: GPU shaders EXIST but `op_id()` returns None (BROKEN GPU path)

These ops have WGSL shaders and `gpu_entry!` registrations, but `op_id()` in `registry.rs:319` returns `None` and `extract_params()` falls to `_ => vec![]`. **The GPU path is silently broken.**

### B1. GaussianBlur
- **Shader:** `gaussian_blur.wgsl` + `box_blur_h.wgsl` + `box_blur_v.wgsl` (separable)
- **Registry:** `registry.rs:895` — `gpu_entry!(..., "box_blur.wgsl")`
- **Missing in:** `op_id()` (line 319) and `extract_params()` (line 403)
- **Fix:** Add `PipelineOp::GaussianBlur { sigma } => Some(OpId::GaussianBlur)` to `op_id()`, add `OpId::GaussianBlur` variant to enum, add param extraction `vec![(sigma.to_bits())]`.

### B2. Autocontrast
- **Shader:** `autocontrast.wgsl` + `autocontrast_histogram.wgsl` + `autocontrast_remap.wgsl` (multi-pass)
- **Registry:** `registry.rs:993` — `gpu_entry!(..., "autocontrast.wgsl")`
- **Missing in:** `op_id()` and `extract_params()`
- **Fix:** Add `PipelineOp::Autocontrast { cutoff } => Some(OpId::Autocontrast)` to `op_id()`, add `OpId::Autocontrast`, add param extraction `vec![(cutoff.to_bits())]`.

### B3. Equalize
- **Shader:** `equalize.wgsl` + `equalize_histogram.wgsl` + `equalize_cdf.wgsl` + `equalize_remap.wgsl` (multi-pass)
- **Registry:** `registry.rs:1009` — `gpu_entry!(..., "equalize.wgsl")`
- **Missing in:** `op_id()` and `extract_params()`
- **Fix:** Add `PipelineOp::Equalize => Some(OpId::Equalize)` to `op_id()`, add `OpId::Equalize`, add param extraction `vec![]`.

---

## Category C: Manifest functions — NO PipelineOp at all (bypass pipeline)

These are image-producing operations that go directly to CPU, bypassing the pipeline system entirely. They cannot benefit from GPU or SIMD.

### C1. getchannel (Image method)
- **Manifest:** `Image.getchannel(channel)` → Image
- **Current impl:** `image.rs:936` — inline CPU loop copying one channel from RGBA
- **Should be:** `PipelineOp::GetChannel { channel: u8 }`
- **Difficulty:** 🟢 LOW
- **GPU benefit:** 🟡 MEDIUM — per-pixel copy, trivially parallel
- **Algorithm:** For each pixel, extract one channel (R=0, G=1, B=2, A=3) and write to all channels of output L image.

### C2. effect_mandelbrot (ImageModule function) ⚠️ VIOLATES THIN-CLIENT RULE
- **Manifest:** `ImageModule.effect_mandelbrot(size, extent, quality)` → Image
- **Current impl:** `operations.py:141` — **Nested Python loops with complex arithmetic!**
- **Should be:** `PipelineOp::EffectMandelbrot { w: u32, h: u32, x0: f64, y0: f64, x1: f64, y1: f64, quality: u32 }`
- **Difficulty:** 🟡 MEDIUM
- **GPU benefit:** 🔴 HIGH — embarrassingly parallel fractal computation
- **Algorithm:** For each pixel (px, py): map to complex plane (cx, cy), iterate z = z² + c up to `quality` times, output iteration count as grayscale.
- **Notes:** This is currently pure Python — violates the thin-client rule badly. MUST move to Rust.

### C3. linear_gradient (ImageModule function) ⚠️ VIOLATES THIN-CLIENT RULE
- **Manifest:** `ImageModule.linear_gradient(mode)` → Image
- **Current impl:** `operations.py:108` — **Python list comprehension generating bytes!**
- **Should be:** `PipelineOp::LinearGradient { mode: ColorMode }`
- **Difficulty:** 🟢 LOW
- **GPU benefit:** 🟡 MEDIUM — trivially parallel pixel generation
- **Algorithm:** For each pixel (x, y) in 256×256: output = y (vertical gradient from black to white). For RGB: replicate across channels.
- **Notes:** Violates thin-client rule. Simple GPU shader.

### C4. radial_gradient (ImageModule function) ⚠️ VIOLATES THIN-CLIENT RULE
- **Manifest:** `ImageModule.radial_gradient(mode)` → Image
- **Current impl:** `operations.py:119` — **Python math in nested loops!**
- **Should be:** `PipelineOp::RadialGradient { mode: ColorMode }`
- **Difficulty:** 🟢 LOW
- **GPU benefit:** 🟡 MEDIUM — trivially parallel
- **Algorithm:** For each pixel (x, y) in 256×256 centered at (128, 128): compute distance from center, normalize by max_dist, output as grayscale. For RGB: replicate.

---

## Category D: ImageDraw — not pipelined (harder GPU targets)

All ImageDraw methods call `materialize()` immediately and draw on CPU `RgbaImage` canvas. These are algorithmic rasterization operations, not pixel-parallel transforms. GPU compute shader rasterization is possible but more complex.

| Method | Algorithm | GPU Difficulty | GPU Benefit |
|--------|-----------|----------------|-------------|
| `line` | Bresenham | 🟡 Medium | 🟢 Low (algorithmic, not parallel) |
| `rectangle` | Boundary fill | 🟢 Low | 🟡 Medium (fill is parallel) |
| `ellipse` | Midpoint ellipse | 🟡 Medium | 🟡 Medium |
| `circle` | Midpoint circle | 🟡 Medium | 🟡 Medium |
| `polygon` | Scanline fill | 🔴 High | 🟡 Medium |
| `regular_polygon` | N-gon + scanline | 🔴 High | 🟡 Medium |
| `rounded_rectangle` | Rectangle + corner arcs | 🟡 Medium | 🟡 Medium |
| `arc` / `chord` / `pieslice` | Angular sweep | 🟡 Medium | 🟡 Medium |
| `point` | Single pixel | 🟢 Low | 🟢 Low |
| `text` / `multiline_text` | Glyph rasterization | 🔴 High | 🟡 Medium |
| `bitmap` | Image compositing | 🟢 Low | 🟡 Medium |
| `shape` | Arbitrary polygon | 🔴 High | 🟡 Medium |

**Recommendation:** ImageDraw GPU is a separate initiative. The fill operations (rectangle fill, ellipse fill, polygon fill) are the most GPU-amenable — each pixel independently tests "am I inside the shape?" and writes fill color. Stroke/border rendering is harder on GPU.

---

## Category E: SIMD — zero registrations

The SIMD pool has 35+ optimized functions in `pool_simd/ops/scalar.rs` but **none are wired into the registry**.

### E1. SIMD functions implemented but not registered
All these exist in `scalar.rs` with `pub fn` signatures, ready to use:
`invert`, `grayscale`, `solarize`, `posterize`, `brightness`, `contrast`, `flip`, `duplicate`, `invert_chops`, `add`, `subtract`, `multiply`, `screen`, `darker`, `lighter`, `difference`, `add_modulo`, `subtract_modulo`, `logical_and`, `logical_or`, `logical_xor`, `color_saturation`, `colorize`, `constant`, `offset`, `overlay`, `hard_light`, `soft_light`, `blend`, `blend_module`

### E2. What needs to happen
For each SIMD function, add a `simd_entry!` line in `register_all()`:
```rust
m.insert("Invert", simd_entry!(op_invert, simd_invert));
```
Where `simd_invert` wraps the scalar SIMD function with the `PipelineOp` → buffer extraction pattern.

### E3. SIMD wrapper pattern needed
The SIMD functions take `(pixels: &mut [u32], mode: u32, ...)` but the registry expects `fn(&DynamicImage, &PipelineOp, Option<&str>) -> Result<DynamicImage, PilError>`. Need adapter/wrapper functions that:
1. Extract pixels from `DynamicImage` as `Vec<u32>`
2. Call the SIMD function
3. Reconstruct `DynamicImage` from modified pixels

---

## Priority Implementation Order

### Phase 1: Bug Fixes (Category B) — ~2 hours
Fix the 3 broken GPU paths that have working shaders:
1. Add `OpId::GaussianBlur`, `OpId::Autocontrast`, `OpId::Equalize` to the `OpId` enum
2. Add match arms in `op_id()` — lines 319-388
3. Add match arms in `extract_params()` — lines 403-656
4. Verify GPU dispatch works

### Phase 2: Simple New PipelineOps (Category C: C1, C3, C4) — ~4 hours
Add `PipelineOp` variants and GPU shaders for:
1. `GetChannel { channel: u8 }` — extract single channel
2. `LinearGradient { mode: ColorMode }` — move from Python to Rust
3. `RadialGradient { mode: ColorMode }` — move from Python to Rust

Each needs: PipelineOp variant → CPU impl → GPU shader → OpId → extract_params → registry entry

### Phase 3: Simple GPU Shaders (Category A: A2, A4, A5, A6) — ~3 hours
GPU shaders for ops that are CPU-only:
1. `RemapPalette` — LUT lookup
2. `Expand` — border fill
3. `CropBorder` — subregion copy
4. `Merge` — band interleaving

### Phase 4: Medium GPU Shaders (Category A: A3, Category C: C2) — ~5 hours
1. `Pad` — content placement with resampling
2. `effect_mandelbrot` — move from Python, add GPU fractal shader

### Phase 5: Hard GPU Shader (Category A: A1) — ~6 hours
1. `Rotate` — arbitrary angle with bilinear/bicubic sampling

### Phase 6: SIMD Wiring (Category E) — ~4 hours
1. Create SIMD adapter/wrapper functions
2. Register all 30 SIMD ops via `simd_entry!`

### Phase 7: ImageDraw GPU (Category D) — separate project, ~2-4 weeks
1. Fill operations first (rectangle fill, ellipse fill, circle fill)
2. Stroke/border operations
3. Text rasterization (most complex)

---

## Files That Need Changes

| File | Phases |
|------|--------|
| `pillow-rs-core/src/pipeline.rs` | 2, 3, 4, 5 — new `PipelineOp` variants |
| `pillow-rs-core/src/compute/registry.rs` | ALL — `OpId` enum, `op_id()`, `extract_params()`, `register_all()` |
| `pillow-rs-core/src/compute/pool_cpu/ops/` | 2, 3, 4, 5 — CPU implementations |
| `pillow-rs-core/src/compute/pool_gpu/shaders/` | 2, 3, 4, 5 — new `.wgsl` shaders |
| `pillow-rs-core/src/ops/` | 2, 4 — move mandelbrot/gradient from Python |
| `pillow-rs-core/src/image.rs` | 2 — route new ops through pipeline |
| `pillow-rs-py/python/pillow_rs/operations.py` | 2, 4 — remove Python loops, delegate to Rust |
| `pillow-rs-core/src/compute/pool_simd/` | 6 — SIMD adapter wrappers |

---

## New WGSL Shaders Needed

| Count | Op | Shader Name | Multi-pass? |
|-------|----|-------------|-------------|
| 1 | GetChannel | `get_channel.wgsl` | No |
| 2 | LinearGradient | `linear_gradient.wgsl` | No |
| 3 | RadialGradient | `radial_gradient.wgsl` | No |
| 4 | EffectMandelbrot | `effect_mandelbrot.wgsl` | No |
| 5 | RemapPalette | `remap_palette.wgsl` | No |
| 6 | Expand | `expand.wgsl` | No |
| 7 | CropBorder | `crop_border.wgsl` | No |
| 8 | Merge | `merge.wgsl` | No |
| 9 | Pad | `pad.wgsl` | No |
| 10 | Rotate | `rotate.wgsl` | No |

---

## New OpId Variants Needed

```rust
// Category B fixes (unblock existing shaders)
GaussianBlur,
Autocontrast,
Equalize,

// Category C new ops
GetChannel,
EffectMandelbrot,
LinearGradient,
RadialGradient,

// Category A new GPU ops
RemapPalette,
Expand,
CropBorder,
Merge,
Pad,
Rotate,
```
