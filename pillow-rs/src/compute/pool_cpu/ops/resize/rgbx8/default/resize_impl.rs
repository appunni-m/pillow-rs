//! Resample RGBX bytes as four native data channels without alpha handling.

use crate::error::PilError;
use crate::ops::pil_resize::precompute_coeffs;
use crate::pipeline::ResampleFilter;
use crate::raster::RgbaImage;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(
    input: &RgbaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> Result<RgbaImage, PilError> {
    let bytes = super::super::super::helpers::byte_samples::resize::<4>(
        input.as_raw(),
        input.dimensions(),
        (width, height),
        filter,
        bounds,
    )?;
    RgbaImage::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::ValueError("resize_rgbx: failed to create output buffer".into()))
}

/// Resizes RGBX bytes into a caller-owned full-width output window.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute_into_window(
    input: &RgbaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    output: &mut [u8],
) -> Result<bool, PilError> {
    if matches!(filter, ResampleFilter::Nearest)
        || width == 0
        || height == 0
        || input.width() == 0
        || input.height() == 0
    {
        return Ok(false);
    }
    let horizontal = precompute_coeffs(width, input.width(), filter);
    let vertical = precompute_coeffs(height, input.height(), filter);
    super::super::super::helpers::byte_samples::filtered_standard_into::<4>(
        input.as_raw(),
        input.dimensions(),
        (width, height),
        &horizontal,
        &vertical,
        output,
    )?;
    Ok(true)
}
