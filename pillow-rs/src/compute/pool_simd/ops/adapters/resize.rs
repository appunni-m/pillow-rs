//! SIMD Resize family dispatch and mode-specific kernels.

#[path = "resize/kernels/i32.rs"]
mod i32_mode;

pub(super) use self::i32_mode::{
    build_resize_i_horizontal_plan, resize_i_horizontal_values, simd_resize_i32,
    simd_resize_i32_boxed,
};
use super::{
    boxed_nearest_axis_maps_identity, native_copy_image_bytes, native_fit_filter,
    native_resize_boxed_bounds, native_resize_boxed_layout_for_image,
    native_resize_boxed_supported_for_image, native_resize_byte_layout_for_image,
    native_resize_supported_for_image, native_small_uniform_byte_image, native_zero_byte_image,
    simd_resize_convolution_boxed, simd_resize_convolution_with_parallel_threshold, simd_resize_f,
    simd_resize_f_boxed, simd_resize_luma16, simd_resize_nearest, simd_resize_nearest_boxed,
    simd_unsupported,
};

use crate::error::PilError;
use crate::image::preserve_mode;
use crate::pipeline::{PipelineOp, ResampleFilter};
use crate::raster::DynamicImage;
use wide::u8x16;

/// Execute a native SIMD resize through the shared family adapter.
pub(crate) fn simd_resize(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    simd_resize_impl(img, op, mode, true, 0, false)
}

fn simd_resize_zero_source(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    channels: usize,
) -> Result<DynamicImage, PilError> {
    if !img.as_bytes().is_empty() {
        return Err(PilError::InternalError(
            "SIMD resize empty-source buffer shape mismatch".into(),
        ));
    }
    let output_len = usize::try_from(output_width)
        .ok()
        .and_then(|width| {
            usize::try_from(output_height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let zero = u8x16::splat(0).to_array();
    let mut output = vec![0u8; output_len];
    let mut vector_blocks = 0u64;
    for block in output.chunks_exact_mut(zero.len()) {
        block.copy_from_slice(&zero);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let remainder = output.len() % zero.len();
    if remainder != 0 {
        let start = output.len() - remainder;
        let mut padded = [0u8; 16];
        padded.copy_from_slice(&zero);
        output[start..].copy_from_slice(&padded[..remainder]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    let result =
        crate::image_utils::raw_bytes_to_image(output_width, output_height, output, channels)?;
    Ok(preserve_mode(img, result))
}

fn simd_resize_boxed(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    filter: ResampleFilter,
    bounds: (f64, f64, f64, f64),
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if !native_resize_boxed_supported_for_image(
        img,
        output_width,
        output_height,
        filter,
        bounds,
        mode,
    ) {
        return Err(simd_unsupported("Resize"));
    }
    let (box_left, box_top, box_right, box_bottom) =
        native_resize_boxed_bounds(img.width(), img.height(), bounds)
            .ok_or_else(|| simd_unsupported("Resize"))?;
    let effective_filter = native_fit_filter(mode, filter);
    let (channels, _) = native_resize_boxed_layout_for_image(img, effective_filter, mode)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    simd_resize_boxed_with_layout(
        img,
        output_width,
        output_height,
        (box_left, box_top, box_right, box_bottom),
        effective_filter,
        channels,
        mode,
    )
}

fn simd_resize_boxed_with_layout(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    bounds: (f64, f64, f64, f64),
    effective_filter: ResampleFilter,
    channels: usize,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if let Some(result) = simd_resize_boxed_shortcut(
        img,
        output_width,
        output_height,
        bounds,
        effective_filter,
        channels,
        mode,
    )? {
        return Ok(result);
    }
    let (box_left, box_top, box_right, box_bottom) = bounds;

    // Pillow takes an integer crop before selecting a filter unless filtered
    // LA/RGBA needs its observable premultiply/unpremultiply round trip.
    let needs_alpha = !matches!(effective_filter, ResampleFilter::Nearest)
        && !matches!(
            mode,
            Some("CMYK" | "F" | "I" | "RGBa" | "RGBX" | "La" | "PA")
        )
        && matches!(
            img.color(),
            crate::raster::ColorType::Rgba8 | crate::raster::ColorType::La8
        );
    let integer_crop = box_left.fract() == 0.0
        && box_top.fract() == 0.0
        && (box_right as f32 - box_left as f32) == output_width as f32
        && (box_bottom as f32 - box_top as f32) == output_height as f32;
    if integer_crop && !needs_alpha {
        let result = img.crop_imm(box_left as u32, box_top as u32, output_width, output_height);
        crate::compute::record_pipeline_operation_path("native-crop");
        return Ok(preserve_mode(img, result));
    }

    if mode == Some("F") && !matches!(effective_filter, ResampleFilter::Nearest) {
        return simd_resize_f_boxed(
            img,
            output_width,
            output_height,
            box_left,
            box_top,
            box_right,
            box_bottom,
            effective_filter,
        );
    }

    if matches!(effective_filter, ResampleFilter::Nearest) {
        simd_resize_nearest_boxed(
            img,
            output_width,
            output_height,
            box_left,
            box_top,
            box_right,
            box_bottom,
            channels,
        )
    } else {
        simd_resize_convolution_boxed(
            img,
            output_width,
            output_height,
            box_left,
            box_top,
            box_right,
            box_bottom,
            effective_filter,
            channels,
            needs_alpha,
            0,
        )
    }
}

fn simd_resize_boxed_shortcut(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    bounds: (f64, f64, f64, f64),
    effective_filter: ResampleFilter,
    channels: usize,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let (box_left, box_top, box_right, box_bottom) = bounds;
    // The CPU executor returns the original stored samples before mode
    // conversion for a full-source identity box.
    if img.width() == output_width
        && img.height() == output_height
        && box_left == 0.0
        && box_top == 0.0
        && box_right == f64::from(img.width() as f32)
        && box_bottom == f64::from(img.height() as f32)
    {
        let result =
            native_copy_image_bytes(img, mode)?.ok_or_else(|| simd_unsupported("Resize"))?;
        return Ok(Some(preserve_mode(img, result)));
    }

    if mode == Some("F")
        && matches!(effective_filter, ResampleFilter::Nearest)
        && img.width() == output_width
        && img.height() == output_height
        && boxed_nearest_axis_maps_identity(img.width(), output_width, box_left, box_right)
        && boxed_nearest_axis_maps_identity(img.height(), output_height, box_top, box_bottom)
    {
        crate::compute::record_pipeline_operation_path("native-f-nearest-identity");
        return Ok(Some(img.clone()));
    }

    if img.width() == 0 || img.height() == 0 {
        return Ok(Some(simd_resize_zero_source(
            img,
            output_width,
            output_height,
            channels,
        )?));
    }

    // Every stored channel remains zero through a normalized boxed filter.
    // Avoid coefficient construction and both separable passes for bounded
    // small inputs where the source scan is cheaper than the vector work.
    if native_zero_byte_image(img, channels) {
        let output_len = usize::try_from(output_width)
            .ok()
            .and_then(|width| usize::try_from(output_height).ok()?.checked_mul(width))
            .and_then(|pixels| pixels.checked_mul(channels))
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let output = vec![0; output_len];
        crate::compute::record_pipeline_operation_path("native-zero-fill");
        crate::compute::record_pipeline_operation_vector_blocks((output_len / 16) as u64);
        crate::compute::record_pipeline_operation_scalar_tail((output_len % 16) as u64);
        let result =
            crate::image_utils::raw_bytes_to_image(output_width, output_height, output, channels)?;
        return Ok(Some(preserve_mode(img, result)));
    }
    Ok(None)
}

pub(super) fn simd_resize_impl(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
    cover_byte_i32: bool,
    parallel_pixel_threshold: usize,
    vectorize_tail: bool,
) -> Result<DynamicImage, PilError> {
    match op {
        PipelineOp::Resize { w, h, filter } => simd_resize_direct(
            img,
            *w,
            *h,
            filter,
            mode,
            cover_byte_i32,
            parallel_pixel_threshold,
            vectorize_tail,
        ),
        PipelineOp::ResizeBoxed {
            w,
            h,
            filter,
            box_coords,
        } => simd_resize_boxed(img, *w, *h, *filter, *box_coords, mode),
        _ => Err(PilError::ValueError("expected Resize op".into())),
    }
}

fn simd_resize_direct(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    filter: &ResampleFilter,
    mode: Option<&str>,
    cover_byte_i32: bool,
    parallel_pixel_threshold: usize,
    vectorize_tail: bool,
) -> Result<DynamicImage, PilError> {
    if matches!(mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
        && matches!(img, DynamicImage::ImageLuma16(_))
    {
        if !native_resize_supported_for_image(img, output_width, output_height, *filter, mode) {
            return Err(simd_unsupported("Resize"));
        }
        return simd_resize_luma16(img, output_width, output_height, filter, mode);
    }
    if img.width() == output_width && img.height() == output_height {
        let result =
            native_copy_image_bytes(img, mode)?.ok_or_else(|| simd_unsupported("Resize"))?;
        return Ok(preserve_mode(img, result));
    }
    if !native_resize_supported_for_image(img, output_width, output_height, *filter, mode) {
        return Err(simd_unsupported("Resize"));
    }

    // `I` and `F` store one signed-integer or float sample in four bytes.
    // Their byte representation is not four independent color channels, so
    // keep them in their sample domain instead of sending them through the
    // native byte resampler.
    if mode == Some("F") && matches!(img, DynamicImage::ImageRgba8(_)) {
        return simd_resize_f(img, output_width, output_height, filter);
    }
    if mode == Some("I") && matches!(img, DynamicImage::ImageRgba8(_)) {
        return simd_resize_i32(img, output_width, output_height, filter);
    }

    let (channels, premultiplied_alpha) =
        native_resize_byte_layout_for_image(img, mode).ok_or_else(|| simd_unsupported("Resize"))?;
    if img.width() == 0 || img.height() == 0 {
        return simd_resize_zero_source(img, output_width, output_height, channels);
    }
    // The zero-fill shortcut is valid only when every stored channel is zero.
    // A PA/LA image can have a zero first sample and a nonzero alpha byte; in
    // that case the resize must preserve the constant tuple instead of
    // discarding the alpha plane.
    if native_small_uniform_byte_image(img, channels)
        && img.as_bytes().iter().all(|&byte| byte == 0)
    {
        let output_len = usize::try_from(output_width)
            .ok()
            .and_then(|width| usize::try_from(output_height).ok()?.checked_mul(width))
            .and_then(|pixels| pixels.checked_mul(channels))
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let vector_len = output_len / 16 * 16;
        let zero_block = u8x16::splat(0).to_array();
        let mut output = Vec::with_capacity(output_len);
        for _ in (0..vector_len).step_by(16) {
            output.extend_from_slice(&zero_block);
        }
        let scalar_tail = output_len - vector_len;
        if scalar_tail != 0 {
            output.extend_from_slice(&zero_block[..scalar_tail]);
        }
        crate::compute::record_pipeline_operation_path("native-zero-fill");
        crate::compute::record_pipeline_operation_vector_blocks((vector_len / 16) as u64);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);
        let result =
            crate::image_utils::raw_bytes_to_image(output_width, output_height, output, channels)?;
        return Ok(preserve_mode(img, result));
    }
    match filter {
        ResampleFilter::Nearest => simd_resize_nearest(img, output_width, output_height, channels),
        _ => simd_resize_convolution_with_parallel_threshold(
            img,
            output_width,
            output_height,
            *filter,
            channels,
            premultiplied_alpha,
            cover_byte_i32,
            parallel_pixel_threshold,
            vectorize_tail,
        ),
    }
}
