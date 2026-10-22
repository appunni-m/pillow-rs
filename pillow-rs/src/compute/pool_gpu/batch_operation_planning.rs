use super::*;

const NATIVE_EXPAND_WORKGROUP_WIDTH: u64 = 16;
const NATIVE_EXPAND_WORKGROUP_HEIGHT: u64 = 16;
pub(super) const NATIVE_COVER_PACKED_OUTPUT_MARKER: u32 = u32::MAX - 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NativeCoverPackedOutputDispatch {
    pub(super) channels: u8,
    pub(super) groups_x: u32,
    pub(super) groups_y: u32,
    pub(super) words_per_row: u32,
    pub(super) word_count: u32,
    pub(super) transfer_bytes: u64,
    pub(super) horizontal_groups_x: u32,
    pub(super) horizontal_groups_y: u32,
    pub(super) horizontal_words_per_row: u32,
    pub(super) horizontal_word_count: u32,
    pub(super) horizontal_transfer_bytes: u64,
}

/// Plan one vertical-resize invocation per row-packed group of four native
/// bytes. L stores four pixels per word; LA stores two pixels per word. Every
/// row has its own four-byte alignment, so the final partial word is stripped
/// when the mapped rows are decoded.
pub(super) fn plan_native_cover_packed_output_dispatch(
    width: u32,
    height: u32,
    source_height: u32,
    channels: u8,
    max_workgroups_per_dimension: u32,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
    output_buffer_bytes: u64,
) -> Option<NativeCoverPackedOutputDispatch> {
    if width == 0
        || height == 0
        || source_height == 0
        || !matches!(channels, 1 | 2)
        || max_workgroups_per_dimension == 0
    {
        return None;
    }
    let pixels = u64::from(width).checked_mul(u64::from(height))?;
    if pixels > u64::from(u32::MAX) {
        return None;
    }
    let pixels_per_word = 4 / u32::from(channels);
    let words_per_row = width.div_ceil(pixels_per_word);
    let word_count = u64::from(words_per_row).checked_mul(u64::from(height))?;
    if word_count == 0 || word_count > u64::from(u32::MAX) {
        return None;
    }
    let transfer_bytes = word_count.checked_mul(std::mem::size_of::<u32>() as u64)?;
    let horizontal_word_count = u64::from(words_per_row).checked_mul(u64::from(source_height))?;
    if horizontal_word_count == 0 || horizontal_word_count > u64::from(u32::MAX) {
        return None;
    }
    let horizontal_transfer_bytes =
        horizontal_word_count.checked_mul(std::mem::size_of::<u32>() as u64)?;
    if transfer_bytes > u64::from(max_storage_buffer_binding_size)
        || transfer_bytes > max_buffer_size
        || transfer_bytes > output_buffer_bytes
        || horizontal_transfer_bytes > u64::from(max_storage_buffer_binding_size)
        || horizontal_transfer_bytes > max_buffer_size
        || horizontal_transfer_bytes > output_buffer_bytes
        || usize::try_from(transfer_bytes).is_err()
        || usize::try_from(horizontal_transfer_bytes).is_err()
    {
        return None;
    }
    let groups_x = words_per_row.div_ceil(16);
    let groups_y = height.div_ceil(16);
    let horizontal_groups_x = words_per_row.div_ceil(16);
    let horizontal_groups_y = source_height.div_ceil(16);
    if groups_x > max_workgroups_per_dimension
        || groups_y > max_workgroups_per_dimension
        || horizontal_groups_x > max_workgroups_per_dimension
        || horizontal_groups_y > max_workgroups_per_dimension
    {
        return None;
    }
    Some(NativeCoverPackedOutputDispatch {
        channels,
        groups_x,
        groups_y,
        words_per_row,
        word_count: u32::try_from(word_count).ok()?,
        transfer_bytes,
        horizontal_groups_x,
        horizontal_groups_y,
        horizontal_words_per_row: words_per_row,
        horizontal_word_count: u32::try_from(horizontal_word_count).ok()?,
        horizontal_transfer_bytes,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NativeExpandOutputDispatch {
    pub(super) groups_x: u32,
    pub(super) groups_y: u32,
    pub(super) words_per_row: u32,
    pub(super) word_count: u32,
    pub(super) transfer_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NativePackedChannelOutputDispatch {
    pub(super) groups_x: u32,
    pub(super) groups_y: u32,
    pub(super) row_tiled: bool,
    pub(super) output_word_count: u32,
    pub(super) transfer_bytes: u64,
}

/// Plan one output invocation per packed word. Each 16×16 workgroup owns 256
/// distinct words; the flattened 2D grid never assigns two invocations to the
/// same word, including when the adapter's X limit requires a second row.
pub(super) fn plan_native_expand_output_dispatch(
    output_bytes: u64,
    max_workgroups_per_dimension: u32,
) -> Option<NativeExpandOutputDispatch> {
    if output_bytes == 0 || max_workgroups_per_dimension == 0 {
        return None;
    }
    let word_count = output_bytes.checked_add(3)?.checked_div(4)?;
    let workgroup_items =
        NATIVE_EXPAND_WORKGROUP_WIDTH.checked_mul(NATIVE_EXPAND_WORKGROUP_HEIGHT)?;
    let required_workgroups = word_count.div_ceil(workgroup_items);
    let max_groups = u64::from(max_workgroups_per_dimension);
    let groups_x = required_workgroups.min(max_groups);
    let groups_y = required_workgroups.div_ceil(groups_x);
    if groups_y > max_groups {
        return None;
    }
    let words_per_row = groups_x.checked_mul(NATIVE_EXPAND_WORKGROUP_WIDTH)?;
    let transfer_bytes = word_count.checked_mul(4)?;
    Some(NativeExpandOutputDispatch {
        groups_x: u32::try_from(groups_x).ok()?,
        groups_y: u32::try_from(groups_y).ok()?,
        words_per_row: u32::try_from(words_per_row).ok()?,
        word_count: u32::try_from(word_count).ok()?,
        transfer_bytes,
    })
}

/// Plan a compact native-L affine-nearest output. The dedicated shader writes
/// four logical pixels to each storage word, so its bounds are the packed
/// readback size and its signed coordinate walk must fit i32.
pub(super) fn plan_native_luma_transform_output(
    ops: &[PipelineOp],
    max_workgroups_per_dimension: u32,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
    capacity_words: u32,
) -> Option<NativeExpandOutputDispatch> {
    let [
        PipelineOp::Transform {
            w,
            h,
            method: TransformMethod::Affine,
            data,
            filter: ResampleFilter::Nearest,
            ..
        },
    ] = ops
    else {
        return None;
    };
    if *w == 0 || *h == 0 || *w > i32::MAX as u32 || *h > i32::MAX as u32 {
        return None;
    }
    let affine: [f64; 6] = data.get(..6)?.try_into().ok()?;
    let [step_x, step_y, origin_x, step_x_y, step_y_y, origin_y] =
        crate::compute::pool_cpu::ops::effects::affine_nearest_fixed(affine, *w, *h)?;
    let max_x = i128::from(*w - 1);
    let max_y = i128::from(*h - 1);
    let coords_fit = |step_x: i64, step_y: i64, origin: i64| {
        let step_x = i128::from(step_x);
        let step_y = i128::from(step_y);
        let origin = i128::from(origin);
        [
            origin,
            origin + max_x * step_x,
            origin + max_y * step_y,
            origin + max_x * step_x + max_y * step_y,
        ]
        .into_iter()
        .all(|value| i32::try_from(value).is_ok())
            && max_x * step_x <= i128::from(i32::MAX)
            && max_x * step_x >= i128::from(i32::MIN)
            && max_y * step_y <= i128::from(i32::MAX)
            && max_y * step_y >= i128::from(i32::MIN)
    };
    if !coords_fit(step_x, step_y, origin_x) || !coords_fit(step_x_y, step_y_y, origin_y) {
        return None;
    }
    let pixels = u64::try_from(CheckedDims::new(*w, *h, 1).ok()?.total_pixels()).ok()?;
    // The shader forms the flat pixel count in u32 before deriving packed
    // output words. Keep that multiplication representable on device.
    if pixels > u64::from(u32::MAX) {
        return None;
    }
    let dispatch = plan_native_expand_output_dispatch(pixels, max_workgroups_per_dimension)?;
    let capacity_bytes = u64::from(capacity_words).checked_mul(4)?;
    (dispatch.transfer_bytes <= capacity_bytes
        && dispatch.transfer_bytes <= u64::from(max_storage_buffer_binding_size)
        && dispatch.transfer_bytes <= max_buffer_size)
        .then_some(dispatch)
}

/// Plan an affine-nearest transform that writes packed native byte channels.
/// One shader invocation samples four pixels and owns the words they occupy,
/// avoiding an RGBA-sized readback for RGB and LA.
pub(super) fn plan_native_transform_output(
    ops: &[PipelineOp],
    output_channels: u32,
    max_workgroups_per_dimension: u32,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
    capacity_words: u32,
) -> Option<NativePackedChannelOutputDispatch> {
    if !matches!(output_channels, 2 | 3) {
        return None;
    }
    let [
        PipelineOp::Transform {
            w,
            h,
            method: TransformMethod::Affine,
            data,
            filter: ResampleFilter::Nearest,
            ..
        },
    ] = ops
    else {
        return None;
    };
    if *w == 0 || *h == 0 || *w > i32::MAX as u32 || *h > i32::MAX as u32 {
        return None;
    }
    let affine: [f64; 6] = data.get(..6)?.try_into().ok()?;
    let [step_x, step_y, origin_x, step_x_y, step_y_y, origin_y] =
        crate::compute::pool_cpu::ops::effects::affine_nearest_fixed(affine, *w, *h)?;
    let max_x = i128::from(*w - 1);
    let max_y = i128::from(*h - 1);
    let coords_fit = |step_x: i64, step_y: i64, origin: i64| {
        let step_x = i128::from(step_x);
        let step_y = i128::from(step_y);
        let origin = i128::from(origin);
        let x_offsets = [0, max_x * step_x];
        let partials = x_offsets.map(|x| origin + x);
        partials
            .into_iter()
            .all(|value| i32::try_from(value).is_ok())
            && [
                origin,
                origin + max_x * step_x,
                origin + max_y * step_y,
                origin + max_x * step_x + max_y * step_y,
            ]
            .into_iter()
            .all(|value| i32::try_from(value).is_ok())
            && max_x * step_x <= i128::from(i32::MAX)
            && max_x * step_x >= i128::from(i32::MIN)
            && max_y * step_y <= i128::from(i32::MAX)
            && max_y * step_y >= i128::from(i32::MIN)
    };
    if !coords_fit(step_x, step_y, origin_x) || !coords_fit(step_x_y, step_y_y, origin_y) {
        return None;
    }

    let pixels = u64::try_from(CheckedDims::new(*w, *h, 1).ok()?.total_pixels()).ok()?;
    let output_channels = u8::try_from(output_channels).ok()?;
    let output_bytes = u64::try_from(
        CheckedDims::new(*w, *h, output_channels)
            .ok()?
            .total_bytes(),
    )
    .ok()?;
    if pixels > u64::from(u32::MAX) || output_bytes > u64::from(u32::MAX) {
        return None;
    }
    let transfer_bytes = output_bytes
        .checked_add(3)?
        .checked_div(4)?
        .checked_mul(4)?;
    let output_word_count = u32::try_from(transfer_bytes / 4).ok()?;
    let output_group_count = pixels.div_ceil(4);
    let workgroup_items =
        NATIVE_EXPAND_WORKGROUP_WIDTH.checked_mul(NATIVE_EXPAND_WORKGROUP_HEIGHT)?;
    let required_workgroups = output_group_count.div_ceil(workgroup_items);
    let max_groups = u64::from(max_workgroups_per_dimension);
    if max_groups == 0 {
        return None;
    }
    let row_groups_x =
        (*w % 4 == 0).then(|| u64::from(*w / 4).div_ceil(NATIVE_EXPAND_WORKGROUP_WIDTH));
    let row_groups_y = u64::from(*h).div_ceil(NATIVE_EXPAND_WORKGROUP_HEIGHT);
    let row_tiled_workgroups = row_groups_x.and_then(|groups_x| groups_x.checked_mul(row_groups_y));
    let row_tiled = row_groups_x.is_some_and(|groups_x| {
        groups_x <= max_groups
            && row_groups_y <= max_groups
            && row_tiled_workgroups.is_some_and(|count| count <= required_workgroups)
    });
    let (groups_x, groups_y) = if row_tiled {
        (row_groups_x?, row_groups_y)
    } else {
        let groups_x = required_workgroups.min(max_groups);
        let groups_y = required_workgroups.div_ceil(groups_x);
        if groups_y > max_groups {
            return None;
        }
        (groups_x, groups_y)
    };
    let capacity_bytes = u64::from(capacity_words).checked_mul(4)?;
    (transfer_bytes <= capacity_bytes
        && transfer_bytes <= u64::from(max_storage_buffer_binding_size)
        && transfer_bytes <= max_buffer_size)
        .then_some(NativePackedChannelOutputDispatch {
            groups_x: u32::try_from(groups_x).ok()?,
            groups_y: u32::try_from(groups_y).ok()?,
            row_tiled,
            output_word_count,
            transfer_bytes,
        })
}

pub(super) fn plan_native_rgb_transform_output(
    ops: &[PipelineOp],
    max_workgroups_per_dimension: u32,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
    capacity_words: u32,
) -> Option<NativePackedChannelOutputDispatch> {
    plan_native_transform_output(
        ops,
        3,
        max_workgroups_per_dimension,
        max_storage_buffer_binding_size,
        max_buffer_size,
        capacity_words,
    )
}

/// Plan the native RGB point/LUT path. Each invocation remaps four RGB
/// pixels and owns the three u32 words containing their 12 output bytes.
/// The packed output preserves RGB channel order while avoiding RGBA staging.
pub(super) fn plan_native_rgb_point_output(
    width: u32,
    height: u32,
    max_workgroups_per_dimension: u32,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
    capacity_words: u32,
) -> Option<NativePackedChannelOutputDispatch> {
    if width == 0 || height == 0 || max_workgroups_per_dimension == 0 {
        return None;
    }
    let pixels = u64::from(width).checked_mul(u64::from(height))?;
    if pixels > u64::from(u32::MAX) {
        return None;
    }
    let output_bytes =
        u64::try_from(CheckedDims::new(width, height, 3).ok()?.total_bytes()).ok()?;
    if output_bytes > u64::from(u32::MAX) {
        return None;
    }
    let transfer_bytes = output_bytes
        .checked_add(3)?
        .checked_div(4)?
        .checked_mul(4)?;
    let output_word_count = u32::try_from(transfer_bytes / 4).ok()?;
    let output_group_count = pixels.div_ceil(4);
    let workgroup_items =
        NATIVE_EXPAND_WORKGROUP_WIDTH.checked_mul(NATIVE_EXPAND_WORKGROUP_HEIGHT)?;
    let required_workgroups = output_group_count.div_ceil(workgroup_items);
    let max_groups = u64::from(max_workgroups_per_dimension);
    let row_groups_x =
        (width % 4 == 0).then(|| u64::from(width / 4).div_ceil(NATIVE_EXPAND_WORKGROUP_WIDTH));
    let row_groups_y = u64::from(height).div_ceil(NATIVE_EXPAND_WORKGROUP_HEIGHT);
    let row_tiled_workgroups = row_groups_x.and_then(|groups_x| groups_x.checked_mul(row_groups_y));
    let row_tiled = row_groups_x.is_some_and(|groups_x| {
        groups_x <= max_groups
            && row_groups_y <= max_groups
            && row_tiled_workgroups.is_some_and(|count| count <= required_workgroups)
    });
    let (groups_x, groups_y) = if row_tiled {
        (row_groups_x?, row_groups_y)
    } else {
        let groups_x = required_workgroups.min(max_groups);
        if groups_x == 0 {
            return None;
        }
        let groups_y = required_workgroups.div_ceil(groups_x);
        if groups_y > max_groups {
            return None;
        }
        (groups_x, groups_y)
    };
    let capacity_bytes = u64::from(capacity_words).checked_mul(4)?;
    (transfer_bytes <= capacity_bytes
        && transfer_bytes <= u64::from(max_storage_buffer_binding_size)
        && transfer_bytes <= max_buffer_size)
        .then_some(NativePackedChannelOutputDispatch {
            groups_x: u32::try_from(groups_x).ok()?,
            groups_y: u32::try_from(groups_y).ok()?,
            row_tiled,
            output_word_count,
            transfer_bytes,
        })
}

pub(super) fn plan_native_la_transform_output(
    ops: &[PipelineOp],
    max_workgroups_per_dimension: u32,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
    capacity_words: u32,
) -> Option<NativePackedChannelOutputDispatch> {
    plan_native_transform_output(
        ops,
        2,
        max_workgroups_per_dimension,
        max_storage_buffer_binding_size,
        max_buffer_size,
        capacity_words,
    )
}

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
