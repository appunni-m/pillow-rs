use super::super::super::pipeline_cache::CachedPipeline;
use super::super::super::{
    AuxiliaryImages, BufferPool, GpuInner, create_sized_buffer, gpu_dispatch_count,
};
use super::super::graph::{Plan, Stage, planned_channels};
use super::{
    EncodedGraph, can_fuse_batch_alpha_composite_mirror, encode_alpha_composite_mirror_stage,
    encode_generic_stage, encode_graph, encode_relocation_stage, encode_rgb_blur_stage,
};
use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::pipeline::PipelineOp;

type NativeBytePipelineSpec = (&'static str, &'static str, &'static str, Option<u32>);

struct NativeByteDispatch<'a> {
    op: &'a PipelineOp,
    secondary: Option<&'a EncodedGraph>,
    current_a: bool,
    cached: &'a CachedPipeline,
    columns: u32,
    rows: u32,
    words: u32,
    param: Option<u32>,
}

fn encode_byte_stage(
    gpu: &GpuInner,
    encoder: &mut wgpu::CommandEncoder,
    buffers: &BufferPool,
    stage: &Stage<'_>,
    secondary: Option<&EncodedGraph>,
    current_a: bool,
) -> Result<bool, PilError> {
    let channels = planned_channels(&stage.mode)?;
    let length = CheckedDims::new(stage.width, stage.height, channels)?.total_bytes();
    let words = u32::try_from(length.div_ceil(4))
        .map_err(|_| PilError::ValueError("native shader index overflow".into()))?;
    let columns = words.min(1024);
    let rows = words.div_ceil(columns);
    let (key, file, source, param) = native_byte_pipeline_spec(stage.op)?;
    let cached = gpu.resolve_pipeline(key, file, source)?;
    encode_native_byte_dispatch(
        gpu,
        encoder,
        buffers,
        NativeByteDispatch {
            op: stage.op,
            secondary,
            current_a,
            cached: &cached,
            columns,
            rows,
            words,
            param,
        },
    )
}

fn native_byte_pipeline_spec(op: &PipelineOp) -> Result<NativeBytePipelineSpec, PilError> {
    let spec = match op {
        PipelineOp::Duplicate => (
            "__internal_duplicate_native_bytes",
            "duplicate.wgsl",
            include_str!("../../shaders/duplicate.wgsl"),
            None,
        ),
        PipelineOp::Invert | PipelineOp::InvertChops => (
            "__internal_solarize_native_bytes",
            "solarize.wgsl",
            include_str!("../../shaders/solarize.wgsl"),
            Some(0),
        ),
        PipelineOp::Solarize { threshold } => (
            "__internal_solarize_native_bytes",
            "solarize.wgsl",
            include_str!("../../shaders/solarize.wgsl"),
            Some(u32::from(*threshold)),
        ),
        PipelineOp::Posterize { bits } => (
            "__internal_posterize_native_bytes",
            "posterize.wgsl",
            include_str!("../../shaders/posterize.wgsl"),
            Some(u32::from(*bits)),
        ),
        _ => {
            return native_byte_composite_pipeline_spec(op)
                .ok_or_else(|| PilError::InternalError("unrecognized native byte stage".into()));
        }
    };
    Ok(spec)
}

fn native_byte_composite_pipeline_spec(op: &PipelineOp) -> Option<NativeBytePipelineSpec> {
    match op {
        PipelineOp::Multiply { .. }
        | PipelineOp::Screen { .. }
        | PipelineOp::Overlay { .. }
        | PipelineOp::HardLight { .. }
        | PipelineOp::SoftLight { .. }
        | PipelineOp::BlendModule { .. } => native_byte_blend_pipeline_spec(op),
        _ => native_byte_arithmetic_pipeline_spec(op),
    }
}

fn native_byte_blend_pipeline_spec(op: &PipelineOp) -> Option<NativeBytePipelineSpec> {
    let spec = match op {
        PipelineOp::Multiply { .. } => (
            "Multiply",
            "multiply.wgsl",
            include_str!("../../shaders/multiply.wgsl"),
            None,
        ),
        PipelineOp::Screen { .. } => (
            "Screen",
            "screen.wgsl",
            include_str!("../../shaders/screen.wgsl"),
            None,
        ),
        PipelineOp::Overlay { .. } => (
            "Overlay",
            "overlay.wgsl",
            include_str!("../../shaders/overlay.wgsl"),
            None,
        ),
        PipelineOp::HardLight { .. } => (
            "HardLight",
            "hard_light.wgsl",
            include_str!("../../shaders/hard_light.wgsl"),
            None,
        ),
        PipelineOp::SoftLight { .. } => (
            "SoftLight",
            "soft_light.wgsl",
            include_str!("../../shaders/soft_light.wgsl"),
            None,
        ),
        PipelineOp::BlendModule { alpha, .. } => (
            "BlendModule",
            "blend_module.wgsl",
            include_str!("../../shaders/blend_module.wgsl"),
            Some((*alpha as f32).to_bits()),
        ),
        _ => return None,
    };
    Some(spec)
}

fn native_byte_arithmetic_pipeline_spec(op: &PipelineOp) -> Option<NativeBytePipelineSpec> {
    let spec = match op {
        PipelineOp::Difference { .. } => (
            "Difference",
            "difference.wgsl",
            include_str!("../../shaders/difference.wgsl"),
            None,
        ),
        PipelineOp::Darker { .. } => (
            "Darker",
            "darker.wgsl",
            include_str!("../../shaders/darker.wgsl"),
            None,
        ),
        PipelineOp::Lighter { .. } => (
            "Lighter",
            "lighter.wgsl",
            include_str!("../../shaders/lighter.wgsl"),
            None,
        ),
        PipelineOp::AddModulo { .. } => (
            "AddModulo",
            "add_modulo.wgsl",
            include_str!("../../shaders/add_modulo.wgsl"),
            None,
        ),
        PipelineOp::SubtractModulo { .. } => (
            "SubtractModulo",
            "subtract_modulo.wgsl",
            include_str!("../../shaders/subtract_modulo.wgsl"),
            None,
        ),
        _ => return None,
    };
    Some(spec)
}

fn encode_native_byte_dispatch(
    gpu: &GpuInner,
    encoder: &mut wgpu::CommandEncoder,
    buffers: &BufferPool,
    dispatch: NativeByteDispatch<'_>,
) -> Result<bool, PilError> {
    let NativeByteDispatch {
        op,
        secondary,
        current_a,
        cached,
        columns,
        rows,
        words,
        param,
    } = dispatch;
    let parameters = native_byte_parameters(columns, rows, words, param, op);
    let uniform = create_sized_buffer(
        &gpu.device,
        "gpu_stream_byte_params",
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        parameters.len() * 4,
        4,
    );
    gpu.queue
        .write_buffer(&uniform, 0, bytemuck::cast_slice(&parameters));
    let (input, output) = if current_a {
        (&buffers.buf_a, &buffers.buf_b)
    } else {
        (&buffers.buf_b, &buffers.buf_a)
    };
    let entries = native_byte_bind_entries(input, output, secondary, &uniform);
    let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("gpu_stream_byte_bind"),
        layout: &cached.bind_group_layout,
        entries: &entries,
    });
    let groups = (columns.div_ceil(16), rows.div_ceil(16));
    if groups.0 > gpu.device.limits().max_compute_workgroups_per_dimension
        || groups.1 > gpu.device.limits().max_compute_workgroups_per_dimension
    {
        return Err(PilError::ValueError(
            "native byte dispatch exceeds device limit".into(),
        ));
    }
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("gpu_stream_native_bytes"),
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

fn native_byte_parameters(
    columns: u32,
    rows: u32,
    words: u32,
    param: Option<u32>,
    op: &PipelineOp,
) -> Vec<u32> {
    let mut parameters = vec![columns, rows, 9, words];
    if let Some(param) = param {
        parameters.push(param);
    }
    if matches!(op, PipelineOp::BlendModule { .. }) {
        parameters.resize(8, 0);
    }
    parameters
}

fn native_byte_bind_entries<'a>(
    input: &'a wgpu::Buffer,
    output: &'a wgpu::Buffer,
    secondary: Option<&'a EncodedGraph>,
    uniform: &'a wgpu::Buffer,
) -> Vec<wgpu::BindGroupEntry<'a>> {
    if let Some(secondary) = secondary {
        vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input.as_entire_binding(),
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
        ]
    } else {
        vec![
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
        ]
    }
}

pub(super) fn encode_graph_stages(
    plan: &Plan<'_>,
    gpu: &'static GpuInner,
    encoder: &mut wgpu::CommandEncoder,
    buffers: &mut BufferPool,
    mut current_a: bool,
    retained: &mut u64,
    dispatches: &mut u64,
    uploaded_bytes: &mut u64,
    auxiliary: &[AuxiliaryImages],
) -> Result<bool, PilError> {
    let mut stage_index = 0;
    while stage_index < plan.stages.len() {
        stage_index += encode_graph_stage(
            plan,
            gpu,
            encoder,
            buffers,
            stage_index,
            &mut current_a,
            retained,
            dispatches,
            uploaded_bytes,
            auxiliary,
        )?;
    }
    Ok(current_a)
}

fn encode_graph_stage(
    plan: &Plan<'_>,
    gpu: &'static GpuInner,
    encoder: &mut wgpu::CommandEncoder,
    buffers: &mut BufferPool,
    stage_index: usize,
    current_a: &mut bool,
    retained: &mut u64,
    dispatches: &mut u64,
    uploaded_bytes: &mut u64,
    auxiliary: &[AuxiliaryImages],
) -> Result<usize, PilError> {
    let stage = &plan.stages[stage_index];
    let secondary = stage
        .secondary
        .as_deref()
        .map(|child| encode_graph(child, gpu, encoder))
        .transpose()?;
    let tertiary = stage
        .tertiary
        .as_deref()
        .map(|child| encode_graph(child, gpu, encoder))
        .transpose()?;
    if let Some(child) = &secondary {
        *retained += child.retained;
        *uploaded_bytes += child.upload;
        *dispatches += child.dispatches;
    }
    if let Some(child) = &tertiary {
        *retained += child.retained;
        *uploaded_bytes += child.upload;
        *dispatches += child.dispatches;
    }
    if stage_index + 1 < plan.stages.len()
        && can_fuse_batch_alpha_composite_mirror(stage, &plan.stages[stage_index + 1])
    {
        *current_a = encode_alpha_composite_mirror_stage(
            gpu,
            encoder,
            buffers,
            stage,
            secondary.as_ref().ok_or_else(|| {
                PilError::InternalError("GPU alpha-composite source was not encoded".into())
            })?,
            *current_a,
        )?;
        *dispatches += 1;
        return Ok(2);
    }
    if stage.flags.rgb_blur {
        *current_a = encode_rgb_blur_stage(gpu, encoder, buffers, stage, *current_a)?;
    } else if matches!(stage.op, PipelineOp::Paste { .. }) {
        *current_a = encode_graph_paste_stage(
            gpu,
            encoder,
            buffers,
            stage,
            secondary.as_ref(),
            tertiary.as_ref(),
            *current_a,
        )?;
    } else if stage.relocation {
        *current_a = encode_relocation_stage(gpu, encoder, buffers, stage, *current_a)?;
    } else if stage.byte {
        *current_a =
            encode_byte_stage(gpu, encoder, buffers, stage, secondary.as_ref(), *current_a)?;
    } else {
        let (next_current_a, stage_retained) = encode_generic_stage(
            gpu,
            encoder,
            buffers,
            stage,
            plan.capacity,
            auxiliary,
            *current_a,
        )?;
        *current_a = next_current_a;
        *retained += stage_retained;
    }
    *dispatches += if matches!(stage.op, PipelineOp::BoxBlur { radius: 1 })
        && matches!(stage.mode.as_str(), "L" | "RGB")
    {
        1
    } else {
        gpu_dispatch_count(
            std::slice::from_ref(stage.op),
            Some(&stage.mode),
            (stage.width, stage.height),
        )
    };
    Ok(1)
}

fn encode_graph_paste_stage(
    gpu: &GpuInner,
    encoder: &mut wgpu::CommandEncoder,
    buffers: &BufferPool,
    stage: &Stage<'_>,
    secondary: Option<&EncodedGraph>,
    tertiary: Option<&EncodedGraph>,
    current_a: bool,
) -> Result<bool, PilError> {
    if matches!(stage.op, PipelineOp::Paste { mask: Some(_), .. }) {
        super::super::paste::encode_masked_paste_stage(
            gpu,
            encoder,
            buffers,
            stage,
            secondary.ok_or_else(|| {
                PilError::InternalError("GPU Paste source was not encoded".into())
            })?,
            tertiary
                .ok_or_else(|| PilError::InternalError("GPU Paste mask was not encoded".into()))?,
            current_a,
        )
    } else {
        super::super::paste::encode_paste_stage(
            gpu,
            encoder,
            buffers,
            stage,
            secondary.ok_or_else(|| {
                PilError::InternalError("GPU Paste source was not encoded".into())
            })?,
            current_a,
        )
    }
}
