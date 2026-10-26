use crate::compute::registry;
use crate::pipeline::PipelineOp;

use super::gpu_blur_pass_work_items;

#[inline]
pub(super) fn gpu_shader_inner_work_items(
    op: &PipelineOp,
    source_dimensions: (u32, u32),
    logical_mode: Option<&str>,
) -> Option<u64> {
    let (source_w, source_h) = source_dimensions;
    let inner_work = match op {
        PipelineOp::BoxBlur { radius } => gpu_blur_pass_work_items(*radius).saturating_mul(2),
        PipelineOp::BoxBlurXY {
            radius_x,
            radius_y,
            passes,
        } => {
            let radius_x = registry::separable_box_blur_params_f32(*radius_x)?[0];
            let radius_y = registry::separable_box_blur_params_f32(*radius_y)?[0];
            gpu_blur_pass_work_items(radius_x)
                .saturating_add(gpu_blur_pass_work_items(radius_y))
                .saturating_mul(u64::from((*passes).max(1)))
        }
        PipelineOp::GaussianBlur { sigma } => {
            let radius = registry::separable_gaussian_blur_radius(*sigma)?;
            gpu_blur_pass_work_items(radius).saturating_mul(6)
        }
        PipelineOp::MedianFilter { size } | PipelineOp::RankFilter { size, .. } => {
            // These shaders insertion-sort four channel arrays. The sort is
            // quadratic in the window area, so size² alone understates the
            // work by orders of magnitude and can admit watchdog-triggering
            // dispatches for the supported 9x9 maximum.
            let area = u64::from(*size).saturating_mul(u64::from(*size));
            let channels = match logical_mode {
                Some("F" | "I" | "L") => 1,
                Some("LA") => 2,
                Some("RGB" | "YCbCr" | "HSV") => 3,
                _ => 4,
            };
            area.saturating_mul(area).saturating_mul(channels)
        }
        PipelineOp::MaxFilter { size } | PipelineOp::MinFilter { size } => u64::from(*size)
            .saturating_mul(u64::from(*size))
            .saturating_mul(4),
        PipelineOp::Filter3x3 { .. } => 9,
        PipelineOp::Filter5x5 { .. } => 25,
        PipelineOp::Reduce { x_factor, y_factor } => {
            let block_w = u64::from((*x_factor).max(1).min(source_w.max(1)));
            let block_h = u64::from((*y_factor).max(1).min(source_h.max(1)));
            block_w.saturating_mul(block_h)
        }
        PipelineOp::PutData { mode, .. } => mode.channels() as u64,
        PipelineOp::Pad { .. } => 4,
        // Even a constant-work shader consumes one invocation per output
        // pixel. Count that work so a long point-operation chain is split
        // before its cumulative dispatch cost can monopolize one submission.
        _ => 1,
    };
    Some(inner_work)
}
