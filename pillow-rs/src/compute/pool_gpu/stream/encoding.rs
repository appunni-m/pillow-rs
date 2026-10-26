//! Command encoding for already-admitted explicit GPU graphs.

mod stage_encoding;

use self::stage_encoding::encode_graph_stages;

use super::super::batch_operation_planning::NativePackedChannelOutputDispatch;
use super::super::pipeline_cache::CachedPipeline;
use super::super::{
    AuxiliaryImages, BufferPool, GpuAuxiliaryCache, GpuInner, ReusableGpuBuffer, StagingBuffer,
    aligned_bytes, create_sized_buffer, gpu_working_set_bytes, op_output_dims,
    plan_extract_band_dispatch, plan_native_rgb_point_output,
};
use super::graph::{LUMA_BAND, Plan, Stage, planned_channels};
use super::{BatchKey, Credit, Job};
use crate::Image;
use crate::checked_dims::CheckedDims;
use crate::compute::registry;
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;
use std::num::NonZeroU64;

struct CompileCredit {
    gpu: &'static GpuInner,
    credit: Option<Credit>,
}
impl Drop for CompileCredit {
    fn drop(&mut self) {
        if self.credit.is_some() {
            // Queue writes survive failed encoding. Flush transfers only and
            // retain their reservation until this fence completes.
            let fence = StagingBuffer::new(&self.gpu.device, 8);
            let mut encoder =
                self.gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("gpu_stream_failed_compile_fence"),
                    });
            encoder.clear_buffer(&fence.buffer, 0, None);
            self.gpu.queue.submit([encoder.finish()]);
            let _ = self.gpu.readback_with(8, &fence.buffer, |_| Ok(()));
        }
    }
}
pub(super) struct EncodedGraph {
    pub(super) buffer: wgpu::Buffer,
    dispatches: u64,
    upload: u64,
    retained: u64,
}
pub(super) fn compile_graph(
    template: &Image,
    key: Option<BatchKey>,
    id: u64,
    plan: Plan<'_>,
    credit: Credit,
    gpu: &'static GpuInner,
) -> Result<Job, PilError> {
    let mut reservation = CompileCredit {
        gpu,
        credit: Some(credit),
    };
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gpu_stream_graph"),
        });
    let encoded = encode_graph(&plan, gpu, &mut encoder)?;
    let length =
        CheckedDims::new(plan.width, plan.height, planned_channels(&plan.mode)?)?.total_bytes();
    if encoded.retained + aligned_bytes(length, 8) as u64 > plan.gpu_bytes {
        return Err(PilError::ValueError(
            "GPU generated resources exceed admission reservation".into(),
        ));
    }
    Ok(Job {
        id,
        key,
        template: template.gpu_result_metadata(),
        width: plan.width,
        height: plan.height,
        mode: plan.mode,
        length,
        output: Some(encoded.buffer),
        command: Some(encoder.finish()),
        credit: reservation.credit.take().ok_or_else(|| {
            PilError::InternalError("GPU compile reservation was already transferred".into())
        })?,
        output_capacity: u64::from(plan.capacity) * 4,
        dispatches: encoded.dispatches,
        shader_work: plan.shader_work,
        op_count: plan.op_count,
        metadata_bytes: plan.metadata_bytes,
        upload: encoded.upload,
    })
}
fn encode_graph(
    plan: &Plan<'_>,
    gpu: &'static GpuInner,
    encoder: &mut wgpu::CommandEncoder,
) -> Result<EncodedGraph, PilError> {
    let source = match &plan.source.cached {
        Some(pixels) => pixels.clone(),
        None => plan.source.source.materialized_shared()?,
    };
    let channels = planned_channels(&plan.source.mode)?;
    let physical_matches = matches!(
        (channels, source.as_ref()),
        (1, DynamicImage::ImageLuma8(_))
            | (2, DynamicImage::ImageLumaA8(_))
            | (3, DynamicImage::ImageRgb8(_))
            | (4, DynamicImage::ImageRgba8(_))
    );
    if !physical_matches
        || source.as_bytes().len()
            != CheckedDims::new(plan.source.width, plan.source.height, channels)?.total_bytes()
    {
        return Err(PilError::ValueError(
            "decoded source does not match the admitted native layout".into(),
        ));
    }
    let mapped_primary_input = gpu.direct_primary_readback
        && plan.stages.len() == 1
        && matches!(plan.stages[0].op, PipelineOp::BoxBlur { radius: 1 })
        && matches!(plan.source.mode.as_str(), "L" | "RGB");
    let mut buffers =
        BufferPool::new_with_mapped_input(&gpu.device, plan.capacity, false, mapped_primary_input);
    let source_bytes = source.as_bytes();
    let aligned = aligned_bytes(source_bytes.len(), 4);
    if mapped_primary_input {
        gpu.upload_bytes_to_mapped_buffer(
            &buffers.buf_a,
            source_bytes,
            source_bytes.len(),
            aligned as u64,
            "GPU stream mapped BoxBlur input",
        )?;
    } else {
        let mut upload = gpu
            .queue
            .write_buffer_with(
                &buffers.buf_a,
                0,
                NonZeroU64::new(aligned as u64)
                    .ok_or_else(|| PilError::InternalError("GPU source upload is empty".into()))?,
            )
            .ok_or_else(|| PilError::InternalError("GPU source upload allocation failed".into()))?;
        upload[..source_bytes.len()].copy_from_slice(source_bytes);
        upload[source_bytes.len()..].fill(0);
        drop(upload);
    }
    let mut current_a = true;
    let mut retained = gpu_working_set_bytes(plan.capacity);
    let mut dispatches = 0;
    let mut uploaded_bytes = source_bytes.len() as u64;
    let auxiliary = [AuxiliaryImages {
        second: None,
        third: None,
    }];
    current_a = encode_graph_stages(
        plan,
        gpu,
        encoder,
        &mut buffers,
        current_a,
        &mut retained,
        &mut dispatches,
        &mut uploaded_bytes,
        &auxiliary,
    )?;
    let output = if current_a {
        buffers.buf_a.clone()
    } else {
        buffers.buf_b.clone()
    };
    Ok(EncodedGraph {
        buffer: output,
        dispatches,
        upload: uploaded_bytes,
        retained,
    })
}

fn encode_generic_stage(
    gpu: &GpuInner,
    encoder: &mut wgpu::CommandEncoder,
    buffers: &mut BufferPool,
    stage: &Stage<'_>,
    capacity: u32,
    auxiliary: &[AuxiliaryImages],
    current_a: bool,
) -> Result<(bool, u64), PilError> {
    reset_generic_stage_arenas(gpu, buffers);
    let encoded_op = generic_stage_operation(stage);
    validate_generic_stage_reservation(gpu, buffers, stage, auxiliary, encoded_op)?;
    let current_a = encode_generic_stage_batch(
        gpu, encoder, buffers, stage, capacity, auxiliary, encoded_op, current_a,
    )?;
    let retained = buffers.params_arena.capacity_bytes
        + buffers.img2_arena.capacity_bytes
        + buffers.img3_arena.capacity_bytes
        + buffers.lut_arena.capacity_bytes;
    Ok((current_a, retained))
}

fn reset_generic_stage_arenas(gpu: &GpuInner, buffers: &mut BufferPool) {
    // A fresh arena per stage prevents queue.write_buffer from changing
    // uniforms used by earlier commands before this graph is submitted.
    buffers.params_arena = ReusableGpuBuffer::new(
        &gpu.device,
        "gpu_stream_params",
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        4,
        4,
    );
    buffers.img2_arena = ReusableGpuBuffer::new(
        &gpu.device,
        "gpu_stream_aux",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        4,
        4,
    );
    buffers.img3_arena = ReusableGpuBuffer::new(
        &gpu.device,
        "gpu_stream_mask",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        4,
        4,
    );
    buffers.lut_arena = ReusableGpuBuffer::new(
        &gpu.device,
        "gpu_stream_lut",
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        1024,
        4,
    );
}

fn generic_stage_operation<'a>(stage: &Stage<'a>) -> &'a PipelineOp {
    // Grayscale on native L/LA selects its luma sample directly.
    // Keep the canonical graph; only the internal kernel is lowered.
    if stage.flags.extract && matches!(stage.op, PipelineOp::Grayscale) {
        &LUMA_BAND
    } else {
        stage.op
    }
}

fn estimate_stage_mode(mode: &str) -> u32 {
    match mode {
        "L" => 0,
        "LA" => 1,
        "RGB" => 2,
        "F" => 8,
        _ => 3,
    }
}

fn prepare_stage_mode(mode: &str) -> u32 {
    match mode {
        "L" => 0,
        "LA" => 1,
        "RGB" => 2,
        "CMYK" => 4,
        "F" => 8,
        "RGBX" => 6,
        _ => 3,
    }
}

fn validate_generic_stage_reservation(
    gpu: &GpuInner,
    buffers: &BufferPool,
    stage: &Stage<'_>,
    auxiliary: &[AuxiliaryImages],
    encoded_op: &PipelineOp,
) -> Result<(), PilError> {
    let estimated = gpu.estimate_resource_bytes(
        encoded_op,
        &auxiliary[0],
        buffers,
        gpu.device.limits().min_uniform_buffer_offset_alignment as usize,
        gpu.device.limits().min_storage_buffer_offset_alignment as usize,
        (stage.width, stage.height),
        estimate_stage_mode(&stage.mode),
        false,
        Some(&stage.mode),
        false,
        stage.ordered_f_resize,
    )?;
    if estimated as u64 > stage.arena_bytes {
        return Err(PilError::ValueError(
            "GPU stage arenas exceed their bounded admission reservation".into(),
        ));
    }
    Ok(())
}

fn encode_generic_stage_batch(
    gpu: &GpuInner,
    encoder: &mut wgpu::CommandEncoder,
    buffers: &mut BufferPool,
    stage: &Stage<'_>,
    capacity: u32,
    auxiliary: &[AuxiliaryImages],
    encoded_op: &PipelineOp,
    current_a: bool,
) -> Result<bool, PilError> {
    let flags = stage.flags;
    let cache = GpuAuxiliaryCache::from_batch(
        std::slice::from_ref(encoded_op),
        auxiliary,
        estimate_stage_mode(&stage.mode),
        capacity,
        gpu.device.limits().min_storage_buffer_offset_alignment as usize,
    )?;
    let prepared = gpu.prepare_batch(
        std::slice::from_ref(encoded_op),
        auxiliary,
        stage.width,
        stage.height,
        prepare_stage_mode(&stage.mode),
        Some(&stage.mode),
        None,
        None,
        false,
        false,
        false,
        false,
        false,
        stage.ordered_f_resize,
        flags.luma_colorize,
        flags.luma_convert,
        false,
        false,
        flags.rgb,
        flags.rgb && !matches!(stage.op, PipelineOp::Convert { .. }),
        false,
        flags.luma_point,
        false,
        flags.luma_order,
        flags.byte_filter,
        false,
        flags.extract,
        flags.grayscale_rgb,
        false,
        false,
        false,
        false,
        None,
        None,
        None,
        buffers,
        &cache,
    )?;
    let current_a = gpu.encode_batch(
        encoder,
        std::slice::from_ref(encoded_op),
        &prepared,
        current_a,
        Some(&stage.mode),
        flags.luma_point,
        false,
        flags.luma_order,
        flags.byte_filter,
        false,
        false,
        false,
        false,
        false,
        false,
    )?;
    drop(prepared);
    Ok(current_a)
}

fn can_fuse_batch_alpha_composite_mirror(first: &Stage<'_>, second: &Stage<'_>) -> bool {
    if first.mode != "RGBA"
        || second.mode != "RGBA"
        || (first.width, first.height) != (second.width, second.height)
    {
        return false;
    }
    match (&first.op, &second.op) {
        (
            PipelineOp::AlphaComposite {
                source,
                dest: (0, 0),
                src: (0, 0),
            },
            PipelineOp::Mirror,
        ) => {
            source.size().ok() == Some((first.width, first.height))
                && first.secondary.as_deref().is_some_and(|child| {
                    child.mode == "RGBA"
                        && (child.width, child.height) == (first.width, first.height)
                })
        }
        _ => false,
    }
}

fn encode_alpha_composite_mirror_stage(
    gpu: &GpuInner,
    encoder: &mut wgpu::CommandEncoder,
    buffers: &BufferPool,
    stage: &Stage<'_>,
    secondary: &EncodedGraph,
    current_a: bool,
) -> Result<bool, PilError> {
    let cached = gpu.resolve_pipeline(
        "__internal_alpha_composite_mirror",
        "alpha_composite_mirror.wgsl",
        include_str!("../shaders/alpha_composite_mirror.wgsl"),
    )?;
    let parameters = [stage.width, stage.height, 3, 0, stage.width, stage.height];
    let uniform = create_sized_buffer(
        &gpu.device,
        "gpu_stream_alpha_composite_mirror_params",
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        std::mem::size_of_val(&parameters),
        4,
    );
    gpu.queue
        .write_buffer(&uniform, 0, bytemuck::cast_slice(&parameters));
    let (destination, output) = if current_a {
        (&buffers.buf_a, &buffers.buf_b)
    } else {
        (&buffers.buf_b, &buffers.buf_a)
    };
    let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("gpu_stream_alpha_composite_mirror_bind"),
        layout: &cached.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: destination.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: secondary.buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: output.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: uniform.as_entire_binding(),
            },
        ],
    });
    let groups_x = stage.width.div_ceil(16);
    let groups_y = stage.height.div_ceil(16);
    let maximum = gpu.device.limits().max_compute_workgroups_per_dimension;
    if groups_x > maximum || groups_y > maximum {
        return Err(PilError::ValueError(
            "GPU alpha-composite mirror dispatch exceeds device limit".into(),
        ));
    }
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("gpu_stream_alpha_composite_mirror"),
        timestamp_writes: None,
    });
    pass.set_pipeline(&cached.pipeline);
    pass.set_bind_group(0, &bind, &[]);
    pass.dispatch_workgroups(groups_x, groups_y, 1);
    crate::compute::record_gpu_shader_dispatch(
        cached.variant_name,
        cached.shader_file,
        u64::from(groups_x) * u64::from(groups_y),
    );
    Ok(!current_a)
}

fn encode_relocation_stage(
    gpu: &GpuInner,
    encoder: &mut wgpu::CommandEncoder,
    buffers: &BufferPool,
    stage: &Stage,
    current_a: bool,
) -> Result<bool, PilError> {
    let output =
        op_output_dims(stage.op, stage.width, stage.height).unwrap_or((stage.width, stage.height));
    let channels = planned_channels(&stage.mode)?;
    let (opcode, offset_x, offset_y, fill) = match stage.op {
        PipelineOp::Transpose { .. } => (registry::extract_params(stage.op)[0], 0, 0, (0, 0, 0, 0)),
        PipelineOp::Mirror => (0, 0, 0, (0, 0, 0, 0)),
        PipelineOp::Flip => (1, 0, 0, (0, 0, 0, 0)),
        PipelineOp::Crop { left, top, .. } => (7, *left, *top, (0, 0, 0, 0)),
        PipelineOp::CropBorder { border } => (7, *border, *border, (0, 0, 0, 0)),
        PipelineOp::Expand { border, fill } => (8, *border, *border, *fill),
        _ => {
            return Err(PilError::InternalError(
                "unrecognized native relocation".into(),
            ));
        }
    };
    let parameters = [
        stage.width,
        stage.height,
        output.0,
        output.1,
        u32::from(channels),
        opcode,
        offset_x,
        offset_y,
        u32::from(fill.0)
            | u32::from(fill.1) << 8
            | u32::from(fill.2) << 16
            | u32::from(fill.3) << 24,
    ];
    let cached = gpu.resolve_pipeline(
        "__internal_relocate_native_bytes",
        "relocate_native_bytes.wgsl",
        include_str!("../shaders/relocate_native_bytes.wgsl"),
    )?;
    let uniform = create_sized_buffer(
        &gpu.device,
        "gpu_stream_relocation_params",
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        parameters.len() * 4,
        4,
    );
    gpu.queue
        .write_buffer(&uniform, 0, bytemuck::cast_slice(&parameters));
    let (input, output_buffer) = if current_a {
        (&buffers.buf_a, &buffers.buf_b)
    } else {
        (&buffers.buf_b, &buffers.buf_a)
    };
    let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("gpu_stream_relocation_bind"),
        layout: &cached.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: uniform.as_entire_binding(),
            },
        ],
    });
    let bytes = CheckedDims::new(output.0, output.1, channels)?.total_bytes();
    let groups = plan_extract_band_dispatch(
        u32::try_from(bytes)
            .map_err(|_| PilError::ValueError("relocation native byte indexing overflow".into()))?,
        1,
        gpu.device.limits().max_compute_workgroups_per_dimension,
    )?;
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("gpu_stream_relocation"),
        timestamp_writes: None,
    });
    pass.set_pipeline(&cached.pipeline);
    pass.set_bind_group(0, &bind, &[]);
    pass.dispatch_workgroups(groups.0, groups.1, 1);
    crate::compute::record_gpu_shader_dispatch(
        cached.variant_name,
        cached.shader_file,
        u64::from(groups.0) * u64::from(groups.1),
    );
    Ok(!current_a)
}

#[inline(always)]
fn encode_rgb_blur_dispatch(
    gpu: &GpuInner,
    encoder: &mut wgpu::CommandEncoder,
    buffers: &BufferPool,
    uniform: &wgpu::Buffer,
    cached: &CachedPipeline,
    plan: NativePackedChannelOutputDispatch,
    current_a: bool,
    bind_group_label: &'static str,
    pass_label: &'static str,
) -> bool {
    let (input, output) = if current_a {
        (&buffers.buf_a, &buffers.buf_b)
    } else {
        (&buffers.buf_b, &buffers.buf_a)
    };
    let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(bind_group_label),
        layout: &cached.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: uniform.as_entire_binding(),
            },
        ],
    });
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some(pass_label),
        timestamp_writes: None,
    });
    pass.set_pipeline(&cached.pipeline);
    pass.set_bind_group(0, &bind, &[]);
    pass.dispatch_workgroups(plan.groups_x, plan.groups_y, 1);
    crate::compute::record_gpu_shader_dispatch(
        cached.variant_name,
        cached.shader_file,
        u64::from(plan.groups_x) * u64::from(plan.groups_y),
    );
    !current_a
}

fn encode_rgb_blur_stage(
    gpu: &GpuInner,
    encoder: &mut wgpu::CommandEncoder,
    buffers: &BufferPool,
    stage: &Stage<'_>,
    mut current_a: bool,
) -> Result<bool, PilError> {
    let limits = gpu.device.limits();
    let plan = plan_native_rgb_point_output(
        stage.width,
        stage.height,
        limits.max_compute_workgroups_per_dimension,
        limits.max_storage_buffer_binding_size,
        limits.max_buffer_size,
        buffers.capacity,
    )
    .ok_or_else(|| PilError::ValueError("native RGB blur dispatch exceeds device limits".into()))?;
    let mut params = vec![
        stage.width,
        stage.height,
        2,
        if plan.row_tiled { 16 } else { 0 },
    ];
    params.extend(registry::extract_params(stage.op));
    let uniform = create_sized_buffer(
        &gpu.device,
        "gpu_stream_rgb_blur_params",
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        params.len() * 4,
        4,
    );
    gpu.queue
        .write_buffer(&uniform, 0, bytemuck::cast_slice(&params));
    if matches!(stage.op, PipelineOp::BoxBlur { radius: 1 }) {
        // The eager GPU route already uses this one-dispatch shader for RGB
        // BoxBlur(1). Keep explicit batches on the same exact two-rounding
        // path instead of writing a full-frame horizontal intermediate.
        let cached = gpu.resolve_pipeline(
            "__internal_blur_hv_rgb_radius_one_packed",
            "box_blur_hv_rgb_radius_one_packed.wgsl",
            include_str!("../shaders/box_blur_hv_rgb_radius_one_packed.wgsl"),
        )?;
        return Ok(encode_rgb_blur_dispatch(
            gpu,
            encoder,
            buffers,
            &uniform,
            &cached,
            plan,
            current_a,
            "gpu_stream_rgb_blur_fused_bind",
            "gpu_stream_rgb_blur_fused",
        ));
    }
    let passes = GpuInner::blur_pass_count(stage.op)
        .ok_or_else(|| PilError::InternalError("missing native RGB blur pass count".into()))?;
    for (key, file, source) in [
        (
            "__internal_blur_h_rgb_packed",
            "box_blur_h_rgb_packed.wgsl",
            include_str!("../shaders/box_blur_h_rgb_packed.wgsl"),
        ),
        (
            "__internal_blur_v_rgb_packed",
            "box_blur_v_rgb_packed.wgsl",
            include_str!("../shaders/box_blur_v_rgb_packed.wgsl"),
        ),
    ] {
        let cached = gpu.resolve_pipeline(key, file, source)?;
        for _ in 0..passes {
            current_a = encode_rgb_blur_dispatch(
                gpu,
                encoder,
                buffers,
                &uniform,
                &cached,
                plan,
                current_a,
                "gpu_stream_rgb_blur_bind",
                "gpu_stream_rgb_blur",
            );
        }
    }
    Ok(current_a)
}
