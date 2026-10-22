//! SIMD adapter for Pillow-compatible GaussianBlur.

use super::box_blur::{
    simd_la_vertical_radius_one_rows, simd_luma_horizontal_radius_one_three_passes_rows,
    simd_luma_vertical_radius_one_rows, simd_native_blur_channels,
    simd_rgb_horizontal_radius_one_rows, simd_rgb_vertical_radius_one_rows,
};
use super::{
    CheckedDims, DynamicImage, PilError, PipelineOp, SIMD_BOX_BLUR_SCALE, native_copy_image_bytes,
    native_small_uniform_byte_image, preserve_mode, simd_pil_box_blur, simd_unsupported,
};

#[path = "gaussian_blur/kernels/la.rs"]
mod la;

pub(super) fn gaussian_blur_radius(sigma: f32) -> Option<f32> {
    if !sigma.is_finite() || sigma <= 0.0 {
        return None;
    }
    let sigma = f64::from(sigma);
    let sigma2 = sigma * sigma / 3.0;
    let l = ((12.0 * sigma2 + 1.0).sqrt() - 1.0) / 2.0;
    let l = l.floor();
    let l1 = l + 1.0;
    let denominator = 6.0 * (sigma2 - l1 * l1);
    if denominator == 0.0 {
        return None;
    }
    let numerator = (2.0 * l + 1.0) * (l * l1 - 3.0 * sigma2);
    let radius = (l + numerator / denominator) as f32;
    radius.is_finite().then_some(radius)
}

pub(super) fn supports_for_image(img: &DynamicImage, sigma: f32, mode: Option<&str>) -> bool {
    let native_byte_channels = super::native_byte_layout(img, mode);
    if sigma == 0.0 {
        super::has_vectorized_native_identity_copy(img, mode)
    } else {
        gaussian_blur_radius(sigma.abs()).is_some_and(|radius| {
            native_byte_channels.is_some() && radius > 0.0 && img.width() != 0 && img.height() != 0
        })
    }
}

pub(super) fn supports_for_shape(
    shape: super::SimdImageShape,
    sigma: f32,
    mode: Option<&str>,
) -> bool {
    let native_byte_channels = super::shape_native_byte_channels(shape, mode);
    if sigma == 0.0 {
        native_byte_channels.is_some_and(|channels| {
            super::shape_has_vectorized_native_identity_copy(shape, channels)
        })
    } else {
        gaussian_blur_radius(sigma.abs()).is_some_and(|radius| {
            native_byte_channels.is_some() && radius > 0.0 && shape.width != 0 && shape.height != 0
        })
    }
}

/// Keep the three LA horizontal intermediates in row-sized scratch buffers.
#[allow(clippy::too_many_arguments)]
pub(super) fn simd_la_horizontal_radius_one_three_passes_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
    scratch_a: &mut [u8],
    scratch_b: &mut [u8],
) {
    let row_stride = width
        .checked_mul(2)
        .expect("validated LA dimensions fit the native row stride");
    debug_assert!(width > 0);
    debug_assert!(height > 0);
    debug_assert_eq!(source.len(), row_stride * height);
    debug_assert_eq!(destination.len(), source.len());
    debug_assert_eq!(scratch_a.len(), row_stride);
    debug_assert_eq!(scratch_b.len(), row_stride);

    let interior_end = width.saturating_sub(2);
    let vector_blocks_per_row = interior_end.saturating_sub(2) / 8;
    let vector_blocks = vector_blocks_per_row
        .saturating_mul(height)
        .saturating_mul(3) as u64;
    let scalar_tail = width
        .saturating_sub(vector_blocks_per_row.saturating_mul(8))
        .saturating_mul(height)
        .saturating_mul(3);
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);
    }

    for y in 0..height {
        let row_start = y * row_stride;
        let source_row = &source[row_start..row_start + row_stride];
        let destination_row = &mut destination[row_start..row_start + row_stride];
        la::horizontal_radius_one(
            source_row,
            scratch_a,
            width,
            whole_weight,
            fractional_weight,
        );
        la::horizontal_radius_one(scratch_a, scratch_b, width, whole_weight, fractional_weight);
        la::horizontal_radius_one(
            scratch_b,
            destination_row,
            width,
            whole_weight,
            fractional_weight,
        );
    }
}

fn simd_pil_gaussian_blur_la_radius_one(
    img: &DynamicImage,
    radius: f32,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let dimensions = CheckedDims::new(img.width(), img.height(), 2)?;
    if img.as_bytes().len() != dimensions.total_bytes() || (radius as i32) != 1 {
        return Err(simd_unsupported("GaussianBlur"));
    }
    if native_small_uniform_byte_image(img, 2) {
        return native_copy_image_bytes(img, mode)?.ok_or_else(|| simd_unsupported("GaussianBlur"));
    }

    let whole_weight = (SIMD_BOX_BLUR_SCALE as f32 / (radius * 2.0 + 1.0)) as u32;
    let integer_window = 3u32;
    let fractional_weight =
        SIMD_BOX_BLUR_SCALE.wrapping_sub(integer_window.wrapping_mul(whole_weight)) / 2;
    let width = dimensions.width as usize;
    let height = dimensions.height as usize;
    crate::compute::record_pipeline_operation_path("vector");
    let mut work = dimensions.alloc_buffer();
    let mut scratch = dimensions.alloc_buffer();
    let row_dimensions = CheckedDims::new(dimensions.width, 1, 2)?;
    let mut row_scratch_a = row_dimensions.alloc_buffer();
    let mut row_scratch_b = row_dimensions.alloc_buffer();

    // ARM64 measurements still show CPU at 6.34–6.37 ms versus SIMD at
    // 15.21–16.49 ms for 1024×768 LA noise. This fusion removes two full-frame
    // horizontal intermediates, but exact Pillow parity still requires six
    // byte-rounded passes; each SIMD block widens interleaved bytes to u32
    // lanes and packs them back. Profiles place the time in this adapter but
    // do not split costs by axis. A rolling vertical-window experiment reached
    // 24.51 ms, so retain direct vertical loads. See
    // docs/evidence/filter/gaussianblur-la-row-fusion-arm64-20261022/README.md.
    // The three byte-rounded horizontal passes stay in two row-sized buffers;
    // vertical passes still run after every row has completed all three.
    simd_la_horizontal_radius_one_three_passes_rows(
        img.as_bytes(),
        &mut work,
        width,
        height,
        whole_weight,
        fractional_weight,
        &mut row_scratch_a,
        &mut row_scratch_b,
    );
    for _ in 0..3 {
        simd_la_vertical_radius_one_rows(
            &work,
            &mut scratch,
            width,
            height,
            whole_weight,
            fractional_weight,
        );
        std::mem::swap(&mut work, &mut scratch);
    }

    let result =
        crate::image_utils::raw_bytes_to_image(dimensions.width, dimensions.height, work, 2)?;
    Ok(preserve_mode(img, result))
}

pub(super) fn simd_pil_gaussian_blur_l_radius_one(
    img: &DynamicImage,
    radius: f32,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let dimensions = CheckedDims::new(img.width(), img.height(), 1)?;
    if img.as_bytes().len() != dimensions.total_bytes() || (radius as i32) != 1 {
        return Err(simd_unsupported("GaussianBlur"));
    }
    if native_small_uniform_byte_image(img, 1) {
        return native_copy_image_bytes(img, mode)?.ok_or_else(|| simd_unsupported("GaussianBlur"));
    }

    let whole_weight = (SIMD_BOX_BLUR_SCALE as f32 / (radius * 2.0 + 1.0)) as u32;
    let integer_window = 3u32;
    let fractional_weight =
        SIMD_BOX_BLUR_SCALE.wrapping_sub(integer_window.wrapping_mul(whole_weight)) / 2;
    let width = dimensions.width as usize;
    let height = dimensions.height as usize;
    crate::compute::record_pipeline_operation_path("vector");
    let mut work = dimensions.alloc_buffer();
    let mut scratch = dimensions.alloc_buffer();
    let row_dimensions = CheckedDims::new(dimensions.width, 1, 1)?;
    let mut row_scratch_a = row_dimensions.alloc_buffer();
    let mut row_scratch_b = row_dimensions.alloc_buffer();

    // Keeping the vertical row window in byte vectors improved this ARM64
    // workload from 9.05–10.35 ms to 6.53–6.64 ms. SIMD still trails CPU at
    // 2.10–2.50 ms: exact Pillow behavior requires three rounded passes on
    // each axis, with each vector block widened for fixed-point arithmetic
    // and packed after every pass. Profiles attribute time to the blur call
    // but do not split horizontal from vertical cost; see focused evidence.
    // Keep Pillow's three horizontal byte-rounding boundaries, then finish
    // each horizontal row before moving to the next one. The full-frame
    // horizontal intermediates stay out of memory; vertical passes still run
    // only after every row has completed all three horizontal passes.
    simd_luma_horizontal_radius_one_three_passes_rows(
        img.as_bytes(),
        &mut work,
        width,
        height,
        whole_weight,
        fractional_weight,
        &mut row_scratch_a,
        &mut row_scratch_b,
    );
    for _ in 0..3 {
        simd_luma_vertical_radius_one_rows(
            &work,
            &mut scratch,
            width,
            height,
            whole_weight,
            fractional_weight,
        );
        std::mem::swap(&mut work, &mut scratch);
    }

    let result =
        crate::image_utils::raw_bytes_to_image(dimensions.width, dimensions.height, work, 1)?;
    Ok(preserve_mode(img, result))
}

fn simd_pil_gaussian_blur_rgb_radius_one(
    img: &DynamicImage,
    radius: f32,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let dimensions = CheckedDims::new(img.width(), img.height(), 3)?;
    if img.as_bytes().len() != dimensions.total_bytes() || !(1.0..2.0).contains(&radius) {
        return Err(simd_unsupported("GaussianBlur"));
    }
    if native_small_uniform_byte_image(img, 3) {
        return native_copy_image_bytes(img, mode)?.ok_or_else(|| simd_unsupported("GaussianBlur"));
    }

    let whole_weight = (SIMD_BOX_BLUR_SCALE as f32 / (radius * 2.0 + 1.0)) as u32;
    let integer_window = 3u32;
    let fractional_weight =
        SIMD_BOX_BLUR_SCALE.wrapping_sub(integer_window.wrapping_mul(whole_weight)) / 2;
    let width = dimensions.width as usize;
    let height = dimensions.height as usize;
    crate::compute::record_pipeline_operation_path("vector");
    let mut work = img.as_bytes().to_vec();
    let mut scratch = dimensions.alloc_buffer();

    // Keep Pillow's three separately rounded horizontal passes, with each
    // pass using vector loads at the native three-byte RGB offsets.
    for _ in 0..3 {
        simd_rgb_horizontal_radius_one_rows(
            &work,
            &mut scratch,
            width,
            height,
            whole_weight,
            fractional_weight,
        );
        std::mem::swap(&mut work, &mut scratch);
    }
    for _ in 0..3 {
        simd_rgb_vertical_radius_one_rows(
            &work,
            &mut scratch,
            width,
            height,
            whole_weight,
            fractional_weight,
        );
        std::mem::swap(&mut work, &mut scratch);
    }

    let result =
        crate::image_utils::raw_bytes_to_image(dimensions.width, dimensions.height, work, 3)?;
    Ok(preserve_mode(img, result))
}

/// Validate and route GaussianBlur to the native SIMD implementation for the
/// image's mode and blur radius.
pub(crate) fn simd_gaussian_blur(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if let PipelineOp::GaussianBlur { sigma } = op {
        let channels =
            simd_native_blur_channels(img, mode).ok_or_else(|| simd_unsupported("GaussianBlur"))?;
        let pixel_count = (img.width() as usize).saturating_mul(img.height() as usize);
        if !sigma.is_finite() || pixel_count == 0 {
            return Err(simd_unsupported("GaussianBlur"));
        }
        let sigma = sigma.abs();
        if sigma == 0.0 {
            return native_copy_image_bytes(img, mode)?
                .ok_or_else(|| simd_unsupported("GaussianBlur"));
        }
        let blur_radius =
            gaussian_blur_radius(sigma).ok_or_else(|| simd_unsupported("GaussianBlur"))?;
        if blur_radius <= 0.0 {
            return Err(simd_unsupported("GaussianBlur"));
        }
        if channels == 1
            && matches!(img, DynamicImage::ImageLuma8(_))
            && matches!(mode, None | Some("L"))
            && (1.0..2.0).contains(&blur_radius)
        {
            return simd_pil_gaussian_blur_l_radius_one(img, blur_radius, mode);
        }
        if channels == 2
            && matches!(img, DynamicImage::ImageLumaA8(_))
            && matches!(mode, Some("LA"))
            && (1.0..2.0).contains(&blur_radius)
        {
            return simd_pil_gaussian_blur_la_radius_one(img, blur_radius, mode);
        }
        if channels == 3
            && matches!(img, DynamicImage::ImageRgb8(_))
            && matches!(mode, None | Some("RGB"))
            && (1.0..2.0).contains(&blur_radius)
        {
            return simd_pil_gaussian_blur_rgb_radius_one(img, blur_radius, mode);
        }
        return simd_pil_box_blur(img, blur_radius, 3, channels, mode, true);
    }
    Err(PilError::ValueError("expected GaussianBlur op".into()))
}
