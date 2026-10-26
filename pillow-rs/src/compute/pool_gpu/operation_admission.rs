use super::*;

/// Check dimensions before creating a device or uploading any image data.
/// Empty images and zero-sized GPU outputs are valid CPU/Pillow states, but
/// they are not valid storage-buffer dispatches: sampling kernels subtract one
/// from source dimensions and readback needs a non-empty copy range. Operations
/// whose host/shader output-size contract is incomplete are deliberately kept
/// on the CPU until that contract is explicit.
pub(super) fn gpu_dimensions_require_cpu(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    // Native-L RankFilter(9) uses eight binary-search passes over its
    // 81-sample neighborhood. The ordinary packed-RGBA shader sorts four
    // channels and has a much larger watchdog estimate; do not let that
    // conservative estimate reject the exact native-L specialization before
    // the order-statistic admission below can select it.
    let packed_luma_rank_filter_9 = gpu_packed_luma_rank_filter_9_input(ops, image, logical_mode);
    let packed_la_rank_filter_3 = gpu_packed_la_rank_filter_3_input(ops, image, logical_mode);
    let dimensions_fit = |w: u32, h: u32| {
        CheckedDims::new(w, h, 1)
            .map(|dims| dims.total_pixels() <= GPU_BUFFER_CAPACITY as usize)
            .unwrap_or(false)
    };

    if !gpu_image_layout_is_supported(image)
        && !gpu_luma16_geometry_is_supported(ops, image, None)
        && !gpu_luma16_convert_is_supported(ops, image)
        && !gpu_luma16_paste_is_supported(ops, image)
    {
        return true;
    }
    let source_mode = mode_code(image);
    if ops
        .iter()
        .any(|op| matches!(op, PipelineOp::Eval { .. }) && extract_lut(op, source_mode).is_none())
    {
        return true;
    }
    let (mut cur_w, mut cur_h) = image.dimensions();
    if cur_w == 0 || cur_h == 0 || !dimensions_fit(cur_w, cur_h) {
        return true;
    }
    for op in ops {
        if gpu_operation_mode_requires_cpu(op, image) {
            return true;
        }
        // CPU PutPixel reports IndexError for an out-of-range coordinate;
        // the shader would silently perform no write. Route that case to the
        // CPU before device initialization so error semantics are preserved.
        if let PipelineOp::PutPixel { x, y, .. } = op {
            if *x >= cur_w || *y >= cur_h {
                return true;
            }
        }
        let next = match op_output_dims(op, cur_w, cur_h) {
            Some(dimensions) => dimensions,
            None if op_has_explicit_output_dimensions(op) => return true,
            None => (cur_w, cur_h),
        };
        if let PipelineOp::Resize { filter, .. }
        | PipelineOp::Scale { filter, .. }
        | PipelineOp::Contain { filter, .. }
        | PipelineOp::Cover { filter, .. } = op
        {
            // A direct F Box reduction to one column can use the compact
            // repeated-coefficient vertical shader. Check that proof before
            // asking for the full vertical table, which could otherwise
            // allocate hundreds of MiB only to reject an over-limit row.
            let compact_vertical = logical_mode == Some("F")
                && gpu_f_resize_compact_box_vertical_only_is_exact(ops, image, logical_mode);
            let compact_box = logical_mode == Some("F")
                && ops.len() == 1
                && matches!(ops.first(), Some(PipelineOp::Resize { filter, .. }) if matches!(filter, ResampleFilter::Box))
                && gpu_f_resize_compact_box_is_exact(ops, image, logical_mode);
            if compact_vertical {
                let horizontal = gpu_resize_coefficients(next.0, cur_w, *filter);
                if resize_coeff_word_count(&horizontal).is_err() {
                    return true;
                }
            } else if compact_box {
                // Marker 13 supplies the compact table for the over-limit
                // axis.  The other axis may still change when its ordinary
                // f64 table fits the binding and every row stays within the
                // ordered reducer's tap cap; the compact proof has already
                // validated both pass results and finite-only chaining.
                let (kernel, support) = filter_from_resample(*filter);
                for (source_size, output_size, compact) in [
                    (cur_w, next.0, gpu_f_resize_compact_box_axis(cur_w, next.0)),
                    (cur_h, next.1, gpu_f_resize_compact_box_axis(cur_h, next.1)),
                ] {
                    if compact {
                        continue;
                    }
                    let coeffs = precompute_coeffs_f64(output_size, source_size, kernel, support);
                    if !gpu_f_resize_f64_coefficients_fit_binding(&coeffs)
                        || coeffs
                            .count
                            .iter()
                            .any(|&count| count > GPU_F_RESIZE_ORDERED_MAX_TAPS)
                    {
                        return true;
                    }
                }
            } else if !gpu_resize_coefficients_are_safe(*filter, (cur_w, cur_h), next) {
                return true;
            }
        }
        if let PipelineOp::Fit {
            bleed,
            centering,
            filter,
            ..
        } = op
        {
            if !gpu_fit_coefficients_are_safe((cur_w, cur_h), next, *bleed, *centering, *filter) {
                return true;
            }
        }
        if let PipelineOp::Pad { filter, .. } = op {
            let Some(((resize_w, resize_h), _)) = gpu_pad_geometry(op, cur_w, cur_h) else {
                return true;
            };
            if !gpu_resize_coefficients_are_safe(*filter, (cur_w, cur_h), (resize_w, resize_h)) {
                return true;
            }
        }
        if next.0 == 0 || next.1 == 0 || !dimensions_fit(next.0, next.1) {
            return true;
        }
        if packed_luma_rank_filter_9 && matches!(op, PipelineOp::RankFilter { size: 9, .. }) {
            if gpu_packed_luma_rank_filter_9_work_items(next) > MAX_GPU_SHADER_WORK_ITEMS {
                return true;
            }
        } else if packed_la_rank_filter_3 {
            let PipelineOp::RankFilter { rank, .. } = op else {
                return true;
            };
            if gpu_packed_la_rank_filter_3_work_items(next, *rank) > MAX_GPU_SHADER_WORK_ITEMS {
                return true;
            }
        } else if gpu_shader_work_requires_cpu(op, (cur_w, cur_h), next, None) {
            return true;
        }
        (cur_w, cur_h) = next;
    }
    false
}

/// Some shader math is layout-safe but still differs from Pillow for a
/// particular native mode. Keep this check beside the dimension preflight so
/// those cases fall back before any device or upload work begins.
pub(super) fn gpu_operation_mode_requires_cpu(op: &PipelineOp, image: &DynamicImage) -> bool {
    let other_is_lab = matches!(op, PipelineOp::BlendModule { other, .. }
        if other.mode().ok().as_deref() == Some("LAB"));
    gpu_operation_color_requires_cpu(op, image.color(), other_is_lab)
}

pub(super) fn gpu_operation_color_requires_cpu(
    op: &PipelineOp,
    color: crate::raster::ColorType,
    other_is_lab: bool,
) -> bool {
    match op {
        // AlphaComposite's public implementation is defined for LA/RGBA
        // canvases. On L/RGB, the CPU operation promotes to RGBA while the
        // packed GPU result would otherwise be fed through preserve_mode.
        PipelineOp::AlphaComposite { .. } => !matches!(
            color,
            crate::raster::ColorType::La8 | crate::raster::ColorType::Rgba8
        ),
        // ImageChops.blend intentionally converts through RGB and restores
        // an opaque alpha for an alpha-bearing source. Image.blend (the
        // module operation) blends every stored channel, including alpha, so
        // all packed byte layouts use the same shader path.
        PipelineOp::BlendModule { .. } => {
            other_is_lab
                || !matches!(
                    color,
                    crate::raster::ColorType::L8
                        | crate::raster::ColorType::La8
                        | crate::raster::ColorType::Rgb8
                        | crate::raster::ColorType::Rgba8
                )
        }
        // PutData and alpha promotion carry the logical source/target layout
        // in the operation. A direct core caller can construct a mismatched
        // pair; CPU converts according to that mode, while the packed shader
        // would otherwise reinterpret the existing storage in place.
        PipelineOp::PutData { mode, .. }
        | PipelineOp::PutAlpha { mode, .. }
        | PipelineOp::PutAlphaData { mode, .. } => !pixel_mode_matches_color(*mode, color),
        // The CPU ImageOps implementation runs Posterize/Solarize through
        // an RGB temporary and preserve_mode, which makes alpha opaque for
        // LA/RGBA. The packed shaders currently retain alpha, so keep those
        // native alpha cases on CPU until the public operation is normalized.
        PipelineOp::Posterize { .. } | PipelineOp::Solarize { .. } => matches!(
            color,
            crate::raster::ColorType::La8 | crate::raster::ColorType::Rgba8
        ),
        // ImageOps.colorize accepts only an L image and always produces RGB.
        // The shader reads the luma byte directly; running it on a packed RGB
        // source would silently colorize the wrong sample contract.
        PipelineOp::Colorize { .. } => !matches!(color, crate::raster::ColorType::L8),
        // Autocontrast and Equalize use the packed byte representation only
        // for native L/RGB images.  Their scalar histogram control plane is
        // exact for those layouts; alpha and typed images have different
        // Pillow contracts and must be rejected before any GPU work starts.
        PipelineOp::Autocontrast { .. }
        | PipelineOp::Equalize
        | PipelineOp::EqualizeMasked { .. } => !matches!(
            color,
            crate::raster::ColorType::L8 | crate::raster::ColorType::Rgb8
        ),
        // getchannel raises IndexError for a band that the source mode does
        // not have; a shader would read byte 3 or an unused packed byte.
        PipelineOp::ExtractBand { index } => {
            usize::from(*index) >= usize::from(color.channel_count())
        }
        _ => false,
    }
}

pub(super) fn pixel_mode_matches_color(mode: PixelMode, color: crate::raster::ColorType) -> bool {
    matches!(
        (mode, color),
        (PixelMode::L, crate::raster::ColorType::L8)
            | (PixelMode::LA, crate::raster::ColorType::La8)
            | (PixelMode::RGB, crate::raster::ColorType::Rgb8)
            | (PixelMode::RGBA, crate::raster::ColorType::Rgba8)
            // P/PA retain raw index samples in the same one-/two-byte
            // buffers as L/LA until the binding restores palette metadata.
            | (PixelMode::P, crate::raster::ColorType::L8)
            | (PixelMode::PA, crate::raster::ColorType::La8)
            | (PixelMode::Mode1, crate::raster::ColorType::L8)
            | (PixelMode::YCbCr, crate::raster::ColorType::Rgb8)
            | (PixelMode::HSV, crate::raster::ColorType::Rgb8)
            // CMYK is represented as C/M/Y/K in the four bytes of Rgba8.
            | (PixelMode::CMYK, crate::raster::ColorType::Rgba8)
            // I/F are four-byte logical planes stored without conversion in
            // the RGBA transport; putdata replaces their raw little-endian
            // samples rather than interpreting them as color channels.
            | (PixelMode::I, crate::raster::ColorType::Rgba8)
            | (PixelMode::F, crate::raster::ColorType::Rgba8)
    )
}

pub(super) fn gpu_auxiliary_layout_is_supported_for_op(
    op: &PipelineOp,
    current_color: crate::raster::ColorType,
    is_second: bool,
    image: &DynamicImage,
) -> bool {
    // A typed I;16 Paste is the one multi-input GPU contract whose source is
    // intentionally not representable by the ordinary RGBA uploader. Keep
    // this exception attached to the exact operation and slot; masks still
    // use the byte-oriented transport below.
    if is_second
        && current_color == crate::raster::ColorType::L16
        && matches!(op, PipelineOp::Paste { .. })
    {
        matches!(image, DynamicImage::ImageLuma16(_))
    } else {
        gpu_image_layout_is_supported(image)
    }
}

pub(super) fn gpu_pipeline_requires_cpu(
    ops: &[PipelineOp],
    image: &DynamicImage,
    auxiliary_images: &[AuxiliaryImages],
    logical_mode: Option<&str>,
) -> bool {
    if ops.len() != auxiliary_images.len() {
        return true;
    }
    if gpu_dimensions_require_cpu(ops, image, logical_mode) {
        return true;
    }
    let dimensions_fit = |w: u32, h: u32| {
        CheckedDims::new(w, h, 1)
            .map(|dims| dims.total_pixels() <= GPU_BUFFER_CAPACITY as usize)
            .unwrap_or(false)
    };
    let (mut cur_w, mut cur_h) = image.dimensions();
    let mut current_color = image.color();
    for (index, op) in ops.iter().enumerate() {
        let auxiliary = &auxiliary_images[index];
        let auxiliary_is_safe = |is_second: bool, image: &DynamicImage| {
            let (w, h) = image.dimensions();
            gpu_auxiliary_layout_is_supported_for_op(op, current_color, is_second, image)
                && w != 0
                && h != 0
                && dimensions_fit(w, h)
        };
        if auxiliary
            .second
            .as_deref()
            .is_some_and(|second| !auxiliary_is_safe(true, second))
            || auxiliary
                .third
                .as_deref()
                .is_some_and(|third| !auxiliary_is_safe(false, third))
        {
            return true;
        }
        if !gpu_auxiliary_shapes_are_safe(op, auxiliary, cur_w, cur_h) {
            return true;
        }
        if !gpu_auxiliary_modes_are_safe_for_color(op, current_color, auxiliary, logical_mode) {
            return true;
        }
        if let Some(next) = op_output_dims(op, cur_w, cur_h) {
            (cur_w, cur_h) = next;
        }
        current_color = gpu_color_after_op(current_color, op);
    }
    false
}

pub(super) fn gpu_color_after_op(
    current: crate::raster::ColorType,
    op: &PipelineOp,
) -> crate::raster::ColorType {
    match op {
        PipelineOp::Convert { mode, .. } => match gpu_convert_target_mode_code(mode) {
            Some(0) => crate::raster::ColorType::L8,
            Some(1) => crate::raster::ColorType::La8,
            Some(2) => crate::raster::ColorType::Rgb8,
            Some(3) => crate::raster::ColorType::Rgba8,
            Some(4) | Some(7) | Some(8) => crate::raster::ColorType::Rgba8,
            Some(5) | Some(6) => crate::raster::ColorType::Rgb8,
            Some(9) => crate::raster::ColorType::L16,
            Some(10) | Some(12) => crate::raster::ColorType::Rgba8,
            Some(11) => crate::raster::ColorType::La8,
            _ => current,
        },
        PipelineOp::Grayscale | PipelineOp::ExtractBand { .. } | PipelineOp::Constant { .. } => {
            crate::raster::ColorType::L8
        }
        PipelineOp::Colorize { .. } => crate::raster::ColorType::Rgb8,
        PipelineOp::Color3DLut { target_mode, .. } => match target_mode {
            PixelMode::RGB => crate::raster::ColorType::Rgb8,
            PixelMode::RGBA | PixelMode::CMYK => crate::raster::ColorType::Rgba8,
            _ => current,
        },
        PipelineOp::ConvertLab { .. } => crate::raster::ColorType::Rgb8,
        _ => current,
    }
}

/// Multi-input shaders use the primary mode word for every packed sample.
/// Require an auxiliary source to have the same native byte layout; otherwise
/// a legal CPU operation such as RGB-vs-L would be interpreted as unrelated
/// channels on GPU. Paste masks have a separate luma/alpha contract and are
/// checked below against the channel the shader actually samples.
pub(super) fn gpu_auxiliary_modes_are_safe(
    op: &PipelineOp,
    image: &DynamicImage,
    auxiliary: &AuxiliaryImages,
) -> bool {
    gpu_auxiliary_modes_are_safe_for_color(op, image.color(), auxiliary, None)
}

pub(super) fn gpu_auxiliary_modes_are_safe_for_color(
    op: &PipelineOp,
    current_color: crate::raster::ColorType,
    auxiliary: &AuxiliaryImages,
    logical_mode: Option<&str>,
) -> bool {
    if matches!(
        op,
        PipelineOp::Autocontrast { mask: Some(_), .. } | PipelineOp::EqualizeMasked { .. }
    ) {
        // ImageOps.autocontrast validates masks as mode 1/L. Both are
        // represented by the Luma8 transport, whose first byte is exactly the
        // nonzero selector consumed by the histogram gather shader.
        return matches!(
            auxiliary.third.as_deref(),
            Some(DynamicImage::ImageLuma8(_))
        );
    }
    if let PipelineOp::Paste { mask_alpha, .. } = op {
        if current_color == crate::raster::ColorType::L16 {
            let source_is_luma16 = auxiliary
                .second
                .as_deref()
                .is_some_and(|source| matches!(source, DynamicImage::ImageLuma16(_)));
            let mask_is_supported = match (*mask_alpha, auxiliary.third.as_deref()) {
                (false, None) | (false, Some(DynamicImage::ImageLuma8(_))) => true,
                (true, Some(DynamicImage::ImageRgba8(_))) => true,
                _ => false,
            };
            return source_is_luma16 && mask_is_supported;
        }
    }
    if let PipelineOp::CompositeModule { mask_alpha, .. } = op {
        let Some(destination) = auxiliary.second.as_ref() else {
            return false;
        };
        let Some(mask) = auxiliary.third.as_ref() else {
            return false;
        };
        // Image.composite converts image1 to image2's mode before the
        // operation is queued. The shader can therefore use one mode word
        // only when both packed sources have the same native layout.
        if destination.color() != current_color {
            return false;
        }
        // Pillow's mask contract selects byte 0 for 1/L and byte 3 for
        // LA/RGBA/RGBa. Reject RGB luma masks here: their CPU path computes
        // weighted luma while the packed shader would read only R.
        return if *mask_alpha {
            matches!(
                mask.as_ref(),
                DynamicImage::ImageLumaA8(_) | DynamicImage::ImageRgba8(_)
            )
        } else {
            matches!(mask.as_ref(), DynamicImage::ImageLuma8(_))
        };
    }
    let requires_matching_source = matches!(
        op,
        PipelineOp::Add { .. }
            | PipelineOp::Subtract { .. }
            | PipelineOp::Multiply { .. }
            | PipelineOp::Screen { .. }
            | PipelineOp::Darker { .. }
            | PipelineOp::Lighter { .. }
            | PipelineOp::Difference { .. }
            | PipelineOp::AddModulo { .. }
            | PipelineOp::SubtractModulo { .. }
            | PipelineOp::LogicalAnd { .. }
            | PipelineOp::LogicalOr { .. }
            | PipelineOp::LogicalXor { .. }
            | PipelineOp::BlendModule { .. }
            | PipelineOp::Paste { .. }
            | PipelineOp::AlphaComposite { .. }
    );
    if requires_matching_source
        && auxiliary
            .second
            .as_ref()
            .is_some_and(|second| second.color() != current_color)
    {
        let native_rgb_to_rgba_paste = matches!(
            (op, logical_mode, current_color, auxiliary.second.as_deref()),
            (
                PipelineOp::Paste { mask: None, .. },
                Some("RGBA"),
                crate::raster::ColorType::Rgba8,
                Some(DynamicImage::ImageRgb8(_))
            )
        );
        if !native_rgb_to_rgba_paste {
            return false;
        }
    }
    if let PipelineOp::Paste {
        mask_alpha: false, ..
    } = op
    {
        // Paste's luma-mask path calls DynamicImage::to_luma8(), which uses
        // weighted RGB conversion for RGB/RGBA masks. The shader samples byte
        // 0 directly, so only native L/LA masks are exact.
        return !auxiliary.third.as_ref().is_some_and(|mask| {
            !matches!(
                mask.as_ref(),
                DynamicImage::ImageLuma8(_) | DynamicImage::ImageLumaA8(_)
            )
        });
    }
    true
}

/// Return the largest packed RGBA image that a batch needs. Image storage is
/// batch-owned, so allocating to the batch's actual high-water mark avoids
/// reserving the global maximum for every concurrent lazy pipeline.
pub(super) fn gpu_batch_capacity(
    ops: &[PipelineOp],
    image: &DynamicImage,
    auxiliary_images: &[AuxiliaryImages],
    logical_mode: Option<&str>,
) -> Result<u32, PilError> {
    if ops.len() != auxiliary_images.len() {
        return Err(PilError::InternalError(
            "GPU operation and auxiliary-image counts differ".into(),
        ));
    }

    let pixels = |(w, h): (u32, u32)| -> Result<usize, PilError> {
        Ok(CheckedDims::new(w, h, 1)?.total_pixels())
    };
    let mut high_water = pixels(image.dimensions())?;
    for auxiliary in auxiliary_images {
        for nested in auxiliary
            .second
            .as_ref()
            .into_iter()
            .chain(auxiliary.third.as_ref())
        {
            high_water = high_water.max(pixels(nested.dimensions())?);
        }
    }

    let (mut cur_w, mut cur_h) = image.dimensions();
    for op in ops {
        let (out_w, out_h) = match op_output_dims(op, cur_w, cur_h) {
            Some(dimensions) => dimensions,
            None if op_has_explicit_output_dimensions(op) => {
                return Err(PilError::ValueError(format!(
                    "GPU operation '{}' has no safe output dimensions",
                    registry::variant_key(op)
                )));
            }
            None => (cur_w, cur_h),
        };
        high_water = high_water.max(pixels((out_w, out_h))?);
        let uses_separable_resize = matches!(op, PipelineOp::ResizeBoxed { .. })
            || matches!(
                op,
                PipelineOp::Resize { filter, .. }
                    if !matches!(filter, ResampleFilter::Nearest)
                        || gpu_resize_nearest_uses_coefficients(logical_mode)
            );
        if uses_separable_resize {
            // Both resize plans materialize a horizontal intermediate before
            // the vertical pass. ResizeBoxed may compact the source rows, but
            // the full source height is a safe upper bound for that span.
            // A wide resize can therefore need more storage than either
            // endpoint; account for the intermediate before allocating.
            high_water = high_water.max(pixels((out_w, cur_h))?);
        }
        if let PipelineOp::PutData { data, mode } = op {
            high_water = high_water.max(data.len().div_ceil(mode.channels()));
        }
        (cur_w, cur_h) = (out_w, out_h);
    }

    if high_water == 0 || high_water > GPU_BUFFER_CAPACITY as usize {
        return Err(PilError::ValueError(format!(
            "GPU batch requires {} pixels; supported capacity is {}",
            high_water, GPU_BUFFER_CAPACITY
        )));
    }
    Ok(high_water as u32)
}

/// Check the size of each batch-owned image buffer against the selected
/// adapter's actual limits before calling `Device::create_buffer`.
///
/// The static pixel cap is intentionally conservative, but keeping this
/// check dynamic prevents a future cap change or an adapter with narrower
/// limits from turning a valid public image into a device validation error.
pub(super) fn gpu_buffer_capacity_exceeds_limits(
    capacity: u32,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
) -> bool {
    let Some(bytes) = u64::from(capacity).checked_mul(4) else {
        return true;
    };
    bytes > u64::from(max_storage_buffer_binding_size) || bytes > max_buffer_size
}

/// Validate the dispatch grid against the adapter limit. Pixel-count limits
/// alone are insufficient: a very wide, short image can fit in storage while
/// still requiring more workgroups in one dimension than the device accepts.
pub(super) fn gpu_dispatch_dimensions_require_cpu(
    ops: &[PipelineOp],
    image_dimensions: (u32, u32),
    max_workgroups_per_dimension: u32,
    logical_mode: Option<&str>,
    packed_luma_pad: bool,
) -> bool {
    if max_workgroups_per_dimension == 0 {
        return true;
    }
    let (mut cur_w, mut cur_h) = image_dimensions;
    if cur_w == 0 || cur_h == 0 {
        return true;
    }
    for op in ops {
        let next = match op_output_dims(op, cur_w, cur_h) {
            Some(dimensions) => dimensions,
            None if op_has_explicit_output_dimensions(op) => return true,
            None => (cur_w, cur_h),
        };
        let dispatch_exceeds_limit = if matches!(op, PipelineOp::Pad { .. })
            && packed_luma_pad
            && matches!(ops, [PipelineOp::Pad { .. }])
        {
            // Packed-L Pad proves the row-packed H/V resize grids and the
            // flattened final placement grid against this adapter's limits.
            !gpu_packed_luma_pad_dispatch_supported(
                op,
                (cur_w, cur_h),
                max_workgroups_per_dimension,
            )
        } else if matches!(op, PipelineOp::Pad { .. }) {
            let Some(((resize_w, resize_h), _)) = gpu_pad_geometry(op, cur_w, cur_h) else {
                return true;
            };
            resize_w.div_ceil(16) > max_workgroups_per_dimension
                || cur_h.div_ceil(16) > max_workgroups_per_dimension
                || resize_w.div_ceil(16) > max_workgroups_per_dimension
                || resize_h.div_ceil(16) > max_workgroups_per_dimension
                || next.0.div_ceil(16) > max_workgroups_per_dimension
                || next.1.div_ceil(16) > max_workgroups_per_dimension
        } else if matches!(
            op,
            PipelineOp::BoxBlur { .. }
                | PipelineOp::BoxBlurXY { .. }
                | PipelineOp::GaussianBlur { .. }
        ) {
            plan_blur_dispatch(next.0, next.1, max_workgroups_per_dimension).is_err()
        } else if matches!(
            op,
            PipelineOp::ExtractBand { .. } | PipelineOp::Constant { .. }
        ) {
            plan_extract_band_dispatch(next.0, next.1, max_workgroups_per_dimension).is_err()
        } else if matches!(op, PipelineOp::Fit { .. } | PipelineOp::ResizeBoxed { .. })
            || matches!(op, PipelineOp::Resize { filter, .. }
            if !matches!(filter, ResampleFilter::Nearest)
                || gpu_resize_nearest_uses_coefficients(logical_mode))
        {
            // The horizontal resize pass is indexed by output x and source
            // y; the vertical pass is indexed by output x and output y.
            let compact_vertical = ops.len() == 1
                && gpu_f_resize_compact_box_vertical_only_geometry(
                    op,
                    (cur_w, cur_h),
                    next,
                    logical_mode,
                );
            next.0.div_ceil(16) > max_workgroups_per_dimension
                || (!compact_vertical && cur_h.div_ceil(16) > max_workgroups_per_dimension)
                || next.1.div_ceil(16) > max_workgroups_per_dimension
        } else {
            next.0.div_ceil(16) > max_workgroups_per_dimension
                || next.1.div_ceil(16) > max_workgroups_per_dimension
        };
        if next.0 == 0 || next.1 == 0 || dispatch_exceeds_limit {
            return true;
        }
        (cur_w, cur_h) = next;
    }
    false
}

/// Keep finite-but-expensive kernels below a conservative watchdog budget.
/// The estimate counts channel-sample work per output pixel; kernels without
/// dynamic inner work return false and remain eligible for GPU dispatch.
pub(super) fn gpu_blur_pass_work_items(radius: u32) -> u64 {
    let taps = u64::from(radius).saturating_mul(2).saturating_add(1);
    // The shader sums four packed channels for every window sample and reads
    // two more packed pixels for fractional edge weights.
    taps.saturating_mul(4).saturating_add(8)
}

pub(super) fn gpu_shader_work_items(
    op: &PipelineOp,
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    logical_mode: Option<&str>,
) -> Option<u64> {
    // The convolution shaders guard their neighborhood loads and copy every
    // pixel when no interior exists. Small non-empty images are therefore
    // valid zero-interior GPU workloads; only the shader's invocation count,
    // not the kernel radius, determines their safety.
    let output_pixels = u64::from(output_dimensions.0) * u64::from(output_dimensions.1);
    let (source_w, source_h) = source_dimensions;
    if matches!(op, PipelineOp::Fit { .. } | PipelineOp::ResizeBoxed { .. })
        || matches!(op, PipelineOp::Resize { filter, .. } | PipelineOp::Scale { filter, .. }
        if !matches!(filter, ResampleFilter::Nearest)
            || gpu_resize_nearest_uses_coefficients(logical_mode))
    {
        // Unlike the single-pass kernels below, the separable resize has one
        // pass over source rows for every output column and a second pass over
        // the final output. Return the total directly; it must not be
        // multiplied by output_pixels a second time.
        return Some(
            u64::from(source_h)
                .saturating_mul(u64::from(output_dimensions.0))
                .saturating_add(output_pixels),
        );
    }
    if matches!(op, PipelineOp::Pad { .. }) {
        let Some(((resize_w, resize_h), _)) =
            gpu_pad_geometry(op, source_dimensions.0, source_dimensions.1)
        else {
            return None;
        };
        // Horizontal resize visits source rows for every intermediate
        // column. The fused RGBX vertical pass and the ordinary final
        // placement each visit the destination canvas once; the unfused path
        // also materializes the intermediate resized frame.
        let vertical_work = if gpu_pad_rgbx_fused_vertical_geometry(
            op,
            source_dimensions,
            logical_mode,
        )
        .is_some()
        {
            output_pixels
        } else {
            u64::from(resize_w)
                .saturating_mul(u64::from(resize_h))
                .saturating_add(output_pixels)
        };
        return Some(
            u64::from(source_dimensions.1)
                .saturating_mul(u64::from(resize_w))
                .saturating_add(vertical_work),
        );
    }
    let inner_work = match op {
        PipelineOp::BoxBlur { radius } => gpu_blur_pass_work_items(*radius).saturating_mul(2),
        PipelineOp::BoxBlurXY {
            radius_x,
            radius_y,
            passes,
        } => {
            let radius_x = registry::separable_box_blur_params_f32(*radius_x)?[0];
            let radius_y = registry::separable_box_blur_params_f32(*radius_y)?[0];
            gpu_blur_pass_work_items(radius_x)
                .saturating_add(gpu_blur_pass_work_items(radius_y))
                .saturating_mul(u64::from((*passes).max(1)))
        }
        PipelineOp::GaussianBlur { sigma } => {
            let radius = registry::separable_gaussian_blur_radius(*sigma)?;
            gpu_blur_pass_work_items(radius).saturating_mul(6)
        }
        PipelineOp::MedianFilter { size } | PipelineOp::RankFilter { size, .. } => {
            // These shaders insertion-sort four channel arrays. The sort is
            // quadratic in the window area, so size² alone understates the
            // work by orders of magnitude and can admit watchdog-triggering
            // dispatches for the supported 9x9 maximum.
            let area = u64::from(*size).saturating_mul(u64::from(*size));
            let channels = match logical_mode {
                Some("F" | "I" | "L") => 1,
                Some("LA") => 2,
                Some("RGB" | "YCbCr" | "HSV") => 3,
                _ => 4,
            };
            area.saturating_mul(area).saturating_mul(channels)
        }
        PipelineOp::MaxFilter { size } | PipelineOp::MinFilter { size } => u64::from(*size)
            .saturating_mul(u64::from(*size))
            .saturating_mul(4),
        PipelineOp::Filter3x3 { .. } => 9,
        PipelineOp::Filter5x5 { .. } => 25,
        PipelineOp::Reduce { x_factor, y_factor } => {
            let block_w = u64::from((*x_factor).max(1).min(source_w.max(1)));
            let block_h = u64::from((*y_factor).max(1).min(source_h.max(1)));
            block_w.saturating_mul(block_h)
        }
        PipelineOp::PutData { mode, .. } => mode.channels() as u64,
        PipelineOp::Pad { .. } => 4,
        // Even a constant-work shader consumes one invocation per output
        // pixel. Count that work so a long point-operation chain is split
        // before its cumulative dispatch cost can monopolize one submission.
        _ => 1,
    };
    Some(output_pixels.saturating_mul(inner_work))
}

pub(super) fn gpu_shader_work_requires_cpu(
    op: &PipelineOp,
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    logical_mode: Option<&str>,
) -> bool {
    gpu_shader_work_items(op, source_dimensions, output_dimensions, logical_mode)
        .is_none_or(|work| work > MAX_GPU_SHADER_WORK_ITEMS)
}

pub(super) fn gpu_operation_is_safe(op: &PipelineOp) -> bool {
    let finite_f32 = |value: f64| value.is_finite() && (value as f32).is_finite();
    match op {
        PipelineOp::Filter3x3 {
            kernel,
            scale,
            offset,
        } => {
            registry::gpu_filter_kernel_is_exact(kernel, *scale, *offset)
                || registry::gpu_filter_rational_denominator(kernel, *scale, *offset).is_some()
        }
        PipelineOp::Filter5x5 {
            kernel,
            scale,
            offset,
        } => {
            registry::gpu_filter_kernel_is_exact(kernel, *scale, *offset)
                || registry::gpu_filter_rational_denominator(kernel, *scale, *offset).is_some()
        }
        PipelineOp::GaussianBlur { sigma } => {
            registry::separable_gaussian_blur_radius(*sigma).is_some()
        }
        PipelineOp::BoxBlur { radius } => *radius <= MAX_GPU_BLUR_RADIUS,
        PipelineOp::BoxBlurXY {
            radius_x,
            radius_y,
            passes,
        } => {
            *passes > 0
                && *passes <= 3
                && registry::separable_box_blur_params_f32(*radius_x).is_some()
                && registry::separable_box_blur_params_f32(*radius_y).is_some()
        }
        PipelineOp::MedianFilter { size }
        | PipelineOp::MaxFilter { size }
        | PipelineOp::MinFilter { size } => {
            *size >= 1 && *size <= MAX_GPU_FILTER_SIZE && *size % 2 == 1
        }
        PipelineOp::RankFilter { size, .. } => {
            *size >= 1 && *size <= MAX_GPU_FILTER_SIZE && *size % 2 == 1
        }
        PipelineOp::Reduce { x_factor, y_factor } => {
            *x_factor <= MAX_GPU_REDUCE_FACTOR
                && *y_factor <= MAX_GPU_REDUCE_FACTOR
                && *x_factor >= 1
                && *y_factor >= 1
        }
        // The CPU implementation shifts by `8 - bits`; zero is not a valid
        // direct PipelineOp value and would otherwise underflow that shift.
        // Public constructors already clamp to 1..=8, but keep the GPU
        // safety gate correct for callers that build the pipeline directly.
        PipelineOp::Posterize { bits } => (1..=8).contains(bits),
        PipelineOp::ExtractBand { index } => *index < 4,
        // Keep the scalar preflight in lockstep with the WGSL fixed-point
        // parameterization. The helper checks every byte result against
        // Pillow's f64 contract before a dispatch is admitted.
        PipelineOp::Brightness { factor } => registry::gpu_brightness_factor_int(*factor).is_some(),
        PipelineOp::Contrast { factor } => registry::gpu_blend_alpha_params(*factor).is_some(),
        PipelineOp::ColorSaturation { factor } => {
            registry::gpu_color_saturation_factor_params(*factor).is_some()
        }
        PipelineOp::Sharpness { factor } => registry::gpu_sharpness_factor_int(*factor).is_some(),
        PipelineOp::Add { scale, offset, .. } => {
            // The scalar preflight exhaustively compares all byte pairs with
            // the f32 values consumed by the real WGSL kernel.
            registry::gpu_chops_affine_supported(*scale, *offset)
        }
        PipelineOp::Subtract { scale, offset, .. } => {
            registry::gpu_chops_affine_supported(*scale, *offset)
        }
        PipelineOp::BlendModule { alpha, .. } => registry::gpu_blend_alpha_params(*alpha).is_some(),
        PipelineOp::Scale { factor, .. } => {
            factor.is_finite()
                && *factor > 0.0
                && (*factor * 65536.0).is_finite()
                && *factor * 65536.0 >= 1.0
                && *factor * 65536.0 <= MAX_GPU_SCALE_FIXED_POINT
        }
        PipelineOp::ResizeBoxed {
            w, h, box_coords, ..
        } => {
            *w > 0
                && *h > 0
                && [box_coords.0, box_coords.1, box_coords.2, box_coords.3]
                    .into_iter()
                    .all(finite_f32)
        }
        PipelineOp::Contain { w, h, .. }
        | PipelineOp::Cover { w, h, .. }
        | PipelineOp::Pad { w, h, .. } => *w > 0 && *h > 0,
        PipelineOp::EffectSpread { distance } => *distance <= i32::MAX as u32,
        PipelineOp::EffectNoise { sigma } => finite_f32(*sigma),
        PipelineOp::Rotate {
            angle,
            center,
            translate,
            ..
        } => {
            angle.is_finite()
                && center
                    .map(|(x, y)| x.is_finite() && y.is_finite())
                    .unwrap_or(true)
                && translate
                    .map(|(x, y)| x.is_finite() && y.is_finite())
                    .unwrap_or(true)
        }
        PipelineOp::Transform {
            w,
            h,
            method,
            data,
            filter,
            ..
        } => {
            let shape_valid = match method {
                TransformMethod::Affine | TransformMethod::Perspective | TransformMethod::Quad => {
                    data.len()
                        >= if matches!(method, TransformMethod::Affine) {
                            6
                        } else {
                            8
                        }
                }
                // The GPU mesh lowering carries one complete 12-value mesh
                // record in its uniform block. Larger meshes remain on the
                // CPU until a bounded auxiliary mesh buffer is available.
                TransformMethod::Mesh => data.len() == 12,
            };
            if gpu_transform_is_projective(op) {
                return gpu_transform_geometry_is_admitted(op, None, (1, 1));
            }
            *w > 0
                && *h > 0
                && shape_valid
                && data.iter().copied().all(finite_f32)
                // The image-aware admission proof owns the narrow
                // direct/axis unit-relocation cases for filtered Quad and
                // one-record Mesh transforms. Keep arbitrary filtered maps
                // on the exact host semantic path before this coarse check
                // reaches execution.
                && (!matches!(method, TransformMethod::Mesh)
                    || matches!(
                        filter,
                        ResampleFilter::Nearest
                            | ResampleFilter::Bilinear
                            | ResampleFilter::Bicubic
                    ))
        }
        PipelineOp::AlphaComposite { dest, src, .. } => *dest == (0, 0) && *src == (0, 0),
        PipelineOp::Autocontrast { cutoff, .. } => finite_f32(*cutoff),
        _ => true,
    }
}

/// Return whether an operation's GPU capability depends on the source image.
///
/// Convolution filters and geometry transforms are valid for more than one
/// concrete image contract. The byte filter path is admitted only for kernels
/// proven exact by the operation validator, while the I-mode path uses the
/// same shader with signed samples and a different accumulation contract.
/// Likewise, Transform admission depends on the materialized source layout
/// and method-specific map proof; a non-finite projective map is still a
/// valid Pillow operation whose exact result must be produced by host control.
/// The operation-only routing pass runs before lazy source materialization, so
/// it must defer these operations to image-aware preflight instead of
/// manufacturing a backend capability error merely because it cannot inspect
/// the concrete image yet.
pub(super) fn gpu_operation_requires_image_context(op: &PipelineOp) -> bool {
    matches!(
        op,
        PipelineOp::Filter3x3 { .. }
            | PipelineOp::Filter5x5 { .. }
            | PipelineOp::Transform { .. }
            | PipelineOp::ResizeBoxed { .. }
    )
}
