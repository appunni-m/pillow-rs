use super::{BatchPipelineOptions, PipelineSource};
use crate::pipeline::PipelineOp;

type BlurPipelineSources = (PipelineSource, PipelineSource);

pub(super) fn native_rgb_packed_blur_sources(
    options: BatchPipelineOptions,
    logical_mode: Option<&str>,
    op: &PipelineOp,
) -> Option<BlurPipelineSources> {
    if !options.native_rgb_packed_output
        || !matches!(logical_mode, None | Some("RGB"))
        || !matches!(op, PipelineOp::BoxBlur { radius: 1 })
    {
        return None;
    }
    Some((
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
    ))
}

pub(super) fn packed_native_byte_blur_sources(
    options: BatchPipelineOptions,
    logical_mode: Option<&str>,
    op: &PipelineOp,
) -> Option<BlurPipelineSources> {
    if !options.packed_native_byte_filter
        || !matches!(
            op,
            PipelineOp::GaussianBlur { .. } | PipelineOp::BoxBlur { radius: 1 }
        )
    {
        return None;
    }
    Some(if matches!(logical_mode, Some("LA")) {
        (
            (
                "__internal_blur_h_la_packed",
                "box_blur_h_la_packed.wgsl",
                include_str!("../shaders/box_blur_h_la_packed.wgsl"),
            ),
            (
                "__internal_blur_v_la_packed",
                "box_blur_v_la_packed.wgsl",
                include_str!("../shaders/box_blur_v_la_packed.wgsl"),
            ),
        )
    } else {
        (
            (
                "__internal_blur_h_luma_packed",
                "box_blur_h_luma_packed.wgsl",
                include_str!("../shaders/box_blur_h_luma_packed.wgsl"),
            ),
            (
                "__internal_blur_v_luma_packed",
                "box_blur_v_luma_packed.wgsl",
                include_str!("../shaders/box_blur_v_luma_packed.wgsl"),
            ),
        )
    })
}
