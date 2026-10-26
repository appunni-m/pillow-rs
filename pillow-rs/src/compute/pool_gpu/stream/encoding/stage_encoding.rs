use super::super::super::{AuxiliaryImages, BufferPool, GpuInner, gpu_dispatch_count};
use super::super::graph::{Plan, Stage};
use super::{
    EncodedGraph, can_fuse_batch_alpha_composite_mirror, encode_alpha_composite_mirror_stage,
    encode_byte_stage, encode_generic_stage, encode_graph, encode_relocation_stage,
    encode_rgb_blur_stage,
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;

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
