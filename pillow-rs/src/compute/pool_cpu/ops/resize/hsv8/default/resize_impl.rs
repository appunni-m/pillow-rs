//! HSV8 resampling over native interleaved samples.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::ops::pil_resize::precompute_coeffs;
use crate::pipeline::ResampleFilter;
use crate::raster::RgbImage;

use super::filtered;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(
    input: &RgbImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> Result<RgbImage, PilError> {
    let source_size = input.dimensions();
    let source_dims = CheckedDims::new_allow_empty(source_size.0, source_size.1, 3)?;
    if input.as_raw().len() != source_dims.total_bytes() {
        return Err(PilError::InternalError(
            "ImageHsv8 Resize received an invalid source buffer".into(),
        ));
    }
    if bounds.is_some()
        || !matches!(filter, ResampleFilter::Bicubic)
        || source_size.0 == 0
        || source_size.1 == 0
        || width == 0
        || height == 0
        || source_size == (width, height)
    {
        return resized_with_helper(input, width, height, filter, bounds);
    }

    CheckedDims::new(width, height, 3)?;
    let horizontal = precompute_coeffs(width, source_size.0, filter);
    let vertical = precompute_coeffs(height, source_size.1, filter);
    if let Some(bytes) = filtered::execute(
        input.as_raw(),
        source_size,
        (width, height),
        &horizontal,
        &vertical,
    ) {
        return image_from_bytes(width, height, bytes);
    }
    let bytes = super::super::super::helpers::byte_samples::filtered_standard_with_coefficients::<3>(
        input.as_raw(),
        source_size,
        (width, height),
        &horizontal,
        &vertical,
    )?;
    image_from_bytes(width, height, bytes)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn resized_with_helper(
    input: &RgbImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> Result<RgbImage, PilError> {
    let bytes = super::super::super::helpers::byte_samples::resize::<3>(
        input.as_raw(),
        input.dimensions(),
        (width, height),
        filter,
        bounds,
    )?;
    image_from_bytes(width, height, bytes)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn image_from_bytes(width: u32, height: u32, bytes: Vec<u8>) -> Result<RgbImage, PilError> {
    RgbImage::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::ValueError("resize_hsv: failed to create output buffer".into()))
}
