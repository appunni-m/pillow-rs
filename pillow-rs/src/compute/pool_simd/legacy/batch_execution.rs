use super::{finish_simd_batch, ops, point_batch, put_pixel_batch};
use crate::compute::registry;
use crate::error::PilError;
use crate::pipeline::{PipelineOp, TransposeTransform};
use crate::raster::DynamicImage;

pub(super) fn execute_batch(
    ops: &[PipelineOp],
    img: &DynamicImage,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    log::debug!(
        "[SIMD] {} op(s) {}x{}: {:?}",
        ops.len(),
        img.width(),
        img.height(),
        ops.iter().map(registry::variant_key).collect::<Vec<_>>()
    );

    let mut current = None;
    let mut current_mode = ops::adapters::simd_initial_mode(img, ops, mode);
    let mut resources = crate::compute::host_resource_telemetry(img);
    let mut index = 0;
    while index < ops.len() {
        let input = current.as_ref().unwrap_or(img);
        let op = &ops[index];
        let op_mode = current_mode.as_deref();
        ensure_supported(input, op, op_mode)?;

        if let Some(step) = try_fusions(ops, index, input, op_mode, &mut resources)? {
            apply_step(step, &mut current, &mut current_mode, &mut index);
            continue;
        }

        if current
            .as_ref()
            .is_some_and(|owned| ops::adapters::simd_in_place_supported(owned, op, op_mode))
        {
            execute_in_place(&mut current, op, op_mode)?;
            current_mode = ops::adapters::simd_mode_after_op(op, op_mode);
            index += 1;
            continue;
        }

        let next = execute_single(input, op, op_mode, &mut resources)?;
        current = Some(next);
        current_mode = ops::adapters::simd_mode_after_op(op, op_mode);
        index += 1;
    }

    finish_simd_batch(img, ops, current, current_mode.as_deref(), resources)
}

struct BatchStep {
    output: Option<DynamicImage>,
    mode: Option<String>,
    next_index: usize,
}

fn apply_step(
    step: BatchStep,
    current: &mut Option<DynamicImage>,
    current_mode: &mut Option<String>,
    index: &mut usize,
) {
    if step.output.is_some() {
        *current = step.output;
    }
    *current_mode = step.mode;
    *index = step.next_index;
}

fn output_step(
    output: DynamicImage,
    last_op: &PipelineOp,
    op_mode: Option<&str>,
    next_index: usize,
) -> BatchStep {
    BatchStep {
        output: Some(output),
        mode: ops::adapters::simd_mode_after_op(last_op, op_mode),
        next_index,
    }
}

fn try_fusions(
    ops: &[PipelineOp],
    index: usize,
    input: &DynamicImage,
    op_mode: Option<&str>,
    resources: &mut crate::compute::PipelineResourceTelemetry,
) -> Result<Option<BatchStep>, PilError> {
    if matches!(&ops[index], PipelineOp::PutPixel { .. })
        && let Some(step) = try_put_pixel(ops, index, input, op_mode, resources)?
    {
        return Ok(Some(step));
    }
    if let Some(step) = try_multiply_screen(ops, index, input, op_mode, resources)? {
        return Ok(Some(step));
    }
    if let Some(step) = try_alpha_composite_mirror(ops, index, input, op_mode, resources)? {
        return Ok(Some(step));
    }
    if let Some(step) = try_transpose_run(ops, index, input, op_mode, resources)? {
        return Ok(Some(step));
    }
    try_point_batch(ops, index, input, op_mode, resources)
}

fn try_put_pixel(
    ops: &[PipelineOp],
    index: usize,
    input: &DynamicImage,
    op_mode: Option<&str>,
    resources: &mut crate::compute::PipelineResourceTelemetry,
) -> Result<Option<BatchStep>, PilError> {
    let Some((next, mode, next_index)) =
        put_pixel_batch::try_execute(input, ops, index, op_mode, resources)?
    else {
        return Ok(None);
    };
    Ok(Some(BatchStep {
        output: Some(next),
        mode,
        next_index,
    }))
}

fn try_multiply_screen(
    ops: &[PipelineOp],
    index: usize,
    input: &DynamicImage,
    op_mode: Option<&str>,
    resources: &mut crate::compute::PipelineResourceTelemetry,
) -> Result<Option<BatchStep>, PilError> {
    if op_mode.is_some() || index + 1 >= ops.len() {
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
    ensure_supported(input, &ops[index + 1], op_mode)?;
    let Some((fused, vector_blocks, scalar_tail)) =
        ops::adapters::simd_fused_multiply_screen(input, first_other, second_other, op_mode)?
    else {
        return Ok(None);
    };

    crate::compute::begin_pipeline_operation_telemetry("Multiply");
    crate::compute::begin_pipeline_operation_telemetry("Screen");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::compute::finish_pipeline_operation_telemetry();
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::compute::finish_pipeline_operation_telemetry();
    crate::compute::account_host_buffer_boundary(resources, input, &fused);
    resources.fused_operation_count = resources.fused_operation_count.saturating_add(2);
    Ok(Some(output_step(
        fused,
        &ops[index + 1],
        op_mode,
        index + 2,
    )))
}

fn try_alpha_composite_mirror(
    ops: &[PipelineOp],
    index: usize,
    input: &DynamicImage,
    op_mode: Option<&str>,
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
        || !matches!(op_mode, None | Some("RGBA"))
        || input.width() < 8
        || input.height() == 0
        || source.size().ok() != Some((input.width(), input.height()))
    {
        return Ok(None);
    }
    ensure_supported(input, &ops[index + 1], op_mode)?;

    crate::compute::begin_pipeline_operation_telemetry("AlphaComposite");
    crate::compute::begin_pipeline_operation_telemetry("Mirror");
    let (fused, composite_blocks, composite_tail, mirror_blocks, mirror_tail) =
        match ops::adapters::simd_alpha_composite_mirror(input, source, op_mode) {
            Ok(result) => result,
            Err(error) => {
                for _ in 0..2 {
                    crate::compute::record_pipeline_operation_path("unsupported");
                    crate::compute::finish_pipeline_operation_telemetry();
                }
                return Err(error);
            }
        };
    crate::compute::record_pipeline_operation_vector_blocks(composite_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(composite_tail);
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::finish_pipeline_operation_telemetry();
    crate::compute::record_pipeline_operation_vector_blocks(mirror_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(mirror_tail);
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::finish_pipeline_operation_telemetry();
    crate::compute::account_host_buffer_boundary(resources, input, &fused);
    resources.fused_operation_count = resources.fused_operation_count.saturating_add(2);
    Ok(Some(output_step(
        fused,
        &ops[index + 1],
        op_mode,
        index + 2,
    )))
}

fn try_transpose_run(
    ops: &[PipelineOp],
    index: usize,
    input: &DynamicImage,
    op_mode: Option<&str>,
    resources: &mut crate::compute::PipelineResourceTelemetry,
) -> Result<Option<BatchStep>, PilError> {
    let PipelineOp::Transpose { method } = &ops[index] else {
        return Ok(None);
    };
    let (combined, consumed) = combine_transposes(ops, index, method);
    if consumed == 1 {
        return Ok(None);
    }
    ensure_all_supported(input, &ops[index..index + consumed], op_mode)?;
    if matches!(combined, TransposeTransform::Identity) {
        if !super::transpose_identity_can_reuse(input) {
            return Ok(None);
        }
        record_transpose_identity(&ops[index..index + consumed]);
        resources.fused_operation_count = resources
            .fused_operation_count
            .saturating_add(consumed as u64);
        return Ok(Some(BatchStep {
            output: None,
            mode: op_mode.map(str::to_owned),
            next_index: index + consumed,
        }));
    }
    let TransposeTransform::Method(method) = combined else {
        unreachable!("identity was handled above")
    };
    let fused = PipelineOp::Transpose { method };
    let next = execute_fused_transpose(input, &fused, op_mode, &ops[index..index + consumed])?;
    crate::compute::account_host_buffer_boundary(resources, input, &next);
    resources.fused_operation_count = resources
        .fused_operation_count
        .saturating_add(consumed as u64);
    Ok(Some(output_step(
        next,
        &ops[index + consumed - 1],
        op_mode,
        index + consumed,
    )))
}

fn combine_transposes(
    ops: &[PipelineOp],
    index: usize,
    first: &crate::pipeline::TransposeMethod,
) -> (TransposeTransform, usize) {
    let mut combined = TransposeTransform::Method(first.clone());
    let mut consumed = 1;
    while index + consumed < ops.len() {
        let PipelineOp::Transpose { method } = &ops[index + consumed] else {
            break;
        };
        combined = combined.then(method);
        consumed += 1;
    }
    (combined, consumed)
}

fn record_transpose_identity(ops: &[PipelineOp]) {
    for op in ops {
        crate::compute::begin_pipeline_operation_telemetry(registry::variant_key(op));
        crate::compute::record_pipeline_operation_path("scalar-control");
        crate::compute::finish_pipeline_operation_telemetry();
    }
}

fn execute_fused_transpose(
    input: &DynamicImage,
    fused: &PipelineOp,
    op_mode: Option<&str>,
    original_ops: &[PipelineOp],
) -> Result<DynamicImage, PilError> {
    let key = registry::variant_key(fused);
    let entry = registry::registry()?
        .get(key)
        .ok_or_else(|| PilError::ValueError(format!("SIMD: unknown op {}", key)))?;
    let f = entry
        .simd_fn
        .ok_or_else(|| PilError::ValueError(format!("SIMD: no native impl for {}", key)))?;
    for op in original_ops {
        crate::compute::begin_pipeline_operation_telemetry(registry::variant_key(op));
    }
    let next = match f(input, fused, op_mode) {
        Ok(next) => next,
        Err(error) => {
            finish_failed_fusion(original_ops.len());
            return Err(error);
        }
    };
    for _ in original_ops {
        crate::compute::record_pipeline_operation_path("native-copy");
        crate::compute::finish_pipeline_operation_telemetry();
    }
    Ok(next)
}

fn try_point_batch(
    ops: &[PipelineOp],
    index: usize,
    input: &DynamicImage,
    op_mode: Option<&str>,
    resources: &mut crate::compute::PipelineResourceTelemetry,
) -> Result<Option<BatchStep>, PilError> {
    let Some((consumed, lut)) = point_batch::fused(&ops[index..], input, op_mode) else {
        return Ok(None);
    };
    ensure_all_supported(input, &ops[index..index + consumed], op_mode)?;
    let original_ops = &ops[index..index + consumed];
    for op in original_ops {
        crate::compute::begin_pipeline_operation_telemetry(registry::variant_key(op));
    }
    let next = match execute_point_lut(input, op_mode, lut) {
        Ok(next) => next,
        Err(error) => {
            finish_failed_fusion(consumed);
            return Err(error);
        }
    };
    for _ in original_ops {
        crate::compute::record_pipeline_operation_path("vector");
        crate::compute::finish_pipeline_operation_telemetry();
    }
    crate::compute::account_host_buffer_boundary(resources, input, &next);
    resources.fused_operation_count = resources
        .fused_operation_count
        .saturating_add(consumed as u64);
    Ok(Some(output_step(
        next,
        &ops[index + consumed - 1],
        op_mode,
        index + consumed,
    )))
}

fn execute_point_lut(
    input: &DynamicImage,
    op_mode: Option<&str>,
    lut: Vec<u8>,
) -> Result<DynamicImage, PilError> {
    if let Some(native) = ops::adapters::native_point_lut(input, op_mode, &lut) {
        return Ok(native);
    }
    let fused = PipelineOp::Eval { lut: lut.into() };
    ops::adapters::simd_eval(input, &fused, op_mode)
}

fn execute_in_place(
    current: &mut Option<DynamicImage>,
    op: &PipelineOp,
    op_mode: Option<&str>,
) -> Result<(), PilError> {
    let mut owned = current
        .take()
        .expect("current.is_some() guarantees an owned image");
    crate::compute::begin_pipeline_operation_telemetry(registry::variant_key(op));
    match ops::adapters::simd_execute_in_place(&mut owned, op, op_mode) {
        Ok(true) => {
            crate::compute::finish_pipeline_operation_telemetry();
            *current = Some(owned);
            Ok(())
        }
        Ok(false) => {
            // Admission and execution share one contract; disagreement is unsupported.
            crate::compute::record_pipeline_operation_path("unsupported");
            crate::compute::finish_pipeline_operation_telemetry();
            Err(PilError::NotImplementedError(format!(
                "SIMD does not support {} for the current image layout/mode",
                registry::variant_key(op)
            )))
        }
        Err(error) => {
            crate::compute::record_pipeline_operation_path("unsupported");
            crate::compute::finish_pipeline_operation_telemetry();
            Err(error)
        }
    }
}

fn execute_single(
    input: &DynamicImage,
    op: &PipelineOp,
    op_mode: Option<&str>,
    resources: &mut crate::compute::PipelineResourceTelemetry,
) -> Result<DynamicImage, PilError> {
    let key = registry::variant_key(op);
    let entry = registry::registry()?
        .get(key)
        .ok_or_else(|| PilError::ValueError(format!("SIMD: unknown op {}", key)))?;
    let f = entry
        .simd_fn
        .ok_or_else(|| PilError::ValueError(format!("SIMD: no native impl for {}", key)))?;
    crate::compute::begin_pipeline_operation_telemetry(key);
    let next = match f(input, op, op_mode) {
        Ok(next) => next,
        Err(error) => {
            crate::compute::record_pipeline_operation_path("unsupported");
            crate::compute::finish_pipeline_operation_telemetry();
            return Err(error);
        }
    };
    crate::compute::finish_pipeline_operation_telemetry();
    crate::compute::account_host_buffer_boundary(resources, input, &next);
    Ok(next)
}

fn ensure_supported(
    input: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<(), PilError> {
    if ops::adapters::simd_supports_for_image(input, op, mode) {
        return Ok(());
    }
    let key = registry::variant_key(op);
    crate::compute::record_pipeline_operation_unsupported(key);
    Err(PilError::NotImplementedError(format!(
        "SIMD does not support {key} for the current image layout/mode"
    )))
}

fn ensure_all_supported(
    input: &DynamicImage,
    ops: &[PipelineOp],
    mode: Option<&str>,
) -> Result<(), PilError> {
    if let Some(unsupported) = ops
        .iter()
        .find(|op| !ops::adapters::simd_supports_for_image(input, op, mode))
    {
        return ensure_supported(input, unsupported, mode);
    }
    Ok(())
}

fn finish_failed_fusion(count: usize) {
    for _ in 0..count {
        crate::compute::record_pipeline_operation_path("unsupported");
        crate::compute::finish_pipeline_operation_telemetry();
    }
}
