//! GPU worker pool — implements BackendImpl for GPU compute backend.
//!
//! Debug logging: set RSPIL_GPU_DEBUG=1 to write per-op logs to /tmp/gpu_debug.log

macro_rules! gpu_log {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        log::debug!("{}", msg);
        if std::env::var("RSPIL_GPU_DEBUG").is_ok() {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true).append(true).open("/tmp/gpu_debug.log")
            {
                let _ = writeln!(f, "{}", msg);
                let _ = f.flush();
            }
        }
    }};
}

// Uses wgpu for compute shader dispatch with packed u32 RGBA buffers
// (R|G<<8|B<<16|A<<24) and 16x16 workgroups.
// GPU init is lazy — happens on first `execute_batch` call.

use crate::checked_dims::CheckedDims;
use crate::compute::registry;
use crate::compute::{Backend, BackendImpl};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use pillow_rs_image::{DynamicImage, RgbaImage};
use std::collections::HashMap;

// ─── BufferPool ────────────────────────────────────────────────────────────

struct BufferPool {
    buf_a: wgpu::Buffer,
    buf_b: wgpu::Buffer,
    buf_img2: wgpu::Buffer, // Second image for dual-input ops
    buf_img3: wgpu::Buffer, // Third image for 3-input ops (Composite/Paste mask)
    params: wgpu::Buffer,
    lut_buf: wgpu::Buffer, // LUT storage buffer for Eval/PointOp (1024 bytes)
    capacity: u32,
}

impl BufferPool {
    fn new(device: &wgpu::Device, capacity: u32) -> Self {
        let size = (capacity * 4) as u64;
        let buf_a = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_buf_a"),
            size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let buf_b = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_buf_b"),
            size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let buf_img2 = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_buf_img2"),
            size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let buf_img3 = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_buf_img3"),
            size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let lut_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_lut"),
            size: 1024, // 256 entries * 4 bytes each
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_params"),
            size: 256 * 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        BufferPool {
            buf_a,
            buf_b,
            buf_img2,
            buf_img3,
            params,
            lut_buf,
            capacity,
        }
    }

    fn upload_rgba(&self, queue: &wgpu::Queue, rgba: &RgbaImage) -> Result<(), PilError> {
        let (w, h) = rgba.dimensions();
        let n = CheckedDims::new(w, h, 1)?.total_pixels();
        if n > self.capacity as usize {
            return Err(PilError::ValueError(format!(
                "BufferPool capacity {} < image size {}",
                self.capacity, n
            )));
        }
        let mut packed: Vec<u32> = Vec::with_capacity(n);
        for px in rgba.pixels() {
            packed.push(
                (px[0] as u32)
                    | ((px[1] as u32) << 8)
                    | ((px[2] as u32) << 16)
                    | ((px[3] as u32) << 24),
            );
        }
        queue.write_buffer(&self.buf_a, 0, bytemuck::cast_slice(&packed));
        queue.write_buffer(&self.buf_b, 0, bytemuck::cast_slice(&packed));
        Ok(())
    }

    /// Upload a second image to buf_img2 for dual-input shader ops.
    fn upload_second(&self, queue: &wgpu::Queue, rgba: &RgbaImage) {
        let (w, h) = rgba.dimensions();
        let n = (w * h) as usize;
        let mut packed: Vec<u32> = Vec::with_capacity(n);
        for px in rgba.pixels() {
            packed.push(
                (px[0] as u32)
                    | ((px[1] as u32) << 8)
                    | ((px[2] as u32) << 16)
                    | ((px[3] as u32) << 24),
            );
        }
        queue.write_buffer(&self.buf_img2, 0, bytemuck::cast_slice(&packed));
    }

    /// Upload a third image to buf_img3 for 3-input shader ops (Composite mask).
    fn upload_third(&self, queue: &wgpu::Queue, rgba: &RgbaImage) {
        let (w, h) = rgba.dimensions();
        let n = (w * h) as usize;
        let mut packed: Vec<u32> = Vec::with_capacity(n);
        for px in rgba.pixels() {
            packed.push(
                (px[0] as u32)
                    | ((px[1] as u32) << 8)
                    | ((px[2] as u32) << 16)
                    | ((px[3] as u32) << 24),
            );
        }
        queue.write_buffer(&self.buf_img3, 0, bytemuck::cast_slice(&packed));
    }

    /// Upload LUT data to the storage buffer for Eval/PointOp shaders.
    fn upload_lut(&self, queue: &wgpu::Queue, lut: &[u32; 256]) {
        queue.write_buffer(&self.lut_buf, 0, bytemuck::cast_slice(&lut[..]));
    }

    fn upload_params(&self, queue: &wgpu::Queue, params: &[u32], w: u32, h: u32, mode: u32) {
        // Uniform buffer layout: [w, h, mode, pad0] + params (tightly packed u32s)
        // mode: 0=L, 1=LA, 2=RGB, 3=RGBA
        // Each u32 field has 4-byte alignment in WGSL uniform address space,
        // so tightly-packed u32s work fine. No padding needed.
        let mut buf = vec![w, h, mode, 0u32];
        buf.extend_from_slice(params);
        queue.write_buffer(&self.params, 0, bytemuck::cast_slice(&buf));
    }
}

// ─── Count shader bindings ─────────────────────────────────────────────────

/// Count the number of `@binding(N)` entries in WGSL source.
/// Returns max binding index + 1, or 0 if no bindings found.
fn count_shader_bindings(source: &str) -> u32 {
    let mut max_binding: i32 = -1;
    for line in source.lines() {
        if let Some(pos) = line.find("@binding(") {
            let rest = &line[pos + 9..];
            if let Some(end) = rest.find(')') {
                if let Ok(n) = rest[..end].trim().parse::<u32>() {
                    max_binding = max_binding.max(n as i32);
                }
            }
        }
    }
    if max_binding >= 0 {
        max_binding as u32 + 1
    } else {
        0
    }
}

/// Detect if a 4-binding shader uses the LUT layout (Eval/PointOp).
/// In LUT layout, `@binding(1)` is `storage read_write` (output).
/// In dual-input layout, `@binding(1)` is `storage read` (input_b).
fn is_lut_shader(source: &str) -> bool {
    for line in source.lines() {
        if line.contains("@binding(1)") {
            return line.contains("read_write");
        }
    }
    false
}

// ─── CachedPipeline ────────────────────────────────────────────────────────

struct CachedPipeline {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    variant_name: &'static str,
    /// Number of bindings in this shader (2-5).
    num_bindings: u32,
    /// True if this is a 4-binding LUT shader (Eval/PointOp).
    is_lut: bool,
}

// ─── GpuInner (lazy-initialized GPU engine) ────────────────────────────────

/// Internal GPU engine. Initialized once and stored in a static OnceLock.
struct GpuInner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    buffers: BufferPool,
    pipelines: HashMap<String, CachedPipeline>,
}

impl GpuInner {
    fn new() -> Option<Self> {
        let instance = wgpu::Instance::default();
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))?;
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("pillow-rs-gpu"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .ok()?;

        let capacity = 4096 * 4096;
        let buffers = BufferPool::new(&device, capacity);

        // Pre-compile all GPU shaders from the unified registry.
        // Shaders that fail validation are silently skipped.
        let mut pipelines = HashMap::new();
        for (&key, entry) in registry::registry().iter() {
            if let Some(source) = entry.gpu_source {
                gpu_log!("[GPU] compiling shader: {}", key);
                if let Some(pipeline) = Self::build_pipeline(&device, key, source) {
                    gpu_log!("[GPU]   -> OK ({} bindings)", pipeline.num_bindings);
                    pipelines.insert(key.to_string(), pipeline);
                } else {
                    gpu_log!("[GPU]   -> SKIPPED (validation failed)");
                }
            }
        }
        gpu_log!("[GPU] total compiled: {} pipelines", pipelines.len());

        Some(GpuInner {
            device,
            queue,
            buffers,
            pipelines,
        })
    }

    fn build_pipeline(
        device: &wgpu::Device,
        variant_name: &'static str,
        shader_source: &str,
    ) -> Option<CachedPipeline> {
        let cs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(variant_name),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let num_bindings = count_shader_bindings(shader_source);

        // Supported: 2-6 binding shaders. 0/1/>6 are invalid.
        if !(2..=6).contains(&num_bindings) {
            return None;
        }

        // Detect if this is a LUT shader (Eval/PointOp) with 4 bindings.
        let is_lut = num_bindings == 4 && is_lut_shader(shader_source);

        // Build bind group layout matching shader declarations.
        // Layout depends on binding count and LUT variant:
        //   2: [input(read), output(read_write)]
        //   3: [input(read), output(read_write), params(uniform)]
        //   4 (dual-input): [input_a(read), input_b(read), output(read_write), params(uniform)]
        //   4 (LUT):        [input(read), output(read_write), params(uniform), lut(read)]
        //   5: [input_a(read), input_b(read), input_c(read), output(read_write), params(uniform)]
        let mut bindings = Vec::with_capacity(num_bindings as usize);

        if num_bindings == 5 {
            // 5-binding: 3 inputs + output + params (Composite/CompositeModule/Paste)
            for i in 0..3 {
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: i,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
            }
            bindings.push(wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
            bindings.push(wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
        } else if num_bindings == 4 {
            if is_lut {
                // LUT layout: [input(read), output(rw), params(uniform), lut(read)]
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
            } else {
                // Dual-input layout
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
            }
        } else {
            // 2 or 3 binding (single-input) layout
            bindings.push(wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
            bindings.push(wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });

            if num_bindings > 2 {
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
            }
        }

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(variant_name),
            entries: &bindings,
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(variant_name),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Use error scope to catch shader validation errors without panicking.
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(variant_name),
            layout: Some(&pipeline_layout),
            module: &cs_module,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // If validation failed, skip this shader — it won't be available on GPU.
        if pollster::block_on(device.pop_error_scope()).is_some() {
            return None;
        }

        Some(CachedPipeline {
            pipeline,
            bind_group_layout,
            variant_name,
            num_bindings,
            is_lut,
        })
    }

    fn make_bind_group(
        &self,
        cached: &CachedPipeline,
        input_buf: &wgpu::Buffer,
        output_buf: &wgpu::Buffer,
        img2_buf: Option<&wgpu::Buffer>,
        img3_buf: Option<&wgpu::Buffer>,
    ) -> wgpu::BindGroup {
        let mut entries = Vec::with_capacity(cached.num_bindings as usize);
        match (cached.num_bindings, cached.is_lut) {
            (5, _) => {
                // 5-binding: [in_a(read), in_b(read), in_c(read), out(rw), params(uniform)]
                entries.push(wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input_buf.as_entire_binding(),
                });
                let second = img2_buf.unwrap_or(output_buf);
                entries.push(wgpu::BindGroupEntry {
                    binding: 1,
                    resource: second.as_entire_binding(),
                });
                let third = img3_buf.unwrap_or(output_buf);
                entries.push(wgpu::BindGroupEntry {
                    binding: 2,
                    resource: third.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 3,
                    resource: output_buf.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.buffers.params.as_entire_binding(),
                });
            }
            (4, true) => {
                // LUT layout: [input(read), output(rw), params(uniform), lut(read)]
                entries.push(wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input_buf.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output_buf.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.buffers.params.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.buffers.lut_buf.as_entire_binding(),
                });
            }
            (4, false) => {
                // Dual-input layout: [input_a(read), input_b(read), output(rw), params(uniform)]
                entries.push(wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input_buf.as_entire_binding(),
                });
                let second = img2_buf.unwrap_or(output_buf);
                entries.push(wgpu::BindGroupEntry {
                    binding: 1,
                    resource: second.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 2,
                    resource: output_buf.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.buffers.params.as_entire_binding(),
                });
            }
            _ => {
                // 2 or 3 binding: [input(read), output(rw), ...params(uniform)]
                entries.push(wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input_buf.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output_buf.as_entire_binding(),
                });
                if cached.num_bindings > 2 {
                    entries.push(wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.buffers.params.as_entire_binding(),
                    });
                }
            }
        }
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(cached.variant_name),
            layout: &cached.bind_group_layout,
            entries: &entries,
        })
    }

    fn dispatch_pass(
        &self,
        cached: &CachedPipeline,
        w: u32,
        h: u32,
        input_buf: &wgpu::Buffer,
        output_buf: &wgpu::Buffer,
        img2_buf: Option<&wgpu::Buffer>,
        img3_buf: Option<&wgpu::Buffer>,
    ) {
        let bind_group = self.make_bind_group(cached, input_buf, output_buf, img2_buf, img3_buf);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("dispatch_pass"),
            });
        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("dispatch_pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&cached.pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            let wgs_x = w.div_ceil(16);
            let wgs_y = h.div_ceil(16);
            cpass.dispatch_workgroups(wgs_x, wgs_y, 1);
        }
        self.queue.submit(Some(encoder.finish()));
        // Drain immediately: prevents ActiveSubmission queue accumulation across
        // hundreds of sequential tests. Without this, wgpu's internal submission
        // tracking overflows and poll(Wait) deadlocks.
        self.device.poll(wgpu::Maintain::Wait);
    }

    fn readback_to_image(
        &self,
        w: u32,
        h: u32,
        final_is_a: bool,
    ) -> Result<DynamicImage, PilError> {
        let src = if final_is_a {
            &self.buffers.buf_a
        } else {
            &self.buffers.buf_b
        };
        let size = CheckedDims::new(w, h, 4)?.total_bytes() as u64;

        // TEST: skip pool, create fresh staging buffer each time
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        gpu_log!("[GPU] readback: create_encoder start");
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        gpu_log!("[GPU] readback: encoder created, copy_buffer_to_buffer start");
        encoder.copy_buffer_to_buffer(src, 0, &staging, 0, size);
        self.queue.submit(Some(encoder.finish()));
        gpu_log!("[GPU] readback: copy submitted, map_async start");

        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        gpu_log!("[GPU] readback: map_async called, poll start");
        self.device.poll(wgpu::Maintain::Wait);
        // Double-poll: single Wait may not drain all pending GPU work when
        // the queue has 300+ accumulated operations. Second poll ensures
        // the staging buffer is fully released before returning to pool.
        self.device.poll(wgpu::Maintain::Wait);
        gpu_log!("[GPU] readback: poll done, recv start");
        rx.recv()
            .map_err(|_| PilError::ValueError("readback channel closed".into()))?
            .map_err(|e| PilError::ValueError(format!("map_async failed: {:?}", e)))?;
        gpu_log!("[GPU] readback: recv done, get_mapped_range start");

        let data = slice.get_mapped_range().to_vec();
        gpu_log!("[GPU] readback: got {} bytes, unmap start", data.len());
        let _ = slice;
        staging.unmap();
        gpu_log!("[GPU] readback: unmap done, final poll start");
        self.device.poll(wgpu::Maintain::Wait);
        // Double-poll: single Wait may not drain all pending GPU work when
        // the queue has 300+ accumulated operations. Second poll ensures
        // the staging buffer is fully released before returning to pool.
        self.device.poll(wgpu::Maintain::Wait);
        gpu_log!("[GPU] readback: final poll done");
        // wgpu#5173: empty submit flushes pending writes.
        self.queue.submit([]);
        self.device.poll(wgpu::Maintain::Wait);

        let n = CheckedDims::new(w, h, 1)?.total_pixels();
        let mut rgba_bytes = Vec::with_capacity(n * 4);
        for &pixel in bytemuck::cast_slice::<u8, u32>(&data)[..n].iter() {
            rgba_bytes.push((pixel & 0xff) as u8);
            rgba_bytes.push(((pixel >> 8) & 0xff) as u8);
            rgba_bytes.push(((pixel >> 16) & 0xff) as u8);
            rgba_bytes.push(((pixel >> 24) & 0xff) as u8);
        }

        let img = RgbaImage::from_raw(w, h, rgba_bytes)
            .ok_or_else(|| PilError::ValueError("bad readback buffer".into()))?;
        Ok(DynamicImage::ImageRgba8(img))
    }

    fn execute_batch_impl(
        &self,
        ops: &[PipelineOp],
        w: u32,
        h: u32,
        mode: u32,
    ) -> Result<(bool, u32, u32), PilError> {
        // Pre-materialize second images to avoid interleaved CPU decode during GPU dispatch.
        // Each dual-input op gets its second image materialized upfront; upload to buf_img2
        // happens right before dispatch (CPU→GPU, not a round trip).
        let second_images: Vec<Option<DynamicImage>> =
            ops.iter().map(extract_second_image).collect();

        // Pre-materialize third images for 3-input ops (Composite/Paste mask).
        let third_images: Vec<Option<DynamicImage>> = ops.iter().map(extract_third_image).collect();

        let mut current_is_a = true;
        let mut cur_w = w;
        let mut cur_h = h;
        gpu_log!(
            "[GPU] batch_impl: {} ops, start dims {}x{}",
            ops.len(),
            cur_w,
            cur_h
        );
        for (i, op) in ops.iter().enumerate() {
            let base_key = registry::variant_key(op);
            gpu_log!("[GPU] batch_impl: op[{}/{}]={}", i + 1, ops.len(), base_key);

            let cached = self.pipelines.get(base_key).ok_or_else(|| {
                PilError::ValueError(format!("GpuPool: no compiled pipeline for '{}'", base_key))
            })?;

            let params = registry::extract_params(op);

            // Pre-compute output dimensions BEFORE dispatch so workgroup count
            // covers the full output image for size-changing ops.
            let out_w = op_output_dims(op, cur_w, cur_h)
                .map(|(w, _)| w)
                .unwrap_or(cur_w);
            let out_h = op_output_dims(op, cur_w, cur_h)
                .map(|(_, h)| h)
                .unwrap_or(cur_h);

            // Append output dimensions as the last params — shaders that declare
            // dst_w/dst_h at the end of their Params struct can read them.
            // Existing shaders ignore extra uniform data after their struct.
            let mut extended = params;
            extended.push(out_w);
            extended.push(out_h);
            self.buffers
                .upload_params(&self.queue, &extended, cur_w, cur_h, mode);

            let (input_buf, output_buf) = if current_is_a {
                (&self.buffers.buf_a, &self.buffers.buf_b)
            } else {
                (&self.buffers.buf_b, &self.buffers.buf_a)
            };

            // Upload pre-materialized second image to buf_img2 if this is a dual-input op.
            let img2_buf: Option<&wgpu::Buffer> = if let Some(ref second) = second_images[i] {
                let second_rgba = second.to_rgba8();
                self.buffers.upload_second(&self.queue, &second_rgba);
                Some(&self.buffers.buf_img2)
            } else {
                None
            };

            // Upload pre-materialized third image to buf_img3 for 3-input ops.
            let img3_buf: Option<&wgpu::Buffer> = if let Some(ref third) = third_images[i] {
                let third_rgba = third.to_rgba8();
                self.buffers.upload_third(&self.queue, &third_rgba);
                Some(&self.buffers.buf_img3)
            } else {
                None
            };

            // Upload LUT data for Eval/PointOp shaders before dispatch.
            if cached.is_lut {
                if let Some(lut_data) = extract_lut(op) {
                    self.buffers.upload_lut(&self.queue, &lut_data);
                }
            }

            self.dispatch_pass(
                cached, out_w, out_h, input_buf, output_buf, img2_buf, img3_buf,
            );
            current_is_a = !current_is_a;

            // Update dimensions after size-changing ops.
            cur_w = out_w;
            cur_h = out_h;
        }
        // After N ops, current_is_a tracks where the latest result lives:
        //   true → buf_a has the final result, false → buf_b
        Ok((current_is_a, cur_w, cur_h))
    }
}

static GPU: std::sync::OnceLock<GpuInner> = std::sync::OnceLock::new();

// ─── Mode helpers ───────────────────────────────────────────────────────────

/// Map a DynamicImage variant to its mode code for the GPU uniform buffer.
/// 0 = L (1 channel), 1 = LA (2 channels), 2 = RGB (3 channels), 3 = RGBA (4 channels)
fn mode_code(img: &DynamicImage) -> u32 {
    match img {
        DynamicImage::ImageLuma8(_) => 0,
        DynamicImage::ImageLumaA8(_) => 1,
        DynamicImage::ImageRgb8(_) => 2,
        DynamicImage::ImageRgba8(_) => 3,
        _ => 3, // fallback: treat as RGBA
    }
}

/// Extract the second (right-hand) image from a dual-input PipelineOp, if present.
/// Returns the materialized DynamicImage ready for GPU upload.
fn extract_second_image(op: &PipelineOp) -> Option<DynamicImage> {
    let arc_img: Option<&std::sync::Arc<crate::image::Image>> = match op {
        PipelineOp::Add { other, .. }
        | PipelineOp::Subtract { other, .. }
        | PipelineOp::Multiply { other }
        | PipelineOp::Screen { other }
        | PipelineOp::Darker { other }
        | PipelineOp::Lighter { other }
        | PipelineOp::Difference { other }
        | PipelineOp::Overlay { other }
        | PipelineOp::HardLight { other }
        | PipelineOp::SoftLight { other }
        | PipelineOp::AddModulo { other }
        | PipelineOp::SubtractModulo { other }
        | PipelineOp::LogicalAnd { other }
        | PipelineOp::LogicalOr { other }
        | PipelineOp::LogicalXor { other }
        | PipelineOp::Blend { other, .. }
        | PipelineOp::Composite { other, .. }
        | PipelineOp::BlendModule { other, .. }
        | PipelineOp::CompositeModule { other, .. } => Some(other),
        PipelineOp::Paste { source, .. } | PipelineOp::AlphaComposite { source, .. } => {
            Some(source)
        }
        _ => None,
    };
    arc_img.and_then(|img| img.materialize().ok())
}

/// Extract the third image (mask) from a 3-input PipelineOp, if present.
/// Returns the materialized DynamicImage ready for GPU upload.
fn extract_third_image(op: &PipelineOp) -> Option<DynamicImage> {
    let arc_img: Option<&std::sync::Arc<crate::image::Image>> = match op {
        PipelineOp::Composite { mask, .. } | PipelineOp::CompositeModule { mask, .. } => Some(mask),
        PipelineOp::Paste { mask, .. } => mask.as_ref(),
        _ => None,
    };
    arc_img.and_then(|img| img.materialize().ok())
}

/// Extract and pack LUT data from a PipelineOp into [u32; 256] for GPU upload.
/// Each u32 packs RGBA channels for one LUT entry (R in byte 0, G byte 1, B byte 2, A byte 3).
fn extract_lut(op: &PipelineOp) -> Option<[u32; 256]> {
    let lut_bytes: &[u8] = match op {
        PipelineOp::Eval { lut } | PipelineOp::PointOp { lut } => lut.as_slice(),
        PipelineOp::RemapPalette { dest_map } => dest_map.as_slice(),
        _ => return None,
    };
    let mut packed = [0u32; 256];
    if lut_bytes.len() == 256 {
        // Single-channel LUT: replicate value across all four bytes.
        for i in 0..256 {
            let v = lut_bytes[i] as u32;
            packed[i] = v | (v << 8) | (v << 16) | (v << 24);
        }
    } else if lut_bytes.len() >= 1024 {
        // Packed RGBA LUT (256 entries * 4 bytes each).
        for (i, p) in packed.iter_mut().enumerate() {
            let base = i * 4;
            *p = (lut_bytes[base] as u32)
                | ((lut_bytes[base + 1] as u32) << 8)
                | ((lut_bytes[base + 2] as u32) << 16)
                | ((lut_bytes[base + 3] as u32) << 24);
        }
    }
    Some(packed)
}

/// Compute output dimensions for a size-changing op given current input dimensions.
/// Returns `None` if the op does not change the image dimensions.
fn op_output_dims(op: &PipelineOp, cur_w: u32, cur_h: u32) -> Option<(u32, u32)> {
    match op {
        PipelineOp::Resize { w, h, .. } => Some((*w.max(&1), *h.max(&1))),
        PipelineOp::Pad { w, h, .. } => Some((*w.max(&1), *h.max(&1))),
        PipelineOp::Crop {
            left,
            top,
            right,
            bottom,
        } => {
            let new_w = right.saturating_sub(*left).max(1);
            let new_h = bottom.saturating_sub(*top).max(1);
            Some((new_w, new_h))
        }
        PipelineOp::Expand { border, .. } => {
            let new_w = (cur_w + 2 * border).max(1);
            let new_h = (cur_h + 2 * border).max(1);
            Some((new_w, new_h))
        }
        PipelineOp::CropBorder { border } => {
            let new_w = cur_w.saturating_sub(2 * border).max(1);
            let new_h = cur_h.saturating_sub(2 * border).max(1);
            Some((new_w, new_h))
        }
        PipelineOp::Rotate { angle, expand, .. } => {
            if *expand {
                let (sw, sh) = (cur_w as f64, cur_h as f64);
                let rad = angle.to_radians();
                let (cos_a, sin_a) = (rad.cos(), rad.sin());
                let corners = [(0.0, 0.0), (sw, 0.0), (sw, sh), (0.0, sh)];
                let (mut min_x, mut min_y, mut max_x, mut max_y) =
                    (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
                for &(cx, cy) in &corners {
                    let rx = cx * cos_a - cy * sin_a;
                    let ry = cx * sin_a + cy * cos_a;
                    min_x = min_x.min(rx);
                    max_x = max_x.max(rx);
                    min_y = min_y.min(ry);
                    max_y = max_y.max(ry);
                }
                let dw = (max_x - min_x).ceil() as u32;
                let dh = (max_y - min_y).ceil() as u32;
                Some((dw.max(1), dh.max(1)))
            } else {
                Some((cur_w, cur_h))
            }
        }
        PipelineOp::Reduce { factor } if *factor > 0 => {
            Some(((cur_w / factor).max(1), (cur_h / factor).max(1)))
        }
        PipelineOp::Scale { factor, .. } => {
            let new_w = (cur_w as f64 * factor).round().max(1.0) as u32;
            let new_h = (cur_h as f64 * factor).round().max(1.0) as u32;
            Some((new_w, new_h))
        }
        _ => None,
    }
}

// ─── GpuPool ───────────────────────────────────────────────────────────────

/// GPU compute pool — wgpu-based compute shader dispatch.
///
/// Uses packed u32 RGBA and 16x16 workgroups. GPU is lazily initialized
/// on first execution. If wgpu is unavailable, execute_batch returns an error.
pub struct GpuPool;

impl GpuPool {
    fn ensure_init() -> Result<&'static GpuInner, PilError> {
        GPU.get_or_init(|| {
            GpuInner::new().expect("Failed to initialize GPU: wgpu adapter or device unavailable")
        });
        GPU.get()
            .ok_or_else(|| PilError::ValueError("GPU not available".into()))
    }
}

// ─── BackendImpl ───────────────────────────────────────────────────────────

impl BackendImpl for GpuPool {
    fn name(&self) -> Backend {
        Backend::Gpu
    }

    fn priority(&self) -> u8 {
        100
    }

    fn supports(&self, op: &PipelineOp) -> bool {
        registry::gpu_supports(op)
    }

    fn execute_batch(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        _mode: Option<&str>,
    ) -> Result<DynamicImage, PilError> {
        let gpu = Self::ensure_init()?;
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let mcode = mode_code(img);
        let op_keys: Vec<&str> = ops.iter().map(|op| registry::variant_key(op)).collect();
        log::debug!(
            "[GPU] {} op(s) {}x{} mode={}: {:?}",
            ops.len(),
            w,
            h,
            mcode,
            op_keys
        );
        gpu_log!("[GPU] step=upload_rgba start");
        gpu.buffers.upload_rgba(&gpu.queue, &rgba)?;
        gpu_log!("[GPU] step=upload_rgba done");
        gpu_log!("[GPU] step=execute_batch_impl start");
        let (final_is_a, final_w, final_h) = gpu.execute_batch_impl(ops, w, h, mcode)?;
        gpu_log!(
            "[GPU] step=execute_batch_impl done final=({},{}) is_a={}",
            final_w,
            final_h,
            final_is_a
        );
        // Ensure GPU is done before readback
        gpu_log!("[GPU] step=poll before readback");
        gpu.device.poll(wgpu::Maintain::Wait);
        gpu_log!("[GPU] step=poll done, readback start");
        let result = gpu.readback_to_image(final_w, final_h, final_is_a)?;
        gpu_log!("[GPU] step=readback done");
        // Detect mode-changing ops that need output mode override.
        // Grayscale: always outputs L, regardless of input mode.
        // Convert: output matches target mode (handled by CPU fallback for now).
        let out_mode: Option<pillow_rs_image::ColorType> =
            if ops.iter().any(|op| matches!(op, PipelineOp::Grayscale)) {
                Some(pillow_rs_image::ColorType::L8)
            } else {
                None
            };
        if let Some(ct) = out_mode {
            // Bypass preserve_mode — use the override color type directly
            match ct {
                pillow_rs_image::ColorType::L8 => Ok(DynamicImage::ImageLuma8(result.to_luma8())),
                pillow_rs_image::ColorType::La8 => {
                    Ok(DynamicImage::ImageLumaA8(result.to_luma_alpha8()))
                }
                pillow_rs_image::ColorType::Rgb8 => Ok(DynamicImage::ImageRgb8(result.to_rgb8())),
                pillow_rs_image::ColorType::Rgba8 => {
                    Ok(DynamicImage::ImageRgba8(result.to_rgba8()))
                }
                _ => Ok(crate::image::preserve_mode(img, result)),
            }
        } else {
            Ok(crate::image::preserve_mode(img, result))
        }
    }
}
