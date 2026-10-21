//! SIMD adapter for Pillow-compatible GaussianBlur.

use super::{
    DynamicImage, PilError, PipelineOp, gaussian_blur_radius, native_copy_image_bytes,
    simd_native_blur_channels, simd_pil_box_blur, simd_pil_gaussian_blur_l_radius_one,
    simd_pil_gaussian_blur_la_radius_one, simd_pil_gaussian_blur_rgb_radius_one, simd_unsupported,
};

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
