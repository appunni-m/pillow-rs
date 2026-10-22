//! SIMD adapter for Pillow-compatible GaussianBlur.

use super::{
    DynamicImage, PilError, PipelineOp, gaussian_blur_radius, native_copy_image_bytes,
    simd_native_blur_channels, simd_pil_box_blur, simd_pil_gaussian_blur_l_radius_one,
    simd_pil_gaussian_blur_la_radius_one, simd_pil_gaussian_blur_rgb_radius_one, simd_unsupported,
};

#[path = "gaussian_blur/kernels/la.rs"]
mod la;

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
