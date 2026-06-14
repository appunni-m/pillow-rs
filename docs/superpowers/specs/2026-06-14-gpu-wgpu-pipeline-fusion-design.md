# GPU Acceleration with wgpu — Backend Abstraction Design

> Status: ✅ Approved | Date: 2026-06-14 | Approach: **ComputeBackend Trait + Declarative Op Map**

## Context

pillow-rs currently has a GPU scaffold in `pillow-rs-core/src/gpu/`:
- `GpuEngine` struct with wgpu Device/Queue — initialization works
- 5 WGSL shaders embedded via `include_str!` — structurally correct
- 7 operation method stubs — ALL return `Err("GPU ... not yet wired")`
- **Zero integration** with Image/Pipeline — GpuEngine is unreferenced outside its own file

This design replaces the scaffold with a **trait-based compute backend** architecture that:
- Supports GPU (wgpu) today, CPU SIMD / CUDA / Metal tomorrow
- Requires touching only **2 files** to add GPU support for an existing operation
- Has **6 write-once-never-edit** engine files
- Achieves **pixel-exact PIL parity** via i32 integer math in WGSL

## User Requirements (confirmed)

| Requirement | Decision |
|-------------|----------|
| Shader organization | **Dedicated micro-shaders** (one WGSL file per operation) |
| GPU↔CPU handover | **Pipeline fusion**: Consecutive GPU ops batched, single upload, single readback |
| Multi-pass ops | **Pre-compute on GPU**: Histogram/CDF stay on GPU (4KB params buffer) |
| Precision | **Pixel-exact**: i32/u32 integer math matching PIL's C accumulation order |
| GPU enablement | **Global flag + per-pipeline injectability** |
| WASM target | **Both native & WASM**: All GPU code in core, JS binding is thin wrapper |
| Test coverage | **Auto-discovered** from fixture JSONs + `@pytest.mark.covers()` decorators |
| Code organization | **Backend abstraction**: Engine files never touched for new ops; multi-arch ready |
| Per-op touch points | **Minimum**: Shader file + 1 line in op_map macro |

## Architecture

### Tiered File Organization

**TIER 1 — Write Once, Never Edit (engine code):**

| File | Purpose | Why never touched |
|------|---------|-------------------|
| `compute/mod.rs` | `ComputeBackend` trait + backend selection | Stable abstraction |
| `compute/gpu_backend.rs` | wgpu engine: buffer pool, ping-pong dispatch, shader compiler, bind group builder | Generic — works with any shader via registry |
| `compute/cpu_backend.rs` | Delegates to existing `execute_op()` | Passthrough only |
| `compute/registry.rs` | `OpRegistry` — maps `PipelineOp` → shader/pipeline, auto-built from macro | Generic data structure |
| `image.rs` | `materialize()` dispatch loop | Uses trait, not concrete types |
| `lib.rs` | Module declarations (`pub mod compute;`) | One-time setup |

**TIER 2 — Append Only (one entry per operation):**

| File | What you add | Example |
|------|-------------|---------|
| `compute/gpu_shaders/<op>.wgsl` | **NEW** — WGSL shader file | `invert.wgsl` |
| `compute/op_map.rs` | **+1 line** — macro invocation | `Invert => "invert.wgsl", 1, false` |

**TIER 3 — Required for ANY new operation (already exists today):**

| File | Purpose |
|------|---------|
| `pipeline.rs` | `PipelineOp` variant |
| `ops/<module>.rs` | CPU implementation |
| `pillow-rs-py/src/lib.rs` | PyO3 binding delegation |
| `pillow-rs-js/src/lib.rs` | WASM binding delegation |
| `manifest.yaml` | API spec |
| `tests/test_gpu.py` | GPU parity test |

### ComputeBackend Trait (Write Once)

```rust
// pillow-rs-core/src/compute/mod.rs — NEVER EDITED AFTER CREATION

use crate::error::PilError;
use crate::pipeline::PipelineOp;
use image::DynamicImage;

/// A compute backend that executes image pipeline operations.
/// 
/// # Adding a new backend (SIMD, CUDA, Metal, etc.)
/// Implement this trait in a new file. The materialize() dispatch loop
/// works with any backend automatically.
/// 
/// # Adding GPU support for a new operation
/// See `compute/op_map.rs` — add a WGSL shader + one macro line.
/// The GpuBackend engine code does NOT need to change.
pub trait ComputeBackend: Send + Sync {
    /// Unique name for logging/benchmarking.
    fn name(&self) -> &'static str;

    /// Check if this backend can execute a given pipeline operation.
    fn supports(&self, op: &PipelineOp) -> bool;

    /// Execute a BATCH of operations.
    /// 
    /// Backends can optimize across the batch:
    /// - GPU: upload once → N dispatches (ping-pong) → readback once
    /// - CPU: iterate ops calling execute_op (no fusion needed)
    /// - SIMD: batch-process contiguous compatible ops
    fn execute_batch(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        explicit_mode: Option<&str>,
    ) -> Result<DynamicImage, PilError>;

    /// Priority for backend selection. Higher = preferred.
    /// GPU = 100, SIMD = 50, CPU = 0 (always last, always available).
    fn priority(&self) -> u8;
}
```

### Declarative Op Map (Append Only)

```rust
// pillow-rs-core/src/compute/op_map.rs
// 
// TO ADD GPU SUPPORT FOR A NEW OPERATION:
//   1. Write the WGSL shader in compute/gpu_shaders/<name>.wgsl
//   2. Add ONE line below using the register_gpu_op! macro
//   3. Done — the engine auto-discovers it at init
//
// The macro expands to:
//   - An OpDef entry in the static OP_REGISTRY
//   - A match arm in op_id(&PipelineOp) -> Option<OpId>
//   - Pipeline compilation at GpuBackend init time

register_gpu_ops! {
    // ── Color single-input ─────────────────────────────
    // (PipelineOp variant,  shader file,            inputs, has_params)
    Invert          => "invert.wgsl",           1,      true,   // —
    Grayscale       => "grayscale.wgsl",        1,      false,  // —
    Solarize        => "solarize.wgsl",         1,      true,   // threshold: u8
    Posterize       => "posterize.wgsl",        1,      true,   // bits: u8
    Brightness      => "brightness.wgsl",       1,      true,   // factor: f64
    Contrast        => "contrast.wgsl",         1,      true,   // factor: f64
    ColorSaturation => "color_saturation.wgsl", 1,      true,   // factor: f64
    Colorize        => "colorize.wgsl",         1,      true,   // black, white
    Constant        => "constant.wgsl",         1,      true,   // value: u8
    Offset          => "offset.wgsl",           1,      true,   // dx, dy: i32

    // ── Blend dual-input ───────────────────────────────
    Multiply        => "multiply.wgsl",         2,      false,  // —
    Screen          => "screen.wgsl",           2,      false,  // —
    Difference      => "difference.wgsl",       2,      false,  // —
    AddModulo       => "add_modulo.wgsl",       2,      false,  // —
    SubtractModulo  => "subtract_modulo.wgsl",  2,      false,  // —
    Darker          => "darker.wgsl",           2,      false,  // —
    Lighter         => "lighter.wgsl",          2,      false,  // —
    Overlay         => "overlay.wgsl",          2,      false,  // —
    HardLight       => "hard_light.wgsl",       2,      false,  // —
    SoftLight       => "soft_light.wgsl",       2,      false,  // —
    LogicalAnd      => "logical_and.wgsl",      2,      false,  // —
    LogicalOr       => "logical_or.wgsl",       2,      false,  // —
    LogicalXor      => "logical_xor.wgsl",      2,      false,  // —
    ChopAdd         => "chop_add.wgsl",         2,      true,   // scale, offset
    ChopSubtract    => "chop_subtract.wgsl",    2,      true,   // scale, offset
    Blend           => "blend.wgsl",            2,      true,   // alpha: f64

    // ── Convolution ────────────────────────────────────
    Filter3x3       => "filter_3x3.wgsl",       1,      true,   // kernel[9], scale, offset
    Filter5x5       => "filter_5x5.wgsl",       1,      true,   // kernel[25], scale, offset
    Sharpen         => "sharpen.wgsl",          1,      false,  // fixed kernel
    UnsharpMask     => "unsharp_mask.wgsl",     1,      true,   // radius, percent, threshold

    // ── Separable blur (2-pass auto-detected) ──────────
    BoxBlur         => "box_blur",              1,      true,   // radius: u32, 2-pass
    GaussianBlur    => "gaussian_blur",         1,      true,   // sigma: f32, 2-pass

    // ── Rank filters ───────────────────────────────────
    MedianFilter    => "median_filter.wgsl",    1,      true,   // size: u32
    MaxFilter       => "max_filter.wgsl",       1,      true,   // size: u32
    MinFilter       => "min_filter.wgsl",       1,      true,   // size: u32
    RankFilter      => "rank_filter.wgsl",      1,      true,   // size: u32, rank: u32

    // ── Resample ───────────────────────────────────────
    ResizeNearest   => "resize_nearest.wgsl",   1,      true,   // dst_w, dst_h
    ResizeBilinear  => "resize_bilinear.wgsl",  1,      true,   // dst_w, dst_h
    ResizeBicubic   => "resize_bicubic.wgsl",   1,      true,   // dst_w, dst_h
    ResizeLanczos   => "resize_lanczos.wgsl",   1,      true,   // dst_w, dst_h
    ResizeBox       => "resize_box.wgsl",       1,      true,   // dst_w, dst_h
    ResizeHamming   => "resize_hamming.wgsl",   1,      true,   // dst_w, dst_h

    // ── Composite ──────────────────────────────────────
    Paste           => "paste.wgsl",            2,      true,   // box coords, mask optional
    AlphaComposite  => "alpha_composite.wgsl",  3,      true,   // source + dest coords
    EffectSpread    => "effect_spread.wgsl",    1,      true,   // distance: u32

    // ── Geometry ───────────────────────────────────────
    Flip            => "flip.wgsl",             1,      false,  // —
    Mirror          => "mirror.wgsl",           1,      false,  // —
    Duplicate       => "duplicate.wgsl",        1,      false,  // identity (pass-through)

    // ── Multi-pass histogram (3 dispatches each) ───────
    Autocontrast    => "autocontrast",          1,      true,   // cutoff: f64, 3-pass
    Equalize        => "equalize",              1,      false,  // 3-pass
}
```

The macro auto-generates:

```rust
// Generated by register_gpu_ops! — never written by hand
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum OpId { Invert = 0, Grayscale = 1, /* ... */ }

static OP_REGISTRY: OnceLock<Vec<OpDef>> = OnceLock::new();

struct OpDef {
    id: OpId,
    variant_name: &'static str,    // "Invert", "Grayscale", etc.
    shader_source: &'static str,   // include_str!(...) at compile time
    input_count: u8,               // 1, 2, or 3
    has_params: bool,              // Whether PipelineOp variant carries params
    is_multi_pass: bool,           // true for BoxBlur, GaussianBlur, Autocontrast, Equalize
}

fn op_id(op: &PipelineOp) -> Option<OpId> {
    // Auto-generated match — one arm per macro entry
    match op {
        PipelineOp::Invert { .. } => Some(OpId::Invert),
        PipelineOp::Grayscale { .. } => Some(OpId::Grayscale),
        PipelineOp::Solarize { .. } => Some(OpId::Solarize),
        // ... all entries from macro
        _ => None,
    }
}

fn extract_params(op: &PipelineOp) -> Vec<u32> {
    // Auto-generated — packs PipelineOp variant fields into flat u32 array
    match op {
        PipelineOp::Filter3x3 { kernel, scale, offset } => {
            let mut p = Vec::with_capacity(11);
            p.extend(kernel.iter().map(|&k| k.to_bits()));  // 9 f32
            p.push(scale.to_bits());                         // 1 f32
            p.push(*offset as u32);                          // 1 i32
            p
        }
        PipelineOp::Solarize { threshold } => vec![*threshold as u32],
        // ... etc
    }
}
```

### GpuBackend (Write Once)

```rust
// pillow-rs-core/src/compute/gpu_backend.rs — NEVER EDITED FOR NEW OPS

pub struct GpuBackend {
    device: wgpu::Device,
    queue: wgpu::Queue,
    buffers: BufferPool,
    pipelines: HashMap<OpId, CachedPipeline>,
    max_size: u32,
}

struct CachedPipeline {
    shader: wgpu::ShaderModule,
    pipeline: wgpu::ComputePipeline,
    bind_group_layout_0: wgpu::BindGroupLayout,  // For single-input ops
    bind_group_layout_1: wgpu::BindGroupLayout,  // For dual-input ops
    bind_group_layout_2: wgpu::BindGroupLayout,  // For triple-input ops
    layout_index: u8,                             // Which layout to use (based on input_count)
}

impl GpuBackend {
    /// Initialize GPU. Automatically compiles all shaders from OP_REGISTRY.
    pub fn new(max_size: u32) -> Option<Self> {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&default()))?;
        let (device, queue) = pollster::block_on(
            adapter.request_device(&default(), None)
        ).ok()?;

        let buffers = BufferPool::new(&device, max_size);
        
        // Compile ALL shaders at init time from the registry
        let pipelines = OP_REGISTRY.get().unwrap().iter().map(|def| {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(def.variant_name),
                source: wgpu::ShaderSource::Wgsl(def.shader_source.into()),
            });
            // Build pipeline from shader — same structure for all ops
            // Layout determined by input_count + has_params
            let (pipeline, layout) = Self::build_pipeline(
                &device, &shader, def.input_count, def.has_params
            );
            (def.id, CachedPipeline { shader, pipeline, layout })
        }).collect();

        Some(Self { device, queue, buffers, pipelines, max_size })
    }

    /// Build a compute pipeline for ANY shader.
    /// Generic — uses the same bind group layout patterns for all ops.
    fn build_pipeline(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        input_count: u8,
        has_params: bool,
    ) -> (wgpu::ComputePipeline, wgpu::BindGroupLayout) {
        // Layout entries based on input_count and has_params:
        // Single-input:  [params?] [input] [output]
        // Dual-input:    [params?] [input_a] [input_b] [output]
        // Triple-input:  [params?] [input_a] [input_b] [input_c] [output]
        let mut entries = Vec::new();
        let mut binding = 0u32;
        if has_params {
            entries.push(/* uniform buffer at binding 0 */);
            binding += 1;
        }
        for _ in 0..input_count {
            entries.push(/* storage read-only buffer at binding N */);
            binding += 1;
        }
        entries.push(/* storage read_write buffer at last binding */);
        
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &entries,
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
        });
        (pipeline, layout)
    }
}

impl ComputeBackend for GpuBackend {
    fn name(&self) -> &'static str { "wgpu" }
    fn priority(&self) -> u8 { 100 }

    fn supports(&self, op: &PipelineOp) -> bool {
        op_id(op).map_or(false, |id| self.pipelines.contains_key(&id))
    }

    fn execute_batch(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        explicit_mode: Option<&str>,
    ) -> Result<DynamicImage, PilError> {
        let (w, h) = (img.width(), img.height());
        self.buffers.ensure_capacity(w * h);

        // 1. Upload source image → buf_a
        self.upload_rgba(img, &self.buffers.buf_a);
        let (mut current_w, mut current_h) = (w, h);
        let mut use_a = true;

        // 2. Generic ping-pong dispatch loop — works for ANY registered op
        for op in ops {
            let id = op_id(op).ok_or(PilError::ValueError("unsupported op".into()))?;
            let cached = &self.pipelines[&id];
            let params = extract_params(op);

            let (src, dst) = if use_a {
                (&self.buffers.buf_a, &self.buffers.buf_b)
            } else {
                (&self.buffers.buf_b, &self.buffers.buf_a)
            };

            if cached.is_two_pass {
                // Blur: H pass (src→dst) → V pass (dst→src)
                self.dispatch_pass(cached, src, dst, &params, current_w, current_h, Pass::Horizontal);
                self.dispatch_pass(cached, dst, src, &params, current_w, current_h, Pass::Vertical);
                // use_a unchanged (net result is back in original buffer)
            } else if cached.is_three_pass {
                // Autocontrast/Equalize: histogram → compute → remap
                self.dispatch_multi_pass(cached, src, dst, &params, current_w, current_h);
                use_a = !use_a;
            } else {
                // Single pass
                self.dispatch_pass(cached, src, dst, &params, current_w, current_h, Pass::Full);
                use_a = !use_a;
            }

            // Track dimension changes from resize ops
            if let Some((new_w, new_h)) = resize_dims(op) {
                current_w = new_w;
                current_h = new_h;
            }
        }

        // 3. Readback final buffer → DynamicImage
        let final_buf = if use_a { &self.buffers.buf_a } else { &self.buffers.buf_b };
        self.readback_to_image(final_buf, current_w, current_h)
    }
}
```

### Pipeline Fusion (unchanged from earlier design)

```
materialize() with ops: [Invert, Brightness, BoxBlur, Crop]
                            ↑────── GPU fused ──────↑  ↑CPU↑

Step 1: Upload source → buf_A
Step 2: Dispatch Invert      (buf_A → buf_B)
Step 3: Dispatch Brightness  (buf_B → buf_A)
Step 4: Dispatch BoxBlur_H   (buf_A → buf_B)
Step 5: Dispatch BoxBlur_V   (buf_B → buf_A)   ← 2-pass auto-handled
Step 6: Readback buf_A → DynamicImage
Step 7: Continue CPU pipeline with Crop
```

### materialize() Dispatch (Write Once)

```rust
// image.rs — materialize() uses ComputeBackend trait, never concrete types
pub fn materialize(&self) -> Result<DynamicImage, PilError> {
    match self {
        Image::Pipeline { source, ops, gpu_enabled, .. } => {
            let mut img = source.materialize()?;
            
            // Get backends sorted by priority (GPU > SIMD > CPU)
            let backends: &[Box<dyn ComputeBackend>] = get_backends();
            // Filter by gpu_enabled flag
            let active: Vec<_> = backends.iter()
                .filter(|b| *gpu_enabled || b.name() == "cpu")
                .collect();

            let mut i = 0;
            while i < ops.len() {
                // Find best backend for this prefix of ops
                for backend in &active {
                    let batch: Vec<_> = ops[i..]
                        .iter()
                        .take_while(|op| backend.supports(op))
                        .collect();
                    
                    if batch.len() >= 1 {
                        img = backend.execute_batch(&batch, &img, explicit_mode.as_deref())?;
                        i += batch.len();
                        break;  // Handled — continue outer loop
                    }
                }
                // No backend supports this op — fall through to CPU
            }
            Ok(img)
        }
        // ... other variants unchanged
    }
}
```

### CPU Backend (Write Once)

```rust
// compute/cpu_backend.rs — thin passthrough, never changes
pub struct CpuBackend;

impl ComputeBackend for CpuBackend {
    fn name(&self) -> &'static str { "cpu" }
    fn priority(&self) -> u8 { 0 }  // Always last
    fn supports(&self, _op: &PipelineOp) -> bool { true }  // Supports everything

    fn execute_batch(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        explicit_mode: Option<&str>,
    ) -> Result<DynamicImage, PilError> {
        // CPU doesn't benefit from fusion — sequential is fine
        let mut img = img.clone();
        for op in ops {
            img = execute_op(&img, op, explicit_mode)?;
        }
        Ok(img)
    }
}
```

### Buffer Pool (Write Once)

```rust
// compute/gpu_backend.rs (or compute/buffers.rs)
struct BufferPool {
    buf_a: wgpu::Buffer,     // Ping-pong A (STORAGE | COPY_SRC | COPY_DST)
    buf_b: wgpu::Buffer,     // Ping-pong B
    staging: wgpu::Buffer,    // GPU→CPU readback (MAP_READ | COPY_DST)
    params: wgpu::Buffer,     // 4KB scratch (UNIFORM | COPY_DST)
    capacity: u32,            // Current max pixel count
}
```

## Multi-Arch Future Support

Adding a new optimization target is **one file**:

```rust
// compute/simd_backend.rs — NEW FILE (future)
pub struct SimdBackend { /* SSE/AVX/NEON */ }

impl ComputeBackend for SimdBackend {
    fn name(&self) -> &'static str { "simd" }
    fn priority(&self) -> u8 { 50 }  // Between GPU and CPU
    fn supports(&self, op: &PipelineOp) -> bool {
        // SIMD-acceleratable ops (per-pixel, no branching)
        matches!(op, PipelineOp::Invert | PipelineOp::Grayscale | /* ... */)
    }
    fn execute_batch(&self, ops, img, mode) -> Result<DynamicImage, PilError> {
        // SSE/AVX batch processing
    }
}

// compute/cuda_backend.rs — NEW FILE (future)
pub struct CudaBackend { /* CUDA context */ }
impl ComputeBackend for CudaBackend { ... }
```

Backend selection is configuration, not code:

```rust
fn get_backends() -> &'static [Box<dyn ComputeBackend>] {
    static BACKENDS: OnceLock<Vec<Box<dyn ComputeBackend>>> = OnceLock::new();
    BACKENDS.get_or_init(|| {
        let mut v: Vec<Box<dyn ComputeBackend>> = vec![];
        if let Some(gpu) = GpuBackend::new(4096 * 4096) {
            v.push(Box::new(gpu));
        }
        // Future: if let Some(simd) = SimdBackend::new() { v.push(Box::new(simd)); }
        v.push(Box::new(CpuBackend));  // Always last — universal fallback
        v.sort_by_key(|b| std::cmp::Reverse(b.priority()));
        v
    })
}
```

## Binding Layers (Thin Wrappers)

### Python (`pillow-rs-py/src/lib.rs`)

```rust
#[pyfunction]
fn enable_gpu() -> PyResult<bool> {
    Ok(pillow_rs_core::compute::enable_gpu())
}

#[pyfunction]
fn disable_gpu() { pillow_rs_core::compute::disable_gpu(); }

#[pyfunction]
fn gpu_available() -> bool { pillow_rs_core::compute::gpu_available(); }
```

### WASM (`pillow-rs-js/src/lib.rs`)

```rust
#[wasm_bindgen] pub fn enableGpu() -> bool { pillow_rs_core::compute::enable_gpu() }
#[wasm_bindgen] pub fn disableGpu() { pillow_rs_core::compute::disable_gpu(); }
#[wasm_bindgen] pub fn gpuAvailable() -> bool { pillow_rs_core::compute::gpu_available(); }
```

## Shader Catalog (~55 WGSL files)

All shaders use unified pixel format: `array<u32>` — each u32 = packed RGBA (`r<<24 | g<<16 | b<<8 | a`). Same byte layout as PIL's internal representation. All math in i32/u32 for pixel-exact PIL parity.

### Category 1: Color Single-Input (10 shaders)

Per-pixel, 1 input → 1 output. i32/u32 math.

| Shader | Op | Math |
|--------|----|----|
| `invert.wgsl` | Invert | `out = 255 - in` per channel |
| `grayscale.wgsl` | Grayscale | `y = (299*r + 587*g + 114*b + 500) / 1000` |
| `solarize.wgsl` | Solarize | `if ch > threshold: 255 - ch` |
| `posterize.wgsl` | Posterize | Quantize to `2^bits` levels |
| `brightness.wgsl` | Brightness | `out = in * factor` (i32, clipped) |
| `contrast.wgsl` | Contrast | `out = (in - 128) * factor + 128` |
| `color_saturation.wgsl` | ColorSaturation | Luma-preserving saturation adjust |
| `colorize.wgsl` | Colorize | Lerp black→white by luma |
| `constant.wgsl` | Constant | Replace all pixels with constant value |
| `offset.wgsl` | Offset | Shift pixels by (dx, dy), wrap or fill |

### Category 2: Blend Dual-Input (16 shaders)

Per-pixel, 2 input → 1 output. i32 math matching PIL's formula order.

| Shader | Op | Formula |
|--------|----|---------|
| `multiply.wgsl` | Multiply | `(a * b + 127) / 255` |
| `screen.wgsl` | Screen | `255 - ((255-a) * (255-b) / 255)` |
| `difference.wgsl` | Difference | `abs(a - b)` |
| `add_modulo.wgsl` | AddModulo | `(a + b) % 256` |
| `subtract_modulo.wgsl` | SubtractModulo | `(a - b + 256) % 256` |
| `darker.wgsl` | Darker | `min(a, b)` |
| `lighter.wgsl` | Lighter | `max(a, b)` |
| `overlay.wgsl` | Overlay | PIL's overlay formula |
| `hard_light.wgsl` | HardLight | PIL's hard_light formula |
| `soft_light.wgsl` | SoftLight | PIL's soft_light formula |
| `logical_and.wgsl` | LogicalAnd | Bitwise AND |
| `logical_or.wgsl` | LogicalOr | Bitwise OR |
| `logical_xor.wgsl` | LogicalXor | Bitwise XOR |
| `chop_add.wgsl` | ChopAdd | `clip(a + b, 0, 255)` with scale + offset |
| `chop_subtract.wgsl` | ChopSubtract | `clip(a - b, 0, 255)` with scale + offset |
| `blend.wgsl` | Blend | `(a*alpha + b*(255-alpha)) / 255` |

### Category 3: Convolution (4 shaders)

1 input → 1 output. Row-grouped i32 accumulation matching PIL's C macros.

| Shader | Op | Kernel |
|--------|----|--------|
| `filter_3x3.wgsl` | Filter3x3 | 3×3, scale + offset |
| `filter_5x5.wgsl` | Filter5x5 | 5×5, scale + offset |
| `sharpen.wgsl` | Sharpen | Fixed 3×3: `[0,-1,0; -1,5,-1; 0,-1,0]` |
| `unsharp_mask.wgsl` | UnsharpMask | Gaussian blur + subtract → sharpen |

### Category 4: Separable Blur (2 ops, 4 passes)

Auto-detected as 2-pass by `is_multi_pass: true` in op_map. Both passes stay on GPU.

| Pass | Algorithm |
|------|-----------|
| `box_blur_h.wgsl` | Horizontal sliding window, i32 accumulator |
| `box_blur_v.wgsl` | Vertical sliding window |
| `gaussian_blur_h.wgsl` | Fractional-radius box blur (PIL's 3-pass method) |
| `gaussian_blur_v.wgsl` | Vertical pass |

### Category 5: Rank Filters (4 shaders)

Window → bitonic sort in workgroup shared memory.

| Shader | Op | Selection |
|--------|----|-----------|
| `median_filter.wgsl` | MedianFilter | n/2-th element |
| `max_filter.wgsl` | MaxFilter | Last element |
| `min_filter.wgsl` | MinFilter | First element |
| `rank_filter.wgsl` | RankFilter | k-th element from params |

### Category 6: Resample (6 shaders)

1 input → 1 output. Coordinate transform matching PIL's `(dx+0.5)*sw/dw - 0.5`.

| Shader | Filter |
|--------|--------|
| `resize_nearest.wgsl` | Nearest-neighbor |
| `resize_bilinear.wgsl` | Bilinear interpolation |
| `resize_bicubic.wgsl` | Bicubic (Catmull-Rom) |
| `resize_lanczos.wgsl` | Lanczos (3-lobed) |
| `resize_box.wgsl` | Box (area-averaging) |
| `resize_hamming.wgsl` | Hamming window |

### Category 7: Composite (3 shaders)

| Shader | Inputs |
|--------|--------|
| `paste.wgsl` | source + dest, box coordinates |
| `alpha_composite.wgsl` | source + dest + mask |
| `effect_spread.wgsl` | source, distance (i32 math) |

### Category 8: Geometry (3 shaders)

| Shader | Transform |
|--------|-----------|
| `flip.wgsl` | Vertical coordinate inversion |
| `mirror.wgsl` | Horizontal coordinate inversion |
| `duplicate.wgsl` | Identity (pipeline pass-through) |

### Category 9: Multi-Pass Histogram (2 ops, 6 passes)

Use tiny params buffer (256×4 u32 = 4KB). 3 dispatches each, zero CPU.

| Pass | Dispatch | Workgroups |
|------|----------|------------|
| `autocontrast_histogram.wgsl` | Many workgroups | `ceil(w*h / 256)` |
| `autocontrast_cutoff.wgsl` | 1 workgroup | 1 |
| `autocontrast_remap.wgsl` | Many workgroups | `ceil(w*h / 256)` |
| `equalize_histogram.wgsl` | Many workgroups | `ceil(w*h / 256)` |
| `equalize_cdf.wgsl` | 1 workgroup | 1 |
| `equalize_remap.wgsl` | Many workgroups | `ceil(w*h / 256)` |

Plus a utility shader `histogram_clear.wgsl` (1 workgroup) to zero the params buffer.

## Pixel-Exact Strategy

### Why i32/u32 instead of f32?

PIL's C code operates in **integer space**:
- Pixels: `u8` (0-255 per channel)
- Convolution accumulators: `i32` (row-grouped, exact sum order)
- Division: `(sum + scale/2) / scale` (integer division with rounding)
- Resize coordinates: `(dx + 0.5) * src_w / dst_w - 0.5` (f32 → i32 floor)

By using the same integer arithmetic in WGSL:
- **No IEEE 754 rounding differences** — integer division is deterministic across all GPUs
- **Same accumulation order** — row-grouped i32 sums match PIL's C macros
- **Same coordinate math** — matching PIL's exact pixel mapping

### Example: Filter3x3 shader

```wgsl
// Match PIL's C macro row-grouped accumulation exactly:
// Row 0: acc = k0*p0 + k1*p1 + k2*p2
// Row 1: acc += k3*p3 + k4*p4 + k5*p5
// Row 2: acc += k6*p6 + k7*p7 + k8*p8
// result = (acc + scale/2) / scale + offset

@group(0) @binding(0) var<uniform> params: FilterParams;  // kernel[9], scale, offset
@group(0) @binding(1) var<storage, read> input: array<u32>;
@group(0) @binding(2) var<storage, read_write> output: array<u32>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.y * u32(params.width) + gid.x;
    if (gid.x >= u32(params.width) || gid.y >= u32(params.height)) { return; }
    
    // Row 0
    var acc_r: i32 = i32(params.kernel[0]) * i32(get_r(idx - params.width - 1));
    var acc_g: i32 = i32(params.kernel[0]) * i32(get_g(idx - params.width - 1));
    // ... row-grouped exactly like PIL's C macro
    let half = params.scale >> 1;
    let r = u32(clamp((acc_r + half) / i32(params.scale) + i32(params.offset), 0, 255));
    // pack and write
    output[idx] = pack_rgba(r, g, b, a);
}
```

## Testing

### What stays untouched:
- `tests/conftest.py` — all fixtures, `assert_images_equal`, `assert_values_equal`
- `tests/test_fixture_parity.py` — 365 parametrized tests
- `tests/fixtures/*.json` — all 365 fixture files
- `scripts/coverage/compute_coverage.py` — auto-discovery unchanged

### GPU Tests (`tests/test_gpu.py`)

```python
@pytest.fixture(autouse=True)
def _gpu_setup():
    enable_gpu()
    yield
    disable_gpu()

@pytest.mark.covers("Image.filter", mode="RGB", target="gpu", variant="Filter3x3")
def test_gpu_filter_3x3():
    pil_img = PILImage.new("RGB", (100, 100), (128, 64, 32))
    rs_img = Image.new("RGB", (100, 100), (128, 64, 32))
    kernel = [0, -1, 0, -1, 5, -1, 0, -1, 0]
    
    pil_result = pil_img.filter(PILFilter.Kernel((3, 3), kernel, scale=1))
    rs_result = rs_img.filter("Kernel", (3, 3), kernel, scale=1)
    
    assert_images_equal(rs_result, pil_result)  # pixel-exact, zero tolerance

@pytest.mark.covers("Image.filter", target="gpu", variant="fused")
def test_gpu_pipeline_fusion():
    """Verify fused ops produce pixel-exact PIL-equivalent output."""
    pil_img = PILImage.new("RGB", (200, 200), (128, 128, 128))
    rs_img = Image.new("RGB", (200, 200), (128, 128, 128))
    
    # Chain: invert → grayscale → box_blur → sharpen
    pil_result = pil_img.copy()
    pil_result = PILOps.invert(pil_result)
    pil_result = PILOps.grayscale(pil_result)
    pil_result = pil_result.filter(PILFilter.BoxBlur(2))
    pil_result = pil_result.filter(PILFilter.SHARPEN)
    
    rs_result = rs_img.copy()
    rs_result = ImageOps.invert(rs_result)
    rs_result = ImageOps.grayscale(rs_result)
    rs_result = rs_result.filter("BoxBlur", 2)
    rs_result = rs_result.filter("SHARPEN")
    
    assert_images_equal(rs_result, pil_result)  # pixel-exact
```

### Coverage Auto-Discovery

GPU tests are auto-discovered by the existing coverage system:
1. **Static decorators**: `@pytest.mark.covers("Image.filter", target="gpu")` — parsed by regex
2. **Fixture JSONs**: `"operation": {"module": "Image", "target": "filter"}` — auto-mapped

No `coverage_map.json` entry needed. `target="gpu"` is already accepted by the coverage validator.

## Implementation Phases

### Phase 1: Core Abstraction
1. Create `compute/mod.rs` — `ComputeBackend` trait
2. Create `compute/cpu_backend.rs` — passthrough to existing `execute_op()`
3. Create `compute/registry.rs` — `OpRegistry`, `OpDef`, codegen macro
4. Create `compute/op_map.rs` — `register_gpu_ops!` macro (initially empty, populated in Phase 3)
5. Update `image.rs` — `materialize()` uses `ComputeBackend` trait
6. Update `lib.rs` — add `pub mod compute;`

### Phase 2: GPU Engine (write once)
1. Create `compute/gpu_backend.rs` — `GpuBackend` struct, buffer pool, ping-pong dispatch
2. Implement `ComputeBackend` for `GpuBackend`
3. Create `compute/gpu_backend/buffers.rs` — `BufferPool`
4. Wire backend selection: `enable_gpu()` / `disable_gpu()`

### Phase 3: Shaders + Op Map (append only)
1. Write ~55 WGSL shaders in `compute/gpu_shaders/`
2. Populate `compute/op_map.rs` — one line per operation
3. Macro auto-generates `op_id()`, `extract_params()`, `OP_REGISTRY`

### Phase 4: Binding Layers
1. `pillow-rs-py/src/lib.rs` — `enable_gpu()`, `disable_gpu()`, `gpu_available()`
2. `pillow-rs-js/src/lib.rs` — `enableGpu()`, `disableGpu()`, `gpuAvailable()`
3. Python wrapper functions in `pillow_rs/__init__.py`

### Phase 5: GPU Tests
1. Write `tests/test_gpu.py` — one test per GPU operation
2. Write `tests/test_gpu_pipeline.py` — pipeline fusion tests
3. Run coverage to verify auto-discovery

### Phase 6: Validation
1. `cargo clippy --all-targets --all-features -- -D warnings`
2. `cargo test -p pillow-rs-core`
3. `python -m pytest tests/test_gpu.py tests/test_gpu_pipeline.py -v`
4. Full suite: `python -m pytest tests/ --json-report --json-report-file=/tmp/report.json`
5. Coverage: `python scripts/coverage/compute_coverage.py manifest.yaml /tmp/report.json`

## Reference Resources

- [gpush](https://github.com/Cod-e-Codes/gpush) — working wgpu image processing toolkit
- [High Performance GPGPU with Rust and wgpu](https://dev.to/jaysmito101/high-performance-gpgpu-with-rust-and-wgpu-4l9i) — compute pipeline walkthrough
- [learn-wgpu](https://deepwiki.com/sotrh/learn-wgpu/2-wgpu-fundamentals) — canonical wgpu tutorial
- [wgpu examples](https://github.com/gfx-rs/wgpu/tree/trunk/examples) — official examples
- Puhu (`puhu/` in repo) — PIL C algorithm reference for pixel-exact math
