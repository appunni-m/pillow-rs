//! Admission planning for bounded lazy GPU operation graphs.

mod layout;
mod resources;

use super::super::{
    GPU_BUFFER_CAPACITY, GpuInner, MAX_GPU_OPS_PER_SUBMISSION, MAX_GPU_SHADER_WORK_ITEMS,
    gpu_buffer_capacity_exceeds_limits,
};
use super::native_channels;
use crate::Image;
use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::GpuPendingGraph;
use crate::pipeline::PipelineOp;

pub(super) fn planned_channels(mode: &str) -> Result<u8, PilError> {
    native_channels(mode).ok_or_else(|| {
        PilError::InternalError(format!("GPU plan has no native channel layout for {mode}"))
    })
}

// Encoding plans refer to the canonical operations. Layout flags select existing
// native shaders; no second public operation class or host image evaluator exists.
#[derive(Default, Clone, Copy)]
pub(super) struct Flags {
    pub(super) extract: bool,
    pub(super) luma_point: bool,
    pub(super) luma_order: bool,
    pub(super) byte_filter: bool,
    pub(super) rgb: bool,
    pub(super) rgb_blur: bool,
    pub(super) grayscale_rgb: bool,
    pub(super) luma_convert: bool,
    pub(super) luma_colorize: bool,
}
pub(super) struct Stage<'a> {
    pub(super) op: &'a PipelineOp,
    pub(super) secondary: Option<Box<Plan<'a>>>,
    pub(super) tertiary: Option<Box<Plan<'a>>>,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) mode: String,
    pub(super) flags: Flags,
    pub(super) byte: bool,
    pub(super) relocation: bool,
    pub(super) ordered_f_resize: bool,
    pub(super) arena_bytes: u64,
}
pub(super) struct Plan<'a> {
    pub(super) source: GpuPendingGraph<'a>,
    pub(super) stages: Vec<Stage<'a>>,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) mode: String,
    pub(super) capacity: u32,
    pub(super) gpu_bytes: u64,
    pub(super) host_bytes: u64,
    pub(super) metadata_bytes: u64,
    pub(super) shader_work: u64,
    pub(super) op_count: usize,
}

#[derive(Default)]
struct GraphBounds {
    work: u64,
    ops: usize,
}
impl GraphBounds {
    fn add(&mut self, work: u64) -> Result<(), PilError> {
        let total = self
            .work
            .checked_add(work)
            .ok_or_else(|| PilError::ValueError("GPU work arithmetic overflow".into()))?;
        if total > MAX_GPU_SHADER_WORK_ITEMS || self.ops >= MAX_GPU_OPS_PER_SUBMISSION {
            return Err(PilError::ValueError(
                "GPU graph exceeds cumulative submission work/operation bounds".into(),
            ));
        }
        self.work = total;
        self.ops += 1;
        Ok(())
    }
}
pub(super) static LUMA_BAND: PipelineOp = PipelineOp::ExtractBand { index: 0 };

pub(super) fn plan_graph<'a>(image: &'a Image, gpu: &GpuInner) -> Result<Plan<'a>, PilError> {
    let mut bounds = GraphBounds::default();
    plan_graph_inner(image, gpu, 0, &mut bounds)
}

fn plan_graph_inner<'a>(
    image: &'a Image,
    gpu: &GpuInner,
    depth: usize,
    bounds: &mut GraphBounds,
) -> Result<Plan<'a>, PilError> {
    if depth > 64 {
        return Err(PilError::ValueError(
            "GPU auxiliary graph exceeds 64 levels".into(),
        ));
    }
    let starting_work = bounds.work;
    let starting_ops = bounds.ops;
    if !cfg!(target_endian = "little") {
        return Err(PilError::ValueError(
            "native GPU batching requires little-endian samples".into(),
        ));
    }
    let mut source = image.gpu_pending_graph()?;
    let mut mode = source.mode.clone();
    let mut width = source.width;
    let mut height = source.height;
    let channels = native_channels(&mode)
        .ok_or_else(|| PilError::ValueError(format!("no native GPU graph layout for {mode}")))?;
    let source_bytes = CheckedDims::new(width, height, channels)?.total_bytes();
    let source_pixels = CheckedDims::new(width, height, 1)?.total_pixels();
    if width == 0 || height == 0 || source_pixels > GPU_BUFFER_CAPACITY as usize {
        return Err(PilError::ValueError(
            "GPU graph dimensions exceed native bounds".into(),
        ));
    }
    let ops = std::mem::take(&mut source.ops);
    let limits = gpu.device.limits();
    let mut capacity = u32::try_from(source_pixels)
        .map_err(|_| PilError::ValueError("GPU shader index overflow".into()))?;
    let mut stages = Vec::new();
    let mut ops = ops.into_iter().peekable();
    while let Some(op) = ops.next() {
        let followed_by_mirror = matches!(ops.peek().copied(), Some(PipelineOp::Mirror));
        let planned = layout::plan_stage(
            op,
            followed_by_mirror,
            &mode,
            (width, height),
            depth,
            gpu,
            &limits,
            bounds,
        )?;
        capacity = capacity.max(planned.capacity);
        stages.push(planned.stage);
        mode = planned.next_mode;
        (width, height) = planned.output;
    }
    if gpu_buffer_capacity_exceeds_limits(
        capacity,
        limits.max_storage_buffer_binding_size,
        limits.max_buffer_size,
    ) {
        return Err(PilError::ValueError(
            "GPU graph storage exceeds device binding limit".into(),
        ));
    }
    let resources = resources::account(
        image,
        source.encoded_bytes,
        source_bytes,
        &stages,
        (width, height),
        &mode,
        capacity,
    )?;
    Ok(Plan {
        source,
        stages,
        width,
        height,
        mode,
        capacity,
        gpu_bytes: resources.gpu_bytes,
        host_bytes: resources.host_bytes,
        metadata_bytes: resources.metadata_bytes,
        shader_work: bounds.work - starting_work,
        op_count: bounds.ops - starting_ops,
    })
}

fn rgba_standalone_native(op: &PipelineOp) -> bool {
    // Reject incomplete channel tables before source decode/upload. The
    // ordinary encoder also validates LUTs, but that is too late for admission.
    if let PipelineOp::Eval { lut } = op {
        return lut.len() == 4 * 256;
    }
    matches!(
        op,
        PipelineOp::Brightness { .. }
            | PipelineOp::ColorSaturation { .. }
            | PipelineOp::Filter3x3 { .. }
            | PipelineOp::Filter5x5 { .. }
            | PipelineOp::BoxBlur { .. }
            | PipelineOp::BoxBlurXY { .. }
            | PipelineOp::GaussianBlur { .. }
            | PipelineOp::MedianFilter { .. }
            | PipelineOp::MinFilter { .. }
            | PipelineOp::MaxFilter { .. }
            | PipelineOp::RankFilter { .. }
            | PipelineOp::Crop { .. }
            | PipelineOp::Transpose { .. }
            | PipelineOp::Flip
            | PipelineOp::Mirror
            | PipelineOp::Expand { .. }
            | PipelineOp::PutPixel {
                palette_index: false,
                ..
            }
            | PipelineOp::ExtractBand { .. }
            | PipelineOp::Grayscale
            | PipelineOp::Constant { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::{
        GraphBounds, MAX_GPU_OPS_PER_SUBMISSION, MAX_GPU_SHADER_WORK_ITEMS, rgba_standalone_native,
    };
    use crate::pipeline::PipelineOp;
    use std::sync::Arc;

    #[test]
    fn rgba_point_admission_requires_all_four_channel_tables() {
        for length in [0, 256, 768, 1023, 1025] {
            let op = PipelineOp::Eval {
                lut: Arc::from(vec![0; length]),
            };
            assert!(!rgba_standalone_native(&op), "invalid LUT length {length}");
        }
        let op = PipelineOp::Eval {
            lut: Arc::from((0..4).flat_map(|_| 0u8..=255).collect::<Vec<_>>()),
        };
        assert!(rgba_standalone_native(&op));
    }

    #[test]
    fn graph_budget_checks_aggregate_boundaries_without_allocations() {
        let mut bounds = GraphBounds::default();
        bounds.add(MAX_GPU_SHADER_WORK_ITEMS - 1).unwrap();
        bounds.add(1).unwrap();
        assert!(bounds.add(1).is_err());
        assert_eq!(bounds.work, MAX_GPU_SHADER_WORK_ITEMS);
        let mut bounds = GraphBounds::default();
        for _ in 0..MAX_GPU_OPS_PER_SUBMISSION {
            bounds.add(0).unwrap();
        }
        assert!(bounds.add(0).is_err());
        let mut overflow = GraphBounds {
            work: u64::MAX,
            ops: 0,
        };
        assert!(overflow.add(1).is_err());
    }
}
