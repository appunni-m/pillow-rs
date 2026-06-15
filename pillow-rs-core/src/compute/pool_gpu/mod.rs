//! GPU worker pool — implements BackendImpl for GPU compute backend.
//!
//! Uses wgpu for compute shader dispatch with packed u32 RGBA buffers
//! (R|G<<8|B<<16|A<<24) and 16x16 workgroups.
//!
//! GPU initialization is lazy — happens on first `execute_batch` call.
//! If wgpu cannot initialize, `execute_batch` returns an error.
//!
//! ## Supported shader binding patterns
//! - 2-bindings: input (storage read), output (storage read_write) — no params
//! - 3-bindings: input, output, params (uniform) — standard single-input ops
//! - 4-bindings: input_a, input_b, output, params — dual-input ops with second image

use crate::compute::registry;
use crate::compute::{Backend, BackendImpl};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use image::{DynamicImage, RgbaImage};
use std::collections::HashMap;

// ─── BufferPool ────────────────────────────────────────────────────────────

struct BufferPool {
    buf_a: wgpu::Buffer,
    buf_b: wgpu::Buffer,
    buf_img2: wgpu::Buffer, // Second image for dual-input ops
    params: wgpu::Buffer,
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
            params,
            capacity,
        }
    }

    fn upload_rgba(&self, queue: &wgpu::Queue, rgba: &RgbaImage) -> Result<(), PilError> {
        let (w, h) = rgba.dimensions();
        let n = (w * h) as usize;
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

// ─── CachedPipeline ────────────────────────────────────────────────────────

struct CachedPipeline {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    variant_name: &'static str,
    /// Number of bindings in this shader (2 or 3 for supported shaders).
    num_bindings: u32,
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
                if let Some(pipeline) = Self::build_pipeline(&device, key, source) {
                    pipelines.insert(key.to_string(), pipeline);
                }
            }
        }

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

        // Supported: 2, 3, or 4 binding shaders. 0/1/more are invalid.
        if num_bindings < 2 || num_bindings > 4 {
            return None;
        }

        // Build bind group layout matching shader declarations.
        // Layout depends on binding count:
        //   2: [input(read), output(read_write)]
        //   3: [input(read), output(read_write), params(uniform)]
        //   4: [input_a(read), input_b(read), output(read_write), params(uniform)]
        let mut bindings = Vec::with_capacity(num_bindings as usize);

        if num_bindings == 4 {
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
        })
    }

    fn make_bind_group(
        &self,
        cached: &CachedPipeline,
        input_buf: &wgpu::Buffer,
        output_buf: &wgpu::Buffer,
        img2_buf: Option<&wgpu::Buffer>,
    ) -> wgpu::BindGroup {
        let mut entries = Vec::with_capacity(cached.num_bindings as usize);
        match cached.num_bindings {
            4 => {
                entries.push(wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input_buf.as_entire_binding(),
                });
                // For dual-input ops, binding 1 is the second image (read-only)
                let buf_b = img2_buf.unwrap_or(output_buf);
                entries.push(wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buf_b.as_entire_binding(),
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
    ) {
        let bind_group = self.make_bind_group(cached, input_buf, output_buf, img2_buf);
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
        let size = (w * h * 4) as u64;

        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_buffer_to_buffer(src, 0, &staging, 0, size);
        self.queue.submit(Some(encoder.finish()));

        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .map_err(|_| PilError::ValueError("readback channel closed".into()))?
            .map_err(|e| PilError::ValueError(format!("map_async failed: {:?}", e)))?;

        let data = slice.get_mapped_range().to_vec();
        staging.unmap();

        let n = (w * h) as usize;
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
    ) -> Result<bool, PilError> {
        // Pre-materialize second images to avoid interleaved CPU decode during GPU dispatch.
        // Each dual-input op gets its second image materialized upfront; upload to buf_img2
        // happens right before dispatch (CPU→GPU, not a round trip).
        let second_images: Vec<Option<DynamicImage>> =
            ops.iter().map(|op| extract_second_image(op)).collect();

        let mut current_is_a = true;
        for (i, op) in ops.iter().enumerate() {
            let base_key = registry::variant_key(op);

            let cached = self.pipelines.get(base_key).ok_or_else(|| {
                PilError::ValueError(format!("GpuPool: no compiled pipeline for '{}'", base_key))
            })?;

            let params = registry::extract_params(op);
            self.buffers.upload_params(&self.queue, &params, w, h, mode);

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

            self.dispatch_pass(cached, w, h, input_buf, output_buf, img2_buf);
            current_is_a = !current_is_a;
        }
        // After N ops, current_is_a tracks where the latest result lives:
        //   true → buf_a has the final result, false → buf_b
        Ok(current_is_a)
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
        eprintln!(
            "[GPU] {} op(s) {}x{} mode={}: {:?}",
            ops.len(),
            w,
            h,
            mcode,
            op_keys
        );
        gpu.buffers.upload_rgba(&gpu.queue, &rgba)?;
        let final_is_a = gpu.execute_batch_impl(ops, w, h, mcode)?;
        // Ensure GPU is done before readback
        gpu.device.poll(wgpu::Maintain::Wait);
        let result = gpu.readback_to_image(w, h, final_is_a)?;
        Ok(crate::image::preserve_mode(img, result))
    }
}
