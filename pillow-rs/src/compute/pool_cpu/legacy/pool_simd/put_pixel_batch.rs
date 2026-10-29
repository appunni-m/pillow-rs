//! SIMD orchestration for adjacent native `PutPixel` operations.

use super::ops::adapters;
use crate::compute::PipelineResourceTelemetry;
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

pub(super) fn try_execute(
    input: &DynamicImage,
    ops: &[PipelineOp],
    index: usize,
    mode: Option<&str>,
    resources: &mut PipelineResourceTelemetry,
) -> Result<Option<(DynamicImage, Option<String>, usize)>, PilError> {
    let mut end = index + 1;
    while end < ops.len() && matches!(&ops[end], PipelineOp::PutPixel { .. }) {
        end += 1;
    }
    let putpixel_ops = &ops[index..end];

    if let Some(thumbnail_op @ PipelineOp::Thumbnail { .. }) = ops.get(end)
        && let Some(next) = try_putpixel_thumbnail(input, putpixel_ops, thumbnail_op, mode)?
    {
        crate::compute::account_host_buffer_boundary(resources, input, &next);
        resources.fused_operation_count = resources
            .fused_operation_count
            .saturating_add((end + 1 - index) as u64);
        let next_mode = super::ops::adapters::simd_mode_after_op(thumbnail_op, mode);
        return Ok(Some((next, next_mode, end + 1)));
    }

    if let Some(next) = try_putpixel_run(input, putpixel_ops, mode)? {
        crate::compute::account_host_buffer_boundary(resources, input, &next);
        resources.fused_operation_count = resources
            .fused_operation_count
            .saturating_add(putpixel_ops.len() as u64);
        let last = putpixel_ops.last().expect("nonempty PutPixel run");
        let next_mode = super::ops::adapters::simd_mode_after_op(last, mode);
        return Ok(Some((next, next_mode, end)));
    }

    Ok(None)
}

fn try_putpixel_thumbnail(
    input: &DynamicImage,
    putpixel_ops: &[PipelineOp],
    thumbnail_op: &PipelineOp,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let prefix_supported = putpixel_ops
        .iter()
        .all(|op| adapters::simd_supports_for_image(input, op, mode));
    if !prefix_supported
        || !adapters::simd_supports_for_image(input, thumbnail_op, mode)
        || !adapters::simd_rgb_putpixel_thumbnail_fusion_supported(
            input,
            putpixel_ops,
            thumbnail_op,
            mode,
        )
    {
        return Ok(None);
    }
    let Some(reduced) =
        adapters::simd_rgb_putpixel_thumbnail_prepare(input, putpixel_ops, thumbnail_op, mode)?
    else {
        return Ok(None);
    };

    for op in putpixel_ops {
        crate::compute::begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(
            op,
        ));
        crate::compute::record_pipeline_operation_path("scalar-control");
        crate::compute::finish_pipeline_operation_telemetry();
    }
    crate::compute::begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(
        thumbnail_op,
    ));
    let next = match adapters::simd_rgb_putpixel_thumbnail_resize(&reduced, thumbnail_op, mode) {
        Ok(next) => next,
        Err(error) => {
            crate::compute::record_pipeline_operation_path("unsupported");
            crate::compute::finish_pipeline_operation_telemetry();
            return Err(error);
        }
    };
    crate::compute::finish_pipeline_operation_telemetry();
    Ok(Some(next))
}

fn try_putpixel_run(
    input: &DynamicImage,
    putpixel_ops: &[PipelineOp],
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    if putpixel_ops.len() <= 1
        || !putpixel_ops
            .iter()
            .all(|op| super::ops::adapters::simd_supports_for_image(input, op, mode))
        || !super::ops::adapters::simd_rgba_putpixel_batch_supported(input, putpixel_ops, mode)
    {
        return Ok(None);
    }
    super::ops::adapters::simd_rgba_putpixel_batch(input, putpixel_ops, mode)
}
