//! Direct one-byte sampling and thresholding for Pillow's bilevel Resize mode.

use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::GrayImage;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(
    input: &GrayImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> Result<GrayImage, PilError> {
    let bytes = super::super::super::helpers::byte_samples::resize::<1>(
        input.as_raw(),
        input.dimensions(),
        (width, height),
        filter,
        bounds,
    )?;
    let mut output = GrayImage::from_raw(width, height, bytes).ok_or_else(|| {
        PilError::ValueError("one-byte Resize failed to create its output plane".into())
    })?;
    for pixel in output.pixels_mut() {
        pixel[0] = if pixel[0] >= 128 { 255 } else { 0 };
    }
    Ok(output)
}
