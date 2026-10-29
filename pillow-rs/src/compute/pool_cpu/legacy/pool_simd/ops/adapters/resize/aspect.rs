use super::{
    AspectDimensions, DynamicImage, PilError, PipelineOp, ResampleFilter,
    SIMD_ASPECT_RESIZE_PARALLEL_PIXEL_THRESHOLD, SIMD_FIT_PARALLEL_PIXEL_THRESHOLD,
    native_contain_supported_for_image, native_copy_image_bytes, native_cover_dimensions,
    native_fit_box, native_fit_filter, native_fit_layout_for_image, native_fit_supported_for_image,
    native_pad_contained_dimensions, native_resize_supported_for_image, native_scale_dimensions,
    native_zero_byte_image, preserve_mode, simd_resize, simd_resize_convolution_boxed,
    simd_resize_f_boxed, simd_resize_impl, simd_resize_nearest_boxed, simd_unsupported, u8x16,
};
fn native_aspect_resize_bytes(
    img: &DynamicImage,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    mode: Option<&str>,
    dimensions: AspectDimensions,
    parallel_pixel_threshold: usize,
    vectorize_tail: bool,
    operation: &str,
    allow_cover_byte_i32: bool,
) -> Result<Option<DynamicImage>, PilError> {
    let Some((output_width, output_height)) =
        dimensions(img.width(), img.height(), target_width, target_height)
    else {
        return Ok(None);
    };
    if !native_resize_supported_for_image(img, output_width, output_height, filter, mode) {
        return Ok(None);
    }
    // Use the common resize dispatcher so logical mode controls alpha
    // premultiplication and I/F keep their one-sample-per-pixel arithmetic.
    // Channel count alone cannot distinguish LA/RGBA from PA/CMYK/RGBa/RGBX.
    let resize = PipelineOp::Resize {
        w: output_width,
        h: output_height,
        filter,
    };
    let use_cover_byte_i32 = allow_cover_byte_i32
        && matches!(filter, ResampleFilter::Bicubic)
        && match mode {
            Some("CMYK") => matches!(img, DynamicImage::ImageRgba8(_)),
            Some("HSV") => matches!(img, DynamicImage::ImageRgb8(_)),
            None | Some("L" | "LA") => {
                matches!(
                    img,
                    DynamicImage::ImageLuma8(_) | DynamicImage::ImageLumaA8(_)
                )
            }
            _ => false,
        };
    let result = if use_cover_byte_i32 {
        simd_resize_impl(
            img,
            &resize,
            mode,
            true,
            parallel_pixel_threshold,
            vectorize_tail,
        )?
    } else {
        simd_resize_impl(
            img,
            &resize,
            mode,
            false,
            parallel_pixel_threshold,
            vectorize_tail,
        )?
    };
    if result.width() != output_width || result.height() != output_height {
        return Err(PilError::InternalError(format!(
            "SIMD {operation} resize shape mismatch"
        )));
    }
    Ok(Some(result))
}

/// Execute `ImageOps.scale` through the native/vectorized resize kernels.
///
/// Scale contributes only scalar dimension calculation and capability
/// preflight. The pixel data plane is delegated to `simd_resize`, which uses
/// native-copy, nearest gather, or vectorized convolution paths and never
/// retries through the CPU adapter.
pub(crate) fn simd_scale(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Scale { factor, filter } = op else {
        return Err(PilError::ValueError("expected Scale op".into()));
    };
    let (w, h) = native_scale_dimensions(img.width(), img.height(), *factor)
        .ok_or_else(|| simd_unsupported("Scale"))?;
    if !native_resize_supported_for_image(img, w, h, *filter, mode) {
        return Err(simd_unsupported("Scale"));
    }
    let resize = PipelineOp::Resize {
        w,
        h,
        filter: *filter,
    };
    match simd_resize(img, &resize, mode) {
        Err(PilError::NotImplementedError(_)) => Err(simd_unsupported("Scale")),
        result => result,
    }
}

/// Execute `ImageOps.contain` with scalar aspect-ratio control and a native
/// SIMD resize data plane. Explicit SIMD reports capability failure before any
/// pixel work when the mode, dimensions, or filter are outside this contract.
pub(crate) fn simd_contain(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Contain { w, h, filter } = op else {
        return Err(PilError::ValueError("expected Contain op".into()));
    };
    if (img.width() == 0 || img.height() == 0)
        && native_contain_supported_for_image(img, *w, *h, *filter, mode)
    {
        crate::compute::record_pipeline_operation_path("scalar-control");
        return Ok(img.clone());
    }
    native_aspect_resize_bytes(
        img,
        *w,
        *h,
        *filter,
        mode,
        native_pad_contained_dimensions,
        SIMD_ASPECT_RESIZE_PARALLEL_PIXEL_THRESHOLD,
        true,
        "Contain",
        false,
    )?
    .ok_or_else(|| simd_unsupported("Contain"))
}

/// Execute `ImageOps.cover` with scalar aspect-ratio control and a native SIMD
/// resize data plane. The operation intentionally retains the core Pillow
/// contract: it resizes to the covering dimensions and does not crop here.
pub(crate) fn simd_cover(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Cover { w, h, filter } = op else {
        return Err(PilError::ValueError("expected Cover op".into()));
    };
    native_aspect_resize_bytes(
        img,
        *w,
        *h,
        *filter,
        mode,
        native_cover_dimensions,
        SIMD_ASPECT_RESIZE_PARALLEL_PIXEL_THRESHOLD,
        true,
        "Cover",
        true,
    )?
    .ok_or_else(|| simd_unsupported("Cover"))
}

/// Execute `ImageOps.fit` with scalar crop-box construction and a native
/// boxed SIMD resize data plane. Indexed P/PA nearest sampling uses the
/// affine boxed path; all other admitted layouts use Pillow's fixed-point
/// two-pass boxed coefficients, including standard-mode nearest filtering.
pub(crate) fn simd_fit(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Fit {
        w,
        h,
        filter,
        bleed,
        centering,
    } = op
    else {
        return Err(PilError::ValueError("expected Fit op".into()));
    };
    if !native_fit_supported_for_image(img, *w, *h, *filter, *bleed, *centering, mode) {
        return Err(simd_unsupported("Fit"));
    }
    let output_width = *w;
    let output_height = *h;
    if img.width() == 0 && output_width == 0 {
        // Preserve Pillow's independent empty-width copy without entering
        // the boxed coefficient builder, which cannot represent a zero
        // output axis. The public geometry guard admits only this exact
        // same-size, no-bleed case.
        crate::compute::record_pipeline_operation_path("scalar-control");
        return Ok(img.clone());
    }
    let (box_left, box_top, box_right, box_bottom) = native_fit_box(
        img.width(),
        img.height(),
        output_width,
        output_height,
        *bleed,
        *centering,
    )
    .ok_or_else(|| simd_unsupported("Fit"))?;
    let resize_filter = native_fit_filter(mode, *filter);
    if let Some(result) = simd_fit_crop_or_float_shortcut(
        img,
        output_width,
        output_height,
        mode,
        (box_left, box_top, box_right, box_bottom),
        resize_filter,
    )? {
        return Ok(result);
    }
    let (channels, premultiplied_alpha) =
        native_fit_layout_for_image(img, mode).ok_or_else(|| simd_unsupported("Fit"))?;
    if img.width() == 0 {
        return simd_fit_empty_source(img, output_width, output_height, channels);
    }
    // A zero source remains zero through Pillow's normalized boxed filters,
    // including the premultiply/unpremultiply alpha path. Fit calls the boxed
    // convolution kernel directly, so apply its zero shortcut here before
    // coefficient construction. Nearest sampling is cheap enough that scanning
    // the whole source would add work instead of removing it.
    if !matches!(resize_filter, ResampleFilter::Nearest) && native_zero_byte_image(img, channels) {
        return simd_fit_zero_image(img, output_width, output_height, channels);
    }
    if matches!(resize_filter, ResampleFilter::Nearest) {
        return simd_resize_nearest_boxed(
            img,
            output_width,
            output_height,
            box_left,
            box_top,
            box_right,
            box_bottom,
            channels,
        );
    }
    simd_resize_convolution_boxed(
        img,
        output_width,
        output_height,
        box_left,
        box_top,
        box_right,
        box_bottom,
        resize_filter,
        channels,
        premultiplied_alpha,
        SIMD_FIT_PARALLEL_PIXEL_THRESHOLD,
    )
}

fn simd_fit_crop_or_float_shortcut(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    mode: Option<&str>,
    bounds: (f64, f64, f64, f64),
    resize_filter: ResampleFilter,
) -> Result<Option<DynamicImage>, PilError> {
    let (box_left, box_top, box_right, box_bottom) = bounds;
    // Pillow's `_resize` takes an integer crop shortcut before selecting the
    // filter. Keep the shortcut only for complete-source identities. A
    // filtered LA/RGBA non-full crop first converts to
    // premultiplied storage, so returning the original bytes here would skip
    // that observable round trip.
    let box_left_f32 = box_left as f32;
    let box_top_f32 = box_top as f32;
    let box_right_f32 = box_right as f32;
    let box_bottom_f32 = box_bottom as f32;
    let finite_box = box_left_f32.is_finite()
        && box_top_f32.is_finite()
        && box_right_f32.is_finite()
        && box_bottom_f32.is_finite();
    let full_source_identity = finite_box
        && img.width() == output_width
        && img.height() == output_height
        && box_left_f32 == 0.0
        && box_top_f32 == 0.0
        && box_right_f32 == img.width() as f32
        && box_bottom_f32 == img.height() as f32;
    if full_source_identity && let Some(result) = native_copy_image_bytes(img, mode)? {
        return Ok(Some(result));
    }
    // `_imaging.c::_resize` takes exact integer crops before dispatching to a
    // filter. Preserve that ordering for F so special values in discarded
    // source pixels cannot participate in a floating-point tail.
    let integer_crop = finite_box
        && box_left_f32.fract() == 0.0
        && box_top_f32.fract() == 0.0
        && box_right_f32 - box_left_f32 == output_width as f32
        && box_bottom_f32 - box_top_f32 == output_height as f32;
    if integer_crop && mode == Some("F") {
        let left = box_left_f32 as u32;
        let top = box_top_f32 as u32;
        return Ok(Some(preserve_mode(
            img,
            img.crop_imm(left, top, output_width, output_height),
        )));
    }
    if mode == Some("F") && matches!(img, DynamicImage::ImageRgba8(_)) {
        return simd_resize_f_boxed(
            img,
            output_width,
            output_height,
            box_left,
            box_top,
            box_right,
            box_bottom,
            resize_filter,
        )
        .map(Some);
    }
    Ok(None)
}

fn simd_fit_empty_source(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    channels: usize,
) -> Result<DynamicImage, PilError> {
    let output_len = usize::try_from(output_width)
        .ok()
        .and_then(|width| usize::try_from(output_height).ok()?.checked_mul(width))
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Fit"))?;
    let mut output = vec![0u8; output_len];
    let zero_block = u8x16::splat(0).to_array();
    let mut vector_blocks = 0u64;
    for block in output.chunks_exact_mut(zero_block.len()) {
        block.copy_from_slice(&zero_block);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let remainder = output.len() % zero_block.len();
    if remainder != 0 {
        let start = output.len() - remainder;
        output[start..].copy_from_slice(&zero_block[..remainder]);
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(u64::from(remainder != 0));
    let result =
        crate::image_utils::raw_bytes_to_image(output_width, output_height, output, channels)?;
    Ok(preserve_mode(img, result))
}

fn simd_fit_zero_image(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    channels: usize,
) -> Result<DynamicImage, PilError> {
    let output_len = usize::try_from(output_width)
        .ok()
        .and_then(|width| usize::try_from(output_height).ok()?.checked_mul(width))
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Fit"))?;
    let output = vec![0; output_len];
    crate::compute::record_pipeline_operation_path("native-zero-fill");
    crate::compute::record_pipeline_operation_vector_blocks((output_len / 16) as u64);
    crate::compute::record_pipeline_operation_scalar_tail((output_len % 16) as u64);
    let result =
        crate::image_utils::raw_bytes_to_image(output_width, output_height, output, channels)?;
    Ok(preserve_mode(img, result))
}
