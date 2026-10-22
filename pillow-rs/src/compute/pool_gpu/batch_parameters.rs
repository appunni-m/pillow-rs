use super::*;

#[derive(Clone, Copy)]
pub(super) struct BatchParameterLayouts {
    pub(super) packed_luma_colorize: bool,
    pub(super) packed_luma_convert: bool,
    pub(super) packed_luma_point: bool,
    pub(super) packed_luma_putdata: bool,
    pub(super) packed_luma_pad: bool,
    pub(super) packed_luma_transform_input: bool,
    pub(super) packed_luma_transform_output: bool,
    pub(super) native_extract_band: bool,
    pub(super) native_grayscale_rgb_input: bool,
    pub(super) native_sharpness_l_input: bool,
    pub(super) native_reduce_rgb_input: bool,
    pub(super) native_expand_channels: Option<u8>,
    pub(super) native_cover_input: bool,
    pub(super) native_rgb_compact_input: bool,
    pub(super) native_rgb_packed_output: Option<NativePackedChannelOutputDispatch>,
    pub(super) native_la_transform_output: Option<NativePackedChannelOutputDispatch>,
    pub(super) native_expand_output: Option<NativeExpandOutputDispatch>,
    pub(super) native_cover_packed_output: Option<NativeCoverPackedOutputDispatch>,
}

#[derive(Clone, Copy)]
pub(super) struct BatchResizeParameterExactness {
    pub(super) constant_bits: Option<u32>,
    pub(super) box_copy_is_exact: bool,
    pub(super) identity_is_exact: bool,
    pub(super) box_average_is_exact: bool,
    pub(super) dyadic_is_exact: bool,
    pub(super) f64_is_exact: bool,
    pub(super) f64_ordered_is_exact: bool,
}

#[derive(Clone, Copy)]
pub(super) struct BatchOperationParameterContext<'a> {
    pub(super) op: &'a PipelineOp,
    pub(super) auxiliary: &'a AuxiliaryImages,
    pub(super) source_dimensions: (u32, u32),
    pub(super) output_dimensions: (u32, u32),
    pub(super) shader_dimensions: (u32, u32),
    pub(super) op_mode: u32,
    pub(super) logical_mode: Option<&'a str>,
    pub(super) contrast_mean: Option<u8>,
    pub(super) source_row_base: u32,
    pub(super) max_workgroups_per_dimension: u32,
    pub(super) resize_exactness: BatchResizeParameterExactness,
    pub(super) layouts: BatchParameterLayouts,
}

pub(super) fn prepare_batch_operation_params(
    context: BatchOperationParameterContext<'_>,
) -> Result<Vec<u32>, PilError> {
    let (shader_w, shader_h) = context.shader_dimensions;
    let mut params = vec![shader_w, shader_h, context.op_mode, context.source_row_base];
    apply_operation_source_flags(context, &mut params)?;
    append_operation_layout_params(context, &mut params)?;
    append_operation_params(context, &mut params)?;
    append_contrast_midpoint(context, &mut params)?;
    append_output_dimensions(context, &mut params);
    Ok(params)
}

fn apply_operation_source_flags(
    context: BatchOperationParameterContext<'_>,
    params: &mut [u32],
) -> Result<(), PilError> {
    if let PipelineOp::Paste { mask, .. } = context.op {
        set_paste_mask_flags(context, mask.as_ref(), params);
    }
    if context.layouts.native_extract_band && matches!(context.op, PipelineOp::ExtractBand { .. }) {
        params[3] = 1;
    }
    if context.layouts.native_grayscale_rgb_input && matches!(context.op, PipelineOp::Grayscale) {
        params[3] = u32::from(context.logical_mode == Some("YCbCr")) + 1;
    }
    if context.layouts.native_sharpness_l_input
        && matches!(context.op, PipelineOp::Sharpness { .. })
    {
        let (width, height) = context.source_dimensions;
        params[3] = u32::from(
            plan_sharpness_l_dispatch(width, height, context.max_workgroups_per_dimension)?
                .row_aligned,
        );
    }
    if context.layouts.native_reduce_rgb_input && matches!(context.op, PipelineOp::Reduce { .. }) {
        params[3] = 1;
    }
    if context.layouts.native_expand_channels.is_some()
        && matches!(context.op, PipelineOp::Expand { .. })
    {
        params[3] = 1 | (u32::from(context.layouts.native_expand_output.is_some()) << 1);
    }
    if context.layouts.native_cover_input && matches!(context.op, PipelineOp::Resize { .. }) {
        params[3] = if context.layouts.native_cover_packed_output.is_some() {
            NATIVE_COVER_PACKED_OUTPUT_MARKER
        } else {
            u32::MAX
        };
    }
    Ok(())
}

fn set_paste_mask_flags(
    context: BatchOperationParameterContext<'_>,
    mask: Option<&Arc<crate::image::Image>>,
    params: &mut [u32],
) {
    if mask.is_some_and(|mask| mask.mode().ok().as_deref() == Some("RGBa")) {
        params[3] |= 1;
    }
    if mask.is_some_and(|mask| mask.mode().ok().as_deref() == Some("1")) {
        params[3] |= 4;
    }
    if mask.is_some()
        && (matches!(context.logical_mode, Some("I;16B"))
            || (matches!(context.logical_mode, Some("I;16" | "I;16N") | None)
                && cfg!(target_endian = "big")))
    {
        params[3] |= 2;
    }
}

fn append_operation_layout_params(
    context: BatchOperationParameterContext<'_>,
    params: &mut [u32],
) -> Result<(), PilError> {
    let (width, height) = context.source_dimensions;
    if context.layouts.packed_luma_putdata && matches!(context.op, PipelineOp::PutData { .. }) {
        let pixel_count = CheckedDims::new(width, height, 1)?.total_pixels();
        params[0] = u32::try_from(pixel_count.div_ceil(4)).map_err(|_| {
            PilError::ValueError(
                "GPU PutData dimensions exceed packed shader indexing limits".into(),
            )
        })?;
        params[1] = 1;
        params[2] = 0;
        params[3] = u32::try_from(pixel_count)
            .map_err(|_| PilError::ValueError("GPU PutData image is too large".into()))?;
    } else if context.layouts.packed_luma_point && matches!(context.op, PipelineOp::Eval { .. }) {
        set_packed_luma_point_params(width, height, params)?;
    } else if context.layouts.native_rgb_packed_output.is_some()
        && matches!(
            context.op,
            PipelineOp::Eval { .. }
                | PipelineOp::Equalize
                | PipelineOp::MedianFilter { size: 3 }
                | PipelineOp::MinFilter { size: 3 }
                | PipelineOp::MaxFilter { size: 3 }
                | PipelineOp::BoxBlur { radius: 1 }
        )
    {
        set_native_rgb_output_params(context, params)?;
    } else if context.layouts.packed_luma_colorize
        && matches!(context.op, PipelineOp::Colorize { .. })
    {
        params[3] = 1;
    } else if context.layouts.packed_luma_convert
        && matches!(context.op, PipelineOp::Convert { .. })
    {
        params[3] = 1;
    } else if context.layouts.native_rgb_compact_input
        && matches!(context.op, PipelineOp::Convert { .. })
    {
        params[3] = 2;
    }
    Ok(())
}

fn set_packed_luma_point_params(
    width: u32,
    height: u32,
    params: &mut [u32],
) -> Result<(), PilError> {
    let pixel_count = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| PilError::ValueError("GPU Point image is too large".into()))?;
    params[0] = u32::try_from(pixel_count.div_ceil(4)).map_err(|_| {
        PilError::ValueError("GPU Point dimensions exceed shader indexing limits".into())
    })?;
    params[1] = 1;
    params[2] = 0;
    params[3] = 0;
    Ok(())
}

fn set_native_rgb_output_params(
    context: BatchOperationParameterContext<'_>,
    params: &mut [u32],
) -> Result<(), PilError> {
    let (width, height) = context.source_dimensions;
    match context.op {
        PipelineOp::Eval { .. } | PipelineOp::Equalize => {
            let plan = native_rgb_output_plan(
                context,
                "packed-RGB point parameters have no checked output plan",
            )?;
            params[0] = width;
            params[1] = height;
            if matches!(context.op, PipelineOp::Eval { .. }) {
                params[2] = 0;
            }
            params[3] = if plan.row_tiled { 16 } else { 0 };
        }
        PipelineOp::MedianFilter { size: 3 } => {
            let plan = native_rgb_output_plan(
                context,
                "packed-RGB median parameters have no checked output plan",
            )?;
            params[0] = width;
            params[1] = height;
            params[3] = if plan.row_tiled { 16 } else { 0 };
        }
        PipelineOp::MaxFilter { size: 3 } => {
            let plan = native_rgb_output_plan(
                context,
                "packed-RGB MaxFilter parameters have no checked output plan",
            )?;
            params[0] = width;
            params[1] = height;
            params[3] = if plan.row_tiled { 16 } else { 0 };
        }
        PipelineOp::MinFilter { size: 3 } => {
            let plan = native_rgb_output_plan(
                context,
                "packed-RGB MinFilter parameters have no checked output plan",
            )?;
            params[0] = width;
            params[1] = height;
            params[3] = if plan.row_tiled { 16 } else { 0 };
        }
        PipelineOp::BoxBlur { radius: 1 } => {
            let plan = native_rgb_output_plan(
                context,
                "packed-RGB BoxBlur parameters have no checked output plan",
            )?;
            params[0] = width;
            params[1] = height;
            params[3] = if plan.row_tiled { 16 } else { 0 };
        }
        _ => {}
    }
    Ok(())
}

fn native_rgb_output_plan(
    context: BatchOperationParameterContext<'_>,
    error: &'static str,
) -> Result<NativePackedChannelOutputDispatch, PilError> {
    context
        .layouts
        .native_rgb_packed_output
        .ok_or_else(|| PilError::InternalError(error.into()))
}

fn append_operation_params(
    context: BatchOperationParameterContext<'_>,
    params: &mut Vec<u32>,
) -> Result<(), PilError> {
    if append_resize_params(context, params)? || append_pad_params(context, params)? {
        return Ok(());
    }
    match context.op {
        PipelineOp::EqualizeMasked { .. } => {
            params[3] = u32::from(context.auxiliary.third.is_some());
        }
        PipelineOp::Autocontrast { cutoff, .. } => {
            append_autocontrast_params(context, *cutoff, params)?
        }
        PipelineOp::Transform { .. } => append_transform_params(context, params)?,
        PipelineOp::Color3DLut {
            size,
            channels,
            source_mode,
            ..
        } => {
            append_color_3d_lut_params(context, *size, *channels, *source_mode, params);
        }
        PipelineOp::Fit { filter, .. } => append_fit_params(context, *filter, params),
        PipelineOp::Colorize { .. } => {}
        _ => params.extend(registry::extract_params(context.op)),
    }
    Ok(())
}

fn append_resize_params(
    context: BatchOperationParameterContext<'_>,
    params: &mut Vec<u32>,
) -> Result<bool, PilError> {
    let Some(filter) = resize_filter(context.op) else {
        return Ok(false);
    };
    let (source_w, source_h) = context.source_dimensions;
    let (out_w, out_h) = context.output_dimensions;
    if matches!(filter, ResampleFilter::Nearest) {
        let nearest_mode = if context.logical_mode == Some("F") {
            7
        } else {
            u32::from(gpu_resize_should_premultiply(
                context.op_mode,
                context.logical_mode,
                ResampleFilter::Nearest,
            ))
        };
        params.extend([
            out_w,
            out_h,
            gpu_resize_channel_count(context.op_mode),
            nearest_mode,
        ]);
        return Ok(true);
    }
    append_filtered_resize_params(
        context,
        filter,
        params,
        (source_w, source_h),
        (out_w, out_h),
    );
    Ok(true)
}

fn resize_filter(op: &PipelineOp) -> Option<ResampleFilter> {
    match op {
        PipelineOp::Resize { filter, .. } | PipelineOp::ResizeBoxed { filter, .. } => Some(*filter),
        _ => None,
    }
}

fn append_filtered_resize_params(
    context: BatchOperationParameterContext<'_>,
    filter: ResampleFilter,
    params: &mut Vec<u32>,
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
) {
    let (cur_w, cur_h) = source_dimensions;
    let (out_w, out_h) = output_dimensions;
    let exact = context.resize_exactness;
    let constant_bits = exact.constant_bits.filter(|_| {
        context.logical_mode == Some("F") && !matches!(filter, ResampleFilter::Nearest)
    });
    let box_copy_is_exact = exact.box_copy_is_exact
        && context.logical_mode == Some("F")
        && matches!(filter, ResampleFilter::Box);
    let identity_is_exact = exact.identity_is_exact && context.logical_mode == Some("F");
    let box_average_is_exact = exact.box_average_is_exact
        && context.logical_mode == Some("F")
        && matches!(filter, ResampleFilter::Box);
    let dyadic_is_exact = exact.dyadic_is_exact
        && context.logical_mode == Some("F")
        && matches!(
            filter,
            ResampleFilter::Bilinear
                | ResampleFilter::Bicubic
                | ResampleFilter::Lanczos
                | ResampleFilter::Hamming
                | ResampleFilter::Box
        );
    let f64_is_exact = exact.f64_is_exact
        && context.logical_mode == Some("F")
        && !matches!(filter, ResampleFilter::Nearest);
    let f64_ordered_is_exact = exact.f64_ordered_is_exact
        && context.logical_mode == Some("F")
        && !matches!(filter, ResampleFilter::Nearest);
    let compact_box_is_exact = f64_ordered_is_exact
        && matches!(filter, ResampleFilter::Box)
        && gpu_f_resize_compact_box_any_axis((cur_w, cur_h), (out_w, out_h));
    let i_f64_is_exact = exact.f64_is_exact
        && context.logical_mode == Some("I")
        && !matches!(filter, ResampleFilter::Nearest);
    let luma16_f64_is_exact = exact.f64_is_exact
        && context.op_mode == 5
        && context
            .logical_mode
            .is_none_or(|mode| matches!(mode, "I;16" | "I;16L" | "I;16B" | "I;16N"))
        && !matches!(filter, ResampleFilter::Nearest);
    append_resize_uniform_words(
        context,
        filter,
        params,
        constant_bits,
        [
            box_copy_is_exact,
            identity_is_exact,
            box_average_is_exact,
            compact_box_is_exact,
            f64_ordered_is_exact,
            f64_is_exact,
            i_f64_is_exact,
            luma16_f64_is_exact,
            dyadic_is_exact,
        ],
    );
}

fn append_resize_uniform_words(
    context: BatchOperationParameterContext<'_>,
    filter: ResampleFilter,
    params: &mut Vec<u32>,
    constant_bits: Option<u32>,
    exact: [bool; 9],
) {
    let [
        box_copy,
        identity,
        box_average,
        compact_box,
        ordered_f64,
        f64,
        i_f64,
        luma16_f64,
        dyadic,
    ] = exact;
    let (out_w, out_h) = context.output_dimensions;
    let coefficient_channels =
        constant_bits.unwrap_or_else(|| gpu_resize_channel_count(context.op_mode));
    let mode = if constant_bits.is_some() {
        2
    } else if box_copy {
        3
    } else if identity {
        5
    } else if box_average {
        4
    } else if compact_box {
        13
    } else if ordered_f64 {
        12
    } else if f64 {
        9
    } else if i_f64 {
        11
    } else if luma16_f64 {
        10
    } else if dyadic {
        6
    } else {
        u32::from(gpu_resize_should_premultiply(
            context.op_mode,
            context.logical_mode,
            filter,
        ))
    };
    params.extend([out_w, out_h, coefficient_channels, mode]);
}

fn append_pad_params(
    context: BatchOperationParameterContext<'_>,
    params: &mut Vec<u32>,
) -> Result<bool, PilError> {
    let PipelineOp::Pad { filter, .. } = context.op else {
        return Ok(false);
    };
    let (cur_w, cur_h) = context.source_dimensions;
    let (resize_w, resize_h, offset_x, offset_y) = gpu_pad_geometry(context.op, cur_w, cur_h)
        .map(|((width, height), (x, y))| (width, height, x, y))
        .ok_or_else(|| PilError::ValueError("GPU Pad has no safe geometry".into()))?;
    let resize_filter = if context.logical_mode == Some("P") {
        ResampleFilter::Nearest
    } else {
        *filter
    };
    let constant_bits = context.resize_exactness.constant_bits.filter(|_| {
        context.logical_mode == Some("F") && !matches!(resize_filter, ResampleFilter::Nearest)
    });
    let f64_is_exact = context.resize_exactness.f64_is_exact
        && context.logical_mode == Some("F")
        && !matches!(resize_filter, ResampleFilter::Nearest);
    let fill = gpu_pad_fill(context.op, context.logical_mode, context.op_mode);
    let (out_w, out_h) = context.output_dimensions;
    params.extend([
        resize_w,
        resize_h,
        constant_bits.unwrap_or_else(|| gpu_resize_channel_count(context.op_mode)),
        if constant_bits.is_some() {
            2
        } else if f64_is_exact {
            9
        } else {
            u32::from(gpu_resize_should_premultiply(
                context.op_mode,
                context.logical_mode,
                resize_filter,
            ))
        },
        out_w,
        out_h,
        fill,
        offset_x,
        offset_y,
    ]);
    if context.layouts.packed_luma_pad {
        params[3] = NATIVE_COVER_PACKED_OUTPUT_MARKER;
    }
    Ok(true)
}

fn append_autocontrast_params(
    context: BatchOperationParameterContext<'_>,
    cutoff: f64,
    params: &mut Vec<u32>,
) -> Result<(), PilError> {
    let (width, height) = context.source_dimensions;
    let selected_pixels =
        gpu_autocontrast_selected_pixels(width, height, context.auxiliary.third.as_deref())?;
    let (cutoff_low, cutoff_high) = gpu_autocontrast_cutoff_indices(selected_pixels, cutoff);
    params[3] = u32::from(context.auxiliary.third.is_some());
    // Keep the original cutoff bits for receipt/debugging compatibility; the
    // exact integer thresholds below avoid f32 cutoff rounding in the control pass.
    params.extend([
        (cutoff as f32).to_bits(),
        cutoff_low,
        cutoff_high,
        selected_pixels.min(u32::MAX as usize) as u32,
    ]);
    Ok(())
}

fn append_transform_params(
    context: BatchOperationParameterContext<'_>,
    params: &mut Vec<u32>,
) -> Result<(), PilError> {
    let PipelineOp::Transform {
        filter,
        method,
        data,
        ..
    } = context.op
    else {
        return Ok(());
    };
    let mut transform_params = registry::extract_params(context.op);
    params[3] = transform_transport_flags(context);
    if matches!(method, TransformMethod::Affine)
        && gpu_transform_uses_nearest(context.logical_mode, *filter)
    {
        set_fixed_affine_transform_params(context, data, &mut transform_params);
    }
    transform_params[8] = gpu_transform_fill(context.op, context.logical_mode, context.op_mode);
    if gpu_transform_uses_nearest(context.logical_mode, *filter)
        && !(context.logical_mode == Some("F") && gpu_transform_is_projective(context.op))
    {
        transform_params[9] = 0;
    }
    params.extend(transform_params);
    params.push(u32::from(gpu_transform_should_premultiply(
        context.op_mode,
        context.logical_mode,
        *filter,
        method.clone(),
    )));
    append_transform_method_words(method, data, params);
    Ok(())
}

fn transform_transport_flags(context: BatchOperationParameterContext<'_>) -> u32 {
    let logical_big_endian = context.logical_mode == Some("I;16B")
        || (cfg!(target_endian = "big") && matches!(context.logical_mode, Some("I;16" | "I;16N")));
    let transport_big_endian =
        matches!(context.logical_mode, Some("I;16B" | "I;16N")) || cfg!(target_endian = "big");
    let mut flags = if context.layouts.native_rgb_compact_input {
        2
    } else {
        u32::from(logical_big_endian != transport_big_endian)
    };
    flags |= u32::from(context.layouts.packed_luma_transform_input) << 2;
    flags |= u32::from(context.layouts.packed_luma_transform_output) << 3;
    flags |= u32::from(
        context
            .layouts
            .native_rgb_packed_output
            .is_some_and(|plan| plan.row_tiled),
    ) << 4;
    flags |= u32::from(
        context
            .layouts
            .native_la_transform_output
            .is_some_and(|plan| plan.row_tiled),
    ) << 5;
    flags
}

fn set_fixed_affine_transform_params(
    context: BatchOperationParameterContext<'_>,
    data: &[f64],
    transform_params: &mut [u32],
) {
    let fixed = |value: f64| (value.mul_add(65_536.0, 0.5).floor() as i64) as u32;
    let a = data.first().copied().unwrap_or(0.0);
    let b = data.get(1).copied().unwrap_or(0.0);
    let c = data.get(2).copied().unwrap_or(0.0);
    let d = data.get(3).copied().unwrap_or(0.0);
    let e = data.get(4).copied().unwrap_or(0.0);
    let f = data.get(5).copied().unwrap_or(0.0);
    let (origin_x, origin_y) = if context
        .logical_mode
        .is_some_and(|mode| matches!(mode, "I;16" | "I;16L" | "I;16B" | "I;16N"))
    {
        // Native I;16 affine geometry rounds the integer destination point
        // before the fixed-point walk; byte, I, and F add half-steps first.
        (fixed(c + 0.5), fixed(f + 0.5))
    } else {
        (fixed(c + a * 0.5 + b * 0.5), fixed(f + d * 0.5 + e * 0.5))
    };
    transform_params[2..8].copy_from_slice(&[
        fixed(a),
        fixed(b),
        origin_x,
        fixed(d),
        fixed(e),
        origin_y,
    ]);
}

fn append_transform_method_words(method: &TransformMethod, data: &[f64], params: &mut Vec<u32>) {
    let method_code = match method {
        TransformMethod::Affine => 0,
        TransformMethod::Perspective => 1,
        TransformMethod::Quad => 2,
        TransformMethod::Mesh => 3,
    };
    params.extend([
        method_code,
        (data.get(6).copied().unwrap_or(0.0) as f32).to_bits(),
        (data.get(7).copied().unwrap_or(0.0) as f32).to_bits(),
    ]);
    if matches!(method, TransformMethod::Mesh) {
        params.extend(
            data.get(8..12)
                .unwrap_or(&[])
                .iter()
                .map(|value| (*value as f32).to_bits()),
        );
    }
}

fn append_color_3d_lut_params(
    context: BatchOperationParameterContext<'_>,
    size: (u32, u32, u32),
    channels: u32,
    source_mode: PixelMode,
    params: &mut Vec<u32>,
) {
    let scale = |extent: u32| {
        ((f64::from(extent.saturating_sub(1)) / 255.0) * f64::from(1u32 << 18)) as u32
    };
    params.extend([
        size.0,
        size.1,
        size.2,
        channels,
        scale(size.0),
        scale(size.1),
        scale(size.2),
    ]);
    params[2] = gpu_pixel_mode_code(source_mode).unwrap_or(context.op_mode);
}

fn append_fit_params(
    context: BatchOperationParameterContext<'_>,
    filter: ResampleFilter,
    params: &mut Vec<u32>,
) {
    let nearest_mode =
        if context.logical_mode == Some("F") && matches!(filter, ResampleFilter::Nearest) {
            7
        } else {
            u32::from(gpu_resize_should_premultiply(
                context.op_mode,
                context.logical_mode,
                filter,
            ))
        };
    let (out_w, out_h) = context.output_dimensions;
    params.extend([
        out_w,
        out_h,
        gpu_resize_channel_count(context.op_mode),
        nearest_mode,
    ]);
}

fn append_contrast_midpoint(
    context: BatchOperationParameterContext<'_>,
    params: &mut Vec<u32>,
) -> Result<(), PilError> {
    if matches!(context.op, PipelineOp::Contrast { .. }) {
        params.push(u32::from(context.contrast_mean.ok_or_else(|| {
            PilError::ValueError("GPU Contrast is missing its scalar midpoint".into())
        })?));
    }
    Ok(())
}

fn append_output_dimensions(context: BatchOperationParameterContext<'_>, params: &mut Vec<u32>) {
    let (out_w, out_h) = context.output_dimensions;
    if let Some(dispatch) = context.layouts.native_expand_output {
        params.extend([dispatch.words_per_row, dispatch.word_count]);
    } else {
        params.extend([out_w, out_h]);
    }
}
