use super::{fused_multiply_screen, fused_point_batch, ops};
use crate::compute::registry;
use crate::compute::{Backend, ExecutionContext};
use crate::error::PilError;
use crate::pipeline::{PipelineOp, TransposeTransform};
use crate::raster::{DynamicImage, GenericImageView};

pub(super) fn execute_batch(
    context: &ExecutionContext,
    ops: &[PipelineOp],
    img: &DynamicImage,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if context.backend() != Backend::Cpu || !context.architecture().supports_cpu() {
        return Err(PilError::InternalError(format!(
            "CPU operation router received {:?}/{:?} context",
            context.backend(),
            context.architecture()
        )));
    }
    let mut result = None;
    let mut current_mode = mode.map(str::to_owned);
    let mut resources = crate::compute::host_resource_telemetry(img);
    let allow_untagged_multiply_fusion = mode.is_none();
    let mut index = 0;

    while index < ops.len() {
        let input = result.as_ref().unwrap_or(img);
        if matches!(&ops[index], PipelineOp::PutPixel { .. })
            && let Some(step) =
                try_put_pixel(ops, index, input, current_mode.as_deref(), &mut resources)?
        {
            apply_step(step, &mut result, &mut current_mode, &mut index);
            continue;
        }
        if matches!(&ops[index], PipelineOp::Transpose { .. })
            && let Some(step) = try_transpose_run(
                context,
                ops,
                index,
                input,
                current_mode.as_deref(),
                &mut resources,
            )?
        {
            apply_step(step, &mut result, &mut current_mode, &mut index);
            continue;
        }
        if matches!(
            &ops[index],
            PipelineOp::AlphaComposite {
                dest: (0, 0),
                src: (0, 0),
                ..
            }
        ) && matches!(ops.get(index + 1), Some(PipelineOp::Mirror))
            && let Some(step) = try_alpha_composite_mirror(
                ops,
                index,
                input,
                current_mode.as_deref(),
                &mut resources,
            )?
        {
            apply_step(step, &mut result, &mut current_mode, &mut index);
            continue;
        }
        if ops::draw::is_draw_op(&ops[index])
            && let Some(step) =
                try_draw_batch(ops, index, input, current_mode.as_deref(), &mut resources)?
        {
            apply_step(step, &mut result, &mut current_mode, &mut index);
            continue;
        }
        if allow_untagged_multiply_fusion
            && matches!(
                (ops.get(index), ops.get(index + 1)),
                (
                    Some(PipelineOp::Multiply { .. }),
                    Some(PipelineOp::Screen { .. })
                )
            )
            && let Some(step) =
                try_multiply_screen(ops, index, input, current_mode.as_deref(), &mut resources)?
        {
            apply_step(step, &mut result, &mut current_mode, &mut index);
            continue;
        }
        if let Some(step) = try_point_batch(
            context,
            ops,
            index,
            input,
            current_mode.as_deref(),
            &mut resources,
        )? {
            apply_step(step, &mut result, &mut current_mode, &mut index);
            continue;
        }

        let op = &ops[index];
        let next = execute_single(context, op, input, current_mode.as_deref(), &mut resources)?;
        result = Some(next);
        current_mode = crate::compute::pool_simd::ops::adapters::simd_mode_after_op(
            op,
            current_mode.as_deref(),
        );
        index += 1;
    }

    let output = result.unwrap_or_else(|| {
        let output = img.clone();
        if !ops.is_empty() {
            // Only checked, exact identity runs can leave a nonempty batch
            // borrowing its input. Return an independent buffer.
            crate::compute::record_pipeline_allocation(output.as_bytes().len());
            crate::compute::account_host_buffer_boundary(&mut resources, img, &output);
        }
        output
    });
    crate::compute::record_pipeline_resource_telemetry(resources);
    Ok(output)
}

struct BatchStep {
    output: Option<DynamicImage>,
    mode: ModeUpdate,
    next_index: usize,
}

enum ModeUpdate {
    Keep,
    Set(Option<String>),
}

fn apply_step(
    step: BatchStep,
    result: &mut Option<DynamicImage>,
    current_mode: &mut Option<String>,
    index: &mut usize,
) {
    if step.output.is_some() {
        *result = step.output;
    }
    if let ModeUpdate::Set(mode) = step.mode {
        *current_mode = mode;
    }
    *index = step.next_index;
}

fn mode_after_ops(
    ops: &[PipelineOp],
    index: usize,
    end: usize,
    mode: Option<&str>,
) -> Option<String> {
    let mut mode_after = None;
    let mut current_mode = mode;
    for op in &ops[index..end] {
        mode_after = crate::compute::pool_simd::ops::adapters::simd_mode_after_op(op, current_mode);
        current_mode = mode_after.as_deref();
    }
    mode_after
}

fn try_put_pixel(
    ops: &[PipelineOp],
    index: usize,
    input: &DynamicImage,
    mode: Option<&str>,
    resources: &mut crate::compute::PipelineResourceTelemetry,
) -> Result<Option<BatchStep>, PilError> {
    let mut end = index + 1;
    while end < ops.len() && matches!(&ops[end], PipelineOp::PutPixel { .. }) {
        end += 1;
    }
    let consumed = end - index;
    if let Some(thumbnail_op @ PipelineOp::Thumbnail { .. }) = ops.get(end)
        && ops::geometry::rgb_putpixel_thumbnail_fusion_supported(
            input,
            &ops[index..end],
            thumbnail_op,
            mode,
        )
    {
        let fused_count = consumed + 1;
        for op in &ops[index..=end] {
            crate::compute::begin_pipeline_operation_telemetry(registry::variant_key(op));
        }
        let next = ops::geometry::execute_rgb_putpixel_thumbnail_fusion(
            input,
            &ops[index..end],
            thumbnail_op,
            mode,
        );
        let next = match next {
            Ok(next) => next,
            Err(error) => {
                finish_cpu_operations(fused_count);
                return Err(error);
            }
        };
        finish_cpu_operations(fused_count);
        crate::compute::account_host_buffer_boundary(resources, input, &next);
        resources.fused_operation_count = resources
            .fused_operation_count
            .saturating_add(fused_count as u64);
        return Ok(Some(BatchStep {
            output: Some(next),
            mode: ModeUpdate::Set(mode_after_ops(ops, index, end + 1, mode)),
            next_index: end + 1,
        }));
    }

    if consumed > 1 {
        for op in &ops[index..end] {
            crate::compute::begin_pipeline_operation_telemetry(registry::variant_key(op));
        }
        let next = ops::effects::op_put_pixel_batch(input, &ops[index..end]);
        finish_cpu_operations(consumed);
        let next = next?;
        crate::compute::account_host_buffer_boundary(resources, input, &next);
        resources.fused_operation_count = resources
            .fused_operation_count
            .saturating_add(consumed as u64);
        return Ok(Some(BatchStep {
            output: Some(next),
            mode: ModeUpdate::Keep,
            next_index: end,
        }));
    }
    Ok(None)
}

fn try_transpose_run(
    context: &ExecutionContext,
    ops: &[PipelineOp],
    index: usize,
    input: &DynamicImage,
    mode: Option<&str>,
    resources: &mut crate::compute::PipelineResourceTelemetry,
) -> Result<Option<BatchStep>, PilError> {
    let PipelineOp::Transpose { method } = &ops[index] else {
        return Ok(None);
    };
    let mut combined = TransposeTransform::Method(method.clone());
    let mut consumed = 1;
    while let Some(PipelineOp::Transpose { method }) = ops.get(index + consumed) {
        combined = combined.then(method);
        consumed += 1;
    }
    if consumed == 1 {
        return Ok(None);
    }

    if matches!(combined, TransposeTransform::Identity)
        && super::super::pool_simd::transpose_identity_can_reuse(input)
    {
        for _ in 0..consumed {
            crate::compute::begin_pipeline_operation_telemetry("Transpose");
            crate::compute::record_pipeline_operation_path("cpu");
            crate::compute::finish_pipeline_operation_telemetry();
        }
        resources.fused_operation_count = resources
            .fused_operation_count
            .saturating_add(consumed as u64);
        return Ok(Some(BatchStep {
            output: None,
            mode: ModeUpdate::Keep,
            next_index: index + consumed,
        }));
    }
    if let TransposeTransform::Method(method) = combined {
        for _ in 0..consumed {
            crate::compute::begin_pipeline_operation_telemetry("Transpose");
        }
        let next = registry::execute_cpu_in_context(
            context,
            &PipelineOp::Transpose { method },
            input,
            mode,
        );
        finish_cpu_operations(consumed);
        let next = next?;
        crate::compute::account_host_buffer_boundary(resources, input, &next);
        resources.fused_operation_count = resources
            .fused_operation_count
            .saturating_add(consumed as u64);
        return Ok(Some(BatchStep {
            output: Some(next),
            mode: ModeUpdate::Keep,
            next_index: index + consumed,
        }));
    }
    Ok(None)
}

fn try_alpha_composite_mirror(
    ops: &[PipelineOp],
    index: usize,
    input: &DynamicImage,
    mode: Option<&str>,
    resources: &mut crate::compute::PipelineResourceTelemetry,
) -> Result<Option<BatchStep>, PilError> {
    if index + 1 >= ops.len() {
        return Ok(None);
    }
    let (
        PipelineOp::AlphaComposite {
            source,
            dest: (0, 0),
            src: (0, 0),
        },
        PipelineOp::Mirror,
    ) = (&ops[index], &ops[index + 1])
    else {
        return Ok(None);
    };
    if !matches!(input, DynamicImage::ImageRgba8(_))
        || !matches!(mode, None | Some("RGBA"))
        || input.width() == 0
        || input.height() == 0
        || source.size().ok() != Some(input.dimensions())
    {
        return Ok(None);
    }

    crate::compute::begin_pipeline_operation_telemetry("AlphaComposite");
    crate::compute::begin_pipeline_operation_telemetry("Mirror");
    let next = match ops::effects::op_alpha_composite_mirror(input, source) {
        Ok(next) => next,
        Err(error) => {
            finish_cpu_operations(2);
            return Err(error);
        }
    };
    finish_cpu_operations(2);
    crate::compute::account_host_buffer_boundary(resources, input, &next);
    resources.fused_operation_count = resources.fused_operation_count.saturating_add(2);
    Ok(Some(BatchStep {
        output: Some(next),
        mode: ModeUpdate::Set(mode_after_ops(ops, index, index + 2, mode)),
        next_index: index + 2,
    }))
}

fn try_draw_batch(
    ops: &[PipelineOp],
    index: usize,
    input: &DynamicImage,
    mode: Option<&str>,
    resources: &mut crate::compute::PipelineResourceTelemetry,
) -> Result<Option<BatchStep>, PilError> {
    let mut end = index + 1;
    while end < ops.len() && ops::draw::is_draw_op(&ops[end]) {
        end += 1;
    }
    if end - index < 2 {
        return Ok(None);
    }
    for op in &ops[index..end] {
        crate::compute::begin_pipeline_operation_telemetry(registry::variant_key(op));
    }
    let next = match ops::draw::execute_draw_batch(input, &ops[index..end], mode) {
        Ok(next) => next,
        Err(error) => {
            finish_cpu_operations(end - index);
            return Err(error);
        }
    };
    finish_cpu_operations(end - index);
    crate::compute::account_host_buffer_boundary(resources, input, &next);
    Ok(Some(BatchStep {
        output: Some(next),
        mode: ModeUpdate::Set(mode_after_ops(ops, index, end, mode)),
        next_index: end,
    }))
}

fn try_multiply_screen(
    ops: &[PipelineOp],
    index: usize,
    input: &DynamicImage,
    mode: Option<&str>,
    resources: &mut crate::compute::PipelineResourceTelemetry,
) -> Result<Option<BatchStep>, PilError> {
    if index + 1 >= ops.len() {
        return Ok(None);
    }
    let (
        PipelineOp::Multiply { other: first_other },
        PipelineOp::Screen {
            other: second_other,
        },
    ) = (&ops[index], &ops[index + 1])
    else {
        return Ok(None);
    };
    let Some(fused) = fused_multiply_screen(input, first_other, second_other, mode)? else {
        return Ok(None);
    };

    crate::compute::begin_pipeline_operation_telemetry("Multiply");
    crate::compute::begin_pipeline_operation_telemetry("Screen");
    finish_cpu_operations(2);
    crate::compute::account_host_buffer_boundary(resources, input, &fused);
    resources.fused_operation_count = resources.fused_operation_count.saturating_add(2);
    Ok(Some(BatchStep {
        output: Some(fused),
        mode: ModeUpdate::Set(mode_after_ops(ops, index, index + 2, mode)),
        next_index: index + 2,
    }))
}

fn try_point_batch(
    context: &ExecutionContext,
    ops: &[PipelineOp],
    index: usize,
    input: &DynamicImage,
    mode: Option<&str>,
    resources: &mut crate::compute::PipelineResourceTelemetry,
) -> Result<Option<BatchStep>, PilError> {
    let Some((consumed, fused)) = fused_point_batch(&ops[index..], input, mode) else {
        return Ok(None);
    };
    for op in &ops[index..index + consumed] {
        crate::compute::begin_pipeline_operation_telemetry(registry::variant_key(op));
    }
    // Keep the copy-on-write boundary for identity runs while avoiding a
    // second allocation and traversal of the image buffer.
    let next = match &fused {
        PipelineOp::Eval { lut }
            if super::point_band_count(input)
                .is_some_and(|bands| super::point_lut_is_identity(lut, bands)) =>
        {
            Ok(input.clone())
        }
        _ => registry::execute_cpu_in_context(context, &fused, input, mode),
    };
    let next = match next {
        Ok(next) => next,
        Err(error) => {
            finish_cpu_operations(consumed);
            return Err(error);
        }
    };
    finish_cpu_operations(consumed);
    crate::compute::account_host_buffer_boundary(resources, input, &next);
    resources.fused_operation_count = resources
        .fused_operation_count
        .saturating_add(consumed as u64);
    Ok(Some(BatchStep {
        output: Some(next),
        mode: ModeUpdate::Set(mode_after_ops(ops, index, index + consumed, mode)),
        next_index: index + consumed,
    }))
}

fn execute_single(
    context: &ExecutionContext,
    op: &PipelineOp,
    input: &DynamicImage,
    mode: Option<&str>,
    resources: &mut crate::compute::PipelineResourceTelemetry,
) -> Result<DynamicImage, PilError> {
    crate::compute::begin_pipeline_operation_telemetry(registry::variant_key(op));
    let execution = registry::execute_cpu_in_context(context, op, input, mode);
    let next = match execution {
        Ok(next) => next,
        Err(error) => {
            finish_cpu_operations(1);
            return Err(error);
        }
    };
    finish_cpu_operations(1);
    crate::compute::account_host_buffer_boundary(resources, input, &next);
    Ok(next)
}

fn finish_cpu_operations(count: usize) {
    for _ in 0..count {
        crate::compute::record_pipeline_operation_path("cpu");
        crate::compute::finish_pipeline_operation_telemetry();
    }
}
