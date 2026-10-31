//! Serial composition of prepared CPU operation strategies.

use crate::compute::registry::support::{OperationInputs, PaletteInput};
use crate::compute::{
    Architecture, Backend, ExecutionContext, account_host_buffer_boundary,
    begin_pipeline_operation_telemetry, finish_pipeline_operation_telemetry, host_resource_telemetry,
    record_pipeline_operation_path,
};
use crate::error::PilError;
use crate::pipeline::{PipelineOp, TransposeTransform};
use crate::raster::DynamicImage;

/// Prepare and execute each operation in order, preserving the mode returned
/// by each operation as the next operation's typed input identity.
pub(super) fn execute_batch(
    ops: &[PipelineOp],
    image: &DynamicImage,
    context: &ExecutionContext,
    palette: PaletteInput<'_>,
) -> Result<DynamicImage, PilError> {
    if context.backend() != Backend::Cpu {
        return Err(PilError::InternalError(
            "CPU batch execution received a non-CPU context".into(),
        ));
    }

    let mut result = None;
    let mut resources = host_resource_telemetry(image);
    let mut index = 0;
    while index < ops.len() {
        let input = result.as_ref().unwrap_or(image);
        if let Some(step) = execute_fused_run(ops, index, input, context)? {
            if let Some(next) = step.output {
                account_host_buffer_boundary(&mut resources, input, &next);
                result = Some(next);
            }
            resources.fused_operation_count = resources
                .fused_operation_count
                .saturating_add(step.operations as u64);
            index = step.next_index;
            continue;
        }

        let op = &ops[index];
        begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(op));
        let operation_palette = match input.mode() {
            crate::raster::Mode::ImagePalette8 | crate::raster::Mode::ImagePaletteAlpha8 => {
                palette
            }
            _ => PaletteInput::default(),
        };
        let inputs = OperationInputs {
            image: input,
            palette: operation_palette,
        };
        let prepared = match crate::compute::registry::prepare_cpu(op, inputs, context) {
            Ok(prepared) => prepared,
            Err(error) => {
                record_pipeline_operation_path("cpu");
                finish_pipeline_operation_telemetry();
                return Err(error);
            }
        };
        let uses_default =
            prepared.selection().implementation.architecture == Architecture::Default;
        let output = match prepared.execute() {
            Ok(output) => output,
            Err(error) => {
                if uses_default {
                    record_pipeline_operation_path("cpu");
                }
                finish_pipeline_operation_telemetry();
                return Err(error);
            }
        };
        if uses_default {
            record_pipeline_operation_path("cpu");
        }
        finish_pipeline_operation_telemetry();
        account_host_buffer_boundary(&mut resources, input, &output);
        result = Some(output);
        index += 1;
    }

    let output = match result {
        Some(output) => output,
        None => {
            let output = image.clone();
            if !ops.is_empty() {
                crate::compute::record_pipeline_allocation(output.as_bytes().len());
                account_host_buffer_boundary(&mut resources, image, &output);
            }
            output
        }
    };
    crate::compute::record_pipeline_resource_telemetry(resources);
    Ok(output)
}

struct FusedStep {
    output: Option<DynamicImage>,
    next_index: usize,
    operations: usize,
}

fn execute_fused_run(
    ops: &[PipelineOp],
    start: usize,
    image: &DynamicImage,
    context: &ExecutionContext,
) -> Result<Option<FusedStep>, PilError> {
    if let Some(step) = execute_chops_multiply_screen_run(ops, start, image, context)? {
        return Ok(Some(step));
    }
    if matches!(&ops[start], PipelineOp::Transpose { .. }) {
        if let Some(step) = execute_transpose_run(ops, start, image, context)? {
            return Ok(Some(step));
        }
    }
    if matches!(&ops[start], PipelineOp::PutPixel { .. }) {
        let end = put_pixel_run_end(ops, start);
        if let Some(step) = execute_rgb_putpixel_thumbnail_run(ops, start, end, image, context)? {
            return Ok(Some(step));
        }
        if end - start > 1 {
            let output = execute_put_pixel_run(&ops[start..end], image, context)?;
            return Ok(Some(FusedStep {
                output: Some(output),
                next_index: end,
                operations: end - start,
            }));
        }
    }
    if matches!(&ops[start], PipelineOp::AlphaComposite { .. }) {
        if let Some(step) = execute_alpha_composite_mirror_run(ops, start, image, context)? {
            return Ok(Some(step));
        }
    }
    if let Some(run) = super::ops::imageops::helpers::point_lut::compose_run(
        &ops[start..],
        image.mode(),
    ) {
        let end = start + run.operations;
        let output = execute_point_run(&ops[start..end], image, context, run.bands, run.values)?;
        return Ok(Some(FusedStep {
            output: Some(output),
            next_index: end,
            operations: run.operations,
        }));
    }
    Ok(None)
}

fn execute_alpha_composite_mirror_run(
    ops: &[PipelineOp],
    start: usize,
    image: &DynamicImage,
    context: &ExecutionContext,
) -> Result<Option<FusedStep>, PilError> {
    let Some(mirror) = ops.get(start + 1) else {
        return Ok(None);
    };
    let strategy = match super::ops::composite::ops::prepare_mirror_fusion(
        image,
        &ops[start],
        mirror,
        context,
    ) {
        Ok(Some(strategy)) => strategy,
        Ok(None) => return Ok(None),
        Err(error) => {
            finish_failed_unprepared_run(&ops[start..start + 2]);
            return Err(error);
        }
    };
    let fused_ops = &ops[start..start + 2];
    let alpha_architecture = strategy.selection().implementation.architecture;
    let mirror_architecture = strategy.mirror_selection().implementation.architecture;
    let execution = match strategy.execute() {
        Ok(execution) => execution,
        Err(error) => {
            record_fused_operation_telemetry(
                &fused_ops[0],
                alpha_architecture,
                if alpha_architecture == Architecture::Default { "cpu" } else { "vector" },
                0,
                0,
            );
            record_fused_operation_telemetry(
                &fused_ops[1],
                mirror_architecture,
                if mirror_architecture == Architecture::Default { "cpu" } else { "native-copy" },
                0,
                0,
            );
            return Err(error);
        }
    };
    let vector = alpha_architecture != Architecture::Default;
    record_fused_operation_telemetry(
        &fused_ops[0],
        alpha_architecture,
        if vector { "vector" } else { "cpu" },
        execution.composite_vector_blocks,
        execution.composite_scalar_tail,
    );
    record_fused_operation_telemetry(
        &fused_ops[1],
        mirror_architecture,
        if vector { "native-copy" } else { "cpu" },
        execution.mirror_vector_blocks,
        execution.mirror_scalar_tail,
    );
    Ok(Some(FusedStep {
        output: Some(execution.output),
        next_index: start + 2,
        operations: 2,
    }))
}

fn record_fused_operation_telemetry(
    op: &PipelineOp,
    architecture: Architecture,
    path: &'static str,
    vector_blocks: u64,
    scalar_tail: u64,
) {
    begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(op));
    crate::compute::record_pipeline_operation_architecture(architecture);
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    record_pipeline_operation_path(path);
    finish_pipeline_operation_telemetry();
}

fn execute_chops_multiply_screen_run(
    ops: &[PipelineOp],
    start: usize,
    image: &DynamicImage,
    context: &ExecutionContext,
) -> Result<Option<FusedStep>, PilError> {
    let Some(screen) = ops.get(start + 1) else {
        return Ok(None);
    };
    let multiply = &ops[start];
    let prepared = match super::ops::chops::ops::prepare_fused_multiply_screen(
        image, multiply, screen, context,
    ) {
        Ok(Some(prepared)) => prepared,
        Ok(None) => return Ok(None),
        Err(error) => {
            finish_failed_unprepared_run(&ops[start..start + 2]);
            return Err(error);
        }
    };

    let fused_ops = &ops[start..start + 2];
    let architecture = prepared.selection().implementation.architecture;
    let uses_default = architecture == Architecture::Default;
    for op in fused_ops {
        begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(op));
    }
    let output = match prepared.execute() {
        Ok(output) => output,
        Err(error) => {
            finish_failed_prepared_run(fused_ops, architecture);
            return Err(error);
        }
    };
    let path = if uses_default { "cpu" } else { "vector" };
    for _ in fused_ops {
        crate::compute::record_pipeline_operation_architecture(architecture);
        record_pipeline_operation_path(path);
        finish_pipeline_operation_telemetry();
    }
    Ok(Some(FusedStep {
        output: Some(output),
        next_index: start + 2,
        operations: 2,
    }))
}

fn execute_rgb_putpixel_thumbnail_run(
    ops: &[PipelineOp],
    start: usize,
    putpixel_end: usize,
    image: &DynamicImage,
    context: &ExecutionContext,
) -> Result<Option<FusedStep>, PilError> {
    let Some(thumbnail) = ops.get(putpixel_end) else {
        return Ok(None);
    };
    if !crate::compute::cpu_rgb_putpixel_thumbnail_fusion_supported(
        image,
        &ops[start..putpixel_end],
        thumbnail,
        Some(image.mode()),
        context,
    ) {
        return Ok(None);
    }

    let end = putpixel_end + 1;
    let fused_ops = &ops[start..end];
    for op in fused_ops {
        begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(op));
    }
    let output = super::ops::thumbnail::ops::execute_rgb_putpixel_thumbnail(
        image,
        &ops[start..putpixel_end],
        thumbnail,
        context,
    );
    finish_cpu_operation_telemetry(fused_ops, Some(Architecture::Default));
    output.map(|output| {
        Some(FusedStep {
            output: Some(output),
            next_index: end,
            operations: fused_ops.len(),
        })
    })
}

fn execute_transpose_run(
    ops: &[PipelineOp],
    start: usize,
    image: &DynamicImage,
    context: &ExecutionContext,
) -> Result<Option<FusedStep>, PilError> {
    let PipelineOp::Transpose { method } = &ops[start] else {
        return Ok(None);
    };
    let mut transform = TransposeTransform::Method(method.clone());
    let mut end = start + 1;
    while let Some(PipelineOp::Transpose { method }) = ops.get(end) {
        transform = transform.then(method);
        end += 1;
    }
    if end - start == 1 {
        return Ok(None);
    }

    let TransposeTransform::Method(method) = transform else {
        if !super::ops::transpose::helpers::pixel::identity_can_defer_output(image) {
            return Ok(None);
        }
        for op in &ops[start..end] {
            begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(op));
            record_pipeline_operation_path("cpu");
            finish_pipeline_operation_telemetry();
        }
        return Ok(Some(FusedStep {
            output: None,
            next_index: end,
            operations: end - start,
        }));
    };

    let strategy = match super::ops::transpose::prepare_transpose(image, &method, context) {
        Ok(strategy) => strategy,
        Err(error) => {
            finish_failed_unprepared_run(&ops[start..end]);
            return Err(error);
        }
    };
    let architecture = strategy.selection().implementation.architecture;
    let uses_default = architecture == Architecture::Default;
    for op in &ops[start..end] {
        begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(op));
    }
    let output = match strategy.execute() {
        Ok(output) => output,
        Err(error) => {
            finish_failed_prepared_run(&ops[start..end], architecture);
            return Err(error);
        }
    };
    finish_prepared_transpose_run(&ops[start..end], uses_default, architecture);
    Ok(Some(FusedStep {
        output: Some(output),
        next_index: end,
        operations: end - start,
    }))
}

fn finish_failed_unprepared_run(ops: &[PipelineOp]) {
    for op in ops {
        begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(op));
        record_pipeline_operation_path("cpu");
        finish_pipeline_operation_telemetry();
    }
}

fn finish_failed_prepared_run(ops: &[PipelineOp], architecture: Architecture) {
    for _ in ops {
        crate::compute::record_pipeline_operation_architecture(architecture);
        if architecture == Architecture::Default {
            record_pipeline_operation_path("cpu");
        }
        finish_pipeline_operation_telemetry();
    }
}

fn finish_prepared_transpose_run(
    ops: &[PipelineOp],
    uses_default: bool,
    architecture: Architecture,
) {
    let path = if uses_default { "cpu" } else { "native-copy" };
    for _ in ops {
        crate::compute::record_pipeline_operation_architecture(architecture);
        record_pipeline_operation_path(path);
        finish_pipeline_operation_telemetry();
    }
}

fn execute_point_run(
    ops: &[PipelineOp],
    image: &DynamicImage,
    context: &ExecutionContext,
    bands: usize,
    lut: Vec<u8>,
) -> Result<DynamicImage, PilError> {
    if super::ops::imageops::helpers::point_lut::is_identity(&lut, bands) {
        for op in ops {
            begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(op));
            record_pipeline_operation_path("cpu");
            finish_pipeline_operation_telemetry();
        }
        return Ok(image.clone());
    }

    let fused = PipelineOp::Eval { lut: lut.into() };
    let prepared = match crate::compute::registry::prepare_cpu(
        &fused,
        OperationInputs {
            image,
            palette: PaletteInput::default(),
        },
        context,
    ) {
        Ok(prepared) => prepared,
        Err(error) => {
            finish_failed_unprepared_run(ops);
            return Err(error);
        }
    };
    let architecture = prepared.selection().implementation.architecture;
    let uses_default = architecture == Architecture::Default;
    for op in ops {
        begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(op));
    }
    let output = match prepared.execute() {
        Ok(output) => output,
        Err(error) => {
            finish_failed_prepared_run(ops, architecture);
            return Err(error);
        }
    };
    let path = if uses_default { "cpu" } else { "vector" };
    for _ in ops {
        crate::compute::record_pipeline_operation_architecture(architecture);
        record_pipeline_operation_path(path);
        finish_pipeline_operation_telemetry();
    }
    Ok(output)
}

fn put_pixel_run_end(ops: &[PipelineOp], start: usize) -> usize {
    let mut end = start + 1;
    while end < ops.len() && matches!(&ops[end], PipelineOp::PutPixel { .. }) {
        end += 1;
    }
    end
}

fn execute_put_pixel_run(
    ops: &[PipelineOp],
    image: &DynamicImage,
    context: &ExecutionContext,
) -> Result<DynamicImage, PilError> {
    use crate::compute::pool_cpu::ops::put::ops::prepare_put_pixel_batch;

    let prepared = match prepare_put_pixel_batch(image, ops, context) {
        Ok(prepared) => prepared,
        Err(error) => {
            for op in ops {
                begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(op));
            }
            finish_cpu_operation_telemetry(ops, None);
            return Err(error);
        }
    };
    let architecture = prepared.selection().implementation.architecture;
    let uses_default = architecture == Architecture::Default;
    if uses_default {
        for op in ops {
            begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(op));
        }
    }
    let output = prepared.execute();
    if uses_default {
        finish_cpu_operation_telemetry(ops, Some(architecture));
    } else if output.is_err() {
        for op in ops {
            begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(op));
            crate::compute::record_pipeline_operation_architecture(architecture);
            record_pipeline_operation_path("vector");
            finish_pipeline_operation_telemetry();
        }
    }
    output
}

fn finish_cpu_operation_telemetry(
    ops: &[PipelineOp],
    architecture: Option<Architecture>,
) {
    for _ in ops {
        if let Some(architecture) = architecture {
            crate::compute::record_pipeline_operation_architecture(architecture);
        }
        record_pipeline_operation_path("cpu");
        finish_pipeline_operation_telemetry();
    }
}

// TODO(pillow-rs#3ptb): migrate PutPixel+Thumbnail, AlphaComposite+Mirror,
// and Draw batches to prepared CPU strategies. PutPixel, transpose,
// Multiply+Screen, and byte-point runs plus identity terminals are active.
