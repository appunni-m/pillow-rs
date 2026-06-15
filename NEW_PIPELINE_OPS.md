# New PipelineOps — Exhaustive Implementation List

## Operations currently bypassing pipeline (materialize() instead of push_op)

### 🔴 NEW PipelineOps to create (no variant exists today)

| # | PipelineOp | Manifest Function | Current Location | Pipeline Bypass |
|---|-----------|-------------------|------------------|-----------------|
| 1 | `GetChannel { channel: u8 }` | `Image.getchannel(channel)` | `image.rs:936` — CPU loop over RGBA | Calls `materialize()`, extracts one channel manually |
| 2 | `SplitBands` | `Image.split()` | `ops/split.rs:36` — CPU deinterleave | Calls `materialize()`, splits channels via loops |
| 3 | `EffectMandelbrot { w, h, x0, y0, x1, y1, quality }` | `ImageModule.effect_mandelbrot()` | `operations.py:141` — **Python nested loops!** | No Rust code at all — violates thin-client rule |
| 4 | `LinearGradient { mode: ColorMode }` | `ImageModule.linear_gradient()` | `operations.py:108` — **Python listcomp!** | No Rust code at all — violates thin-client rule |
| 5 | `RadialGradient { mode: ColorMode }` | `ImageModule.radial_gradient()` | `operations.py:119` — **Python math loops!** | No Rust code at all — violates thin-client rule |

### 🟡 EXISTING PipelineOp — but materialize() bypasses it (needs re-wiring, not new variant)

| # | PipelineOp | Manifest Function | Current Bug |
|---|-----------|-------------------|-------------|
| 6 | `Quantize { colors, dither }` | `Image.quantize()` | `ops/quantize.rs:926` — calls `materialize()` directly, does median cut on CPU. PipelineOp::Quantize EXISTS but is NEVER used. |
| 7 | `RemapPalette { dest_map }` | `Image.remap_palette()` | `image.rs:1210` — calls `materialize()` directly, does LUT remap on CPU. PipelineOp::RemapPalette EXISTS but is NEVER used. |
| 8 | `Convert { mode, matrix, dither }` | `Image.convert()` — matrix path only | `ops/convert.rs:56` — matrix-based conversion calls materialize() directly. Standard mode conversion correctly uses push_op. Need to extend PipelineOp::Convert to support matrix. |

---

## Detailed Implementation Specs

### 1. PipelineOp::GetChannel

**Current code (image.rs:936-958):**
```rust
pub fn getchannel(&self, channel: i32) -> Result<Image, PilError> {
    let img = self.materialize()?;            // ← BYPASS
    let bands = img.color().channel_count();
    let ch = if channel < 0 { (bands as i32 + channel) as usize } else { channel as usize };
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut gray = image::GrayImage::new(w, h);
    for (gp, rp) in gray.pixels_mut().zip(rgba.pixels()) {
        gp[0] = rp[ch.min(3)];
    }
    Ok(Image::Loaded(DynamicImage::ImageLuma8(gray), None))
}
```

**Should be:**
```rust
pub fn getchannel(&self, channel: i32) -> Result<Image, PilError> {
    let img = self.materialize()?;
    let bands = img.color().channel_count();
    let ch = if channel < 0 { (bands as i32 + channel) as usize } else { channel as usize };
    Ok(Image::push_op(self, PipelineOp::GetChannel { channel: ch as u8 }))
}
```

**CPU impl:** Extract one RGBA channel to L-mode output. Already implemented inline — move to `pool_cpu/ops/color.rs`.
**GPU shader:** `get_channel.wgsl` — 1-texture-load per pixel, extract one component, broadcast to RGBA. ~20 lines. Use existing Invert/Grayscale as template.
**Difficulty:** 🟢 Trivial — per-pixel copy
**GPU benefit:** 🟡 Medium

---

### 2. PipelineOp::SplitBands

**Current code (ops/split.rs):**
```rust
pub fn split(&self) -> Result<Vec<Image>, PilError> {
    let img = self.materialize()?;            // ← BYPASS
    // ... deinterleave pixels into separate band buffers ...
}
```

**Challenge:** `split()` returns `Vec<Image>` — multiple output images. Current pipeline model produces one output image per op. This is a design decision:

**Option A:** Keep `split()` as immediate operation (not pipeline op). It returns multiple images which don't fit the single-output pipeline.
**Option B:** Create individual `ExtractBand { index: u8 }` PipelineOp, and call it N times. Each extract is a pipeline op producing one band image.

**Recommendation:** Option A for now. Split is rarely chained — users typically split then work with individual bands. Not worth the pipeline complexity for the multi-output case.

**Difficulty:** 🟡 Medium (design decision)
**GPU benefit:** 🟢 Low (rarely benefits from chaining)

---

### 3. PipelineOp::EffectMandelbrot

**Current code (operations.py:141-167):**
```python
def effect_mandelbrot(size, extent, quality):
    w, h = size
    x0, y0, x1, y1 = extent
    data = bytearray(w * h)
    idx = 0
    for py in range(h):                     # ← Python nested loops!
        cy = y0 + (py / h) * (y1 - y0)
        for px in range(w):
            cx = x0 + (px / w) * (x1 - x0)
            zx, zy = 0.0, 0.0
            for i in range(quality):
                zx2 = zx * zx - zy * zy + cx
                zy2 = 2.0 * zx * zy + cy
                zx, zy = zx2, zy2
                if zx * zx + zy * zy > 4.0:
                    break
            data[idx] = i * 255 // quality
            idx += 1
    return Image.frombytes('L', size, bytes(data))
```

**Should be:**
```rust
// In image.rs or ops/module_fns.rs:
pub fn effect_mandelbrot(w: u32, h: u32, x0: f64, y0: f64, x1: f64, y1: f64, quality: u32) -> Image {
    // Create an empty source image, then run mandelbrot as pipeline op
    let src = Image::new_blank(w, h, "L")?;
    Image::push_op(&src, PipelineOp::EffectMandelbrot { w, h, x0, y0, x1, y1, quality })
}
```

**PipelineOp variant:**
```rust
EffectMandelbrot {
    w: u32, h: u32,
    x0: f64, y0: f64, x1: f64, y1: f64,
    quality: u32,
}
```

**CPU impl:** Move Python loops to Rust (identical algorithm). ~30 lines.
**GPU shader:** Full WGSL design in `GPU_ALGORITHM_RESEARCH.md` Section 3.2. ~40 lines. `@workgroup_size(8,8)`.
**Python binding:** Delete the Python loop code, delegate to `_core.effect_mandelbrot(...)`.
**Difficulty:** 🟡 Medium
**GPU benefit:** 🔴 HIGH — embarrassingly parallel

---

### 4. PipelineOp::LinearGradient

**Current code (operations.py:108-116):**
```python
def linear_gradient(mode: str) -> Image:
    if mode == "L":
        data = bytes(i for i in range(256) for _ in range(256))    # ← Python listcomp!
    elif mode == "RGB":
        data = bytes(i for i in range(256) for _ in range(256) for _ in range(3))
    return Image.frombytes(mode, (256, 256), data)
```

**Should be:**
```rust
// op args: mode only (always 256x256 output)
LinearGradient { mode: ColorMode }
```

**CPU impl:** For each (x, y) in 256×256: output = y (vertical ramp from 0→255). For RGB: replicate across all 3 channels. ~15 lines.
**GPU shader:** `linear_gradient.wgsl` — `let value = f32(gid.y) / 255.0; output = vec4(value, value, value, 1.0)`. ~20 lines.
**Python binding:** Delete Python code, delegate to Rust.
**Difficulty:** 🟢 Trivial
**GPU benefit:** 🟡 Medium

---

### 5. PipelineOp::RadialGradient

**Current code (operations.py:119-138):**
```python
def radial_gradient(mode: str) -> Image:
    cx, cy = 128.0, 128.0
    max_dist = (cx * cx + cy * cy) ** 0.5
    if mode == "L":
        data = bytes(
            min(int(((x - cx) ** 2 + (y - cy) ** 2) ** 0.5 / max_dist * 255 + 0.5), 255)
            for y in range(256) for x in range(256)               # ← Python math in loops!
        )
    return Image.frombytes(mode, (256, 256), data)
```

**Should be:**
```rust
RadialGradient { mode: ColorMode }
```

**CPU impl:** For each (x, y) in 256×256: compute distance from center (128, 128), normalize by max_dist (≈181), output as grayscale value. ~20 lines.
**GPU shader:** `radial_gradient.wgsl` — `let d = length(vec2f(gid.x, gid.y) - vec2f(128.0)); let value = d / 181.0;`. ~20 lines.
**Python binding:** Delete Python code, delegate to Rust.
**Difficulty:** 🟢 Trivial
**GPU benefit:** 🟡 Medium

---

## 🟡 Existing PipelineOps to RE-WIRE (not new variants)

### 6. Quantize → use existing PipelineOp::Quantize

**Current bug (ops/quantize.rs:918-947):**
```rust
pub fn quantize(&self, colors: u32, ...) -> Result<Image, PilError> {
    let img = self.materialize()?;                    // ← BYPASSES pipeline
    // ... median cut on CPU ...
    Ok(Image::Pipeline { source, ops: vec![], ... })  // ← Returns EMPTY pipeline!
}
```

**Fix:** Replace with `Image::push_op(self, PipelineOp::Quantize { colors, dither: true })`.
**CPU impl already registered:** `op_quantize()` in `pool_cpu/ops/color.rs`.
**GPU shader already exists:** `quantize.wgsl`.

### 7. RemapPalette → use existing PipelineOp::RemapPalette

**Current bug (image.rs:1209-1258):**
```rust
pub fn remap_palette(&self, dest_map: &[u8]) -> Result<Image, PilError> {
    let img = self.materialize()?;    // ← BYPASSES pipeline
    // ... LUT remap on CPU ...
}
```

**Fix:** Replace with `Image::push_op(self, PipelineOp::RemapPalette { dest_map: dest_map.to_vec() })`.
**CPU impl already registered:** `op_remap_palette()` in `pool_cpu/ops/color.rs`.

### 8. Convert (matrix path) → extend existing PipelineOp::Convert

**Current:** Standard mode conversion uses `push_op(PipelineOp::Convert { mode, matrix: None, dither })`. Matrix-based conversion (4-tuple or 12-tuple) calls `materialize()` directly because PipelineOp::Convert stores `matrix: Option<Vec<f64>>` but the GPU path ignores it.

**Fix:** Ensure matrix field is respected in CPU pool convert implementation. The PipelineOp already has the matrix field — just needs to route through correctly for all paths.

---

## Files to Create/Modify

| Phase | File | Action |
|-------|------|--------|
| **1** | `pipeline.rs` | Add 5 new variants: GetChannel, SplitBands, EffectMandelbrot, LinearGradient, RadialGradient |
| **2** | `compute/registry.rs` | Add OpId variants, op_id() arms, extract_params() arms, register_all() entries |
| **3** | `compute/pool_cpu/ops/color.rs` | Add CPU impl for GetChannel |
| **3** | `compute/pool_cpu/ops/effects.rs` | Add CPU impl for EffectMandelbrot |
| **3** | `compute/pool_cpu/ops/imageops.rs` | Add CPU impl for LinearGradient, RadialGradient |
| **4** | `compute/pool_gpu/shaders/` | New WGSL shaders: get_channel.wgsl, effect_mandelbrot.wgsl, linear_gradient.wgsl, radial_gradient.wgsl |
| **5** | `ops/module_fns.rs` | Add Rust entry points for effect_mandelbrot, linear_gradient, radial_gradient |
| **6** | `pillow-rs-py/src/lib.rs` | Bind new Rust functions |
| **7** | `operations.py` | Delete Python loops, delegate to Rust |
| **8** | `image.rs` | Rewire getchannel() to use push_op |
| **9** | `ops/quantize.rs` | Rewire quantize() to use push_op |
| **10** | `image.rs` | Rewire remap_palette() to use push_op |

---

## Estimated Effort

| New PipelineOp | Difficulty | CPU Rust | GPU WGSL | Python Cleanup | Total |
|---------------|-----------|----------|----------|----------------|-------|
| GetChannel | 🟢 Trivial | 0.5h | 0.5h | — | 1h |
| SplitBands | 🟡 Skip for now | — | — | — | — |
| EffectMandelbrot | 🟡 Medium | 1h | 1h | 0.5h | 2.5h |
| LinearGradient | 🟢 Trivial | 0.3h | 0.3h | 0.3h | 1h |
| RadialGradient | 🟢 Trivial | 0.3h | 0.3h | 0.3h | 1h |
| **Re-wiring (Bug fixes)** | | | | | |
| Quantize → push_op | 🟢 | — | — | — | 0.3h |
| RemapPalette → push_op | 🟢 | — | — | — | 0.3h |
| Convert matrix path | 🟡 | — | — | — | 0.5h |
| **Total** | | | | | **~7h** |
