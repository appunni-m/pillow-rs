// Canonical GPU pool implementation — annotated with design rationale.
// This file illustrates every key design decision in the GPU backend,
// serving as a template for future backends (SIMD, WebGPU-native, etc.).
//
// Located at: pillow-rs/src/compute/pool_gpu/mod.rs

// ═══════════════════════════════════════════════════════════════════════════
// DESIGN DECISION 1: BufferPool with ping-pong double-buffering
// ═══════════════════════════════════════════════════════════════════════════
//
// Two storage buffers (buf_a, buf_b) alternate as input/output between ops.
// This avoids a GPU→CPU round trip after every operation in a pipeline chain.
// Data stays on GPU for the ENTIRE pipeline; readback happens only once at the end.
//
// buf_img2 is a dedicated buffer for the second image in dual-input ops
// (blend, composite, add, subtract, etc.). Uploaded right before dispatch.
//
// params is a uniform buffer for the Params struct (width, height, mode, _pad
// plus operation-specific parameters). Sized at 256*16 = 4096 bytes to
// accommodate the largest param set (Filter5x5: 25 f32 values + offset).

struct BufferPool {
    buf_a: wgpu::Buffer,
    buf_b: wgpu::Buffer,
    buf_img2: wgpu::Buffer,
    params: wgpu::Buffer,
    capacity: u32,   // max pixels (4096*4096 = 16M)
}

// ═══════════════════════════════════════════════════════════════════════════
// DESIGN DECISION 2: upload_rgba writes to BOTH buffers
// ═══════════════════════════════════════════════════════════════════════════
//
// Both buf_a and buf_b start with identical data. This is correct because:
// - After first op: input=which, output=!which. The output now has the result.
// - After flip: the new "which" points to the result buffer.
// - The other buffer still has old data, but it becomes the OUTPUT for the
//   next op (overwritten entirely), so stale data doesn't matter.
//
// Writing to both buffers ensures the first op can read from either one.

fn upload_rgba(&self, queue: &wgpu::Queue, rgba: &RgbaImage) {
    let mut packed = Vec::with_capacity((w*h) as usize);
    for px in rgba.pixels() {
        packed.push((px[0] as u32) | ((px[1] as u32) << 8)
                  | ((px[2] as u32) << 16) | ((px[3] as u32) << 24));
    }
    queue.write_buffer(&self.buf_a, 0, bytemuck::cast_slice(&packed));
    queue.write_buffer(&self.buf_b, 0, bytemuck::cast_slice(&packed));
}

// ═══════════════════════════════════════════════════════════════════════════
// DESIGN DECISION 3: Standard uniform buffer layout
// ═══════════════════════════════════════════════════════════════════════════
//
// Layout: [width: u32, height: u32, mode: u32, _pad: u32, ...op_params]
// - width/height: image dimensions for bounds checking
// - mode: 0=L, 1=LA, 2=RGB, 3=RGBA — drives channel selection in shaders
// - _pad: keeps 16-byte alignment (WGSL uniform address space)
// - op_params: operation-specific (threshold for solarize, kernel for filter, etc.)
//
// This layout is IDENTICAL across ALL shaders — the first 4 fields never change.
// Mode is placed at offset 8 (not 0) because width/height are needed for bounds
// checking in the first line of every main() function.

fn upload_params(&self, queue: &wgpu::Queue, params: &[u32], w: u32, h: u32, mode: u32) {
    let mut buf = vec![w, h, mode, 0u32];
    buf.extend_from_slice(params);
    queue.write_buffer(&self.params, 0, bytemuck::cast_slice(&buf));
}

// ═══════════════════════════════════════════════════════════════════════════
// DESIGN DECISION 4: count_shader_bindings parses WGSL source
// ═══════════════════════════════════════════════════════════════════════════
//
// Instead of hardcoding binding counts per op, this function scans the WGSL
// source for @binding(N) annotations. This makes the build_pipeline function
// adaptive: any shader with 2-4 bindings is supported automatically.
// Shaders with 0, 1, or 5+ bindings are skipped (return None).

fn count_shader_bindings(source: &str) -> u32 { /* ... */ }

// ═══════════════════════════════════════════════════════════════════════════
// DESIGN DECISION 5: Shader validation via error scope
// ═══════════════════════════════════════════════════════════════════════════
//
// wgpu shader module creation is lazy (no validation). Validation happens at
// compute pipeline creation. push_error_scope catches validation errors so a
// single bad shader doesn't crash the entire GPU init.
//
// IMPLICATION: A shader that fails validation is SILENTLY unavailable on GPU.
// The op falls back to CPU because the pipeline isn't in the HashMap.
//
// Debug by printing pipeline count after init — should match gpu_entry count.

fn build_pipeline(device, variant_name, shader_source) -> Option<CachedPipeline> {
    let cs_module = device.create_shader_module(...);
    let num_bindings = count_shader_bindings(shader_source);
    if num_bindings < 2 || num_bindings > 4 { return None; }

    device.push_error_scope(wgpu::ErrorFilter::Validation);
    let pipeline = device.create_compute_pipeline(...);
    if pollster::block_on(device.pop_error_scope()).is_some() {
        return None; // Validation failed — skip this shader
    }
    Some(CachedPipeline { pipeline, bind_group_layout, variant_name, num_bindings })
}

// ═══════════════════════════════════════════════════════════════════════════
// DESIGN DECISION 6: Ping-pong buffer tracking
// ═══════════════════════════════════════════════════════════════════════════
//
// current_is_a tracks WHICH buffer holds the latest result.
// Before ops: current_is_a = true (input is in buf_a).
// After each op: input = current buffer, output = other buffer, flip.
//
// CRITICAL: Return Ok(current_is_a), NOT Ok(!current_is_a).
// Bug was present in the original code — after 1 op, current_is_a=false
// (result in buf_b), but original returned !false=true (read buf_a — WRONG).
//
// Trace for 1 op:  start=true → output=buf_b → flip → current=false
//   return current (=false) → read buf_b ✓ (has the result)
// Trace for 2 ops: start=true → op1:b→flip→false → op2:a→flip→true
//   return current (=true) → read buf_a ✓ (has the final result)

fn execute_batch_impl(&self, ops, w, h, mode) -> Result<bool, PilError> {
    let mut current_is_a = true;
    for op in ops {
        let (input_buf, output_buf) = if current_is_a {
            (&self.buffers.buf_a, &self.buffers.buf_b)
        } else {
            (&self.buffers.buf_b, &self.buffers.buf_a)
        };
        // ... dispatch compute from input_buf to output_buf ...
        current_is_a = !current_is_a;
    }
    Ok(current_is_a) // NOT !current_is_a — this was the critical fix
}

// ═══════════════════════════════════════════════════════════════════════════
// DESIGN DECISION 7: Pre-materialize second images
// ═══════════════════════════════════════════════════════════════════════════
//
// Before the dispatch loop, scan all ops and materialize any dual-input
// second images. This avoids interleaved CPU work (decode + to_rgba8 + pack)
// during the GPU dispatch loop, keeping the GPU path clean.
//
// The upload itself (write_buffer) is CPU→GPU and happens per-op, which is
// acceptable since it doesn't involve a GPU→CPU round trip.

fn execute_batch_impl(...) {
    let second_images: Vec<Option<DynamicImage>> = ops.iter()
        .map(|op| extract_second_image(op))
        .collect();
    // ... dispatch loop: if second_images[i].is_some(), upload to buf_img2 ...
}

// ═══════════════════════════════════════════════════════════════════════════
// DESIGN DECISION 8: eprintln! for GPU execution logging
// ═══════════════════════════════════════════════════════════════════════════
//
// The `log` crate requires a logger backend (env_logger, console_log, etc.)
// which isn't initialized by default in Python/WASM environments. Using
// eprintln! directly writes to stderr, which is captured by pytest with
// --capture=no and visible in both native and WASM console.
//
// Message format: "[GPU] N op(s) WxH mode=M: [\"Op1\", \"Op2\", ...]"

// ═══════════════════════════════════════════════════════════════════════════
// DESIGN DECISION 9: Lazy GPU initialization via OnceLock
// ═══════════════════════════════════════════════════════════════════════════
//
// GPU is initialized on first use, not at import time. This avoids:
// - Startup cost when GPU isn't needed
// - Crash on machines without GPU when tests use CPU fallback
// - WASM issues with early wgpu initialization

static GPU: std::sync::OnceLock<GpuInner> = std::sync::OnceLock::new();

impl GpuPool {
    fn ensure_init() -> Result<&'static GpuInner, PilError> {
        GPU.get_or_init(|| GpuInner::new().expect("GPU init failed"));
        GPU.get().ok_or_else(|| PilError::ValueError("GPU not available".into()))
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// DESIGN DECISION 10: 16x16 workgroup size
// ═══════════════════════════════════════════════════════════════════════════
//
// 16×16 = 256 threads per workgroup is the sweet spot for image processing:
// - Fits within WGSL's max workgroup size (256 invocations)
// - Powers of 2 for efficient dispatch
// - Matches common tile sizes for cache efficiency
// - warp/wavefront-friendly on all GPU architectures
//
// dispatch_workgroups(w.div_ceil(16), h.div_ceil(16), 1) covers the full image.
