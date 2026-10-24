//! Encoding for native byte Paste stages in explicit GPU graphs.

use super::super::{BufferPool, GpuInner, create_sized_buffer, plan_extract_band_dispatch};
use super::encoding::EncodedGraph;
use super::graph::{Stage, planned_channels};
use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::pipeline::PipelineOp;

pub(super) fn encode_paste_stage(
    gpu: &GpuInner,
    encoder: &mut wgpu::CommandEncoder,
    buffers: &BufferPool,
    stage: &Stage<'_>,
    source: &EncodedGraph,
    current_a: bool,
) -> Result<bool, PilError> {
    let PipelineOp::Paste {
        x, y, mask: None, ..
    } = stage.op
    else {
        return Err(PilError::InternalError(
            "GPU Paste stage has an unsupported descriptor".into(),
        ));
    };
    let source_plan = stage
        .secondary
        .as_deref()
        .ok_or_else(|| PilError::InternalError("GPU Paste source plan is missing".into()))?;
    let channels = planned_channels(&stage.mode)?;
    let dimensions = CheckedDims::new(stage.width, stage.height, channels)?;
    let words = u32::try_from(dimensions.total_bytes().div_ceil(4))
        .map_err(|_| PilError::ValueError("GPU paste byte indexing overflow".into()))?;
    let parameters = [
        stage.width,
        stage.height,
        source_plan.width,
        source_plan.height,
        dimensions.total_pixels() as u32,
        words,
        u32::from(channels),
        0,
        *x as u32,
        *y as u32,
        0,
        0,
    ];
    let cached = gpu.resolve_pipeline(
        "__internal_stream_native_paste",
        "paste_native_bytes.wgsl",
        include_str!("../shaders/paste_native_bytes.wgsl"),
    )?;
    let uniform = create_sized_buffer(
        &gpu.device,
        "gpu_stream_paste_params",
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        48,
        4,
    );
    gpu.queue
        .write_buffer(&uniform, 0, bytemuck::cast_slice(&parameters));
    let (input, output) = if current_a {
        (&buffers.buf_a, &buffers.buf_b)
    } else {
        (&buffers.buf_b, &buffers.buf_a)
    };
    let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("gpu_stream_paste_bind"),
        layout: &cached.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: source.buffer.as_entire_binding(),
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
    let groups = plan_extract_band_dispatch(
        u32::try_from(dimensions.total_bytes())
            .map_err(|_| PilError::ValueError("GPU Paste output is too large".into()))?,
        1,
        gpu.device.limits().max_compute_workgroups_per_dimension,
    )?;
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("gpu_stream_paste"),
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

fn masked_paste_parameters(stage: &Stage<'_>) -> Result<(u32, [u32; 16]), PilError> {
    let PipelineOp::Paste {
        x,
        y,
        w,
        h,
        mask_alpha: false,
        ..
    } = stage.op
    else {
        return Err(PilError::InternalError(
            "GPU masked Paste stage has an unsupported descriptor".into(),
        ));
    };
    let source_plan = stage
        .secondary
        .as_deref()
        .ok_or_else(|| PilError::InternalError("GPU masked Paste source plan is missing".into()))?;
    let mask_plan = stage
        .tertiary
        .as_deref()
        .ok_or_else(|| PilError::InternalError("GPU masked Paste mask plan is missing".into()))?;
    let channels = planned_channels(&stage.mode)?;
    if !matches!(channels, 1 | 3 | 4) {
        return Err(PilError::ValueError(
            "GPU masked Paste has an unsupported native channel count".into(),
        ));
    }
    let region = (
        u32::try_from(*w)
            .map_err(|_| PilError::ValueError("GPU masked Paste width is invalid".into()))?,
        u32::try_from(*h)
            .map_err(|_| PilError::ValueError("GPU masked Paste height is invalid".into()))?,
    );
    let source_is_broadcast = (source_plan.width, source_plan.height) == (1, 1) && region != (1, 1);
    if region != (mask_plan.width, mask_plan.height)
        || (!source_is_broadcast && (source_plan.width, source_plan.height) != region)
        || source_plan.mode != stage.mode
        || mask_plan.mode != "L"
    {
        return Err(PilError::ValueError(
            "GPU masked Paste source or mask layout changed after admission".into(),
        ));
    }
    let destination = CheckedDims::new(stage.width, stage.height, channels)?;
    let destination_bytes = u32::try_from(destination.total_bytes())
        .map_err(|_| PilError::ValueError("GPU masked Paste output is too large".into()))?;
    let word_count = u32::try_from(destination_bytes.div_ceil(4))
        .map_err(|_| PilError::ValueError("GPU masked Paste output is too large".into()))?;
    let parameters = [
        stage.width,
        stage.height,
        source_plan.width,
        source_plan.height,
        mask_plan.width,
        mask_plan.height,
        destination_bytes,
        word_count,
        u32::from(channels),
        *x as u32,
        *y as u32,
        u32::from(source_is_broadcast),
        0,
        0,
        0,
        0,
    ];
    Ok((destination_bytes, parameters))
}

pub(super) fn encode_masked_paste_stage(
    gpu: &GpuInner,
    encoder: &mut wgpu::CommandEncoder,
    buffers: &BufferPool,
    stage: &Stage<'_>,
    source: &EncodedGraph,
    mask: &EncodedGraph,
    current_a: bool,
) -> Result<bool, PilError> {
    let (destination_bytes, parameters) = masked_paste_parameters(stage)?;
    let cached = gpu.resolve_pipeline(
        "__internal_stream_native_masked_paste",
        "paste_native_masked_bytes.wgsl",
        include_str!("../shaders/paste_native_masked_bytes.wgsl"),
    )?;
    let uniform = create_sized_buffer(
        &gpu.device,
        "gpu_stream_masked_paste_params",
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        std::mem::size_of_val(&parameters),
        4,
    );
    gpu.queue
        .write_buffer(&uniform, 0, bytemuck::cast_slice(&parameters));
    let (input, output) = if current_a {
        (&buffers.buf_a, &buffers.buf_b)
    } else {
        (&buffers.buf_b, &buffers.buf_a)
    };
    let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("gpu_stream_masked_paste_bind"),
        layout: &cached.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: source.buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: mask.buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: output.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: uniform.as_entire_binding(),
            },
        ],
    });
    let groups = plan_extract_band_dispatch(
        destination_bytes,
        1,
        gpu.device.limits().max_compute_workgroups_per_dimension,
    )?;
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("gpu_stream_masked_paste"),
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
