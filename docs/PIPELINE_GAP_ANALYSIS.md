# Pipeline Gap Analysis — Consolidated (2026-06-16)

## Current State

| Metric | Count |
|--------|-------|
| `PipelineOp` variants in `pipeline.rs` | 65+ |
| GPU shaders in `pool_gpu/shaders/` | 82 `.wgsl` files |
| `OpId` variants (GPU-dispatchable) | 58 |
| `gpu_entry!` registrations | 71 |
| `cpu_only` registrations | 7 (Merge, ExtractBand, LinearGradient, RadialGradient, EffectMandelbrot, DrawLine..DrawPieslice via macro) |
| `simd_entry!` registrations | **0** (71 functions exist in scalar.rs, zero wired) |
| Manifest functions NOT pipelined | ~5 (text, bitmap compositing, quantize, convert matrix, convert nonstandard) |

---

## ✅ Previously Fixed (no longer gaps)

| Old Category | Item | Fix |
|-------------|------|-----|
| B | GaussianBlur GPU dispatch broken | OpId::GaussianBlur added, GPU dispatch wired |
| B | Autocontrast GPU dispatch broken | OpId::Autocontrast added, GPU dispatch wired |
| B | Equalize GPU dispatch broken | OpId::Equalize added, GPU dispatch wired |
| A | Rotate CPU-only | GPU shader + OpId::Rotate, now GPU-dispatched |
| A | RemapPalette CPU-only | GPU shader + OpId::RemapPalette, now GPU-dispatched |
| A | Pad CPU-only | GPU shader + OpId::Pad, now GPU-dispatched |
| A | Expand CPU-only | GPU shader + OpId::Expand, now GPU-dispatched |
| A | CropBorder CPU-only | GPU shader + OpId::CropBorder, now GPU-dispatched |
| C1 | getchannel bypassed pipeline | Now uses push_op(PipelineOp::ExtractBand) |
| C2 | effect_mandelbrot in Python | Moved to Rust, pipelined via EffectMandelbrot |
| C3 | linear_gradient in Python | Moved to Rust, pipelined via LinearGradient |
| C4 | radial_gradient in Python | Moved to Rust, pipelined via RadialGradient |
| — | remap_palette bypassed pipeline | Now uses push_op(PipelineOp::RemapPalette) |
| — | Draw ops (line, rect, ellipse, etc.) in CPU loops | Now use push_op with PipelineOp::Draw* variants |
| — | Python thin-client violations (loops/arithmetic) | All moved to Rust |

---

## 🔴 GAP 1: Quantize bypasses pipeline

**Location:** `ops/quantize.rs:1065-1095`

**Problem:** `quantize()` calls `self.materialize()?`, does median cut on CPU, then returns `Image::Pipeline { ops: vec![], ... }` — an EMPTY pipeline. `PipelineOp::Quantize` + `quantize.wgsl` GPU shader EXIST but are NEVER used.

**Fix:**
```rust
// Instead of materialize + CPU median cut:
Ok(Image::push_op(self, PipelineOp::Quantize { colors: n_colors as u32, dither: true }))
```

**CPU impl already registered:** `op_quantize()` in `pool_cpu/ops/color.rs`
**GPU shader already exists:** `quantize.wgsl`
**Difficulty:** 🟢 Trivial — one-line change + ensure mode tagging preserved (current impl tags output as P-mode)

---

## 🔴 GAP 2: SIMD pool — zero registrations

**Location:** `compute/registry.rs` — zero `simd_entry!` calls
**SIMD functions exist:** 71 functions in `pool_simd/ops/scalar.rs`

**Problem:** The SIMD infrastructure (SIMD entry macro, pool, dispatch) is fully built but nothing is wired. Every op falls through to CPU. The `simd_entry!` macro exists at line 61 of registry.rs but is never called.

**List of unwired SIMD functions:**
`invert`, `grayscale`, `solarize`, `posterize`, `brightness`, `contrast`, `flip`, `duplicate`, `invert_chops`, `add`, `subtract`, `multiply`, `screen`, `darker`, `lighter`, `difference`, `add_modulo`, `subtract_modulo`, `logical_and`, `logical_or`, `logical_xor`, `color_saturation`, `colorize`, `constant`, `offset`, `overlay`, `hard_light`, `soft_light`, `blend`, `blend_module`

**Fix:** For each SIMD function, add a wrapper matching the `CpuOpFn` signature and register:
```rust
m.insert("Invert", simd_entry!(op_invert, simd_invert));
```
Need adapter: `fn(&DynamicImage, &PipelineOp, Option<&str>) -> Result<DynamicImage, PilError>` that extracts RGBA pixels → calls SIMD → reconstructs DynamicImage.

**Difficulty:** 🟡 Medium (bulk work, repetitive pattern)
**Performance benefit:** 🔴 HIGH — native SIMD for all 30+ pixel ops

---

## 🟡 GAP 3: Merge — GPU shader exists but registered as cpu_only

**Location:** `registry.rs:1818` — `OpEntry::cpu_only(...)`

**Problem:** `merge.wgsl` shader exists, `OpId::Merge` exists, `op_id()` returns `Some(OpId::Merge)`, but the registry uses `cpu_only`. The comment at line 1815 explains the binding layout (4 bindings with extra_bands as storage read) is incompatible with the existing single/dual-image upload infrastructure.

**Fix:** Extend GPU upload infrastructure to support multi-band input (N storage textures). Non-trivial — requires binding layout changes.

**Difficulty:** 🔴 Hard (infrastructure change)
**GPU benefit:** 🟡 Medium

---

## 🟡 GAP 4: ExtractBand — CPU-only, no GPU shader

**Location:** `registry.rs:2010` — `OpEntry::cpu_only(...)`
**PipelineOp:** `ExtractBand { index: u8 }`

**Problem:** No GPU shader, no OpId variant. Extracting a single channel is trivially parallel — a ~20-line WGSL shader.

**Fix:** Write `extract_band.wgsl`, add `OpId::ExtractBand`, add to op_id() + extract_params(), change registry to gpu_entry.

**Difficulty:** 🟢 Trivial
**GPU benefit:** 🟡 Medium (per-pixel copy)

---

## 🟡 GAP 5: LinearGradient, RadialGradient, EffectMandelbrot — CPU-only, no GPU shaders

**Location:** `registry.rs:2021-2063` — all `OpEntry::cpu_only(...)`

**Problem:** These are embarrassingly parallel but have no GPU shaders.
- `LinearGradient { mode }` — 256×256 vertical ramp
- `RadialGradient { mode }` — 256×256 radial from center
- `EffectMandelbrot { w, h, x0, y0, x1, y1, quality }` — Mandelbrot fractal

**Fix:** Write WGSL shaders for each (~20-40 lines each), add OpId variants, wire through GPU dispatch.

**Difficulty:** 🟢 Linear/Radial (trivial), 🟡 Mandelbrot (medium, complex math)
**GPU benefit:** 🔴 HIGH for Mandelbrot (per-pixel independent fractal iteration)

---

## 🟡 GAP 6: ImageDraw ops — CPU-only, no GPU shaders

**Location:** `registry.rs:2067-2140` — all via `draw_entry!` macro → `cpu_only`

**PipelineOps:** `DrawLine`, `DrawRectangle`, `DrawRoundedRect`, `DrawEllipse`, `DrawCircle`, `DrawPolygon`, `DrawArc`, `DrawChord`, `DrawPieslice`, `DrawPoint`

**Problem:** All draw ops execute on CPU. Fill operations (rectangle fill, ellipse fill, circle fill, polygon fill) are GPU-amenable (each pixel independently tests "am I inside the shape?"). Stroke/border rendering is harder on GPU.

**GPU Difficulty by shape:**
| Op | Fill GPU | Stroke GPU |
|----|----------|------------|
| DrawRectangle | 🟢 Trivial (bound check) | 🟢 Trivial |
| DrawCircle | 🟢 Trivial (distance check) | 🟡 Medium |
| DrawEllipse | 🟢 Trivial (implicit equation) | 🟡 Medium |
| DrawRoundedRect | 🟡 Medium (rect + corner arcs) | 🟡 Medium |
| DrawPolygon | 🔴 Hard (point-in-polygon) | 🔴 Hard |
| DrawArc/Chord/Pieslice | 🟡 Medium (angle + distance) | 🟡 Medium |
| DrawLine | 🔴 Hard (Bresenham is serial) | N/A |
| DrawPoint | 🟢 Trivial | N/A |

**Fix:** Start with simplest fill ops (rectangle, circle, ellipse), then tackle stroke variants.

**Difficulty:** 🟡 Medium to 🔴 Hard depending on shape
**GPU benefit:** 🟡 Medium for fill, 🟢 Low for strokes

---

## 🟢 GAP 7: Text/bitmap in draw still materialize inline

**Location:** `draw/mod.rs:879` (text) and `draw/mod.rs:261` (bitmap)

**Problem:** While draw ops (line, rectangle, etc.) use push_op, text rendering and bitmap compositing still call `self.image.materialize()?` directly for pixel-level compositing. This breaks pipeline fusion — any text/bitmap call forces materialization of all pending ops.

**Fix for bitmap:** Convert to push_op with `PipelineOp::DrawBitmap { x, y, bitmap_data, mask_mode, fill }`. The bitmap compositing is per-pixel blending (parallel).
**Fix for text:** More complex — involves font rasterization, glyph placement, spacing. Could be a multi-step pipeline: rasterize glyphs → place → composite.

**Difficulty:** 🟡 Medium (bitmap), 🔴 Hard (text)
**Impact:** 🟡 Medium — breaks pipeline batching when bitmap/text is used

---

## 🟢 GAP 8: Convert — matrix and nonstandard-mode paths materialize directly

**Location:** `ops/convert.rs:58-101`

**Problem:** Standard mode conversion correctly uses push_op but these paths bypass:
- Matrix-based conversion (custom 4-tuple or 12-tuple matrix) — line 61
- Non-standard to standard mode conversion (CMYK→RGB, HSV→RGB, etc.) — line 75
- Binary mode "1" conversion — line 109

`PipelineOp::Convert` already has a `matrix: Option<Vec<f64>>` field.

**Fix:** Extend the Convert CPU implementation to handle matrix + nonstandard source modes, then route all paths through push_op.

**Difficulty:** 🟡 Medium
**Impact:** 🟢 Low (rare conversion paths)

---

## Summary Table

| # | Gap | Type | Difficulty | Benefit | Status |
|---|-----|------|-----------|---------|--------|
| 1 | Quantize bypasses pipeline | Bug fix | 🟢 Trivial | 🟡 Medium | PipelineOp+shader exist, unused |
| 2 | SIMD zero registrations | Missing wiring | 🟡 Medium | 🔴 High | 71 functions unwired |
| 3 | Merge GPU shader unused | Binding infra | 🔴 Hard | 🟡 Medium | Shader exists, cpu_only |
| 4 | ExtractBand no GPU | Missing shader | 🟢 Trivial | 🟡 Medium | CPU-only |
| 5 | Gradient/Mandelbrot no GPU | Missing shaders | 🟢-🟡 | 🔴 High | CPU-only |
| 6 | Draw ops no GPU | Missing shaders | 🟡-🔴 | 🟡 Medium | CPU-only |
| 7 | Text/bitmap materialize | Pipeline bypass | 🟡-🔴 | 🟡 Medium | Inline compositing |
| 8 | Convert edge paths materialize | Pipeline bypass | 🟡 Medium | 🟢 Low | Rare paths |

---

## Priority Implementation Order

### Phase 1: Quick wins (~3 hours)
1. **Fix Quantize** (Gap 1) — one-line change, shader already exists
2. **ExtractBand GPU shader** (Gap 4) — ~20 line WGSL
3. **LinearGradient GPU shader** (Gap 5) — ~20 line WGSL
4. **RadialGradient GPU shader** (Gap 5) — ~20 line WGSL

### Phase 2: Medium effort (~5 hours)
5. **EffectMandelbrot GPU shader** (Gap 5) — ~40 line WGSL with complex math
6. **Simple draw fill GPU** (Gap 6) — rectangle fill, circle fill, ellipse fill

### Phase 3: Infrastructure (~1-2 weeks)
7. **SIMD wiring** (Gap 2) — wire all 30 SIMD functions
8. **Merge GPU binding** (Gap 3) — extend multi-image upload infra

### Phase 4: Harder challenges (~1-2 weeks)
9. **Draw stroke GPU** (Gap 6) — polygon, arc, line GPU
10. **Text/bitmap pipeline** (Gap 7) — push_op for bitmap and text

### Phase 5: Polish (~3 hours)
11. **Convert edge paths** (Gap 8) — matrix + nonstandard through pipeline

---

## Files That Need Changes

| File | Gaps |
|------|------|
| `pillow-rs/src/ops/quantize.rs` | Gap 1 — rewire to push_op |
| `pillow-rs/src/compute/pool_simd/` | Gap 2 — create adapter wrappers |
| `pillow-rs/src/compute/registry.rs` | All gaps — OpId variants, op_id arms, registry entries |
| `pillow-rs/src/pipeline.rs` | Gap 7 — DrawBitmap variant |
| `pillow-rs/src/compute/pool_gpu/shaders/` | Gaps 4-6 — new WGSL shaders |
| `pillow-rs/src/compute/pool_cpu/ops/` | Gaps 4-7 — CPU impls for new ops |
| `pillow-rs/src/draw/mod.rs` | Gap 7 — bitmap/text pipeline |
| `pillow-rs/src/ops/convert.rs` | Gap 8 — route all paths through push_op |
