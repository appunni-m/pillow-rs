use std::collections::HashMap;

use image::{DynamicImage, RgbaImage};

use crate::compute::registry::{self, OpDef, OpId};
use crate::compute::{Backend, ComputeBackend};
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::pipeline::PipelineOp;

// ─── BufferPool ───

struct BufferPool {
    buf_a: wgpu::Buffer,
    buf_b: wgpu::Buffer,
    params: wgpu::Buffer,
    capacity: u32, // number of u32 (RGBA pixels)
}

impl BufferPool {
    fn new(device: &wgpu::Device, capacity: u32) -> Self {
        let size = (capacity * 4) as u64; // 4 bytes per u32
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
        // Params buffer: small, fixed at 16 u32s (64 bytes) for flexibility
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_params"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        BufferPool {
            buf_a,
            buf_b,
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
        // Pack RGBA bytes into u32
        let mut packed: Vec<u32> = Vec::with_capacity(n);
        for px in rgba.pixels() {
            packed.push(
                (px[0] as u32)
                    | ((px[1] as u32) << 8)
                    | ((px[2] as u32) << 16)
                    | ((px[3] as u32) << 24),
            );
        }
        let _size = (n * 4) as u64;
        // Write to both buffers (A = input, B = output for first pass)
        queue.write_buffer(&self.buf_a, 0, bytemuck::cast_slice(&packed));
        queue.write_buffer(&self.buf_b, 0, bytemuck::cast_slice(&packed));
        Ok(())
    }

    fn upload_params(&self, queue: &wgpu::Queue, params: &[u32], w: u32, h: u32) {
        // First 4 params: width, height, pad, pad
        let mut buf = vec![w, h, 0u32, 0u32];
        buf.extend_from_slice(params);
        queue.write_buffer(&self.params, 0, bytemuck::cast_slice(&buf));
    }
}

// ─── CachedPipeline ───

struct CachedPipeline {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    _input_count: u8,
    has_params: bool,
    _is_multi_pass: bool,
    pass_count: u8,
    variant_name: &'static str,
}

// ─── GpuBackend ───

pub struct GpuBackend {
    device: wgpu::Device,
    queue: wgpu::Queue,
    buffers: BufferPool,
    pipelines: HashMap<OpId, CachedPipeline>,
}

impl GpuBackend {
    /// Create a new GPU backend. Returns None if wgpu cannot initialize.
    pub fn new(capacity: u32) -> Option<Self> {
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

        let buffers = BufferPool::new(&device, capacity);

        // Pre-compile all shaders from the registry
        let registry = registry::get_registry();
        let mut pipelines = HashMap::new();
        for def in registry.iter() {
            if let Some(pipeline) = Self::build_pipeline(&device, def) {
                pipelines.insert(def.id, pipeline);
            }
        }

        Some(GpuBackend {
            device,
            queue,
            buffers,
            pipelines,
        })
    }

    fn build_pipeline(device: &wgpu::Device, def: &OpDef) -> Option<CachedPipeline> {
        let cs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(def.variant_name),
            source: wgpu::ShaderSource::Wgsl(def.shader_source.into()),
        });

        let mut bindings = Vec::new();
        // Binding 0: input storage buffer
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
        // Binding 1: output storage buffer
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
        // Binding 2: params uniform buffer (if has_params)
        if def.has_params {
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

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(def.variant_name),
            entries: &bindings,
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(def.variant_name),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(def.variant_name),
            layout: Some(&pipeline_layout),
            module: &cs_module,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Some(CachedPipeline {
            pipeline,
            bind_group_layout,
            _input_count: def.input_count,
            has_params: def.has_params,
            _is_multi_pass: def.is_multi_pass,
            pass_count: def.pass_count,
            variant_name: def.variant_name,
        })
    }

    /// Create a bind group for a dispatch pass.
    fn make_bind_group(
        &self,
        cached: &CachedPipeline,
        input_buf: &wgpu::Buffer,
        output_buf: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        let mut entries = Vec::new();
        // Binding 0: input
        entries.push(wgpu::BindGroupEntry {
            binding: 0,
            resource: input_buf.as_entire_binding(),
        });
        // Binding 1: output
        entries.push(wgpu::BindGroupEntry {
            binding: 1,
            resource: output_buf.as_entire_binding(),
        });
        // Binding 2: params (if has_params)
        if cached.has_params {
            entries.push(wgpu::BindGroupEntry {
                binding: 2,
                resource: self.buffers.params.as_entire_binding(),
            });
        }

        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(cached.variant_name),
            layout: &cached.bind_group_layout,
            entries: &entries,
        })
    }

    /// Dispatch a compute shader with the given input -> output buffers.
    fn dispatch_pass(
        &self,
        cached: &CachedPipeline,
        w: u32,
        h: u32,
        input_buf: &wgpu::Buffer,
        output_buf: &wgpu::Buffer,
    ) {
        let bind_group = self.make_bind_group(cached, input_buf, output_buf);
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
            // 16x16 workgroups
            let wgs_x = w.div_ceil(16);
            let wgs_y = h.div_ceil(16);
            cpass.dispatch_workgroups(wgs_x, wgs_y, 1);
        }
        self.queue.submit(Some(encoder.finish()));
    }

    /// Readback the result buffer as an RGBA image.
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

        // Unpack u32 -> RGBA bytes
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
}

impl ComputeBackend for GpuBackend {
    fn name(&self) -> Backend {
        Backend::Gpu
    }

    fn priority(&self) -> u8 {
        100
    }

    fn supports(&self, op: &PipelineOp) -> bool {
        registry::op_id(op).is_some()
    }

    fn execute_batch(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        explicit_mode: Option<&str>,
        _palette: Option<&[u8]>,
    ) -> Result<DynamicImage, PilError> {
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let _n_pixels = w * h;

        // Determine output mode: if explicit_mode is "L" and only one op,
        // check if it outputs grayscale
        let _ = explicit_mode; // preserved in readback mode if needed

        // Upload RGBA -> packed u32
        self.buffers.upload_rgba(&self.queue, &rgba)?;

        // Determine which buffer holds the current result
        // true = buf_a is current, false = buf_b is current
        let mut current_is_a = true;

        for op in ops {
            let id = registry::op_id(op).unwrap();
            let cached = self
                .pipelines
                .get(&id)
                .ok_or_else(|| PilError::ValueError(format!("No GPU pipeline for {:?}", id)))?;

            // Upload params
            let params = registry::extract_params(op);
            self.buffers.upload_params(&self.queue, &params, w, h);

            let passes = cached.pass_count;

            for _pass in 0..passes {
                let (input_buf, output_buf) = if current_is_a {
                    (&self.buffers.buf_a, &self.buffers.buf_b)
                } else {
                    (&self.buffers.buf_b, &self.buffers.buf_a)
                };

                self.dispatch_pass(cached, w, h, input_buf, output_buf);
                current_is_a = !current_is_a;
            }
        }

        // The final result is in the buffer that was NOT written to last
        let final_is_a = !current_is_a;
        let result = self.readback_to_image(w, h, final_is_a)?;
        // Preserve the original image mode (convert RGBA back to L/LA/RGB as needed)
        Ok(preserve_mode(img, result))
    }
}
