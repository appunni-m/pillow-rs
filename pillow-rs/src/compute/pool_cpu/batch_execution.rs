//! Serial composition of prepared CPU operation strategies.

use crate::raster::GenericImageView;

use crate::compute::pool_cpu::ops::imageops::helpers::hidden_lanes as imageops_hidden_lanes;
use crate::compute::registry::support::{OperationInputs, PaletteInput, StrategySelection};
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
        index = execute_batch_step(
            ops,
            index,
            image,
            context,
            palette,
            &mut result,
            &mut resources,
        )?;
    }

    let output = finish_batch_output(ops, image, result, &mut resources);
    crate::compute::record_pipeline_resource_telemetry(resources);
    Ok(output)
}

fn execute_batch_step(
    ops: &[PipelineOp],
    index: usize,
    image: &DynamicImage,
    context: &ExecutionContext,
    palette: PaletteInput<'_>,
    result: &mut Option<DynamicImage>,
    resources: &mut crate::compute::PipelineResourceTelemetry,
) -> Result<usize, PilError> {
    let fused = {
        let input = result.as_ref().unwrap_or(image);
        execute_fused_run(ops, index, input, context)?
    };
    if let Some(step) = fused {
        let input = result.as_ref().unwrap_or(image);
        if let Some(next) = step.output {
            account_host_buffer_boundary(resources, input, &next);
            *result = Some(next);
        }
        resources.fused_operation_count = resources
            .fused_operation_count
            .saturating_add(step.operations as u64);
        return Ok(step.next_index);
    }

    let op = &ops[index];
    if execute_owned_special(op, context, result, resources)? {
        return Ok(index + 1);
    }
    execute_prepared_operation(op, image, context, palette, result, resources)?;
    Ok(index + 1)
}

fn execute_owned_special(
    op: &PipelineOp,
    context: &ExecutionContext,
    result: &mut Option<DynamicImage>,
    resources: &mut crate::compute::PipelineResourceTelemetry,
) -> Result<bool, PilError> {
    if matches!(op, PipelineOp::Invert | PipelineOp::InvertChops)
        && let Some(mut owned) = result.take()
    {
        if let Some(plan) = prepare_owned_invert(&owned, op, context) {
            begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(op));
            if let Err(error) = execute_owned_invert(&mut owned, &plan) {
                finish_failed_prepared_run(std::slice::from_ref(op), &plan.selection);
                return Err(error);
            }
            crate::compute::record_pipeline_operation_selection(&plan.selection);
            finish_pipeline_operation_telemetry();
            *result = Some(owned);
            return Ok(true);
        }
        *result = Some(owned);
    }

    if matches!(op, PipelineOp::Add { .. } | PipelineOp::Subtract { .. })
        && let Some(mut owned) = result.take()
    {
        execute_owned_chops_affine(&mut owned, op, context, resources)?;
        *result = Some(owned);
        return Ok(true);
    }
    Ok(false)
}

fn execute_prepared_operation(
    op: &PipelineOp,
    image: &DynamicImage,
    context: &ExecutionContext,
    palette: PaletteInput<'_>,
    result: &mut Option<DynamicImage>,
    resources: &mut crate::compute::PipelineResourceTelemetry,
) -> Result<(), PilError> {
    let input = result.as_ref().unwrap_or(image);
    begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(op));
    let operation_palette = match input.mode() {
        crate::raster::Mode::ImagePalette8 | crate::raster::Mode::ImagePaletteAlpha8 => palette,
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
    let uses_default = prepared.selection().implementation.architecture == Architecture::Default;
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
    account_host_buffer_boundary(resources, input, &output);
    *result = Some(output);
    Ok(())
}

fn finish_batch_output(
    ops: &[PipelineOp],
    image: &DynamicImage,
    result: Option<DynamicImage>,
    resources: &mut crate::compute::PipelineResourceTelemetry,
) -> DynamicImage {
    match result {
        Some(output) => output,
        None => {
            let output = image.clone();
            if !ops.is_empty() {
                crate::compute::record_pipeline_allocation(output.as_bytes().len());
                account_host_buffer_boundary(resources, image, &output);
            }
            output
        }
    }
}

struct OwnedInvertPlan {
    mode: crate::raster::Mode,
    selection: StrategySelection,
    channels: usize,
    kind: OwnedInvertKind,
}

#[derive(Clone, Copy)]
enum OwnedInvertKind {
    ImageOpsPoint,
    Chops,
}

fn execute_owned_chops_affine(
    image: &mut DynamicImage,
    op: &PipelineOp,
    context: &ExecutionContext,
    resources: &mut crate::compute::PipelineResourceTelemetry,
) -> Result<(), PilError> {
    begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(op));
    let plan = match super::ops::chops::ops::prepare_plan(
        OperationInputs {
            image: &*image,
            palette: PaletteInput::default(),
        },
        op,
        context,
    ) {
        Ok(plan) => plan,
        Err(error) => {
            record_pipeline_operation_path("cpu");
            finish_pipeline_operation_telemetry();
            return Err(error);
        }
    };
    let architecture = plan.selection().implementation.architecture;
    let uses_default = architecture == Architecture::Default;
    let can_execute_in_place = plan.can_execute_in_place(image);
    crate::compute::record_pipeline_operation_selection(plan.selection());
    if uses_default || !can_execute_in_place {
        let output = match plan.execute(image) {
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
        account_host_buffer_boundary(resources, image, &output);
        *image = output;
    } else if let Err(error) = plan.execute_in_place(image) {
        // The archived SIMD batch reports an admitted in-place kernel failure
        // as unsupported; keep the failed architecture route visible here.
        record_pipeline_operation_path("unsupported");
        finish_pipeline_operation_telemetry();
        return Err(error);
    }
    finish_pipeline_operation_telemetry();
    Ok(())
}

fn prepare_owned_invert(
    image: &DynamicImage,
    op: &PipelineOp,
    context: &ExecutionContext,
) -> Option<OwnedInvertPlan> {
    let mode = image.mode();
    let (support, kind) = match op {
        PipelineOp::Invert => (
            crate::compute::pool_cpu::ops::imageops::ops::INVERT_SUPPORT.for_mode(mode),
            OwnedInvertKind::ImageOpsPoint,
        ),
        PipelineOp::InvertChops => {
            (
                crate::compute::pool_cpu::ops::chops::ops::INVERT_SUPPORT.for_mode(mode),
                OwnedInvertKind::Chops,
            )
        }
        _ => return None,
    };
    if !matches!(
        mode,
        crate::raster::Mode::One
            | crate::raster::Mode::ImageLuma8
            | crate::raster::Mode::ImageLumaA8
            | crate::raster::Mode::ImageLaPremultiplied8
            | crate::raster::Mode::ImagePalette8
            | crate::raster::Mode::ImagePaletteAlpha8
            | crate::raster::Mode::ImageRgb8
            | crate::raster::Mode::ImageRgba8
            | crate::raster::Mode::ImageRgbPremultiplied8
            | crate::raster::Mode::ImageRgbx8
            | crate::raster::Mode::ImageCmyk8
            | crate::raster::Mode::ImageYCbCr8
            | crate::raster::Mode::ImageLab8
            | crate::raster::Mode::ImageHsv8
    ) {
        return None;
    }
    let channels = mode.band_count();
    let selection = super::ops::imageops::helpers::strategy::select_invert_strategy(
        support,
        image.width(),
        image.height(),
        image.as_bytes().len(),
        context,
    )
    .ok()?;
    let architecture = selection.implementation.architecture;
    (architecture != Architecture::Default).then_some(OwnedInvertPlan {
        mode,
        selection,
        channels,
        kind,
    })
}

// TODO(pillow-rs#3ptb): verify raw-byte Paste follow-on parity for both
// in-place inversion lane policies before closing this source path.
fn execute_owned_invert(image: &mut DynamicImage, plan: &OwnedInvertPlan) -> Result<(), PilError> {
    if image.mode() != plan.mode {
        return Err(PilError::InternalError(
            "owned Invert mode changed after strategy preparation".into(),
        ));
    }
    let (width, height) = image.dimensions();
    let mut hidden_lanes = image.take_hidden_pixel_lanes();
    let result = if let Some(bytes) = image.as_bytes_mut() {
        crate::compute::pool_cpu::helpers::bytes::native_invert::invert_in_place(
            bytes,
            width,
            height,
            plan.channels,
            true,
        )
    } else {
        Err(PilError::InternalError(
            "owned Invert strategy has no mutable native bytes".into(),
        ))
    };
    if let Err(error) = result {
        if !image.replace_hidden_pixel_lanes(hidden_lanes) {
            return Err(PilError::InternalError(
                "owned Invert failed to restore source physical lanes".into(),
            ));
        }
        return Err(error);
    }

    match plan.kind {
        OwnedInvertKind::ImageOpsPoint => {
            imageops_hidden_lanes::initialize_point_output_lanes(image)
        }
        OwnedInvertKind::Chops => {
            if let Some(lanes) = &mut hidden_lanes {
                lanes.map_values_in_place(|value| !value);
            }
            if image.replace_hidden_pixel_lanes(hidden_lanes) {
                Ok(())
            } else {
                Err(PilError::InternalError(
                    "owned Chops Invert physical lane extent mismatch".into(),
                ))
            }
        }
    }
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
    if crate::compute::pool_cpu::ops::draw::is_draw_op(&ops[start]) {
        let end = draw_run_end(ops, start);
        if end - start == 1 {
            return Ok(None);
        }
        let run = &ops[start..end];
        let selection = match super::ops::draw::ops::prepare_batch(image, run, context) {
            Ok(selection) => selection,
            Err(error) => {
                finish_failed_unprepared_run(run);
                return Err(error);
            }
        };
        let architecture = selection.implementation.architecture;
        let output = match super::ops::draw::ops::execute_batch(
            image,
            run,
            image.mode(),
            selection,
        ) {
            Ok(output) => output,
            Err(error) => {
                for op in run {
                    record_fused_operation_telemetry(
                        op,
                        &selection,
                        if architecture == Architecture::Default { "cpu" } else { "vector" },
                        0,
                        0,
                    );
                }
                return Err(error);
            }
        };
        for op in run {
            record_fused_operation_telemetry(
                op,
                &selection,
                if architecture == Architecture::Default { "cpu" } else { "vector" },
                0,
                0,
            );
        }
        return Ok(Some(FusedStep {
            output: Some(output),
            next_index: end,
            operations: run.len(),
        }));
    }
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

fn draw_run_end(ops: &[PipelineOp], start: usize) -> usize {
    let mut end = start + 1;
    while end < ops.len() && crate::compute::pool_cpu::ops::draw::is_draw_op(&ops[end]) {
        end += 1;
    }
    end
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
    let alpha_selection = *strategy.selection();
    let mirror_selection = *strategy.mirror_selection();
    let alpha_architecture = alpha_selection.implementation.architecture;
    let mirror_architecture = mirror_selection.implementation.architecture;
    let execution = match strategy.execute() {
        Ok(execution) => execution,
        Err(error) => {
            record_fused_operation_telemetry(
                &fused_ops[0],
                &alpha_selection,
                if alpha_architecture == Architecture::Default { "cpu" } else { "vector" },
                0,
                0,
            );
            record_fused_operation_telemetry(
                &fused_ops[1],
                &mirror_selection,
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
        &alpha_selection,
        if vector { "vector" } else { "cpu" },
        execution.composite_vector_blocks,
        execution.composite_scalar_tail,
    );
    record_fused_operation_telemetry(
        &fused_ops[1],
        &mirror_selection,
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
    selection: &StrategySelection,
    path: &'static str,
    vector_blocks: u64,
    scalar_tail: u64,
) {
    begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(op));
    crate::compute::record_pipeline_operation_selection(selection);
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
    let selection = *prepared.selection();
    let output = match prepared.execute() {
        Ok(output) => output,
        Err(error) => {
            for op in fused_ops {
                begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(op));
            }
            finish_failed_prepared_run(fused_ops, &selection);
            return Err(error);
        }
    };
    for op in fused_ops {
        record_fused_operation_telemetry(
            op,
            &selection,
            output.path,
            output.vector_blocks,
            output.scalar_tail,
        );
    }
    Ok(Some(FusedStep {
        output: Some(output.image),
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
    let Some(plan) = super::ops::thumbnail::ops::prepare_rgb_putpixel_thumbnail(
        image,
        &ops[start..putpixel_end],
        thumbnail,
        context,
    )? else {
        return Ok(None);
    };

    let end = putpixel_end + 1;
    let fused_ops = &ops[start..end];
    let selection = *plan.selection();
    for op in fused_ops {
        begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(op));
    }
    let output = plan.execute();
    finish_cpu_operation_telemetry(fused_ops, Some(&selection));
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
        // No transpose implementation runs for a cancelled sequence. Preserve
        // that fact in the receipt rather than attributing the no-op to CPU or
        // to an architecture that was never selected.
        if !super::ops::transpose::helpers::pixel::identity_can_defer_output(image) {
            return Ok(None);
        }
        for op in &ops[start..end] {
            begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(op));
            record_pipeline_operation_path("native-identity");
            finish_pipeline_operation_telemetry();
        }
        return Ok(Some(FusedStep {
            output: None,
            next_index: end,
            operations: end - start,
        }));
    };

    let strategy = match super::ops::transpose::ops::prepare_transpose(image, &method, context) {
        Ok(strategy) => strategy,
        Err(error) => {
            finish_failed_unprepared_run(&ops[start..end]);
            return Err(error);
        }
    };
    let selection = *strategy.selection();
    let architecture = selection.implementation.architecture;
    let uses_default = architecture == Architecture::Default;
    for op in &ops[start..end] {
        begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(op));
    }
    let output = match strategy.execute() {
        Ok(output) => output,
        Err(error) => {
            finish_failed_prepared_run(&ops[start..end], &selection);
            return Err(error);
        }
    };
    finish_prepared_transpose_run(&ops[start..end], uses_default, &selection);
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

fn finish_failed_prepared_run(ops: &[PipelineOp], selection: &StrategySelection) {
    let architecture = selection.implementation.architecture;
    for _ in ops {
        crate::compute::record_pipeline_operation_selection(selection);
        if architecture == Architecture::Default {
            record_pipeline_operation_path("cpu");
        }
        finish_pipeline_operation_telemetry();
    }
}

fn finish_prepared_transpose_run(
    ops: &[PipelineOp],
    uses_default: bool,
    selection: &StrategySelection,
) {
    let path = if uses_default { "cpu" } else { "native-copy" };
    for _ in ops {
        crate::compute::record_pipeline_operation_selection(selection);
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
            // No mode/architecture kernel runs for an identity LUT. The one
            // required owned clone is a native identity operation.
            record_pipeline_operation_path("native-identity");
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
    let selection = *prepared.selection();
    let architecture = selection.implementation.architecture;
    let uses_default = architecture == Architecture::Default;
    for op in ops {
        begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(op));
    }
    let output = match prepared.execute() {
        Ok(output) => output,
        Err(error) => {
            finish_failed_prepared_run(ops, &selection);
            return Err(error);
        }
    };
    let path = if uses_default { "cpu" } else { "vector" };
    for _ in ops {
        crate::compute::record_pipeline_operation_selection(&selection);
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
    let selection = *prepared.selection();
    let architecture = selection.implementation.architecture;
    let uses_default = architecture == Architecture::Default;
    if uses_default {
        for op in ops {
            begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(op));
        }
    }
    let output = prepared.execute();
    if uses_default {
        finish_cpu_operation_telemetry(ops, Some(&selection));
    } else if output.is_err() {
        for op in ops {
            begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(op));
            crate::compute::record_pipeline_operation_selection(&selection);
            record_pipeline_operation_path("vector");
            finish_pipeline_operation_telemetry();
        }
    }
    output
}

fn finish_cpu_operation_telemetry(
    ops: &[PipelineOp],
    selection: Option<&StrategySelection>,
) {
    let architecture = selection.map(|selection| selection.implementation.architecture);
    let path = match architecture {
        Some(Architecture::Default) | None => "cpu",
        Some(_) => "vector",
    };
    for _ in ops {
        if let Some(selection) = selection {
            crate::compute::record_pipeline_operation_selection(selection);
        }
        record_pipeline_operation_path(path);
        finish_pipeline_operation_telemetry();
    }
}

// TODO(pillow-rs#3ptb): verify prepared-batch output, fallback, and selection
// receipts across the frozen CPU parity matrix. Source review maps each batch
// route to its exact-mode contract: generic registry preparation; Draw runs;
// guarded AlphaComposite+Mirror; same-source Multiply+Screen; composed byte
// point LUTs; transpose runs; prepared PutPixel batches; and RGB PutPixel plus
// Thumbnail using the selected Resize/ResizeBoxed child. Owned Invert and
// Chops Add/Sub use the selected mode/architecture strategy only when its
// in-place guard admits it, otherwise they use the prepared allocating route.
