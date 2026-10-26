//! GPU shader pipeline construction and cache lookup.

use super::*;

// ─── Count shader bindings ─────────────────────────────────────────────────

/// Count the number of `@binding(N)` entries in WGSL source.
/// Returns max binding index + 1, or 0 if no bindings found.
fn count_shader_bindings(source: &str) -> u32 {
    let mut max_binding: i32 = -1;
    for line in source.lines() {
        if let Some(pos) = line.find("@binding(") {
            let rest = &line[pos + 9..];
            if let Some(end) = rest.find(')')
                && let Ok(n) = rest[..end].trim().parse::<u32>()
            {
                max_binding = max_binding.max(n as i32);
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

/// Detect the two-binding generator layout used by kernels that do not read
/// the current pipeline image. Their binding 0 is the writable output and
/// binding 1 is the uniform parameter block, unlike ordinary two-binding
/// kernels whose binding 0 is a read-only input.
fn is_output_only_shader(source: &str) -> bool {
    source
        .lines()
        .any(|line| line.contains("@binding(0)") && line.contains("var<storage, read_write>"))
}

// ─── CachedPipeline ────────────────────────────────────────────────────────

pub(super) struct CachedPipeline {
    pub(super) pipeline: wgpu::ComputePipeline,
    pub(super) bind_group_layout: wgpu::BindGroupLayout,
    pub(super) variant_name: &'static str,
    pub(super) shader_file: &'static str,
    /// Number of bindings in this shader (2-5).
    pub(super) num_bindings: u32,
    /// True if this is a 4-binding LUT shader (Eval/PointOp).
    pub(super) is_lut: bool,
    /// True if this is a two-binding generator shader with no input image.
    pub(super) is_output_only: bool,
    /// True if the shader reads an auxiliary source and updates the current
    /// destination buffer in place (AlphaComposite).
    pub(super) is_in_place: bool,
}

impl GpuInner {
    pub(super) fn resolve_pipeline(
        &self,
        key: &'static str,
        shader_file: &'static str,
        source: &'static str,
    ) -> Result<Arc<CachedPipeline>, PilError> {
        let mut pipelines = self
            .pipelines
            .lock()
            .map_err(|_| PilError::InternalError("GPU pipeline-cache lock is poisoned".into()))?;
        if let Some(pipeline) = pipelines.get(key) {
            return Ok(Arc::clone(pipeline));
        }

        gpu_log!("[GPU] compiling shader on first use: {key}");
        let pipeline =
            Self::build_pipeline(&self.device, key, shader_file, source).ok_or_else(|| {
                PilError::ValueError(format!(
                    "GPU operation '{key}' has no validated executable pipeline"
                ))
            })?;
        gpu_log!(
            "[GPU] compiled shader on first use: {key} ({} bindings)",
            pipeline.num_bindings
        );
        let pipeline = Arc::new(pipeline);
        pipelines.insert(key, Arc::clone(&pipeline));
        Ok(pipeline)
    }

    fn push_storage_binding(
        bindings: &mut Vec<wgpu::BindGroupLayoutEntry>,
        binding: u32,
        read_only: bool,
    ) {
        bindings.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });
    }

    fn push_uniform_binding(bindings: &mut Vec<wgpu::BindGroupLayoutEntry>, binding: u32) {
        bindings.push(wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });
    }

    fn bind_group_layout_entries(
        num_bindings: u32,
        is_lut: bool,
        is_output_only: bool,
    ) -> Vec<wgpu::BindGroupLayoutEntry> {
        let mut bindings = Vec::with_capacity(num_bindings as usize);
        if num_bindings == 5 {
            for binding in 0..3 {
                Self::push_storage_binding(&mut bindings, binding, true);
            }
            Self::push_storage_binding(&mut bindings, 3, false);
            Self::push_uniform_binding(&mut bindings, 4);
        } else if num_bindings == 4 {
            if is_lut {
                Self::push_storage_binding(&mut bindings, 0, true);
                Self::push_storage_binding(&mut bindings, 1, false);
                Self::push_uniform_binding(&mut bindings, 2);
                Self::push_storage_binding(&mut bindings, 3, true);
            } else {
                Self::push_storage_binding(&mut bindings, 0, true);
                Self::push_storage_binding(&mut bindings, 1, true);
                Self::push_storage_binding(&mut bindings, 2, false);
                Self::push_uniform_binding(&mut bindings, 3);
            }
        } else if is_output_only {
            Self::push_storage_binding(&mut bindings, 0, false);
            Self::push_uniform_binding(&mut bindings, 1);
        } else {
            Self::push_storage_binding(&mut bindings, 0, true);
            Self::push_storage_binding(&mut bindings, 1, false);
            if num_bindings > 2 {
                Self::push_uniform_binding(&mut bindings, 2);
            }
        }
        bindings
    }

    fn build_pipeline(
        device: &wgpu::Device,
        variant_name: &'static str,
        shader_file: &'static str,
        shader_source: &str,
    ) -> Option<CachedPipeline> {
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let cs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(variant_name),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });
        if let Some(error) = pollster::block_on(device.pop_error_scope()) {
            gpu_log!(
                "[GPU] shader module validation failed: variant={variant_name} file={shader_file}: {error}"
            );
            return None;
        }

        let num_bindings = count_shader_bindings(shader_source);

        // Supported: 2-5 binding shaders. 0/1/>5 are invalid.
        if !(2..=5).contains(&num_bindings) {
            gpu_log!(
                "[GPU] shader rejected: variant={variant_name} file={shader_file}: bindings={num_bindings}"
            );
            return None;
        }

        // Detect if this is a LUT shader (Eval/PointOp) with 4 bindings.
        let is_lut = num_bindings == 4 && is_lut_shader(shader_source);
        let is_output_only = num_bindings == 2 && is_output_only_shader(shader_source);
        let is_in_place = variant_name == "AlphaComposite";

        let bindings = Self::bind_group_layout_entries(num_bindings, is_lut, is_output_only);

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
        if let Some(error) = pollster::block_on(device.pop_error_scope()) {
            gpu_log!(
                "[GPU] shader validation failed: variant={variant_name} file={shader_file}: {error}"
            );
            return None;
        }

        Some(CachedPipeline {
            pipeline,
            bind_group_layout,
            variant_name,
            shader_file,
            num_bindings,
            is_lut,
            is_output_only,
            is_in_place,
        })
    }
}
