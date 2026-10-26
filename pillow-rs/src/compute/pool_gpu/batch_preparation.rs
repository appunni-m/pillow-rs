use super::*;

type PreparationPipelineSpec = (&'static str, &'static str, &'static str);

#[derive(Clone, Copy)]
pub(super) struct BatchPreparationPipelineOptions<'a> {
    pub(super) logical_mode: Option<&'a str>,
    pub(super) packed_native_byte_filter: bool,
    pub(super) native_rgb_packed_output: bool,
    pub(super) native_la_transform_output: bool,
    pub(super) packed_luma_transform_output: bool,
    pub(super) packed_luma_point: bool,
    pub(super) packed_luma_order_statistic: bool,
    pub(super) native_sharpness_l_input: bool,
    pub(super) native_sharpness_la_input: bool,
    pub(super) native_sharpness_rgb_input: bool,
}

impl GpuInner {
    pub(super) fn resolve_preparation_pipeline(
        &self,
        ops: &[PipelineOp],
        index: usize,
        dimensions: (u32, u32),
        options: &BatchPreparationPipelineOptions<'_>,
    ) -> Result<Arc<CachedPipeline>, PilError> {
        let spec = preparation_pipeline_spec(ops, index, dimensions, options)?;
        self.resolve_pipeline(spec.0, spec.1, spec.2)
    }
}

fn preparation_pipeline_spec(
    ops: &[PipelineOp],
    index: usize,
    dimensions: (u32, u32),
    options: &BatchPreparationPipelineOptions<'_>,
) -> Result<PreparationPipelineSpec, PilError> {
    let op = &ops[index];
    if let Some(spec) =
        fused_or_histogram_pipeline_spec(ops, index, options.logical_mode, dimensions)
    {
        return Ok(spec);
    }
    if let Some(spec) = packed_luma_la_filter_pipeline_spec(op, options) {
        return Ok(spec);
    }
    if let Some(spec) = packed_color_filter_pipeline_spec(op, options) {
        return Ok(spec);
    }
    if let Some(spec) = blur_resize_pipeline_spec(op, options.logical_mode) {
        return Ok(spec);
    }
    if let Some(spec) = transform_pipeline_spec(op, dimensions, options) {
        return Ok(spec);
    }
    if let Some(spec) = packed_luma_point_pipeline_spec(op, options) {
        return Ok(spec);
    }
    if let Some(spec) = packed_luma_order_statistic_pipeline_spec(op, dimensions, options) {
        return Ok(spec);
    }
    if let Some(spec) = native_sharpness_pipeline_spec(op, options) {
        return Ok(spec);
    }

    registered_pipeline_spec(op)
}

fn fused_or_histogram_pipeline_spec(
    ops: &[PipelineOp],
    index: usize,
    logical_mode: Option<&str>,
    dimensions: (u32, u32),
) -> Option<PreparationPipelineSpec> {
    if can_fuse_gpu_multiply_screen(ops, index) {
        return Some((
            "__internal_multiply_screen",
            "multiply_screen.wgsl",
            include_str!("shaders/multiply_screen.wgsl"),
        ));
    }
    if can_fuse_gpu_alpha_composite_mirror(ops, index, logical_mode, dimensions) {
        return Some((
            "__internal_alpha_composite_mirror",
            "alpha_composite_mirror.wgsl",
            include_str!("shaders/alpha_composite_mirror.wgsl"),
        ));
    }
    // Histogram-driven operations use the clear pass's public parameter
    // contract here; encoding resolves their full multi-pass plan later.
    if matches!(
        &ops[index],
        PipelineOp::Autocontrast { .. } | PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. }
    ) {
        return Some((
            "__internal_histogram_clear",
            "histogram_clear.wgsl",
            include_str!("shaders/histogram_clear.wgsl"),
        ));
    }
    None
}

fn packed_luma_la_filter_pipeline_spec(
    op: &PipelineOp,
    options: &BatchPreparationPipelineOptions<'_>,
) -> Option<PreparationPipelineSpec> {
    if options.packed_native_byte_filter
        && matches!(options.logical_mode, None | Some("L"))
        && matches!(op, PipelineOp::Filter5x5 { .. })
    {
        return Some((
            "__internal_filter_5x5_luma_packed",
            "filter_5x5_luma_packed.wgsl",
            include_str!("shaders/filter_5x5_luma_packed.wgsl"),
        ));
    }
    if options.packed_native_byte_filter
        && matches!(options.logical_mode, Some("LA"))
        && matches!(op, PipelineOp::Filter3x3 { .. })
    {
        return Some((
            "__internal_filter_3x3_la_binomial_packed",
            "filter_3x3_la_binomial_packed.wgsl",
            include_str!("shaders/filter_3x3_la_binomial_packed.wgsl"),
        ));
    }
    if options.packed_native_byte_filter
        && matches!(options.logical_mode, Some("LA"))
        && matches!(op, PipelineOp::Filter5x5 { .. })
    {
        return Some((
            "__internal_filter_5x5_la_packed",
            "filter_5x5_la_packed.wgsl",
            include_str!("shaders/filter_5x5_la_packed.wgsl"),
        ));
    }
    if options.packed_native_byte_filter
        && matches!(options.logical_mode, Some("LA"))
        && matches!(op, PipelineOp::MedianFilter { size: 3 })
    {
        return Some((
            "__internal_median_filter_3x3_la_packed",
            "median_filter_3x3_la_packed.wgsl",
            include_str!("shaders/median_filter_3x3_la_packed.wgsl"),
        ));
    }
    if options.packed_native_byte_filter
        && matches!(options.logical_mode, Some("LA"))
        && matches!(op, PipelineOp::RankFilter { size: 3, rank } if *rank < 9)
    {
        return Some((
            "__internal_rank_filter_3x3_la_packed",
            "rank_filter_3x3_la_packed.wgsl",
            include_str!("shaders/rank_filter_3x3_la_packed.wgsl"),
        ));
    }
    if options.packed_native_byte_filter
        && matches!(options.logical_mode, Some("LA"))
        && matches!(op, PipelineOp::MaxFilter { size: 3 })
    {
        return Some((
            "__internal_max_filter_3x3_la_packed",
            "max_filter_3x3_la_packed.wgsl",
            include_str!("shaders/max_filter_3x3_la_packed.wgsl"),
        ));
    }
    if options.packed_native_byte_filter
        && matches!(options.logical_mode, Some("LA"))
        && matches!(op, PipelineOp::MinFilter { size: 3 })
    {
        return Some((
            "__internal_min_filter_3x3_la_packed",
            "min_filter_3x3_la_packed.wgsl",
            include_str!("shaders/min_filter_3x3_la_packed.wgsl"),
        ));
    }
    None
}

fn packed_color_filter_pipeline_spec(
    op: &PipelineOp,
    options: &BatchPreparationPipelineOptions<'_>,
) -> Option<PreparationPipelineSpec> {
    if options.native_rgb_packed_output && matches!(op, PipelineOp::MedianFilter { size: 3 }) {
        return Some((
            "__internal_median_filter_3x3_rgb_packed",
            "median_filter_3x3_rgb_packed.wgsl",
            include_str!("shaders/median_filter_3x3_rgb_packed.wgsl"),
        ));
    }
    if options.native_rgb_packed_output && matches!(op, PipelineOp::MaxFilter { size: 3 }) {
        return Some((
            "__internal_max_filter_3x3_rgb_packed",
            "max_filter_3x3_rgb_packed.wgsl",
            include_str!("shaders/max_filter_3x3_rgb_packed.wgsl"),
        ));
    }
    if options.native_rgb_packed_output && matches!(op, PipelineOp::MinFilter { size: 3 }) {
        return Some((
            "__internal_min_filter_3x3_rgb_packed",
            "min_filter_3x3_rgb_packed.wgsl",
            include_str!("shaders/min_filter_3x3_rgb_packed.wgsl"),
        ));
    }
    if options.native_rgb_packed_output
        && matches!(options.logical_mode, None | Some("RGB"))
        && matches!(op, PipelineOp::BoxBlur { radius: 1 })
    {
        return Some((
            "__internal_blur_h_rgb_packed",
            "box_blur_h_rgb_packed.wgsl",
            include_str!("shaders/box_blur_h_rgb_packed.wgsl"),
        ));
    }
    if options.packed_native_byte_filter
        && matches!(
            op,
            PipelineOp::GaussianBlur { .. } | PipelineOp::BoxBlur { radius: 1 }
        )
    {
        return if matches!(options.logical_mode, Some("LA")) {
            Some((
                "__internal_blur_h_la_packed",
                "box_blur_h_la_packed.wgsl",
                include_str!("shaders/box_blur_h_la_packed.wgsl"),
            ))
        } else {
            Some((
                "__internal_blur_h_luma_packed",
                "box_blur_h_luma_packed.wgsl",
                include_str!("shaders/box_blur_h_luma_packed.wgsl"),
            ))
        };
    }
    None
}

fn blur_resize_pipeline_spec(
    op: &PipelineOp,
    logical_mode: Option<&str>,
) -> Option<PreparationPipelineSpec> {
    if GpuInner::blur_pass_count(op).is_some() {
        return Some((
            "__internal_blur_h",
            "box_blur_h.wgsl",
            include_str!("shaders/box_blur_h.wgsl"),
        ));
    }
    // Pad's first pass is the exact separable resize; its placement pass is
    // resolved later in batch encoding.
    if matches!(op, PipelineOp::Pad { .. }) {
        return Some((
            "__internal_resize_h",
            "resize_convolution_h.wgsl",
            include_str!("shaders/resize_convolution_h.wgsl"),
        ));
    }
    // Fit keeps its fractional crop box in host-built coefficients while the
    // device executes the same convolution path as Resize.
    if matches!(op, PipelineOp::Fit { .. }) {
        return Some((
            "__internal_resize_h",
            "resize_convolution_h.wgsl",
            include_str!("shaders/resize_convolution_h.wgsl"),
        ));
    }
    if matches!(op, PipelineOp::Resize { filter, .. }
        if !matches!(filter, ResampleFilter::Nearest)
            || gpu_resize_nearest_uses_coefficients(logical_mode))
    {
        return Some((
            "__internal_resize_h",
            "resize_convolution_h.wgsl",
            include_str!("shaders/resize_convolution_h.wgsl"),
        ));
    }
    None
}

fn transform_pipeline_spec(
    op: &PipelineOp,
    dimensions: (u32, u32),
    options: &BatchPreparationPipelineOptions<'_>,
) -> Option<PreparationPipelineSpec> {
    if options.native_rgb_packed_output && matches!(op, PipelineOp::Eval { .. }) {
        return Some((
            "__internal_point_rgb_packed",
            "point_rgb_packed.wgsl",
            include_str!("shaders/point_rgb_packed.wgsl"),
        ));
    }
    if options.native_rgb_packed_output && matches!(op, PipelineOp::Transform { .. }) {
        return Some((
            "__internal_transform_rgb_nearest_packed",
            "transform_rgb_nearest_packed.wgsl",
            include_str!("shaders/transform_rgb_nearest_packed.wgsl"),
        ));
    }
    if options.native_la_transform_output && matches!(op, PipelineOp::Transform { .. }) {
        return Some((
            "__internal_transform_la_nearest_packed",
            "transform_la_nearest_packed.wgsl",
            include_str!("shaders/transform_la_nearest_packed.wgsl"),
        ));
    }
    if options.packed_luma_transform_output && matches!(op, PipelineOp::Transform { .. }) {
        return Some((
            "__internal_transform_l_nearest_packed",
            "transform_l_nearest_packed.wgsl",
            include_str!("shaders/transform_l_nearest_packed.wgsl"),
        ));
    }
    if gpu_transform_uses_geometry_table(op, options.logical_mode, dimensions) {
        return Some((
            "__internal_transform_geometry",
            "transform_geometry.wgsl",
            include_str!("shaders/transform_geometry.wgsl"),
        ));
    }
    None
}

fn packed_luma_point_pipeline_spec(
    op: &PipelineOp,
    options: &BatchPreparationPipelineOptions<'_>,
) -> Option<PreparationPipelineSpec> {
    if options.packed_luma_point && matches!(op, PipelineOp::Eval { .. }) {
        return Some((
            "__internal_point_luma_packed",
            "point_luma_packed.wgsl",
            include_str!("shaders/point_luma_packed.wgsl"),
        ));
    }
    None
}

fn packed_luma_order_statistic_pipeline_spec(
    op: &PipelineOp,
    dimensions: (u32, u32),
    options: &BatchPreparationPipelineOptions<'_>,
) -> Option<PreparationPipelineSpec> {
    if !options.packed_luma_order_statistic {
        return None;
    }
    if matches!(op, PipelineOp::MedianFilter { size: 3 }) {
        return Some((
            "__internal_median_filter_3x3_luma_packed",
            "median_filter_3x3_luma_packed.wgsl",
            include_str!("shaders/median_filter_3x3_luma_packed.wgsl"),
        ));
    }
    if matches!(op, PipelineOp::MaxFilter { size: 3 }) {
        return Some((
            "__internal_max_filter_3x3_luma_packed",
            "max_filter_3x3_luma_packed.wgsl",
            include_str!("shaders/max_filter_3x3_luma_packed.wgsl"),
        ));
    }
    if matches!(op, PipelineOp::MinFilter { size: 3 }) {
        return Some((
            "__internal_min_filter_3x3_luma_packed",
            "min_filter_3x3_luma_packed.wgsl",
            include_str!("shaders/min_filter_3x3_luma_packed.wgsl"),
        ));
    }
    if matches!(op, PipelineOp::RankFilter { size: 3, rank: 1 }) {
        return Some((
            "__internal_rank_filter_3x3_luma_packed",
            "rank_filter_3x3_luma_packed.wgsl",
            include_str!("shaders/rank_filter_3x3_luma_packed.wgsl"),
        ));
    }
    if matches!(op, PipelineOp::RankFilter { size: 9, .. }) {
        let (width, _) = dimensions;
        return if width % 4 == 0 {
            Some((
                "__internal_rank_filter_9x9_luma_row_packed",
                "rank_filter_9x9_luma_row_packed.wgsl",
                include_str!("shaders/rank_filter_9x9_luma_row_packed.wgsl"),
            ))
        } else {
            Some((
                "__internal_rank_filter_9x9_luma_packed",
                "rank_filter_9x9_luma_packed.wgsl",
                include_str!("shaders/rank_filter_9x9_luma_packed.wgsl"),
            ))
        };
    }
    None
}

fn native_sharpness_pipeline_spec(
    op: &PipelineOp,
    options: &BatchPreparationPipelineOptions<'_>,
) -> Option<PreparationPipelineSpec> {
    if options.native_sharpness_l_input && matches!(op, PipelineOp::Sharpness { .. }) {
        return Some((
            "__internal_sharpness_l_native",
            "sharpness_l_native.wgsl",
            include_str!("shaders/sharpness_l_native.wgsl"),
        ));
    }
    if options.native_sharpness_la_input && matches!(op, PipelineOp::Sharpness { .. }) {
        return Some((
            "__internal_sharpness_la_native",
            "sharpness_la_native.wgsl",
            include_str!("shaders/sharpness_la_native.wgsl"),
        ));
    }
    if options.native_sharpness_rgb_input && matches!(op, PipelineOp::Sharpness { .. }) {
        return Some((
            "__internal_sharpness_rgb_native",
            "sharpness_rgb_native.wgsl",
            include_str!("shaders/sharpness_rgb_native.wgsl"),
        ));
    }
    None
}

fn registered_pipeline_spec(op: &PipelineOp) -> Result<PreparationPipelineSpec, PilError> {
    let base_key = registry::variant_key(op);
    let entry = registry::registry()?.get(base_key).ok_or_else(|| {
        PilError::ValueError(format!(
            "GPU operation '{base_key}' has no registered shader entry"
        ))
    })?;
    let source = entry.gpu_source.ok_or_else(|| {
        PilError::ValueError(format!(
            "GPU operation '{base_key}' has no registered shader source"
        ))
    })?;
    let shader_file = entry.gpu_shader.ok_or_else(|| {
        PilError::ValueError(format!(
            "GPU operation '{base_key}' has no registered shader name"
        ))
    })?;
    Ok((base_key, shader_file, source))
}

#[cfg(target_endian = "little")]
#[inline(always)]
fn direct_alpha_composite_rgba_bytes<'a>(
    ops: &[PipelineOp],
    auxiliary_images: &'a [AuxiliaryImages],
    logical_mode: Option<&str>,
    dimensions: (u32, u32),
) -> Result<Option<&'a [u8]>, PilError> {
    let (width, height) = dimensions;
    if !can_fuse_gpu_alpha_composite_mirror(ops, 0, logical_mode, dimensions)
        || width == 0
        || height == 0
    {
        return Ok(None);
    }
    match auxiliary_images
        .first()
        .and_then(|auxiliary| auxiliary.second.as_deref())
    {
        Some(DynamicImage::ImageRgba8(rgba)) => {
            let expected_bytes = CheckedDims::new(width, height, 4)?.total_bytes();
            Ok((rgba.as_raw().len() == expected_bytes).then_some(rgba.as_raw().as_slice()))
        }
        _ => Ok(None),
    }
}

#[inline(always)]
fn validate_batch_operation_count(
    ops: &[PipelineOp],
    auxiliary_images: &[AuxiliaryImages],
) -> Result<(), PilError> {
    if ops.len() != auxiliary_images.len() {
        return Err(PilError::InternalError(
            "GPU operation and auxiliary-image counts differ".into(),
        ));
    }
    Ok(())
}

impl GpuInner {
    pub(super) fn prepare_batch<'a>(
        &self,
        ops: &[PipelineOp],
        auxiliary_images: &[AuxiliaryImages],
        w: u32,
        h: u32,
        mode: u32,
        logical_mode: Option<&str>,
        contrast_mean: Option<u8>,
        f_resize_constant_bits: Option<u32>,
        f_resize_box_copy_is_exact: bool,
        f_resize_identity_is_exact: bool,
        f_resize_box_average_is_exact: bool,
        f_resize_dyadic_is_exact: bool,
        f_resize_f64_is_exact: bool,
        f_resize_f64_ordered_is_exact: bool,
        packed_luma_colorize: bool,
        packed_luma_convert: bool,
        packed_luma_transform_input: bool,
        packed_luma_transform_output: bool,
        native_rgb_compact_input: bool,
        native_rgb_packed_output: bool,
        native_la_transform_output: bool,
        packed_luma_point: bool,
        packed_luma_putdata: bool,
        packed_luma_order_statistic: bool,
        packed_native_byte_filter: bool,
        packed_luma_pad: bool,
        native_extract_band: bool,
        native_grayscale_rgb_input: bool,
        native_sharpness_l_input: bool,
        native_sharpness_la_input: bool,
        native_sharpness_rgb_input: bool,
        native_reduce_rgb_input: bool,
        native_expand_channels: Option<u8>,
        native_cover_input_channels: Option<u8>,
        native_cover_packed_output: Option<NativeCoverPackedOutputDispatch>,
        buffers: &'a mut BufferPool,
        auxiliary_cache: &GpuAuxiliaryCache,
    ) -> Result<PreparedGpuBatch<'a>, PilError> {
        validate_batch_operation_count(ops, auxiliary_images)?;

        // A full-frame RGBA composite has one already-packed auxiliary image.
        // Put its bytes directly into the reusable device arena instead of
        // first duplicating them into the temporary host-side u32 arena. The
        // specialized pair is deliberately narrow; all other image formats
        // and operation graphs retain the general pack-and-arena path.
        #[cfg(target_endian = "little")]
        let direct_alpha_composite_rgba =
            direct_alpha_composite_rgba_bytes(ops, auxiliary_images, logical_mode, (w, h))?;
        #[cfg(target_endian = "big")]
        let direct_alpha_composite_rgba: Option<&[u8]> = None;

        let limits = self.device.limits();
        let uniform_alignment = limits.min_uniform_buffer_offset_alignment as usize;
        let storage_alignment = limits.min_storage_buffer_offset_alignment as usize;
        let mut scratch = BatchPreparationScratch::new(ops.len());
        let packed_output_plans = self.prepare_batch_packed_output_plans(
            BatchPackedOutputPlanningContext {
                ops,
                source_dimensions: (w, h),
                native_rgb_packed_output,
                native_la_transform_output,
                max_workgroups_per_dimension: limits.max_compute_workgroups_per_dimension,
                max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size,
                max_buffer_size: limits.max_buffer_size,
            },
            buffers.capacity,
        )?;
        let native_rgb_packed_output = packed_output_plans.native_rgb;
        let native_la_transform_output = packed_output_plans.native_la;
        let pipeline_options = BatchPreparationPipelineOptions {
            logical_mode,
            packed_native_byte_filter,
            native_rgb_packed_output: native_rgb_packed_output.is_some(),
            native_la_transform_output: native_la_transform_output.is_some(),
            packed_luma_transform_output,
            packed_luma_point,
            packed_luma_order_statistic,
            native_sharpness_l_input,
            native_sharpness_la_input,
            native_sharpness_rgb_input,
        };
        let parameter_layouts = BatchParameterLayouts {
            packed_luma_colorize,
            packed_luma_convert,
            packed_luma_point,
            packed_luma_putdata,
            packed_luma_pad,
            packed_luma_transform_input,
            packed_luma_transform_output,
            native_extract_band,
            native_grayscale_rgb_input,
            native_sharpness_l_input,
            native_reduce_rgb_input,
            native_expand_channels,
            native_cover_input: native_cover_input_channels.is_some(),
            native_rgb_compact_input,
            native_rgb_packed_output,
            native_la_transform_output,
            native_expand_output: None,
            native_cover_packed_output,
        };
        let final_dims = self.prepare_batch_operations(
            BatchOperationPreparationOptions {
                ops,
                auxiliary_images,
                logical_mode,
                contrast_mean,
                resize_exactness: BatchResizeParameterExactness {
                    constant_bits: f_resize_constant_bits,
                    box_copy_is_exact: f_resize_box_copy_is_exact,
                    identity_is_exact: f_resize_identity_is_exact,
                    box_average_is_exact: f_resize_box_average_is_exact,
                    dyadic_is_exact: f_resize_dyadic_is_exact,
                    f64_is_exact: f_resize_f64_is_exact,
                    f64_ordered_is_exact: f_resize_f64_ordered_is_exact,
                },
                layouts: parameter_layouts,
                pipeline_options,
                packed_luma_putdata,
                direct_alpha_composite_rgba,
                uniform_alignment,
                storage_alignment,
                max_workgroups_per_dimension: limits.max_compute_workgroups_per_dimension,
                max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size,
                max_buffer_size: limits.max_buffer_size,
                auxiliary_cache,
            },
            buffers,
            &mut scratch,
            (w, h, mode),
        )?;

        self.finalize_prepared_batch(
            buffers,
            auxiliary_cache,
            scratch.into_output(BatchPreparationOutputOptions {
                uniform_alignment,
                storage_alignment,
                direct_img2_bytes: direct_alpha_composite_rgba.unwrap_or_default(),
                native_rgb_packed_output,
                native_la_transform_output,
                native_cover_packed_output,
                final_dims,
            }),
        )
    }
}
