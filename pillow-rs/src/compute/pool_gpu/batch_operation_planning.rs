use super::*;

pub(super) struct BatchOperationPreparationPlan {
    pub(super) output_dimensions: (u32, u32),
    pub(super) shader_dimensions: (u32, u32),
    pub(super) op_mode: u32,
    pub(super) source_row_base: u32,
    pub(super) native_expand_output: Option<NativeExpandOutputDispatch>,
    pub(super) native_luma_transform_output: Option<NativeExpandOutputDispatch>,
    pub(super) boxed_resize_coefficients: Option<(FilterCoeffs, FilterCoeffs)>,
    pub(super) resize_source_row_range: Option<(u32, u32)>,
}

pub(super) struct BatchPackedOutputPlans {
    pub(super) native_rgb: Option<NativePackedChannelOutputDispatch>,
    pub(super) native_la: Option<NativePackedChannelOutputDispatch>,
}

#[derive(Clone, Copy)]
pub(super) struct BatchPackedOutputPlanningContext<'a> {
    pub(super) ops: &'a [PipelineOp],
    pub(super) source_dimensions: (u32, u32),
    pub(super) native_rgb_packed_output: bool,
    pub(super) native_la_transform_output: bool,
    pub(super) max_workgroups_per_dimension: u32,
    pub(super) max_storage_buffer_binding_size: u32,
    pub(super) max_buffer_size: u64,
}

struct BoxedResizeSourceRows {
    coefficients: (FilterCoeffs, FilterCoeffs),
    source_rows: (u32, u32),
}

#[derive(Clone, Copy)]
pub(super) struct BatchOperationPlanningContext<'a> {
    pub(super) ops: &'a [PipelineOp],
    pub(super) op: &'a PipelineOp,
    pub(super) source_dimensions: (u32, u32),
    pub(super) current_mode: u32,
    pub(super) logical_mode: Option<&'a str>,
    pub(super) output_only_pipeline: bool,
    pub(super) native_expand_channels: Option<u8>,
    pub(super) packed_luma_transform_output: bool,
    pub(super) max_workgroups_per_dimension: u32,
    pub(super) max_storage_buffer_binding_size: u32,
    pub(super) max_buffer_size: u64,
}

impl GpuInner {
    pub(super) fn prepare_batch_packed_output_plans(
        &self,
        context: BatchPackedOutputPlanningContext<'_>,
        capacity_words: u32,
    ) -> Result<BatchPackedOutputPlans, PilError> {
        let native_rgb = plan_native_rgb_output(context, capacity_words)?;
        let native_la = plan_native_la_output(context, capacity_words)?;
        Ok(BatchPackedOutputPlans {
            native_rgb,
            native_la,
        })
    }

    pub(super) fn prepare_batch_operation_plan(
        &self,
        context: BatchOperationPlanningContext<'_>,
        buffers: &BufferPool,
    ) -> Result<BatchOperationPreparationPlan, PilError> {
        let (cur_w, cur_h) = context.source_dimensions;
        let (out_w, out_h) = op_output_dims(context.op, cur_w, cur_h).unwrap_or((cur_w, cur_h));
        self.validate_output_dims(buffers, out_w, out_h)?;
        let native_expand_output = plan_native_expand_output(context, (out_w, out_h))?;
        let native_luma_transform_output =
            plan_native_luma_transform_output_for_op(context, buffers.capacity)?;
        let (boxed_resize_coefficients, resize_source_row_range) =
            match boxed_resize_source_rows(context, (cur_w, cur_h), (out_w, out_h)) {
                Some(plan) => (Some(plan.coefficients), Some(plan.source_rows)),
                None => (None, None),
            };
        let shader_dimensions = shader_dimensions(
            context.op,
            context.output_only_pipeline,
            (cur_w, cur_h),
            (out_w, out_h),
        );
        let op_mode = batch_operation_mode(context.op, context.logical_mode, context.current_mode);
        let source_row_base = resize_source_row_range.map_or(0, |(first, _)| first);
        Ok(BatchOperationPreparationPlan {
            output_dimensions: (out_w, out_h),
            shader_dimensions,
            op_mode,
            source_row_base,
            native_expand_output,
            native_luma_transform_output,
            boxed_resize_coefficients,
            resize_source_row_range,
        })
    }
}

fn plan_native_rgb_output(
    context: BatchPackedOutputPlanningContext<'_>,
    capacity_words: u32,
) -> Result<Option<NativePackedChannelOutputDispatch>, PilError> {
    if !context.native_rgb_packed_output {
        return Ok(None);
    }
    let (width, height) = context.source_dimensions;
    let output_plan = if matches!(
        context.ops,
        [PipelineOp::Eval { .. }
            | PipelineOp::Equalize
            | PipelineOp::MedianFilter { size: 3 }
            | PipelineOp::MinFilter { size: 3 }
            | PipelineOp::MaxFilter { size: 3 }
            | PipelineOp::BoxBlur { radius: 1 }]
    ) {
        plan_native_rgb_point_output(
            width,
            height,
            context.max_workgroups_per_dimension,
            context.max_storage_buffer_binding_size,
            context.max_buffer_size,
            capacity_words,
        )
    } else {
        plan_native_rgb_transform_output(
            context.ops,
            context.max_workgroups_per_dimension,
            context.max_storage_buffer_binding_size,
            context.max_buffer_size,
            capacity_words,
        )
    };
    output_plan.map(Some).ok_or_else(|| {
        PilError::InternalError("GPU packed-RGB operation lost its checked output plan".into())
    })
}

fn plan_native_la_output(
    context: BatchPackedOutputPlanningContext<'_>,
    capacity_words: u32,
) -> Result<Option<NativePackedChannelOutputDispatch>, PilError> {
    if !context.native_la_transform_output {
        return Ok(None);
    }
    plan_native_la_transform_output(
        context.ops,
        context.max_workgroups_per_dimension,
        context.max_storage_buffer_binding_size,
        context.max_buffer_size,
        capacity_words,
    )
    .map(Some)
    .ok_or_else(|| {
        PilError::InternalError("GPU packed-LA transform lost its checked output plan".into())
    })
}

fn plan_native_expand_output(
    context: BatchOperationPlanningContext<'_>,
    output_dimensions: (u32, u32),
) -> Result<Option<NativeExpandOutputDispatch>, PilError> {
    let Some(channels) = context.native_expand_channels else {
        return Ok(None);
    };
    // CMYK already occupies the generic four-byte GPU transport. Its compact
    // word loop adds index arithmetic without reducing I/O.
    if context.ops.len() != 1
        || !matches!(context.op, PipelineOp::Expand { .. })
        || context.logical_mode == Some("CMYK")
    {
        return Ok(None);
    }
    let output_bytes =
        CheckedDims::new(output_dimensions.0, output_dimensions.1, channels)?.total_bytes();
    let output_bytes = u64::try_from(output_bytes)
        .map_err(|_| PilError::ValueError("GPU Expand output is too large".into()))?;
    Ok(plan_native_expand_output_dispatch(
        output_bytes,
        context.max_workgroups_per_dimension,
    ))
}

fn plan_native_luma_transform_output_for_op(
    context: BatchOperationPlanningContext<'_>,
    capacity: u32,
) -> Result<Option<NativeExpandOutputDispatch>, PilError> {
    if !context.packed_luma_transform_output
        || context.ops.len() != 1
        || !matches!(context.op, PipelineOp::Transform { .. })
    {
        return Ok(None);
    }
    plan_native_luma_transform_output(
        std::slice::from_ref(context.op),
        context.max_workgroups_per_dimension,
        context.max_storage_buffer_binding_size,
        context.max_buffer_size,
        capacity,
    )
    .map(Some)
    .ok_or_else(|| {
        PilError::InternalError("GPU packed-L transform lost its checked output plan".into())
    })
}

fn boxed_resize_source_rows(
    context: BatchOperationPlanningContext<'_>,
    input_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
) -> Option<BoxedResizeSourceRows> {
    let PipelineOp::ResizeBoxed {
        filter, box_coords, ..
    } = context.op
    else {
        return None;
    };
    let (horizontal, mut vertical) =
        gpu_resize_boxed_coefficients(input_dimensions, output_dimensions, *box_coords, *filter);
    let source_rows = compact_resize_coeffs_to_source_span(&mut vertical, input_dimensions.1)
        .unwrap_or((0, input_dimensions.1));
    Some(BoxedResizeSourceRows {
        coefficients: (horizontal, vertical),
        source_rows,
    })
}

fn shader_dimensions(
    op: &PipelineOp,
    output_only_pipeline: bool,
    input_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
) -> (u32, u32) {
    if output_only_pipeline || cached_pipeline_uses_output_dimensions(op) {
        output_dimensions
    } else {
        input_dimensions
    }
}

fn cached_pipeline_uses_output_dimensions(op: &PipelineOp) -> bool {
    matches!(
        op,
        PipelineOp::Transpose {
            method: TransposeMethod::Rotate90
                | TransposeMethod::Rotate270
                | TransposeMethod::Transpose
                | TransposeMethod::Transverse
        }
    )
}

fn batch_operation_mode(op: &PipelineOp, logical_mode: Option<&str>, current_mode: u32) -> u32 {
    if matches!(op, PipelineOp::Reduce { .. })
        && matches!(logical_mode, Some("RGBa" | "RGBX" | "PA" | "La"))
    {
        4
    } else {
        current_mode
    }
}
