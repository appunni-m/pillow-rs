# Backend Architecture Reference — pillow-rs Compute Pipeline

## Module Structure

```
pillow-rs/src/compute/
├── mod.rs              # Backend enum, BackendImpl trait, router, activation
├── registry.rs         # OpEntry, gpu_entry! macro, variant_key, extract_params, register_all
├── pool_cpu/
│   ├── mod.rs          # CpuPool struct, BackendImpl for CPU
│   └── ops/
│       ├── mod.rs
│       ├── chops.rs    # ImageChops operations (add, subtract, blend, etc.)
│       ├── color.rs    # Color operations (convert, quantize, remap_palette)
│       ├── effects.rs  # Effects (paste, alpha_composite, eval, etc.)
│       ├── enhance.rs  # Enhance operations (brightness, contrast, etc.)
│       ├── filter.rs   # Filter operations (blur, median, rank, kernel, etc.)
│       ├── geometry.rs # Geometry operations (resize, crop, rotate, etc.)
│       └── imageops.rs # ImageOps (autocontrast, equalize, invert, etc.)
├── pool_gpu/
│   └── mod.rs          # GpuPool struct, GpuInner, BufferPool, BackendImpl for GPU
└── gpu_shaders/
    └── *.wgsl          # One WGSL compute shader per operation
```

## Backend Trait

```rust
pub trait BackendImpl: Send + Sync {
    fn name(&self) -> Backend;
    fn priority(&self) -> u8;   // higher = preferred when multiple support all ops
    fn supports(&self, op: &PipelineOp) -> bool;
    fn execute_batch(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
    ) -> Result<DynamicImage, PilError>;
}
```

Priorities: CPU=0, GPU=100, SIMD=50 (proposed)

## Routing Algorithm

```
1. If explicit backend set (Image.use_backend()), use it
2. For each pool (sorted by priority descending):
   - If pool is active AND supports ALL ops in the pipeline → use it
3. Fall back to CPU (universal — always active, supports everything)
```

**Critical invariant:** The entire pipeline runs on ONE backend. No per-op switching.

## Buffer Management

### BufferPool Layout

```rust
struct BufferPool {
    buf_a: wgpu::Buffer,     // Primary ping-pong buffer
    buf_b: wgpu::Buffer,     // Secondary ping-pong buffer
    buf_img2: wgpu::Buffer,  // Second image for dual-input ops
    params: wgpu::Buffer,    // Uniform buffer (256*16 = 4096 bytes)
    capacity: u32,           // Max pixels (4096*4096)
}
```

### Ping-Pong Flow

```
Start: upload input → buf_a + buf_b (identical copies)
Op 1: read buf_a → write buf_b  (result in buf_b), current_is_a = false
Op 2: read buf_b → write buf_a  (result in buf_a), current_is_a = true
...
After N ops: return current_is_a (NOT !current_is_a)
```

### Bind Group Layouts

**2-binding (no params):** input:ro, output:rw
**3-binding (with params):** input:ro, output:rw, params:uniform
**4-binding (dual-input):** input_a:ro, input_b:ro, output:rw, params:uniform

Currently `build_pipeline()` rejects bindings <2 or >4. 5-binding shaders
(composite, paste) need extended support.

## Uniform Buffer Layout

```rust
// Rust: [w, h, mode, _pad, ...op_specific_params]
let mut buf = vec![w, h, mode, 0u32];
buf.extend_from_slice(op_params);
```

```wgsl
// WGSL: matches byte-for-byte
struct Params {
    width: u32,   // offset 0
    height: u32,  // offset 4
    mode: u32,    // offset 8 — 0=L, 1=LA, 2=RGB, 3=RGBA
    _pad: u32,    // offset 12
    // ... op-specific params at offset 16+
}
```

## Mode Code Encoding

| Code | Mode | Channels | RGBA Representation (from to_rgba8()) |
|------|------|----------|---------------------------------------|
| 0 | L | 1 (luma) | R=G=B=luma, A=255 |
| 1 | LA | 2 (luma+alpha) | R=G=B=luma, A=alpha |
| 2 | RGB | 3 | R,G,B, A=255 |
| 3 | RGBA | 4 | R,G,B,A |

## preserve_mode() — Critical for L/LA Correctness

After GPU processing, the result is always `ImageRgba8`. The `preserve_mode()`
function converts back to the original color type.

**Bug fixed:** Original code used `result.to_luma8()` which does BT.601 weighted
averaging (0.299R + 0.587G + 0.114B). After mode-aware GPU processing, only R
is updated (G/B may be stale copies of the original luma). The weighted average
would combine new R with old G/B, producing wrong values.

**Fix:** For L mode, extract R channel directly:
```rust
image::ColorType::L8 => {
    let rgba = result.to_rgba8();
    let (w, h) = rgba.dimensions();
    let luma: Vec<u8> = rgba.pixels().map(|px| px[0]).collect();
    DynamicImage::ImageLuma8(GrayImage::from_raw(w, h, luma).unwrap_or_else(|| result.to_luma8()))
}
```

For LA mode, extract R (luma) and A (alpha) directly.

## Shader Validation

Shaders are compiled at `GpuInner::new()` time:

```rust
device.push_error_scope(wgpu::ErrorFilter::Validation);
let pipeline = device.create_compute_pipeline(&desc);
if pollster::block_on(device.pop_error_scope()).is_some() {
    return None;  // Silently skip — no error propagation
}
```

**Implication:** If a shader fails validation, it's silently unavailable on GPU.
The op falls back to CPU because `supports()` checks `gpu_shader.is_some()` from
the registry, but the actual pipeline lookup in `execute_batch_impl` will fail with
"no compiled pipeline". This should never happen because registry entries match
compiled pipelines, but debug by checking pipeline count after init.

## Adding a New Backend

1. Create `pool_<name>/mod.rs` implementing `BackendImpl`
2. Add pool to `pools()` in `mod.rs` (sorted by priority)
3. Register ops in `registry.rs` with backend-specific fields (add fields to `OpEntry` if needed)
4. Update `manifest.yaml` with `supported_targets` entries
5. Update binding code to expose `enable_backend("<name>")`
6. Add tests with `--backend=<name>`
