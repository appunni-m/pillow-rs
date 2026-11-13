//! Resize LumaA8 with Pillow-compatible premultiply and unpremultiply passes.

use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::GrayAlphaImage;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(
    input: &GrayAlphaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> Result<GrayAlphaImage, PilError> {
    let bytes = super::super::super::helpers::alpha_samples::resize::<2>(
        input.as_raw(),
        input.dimensions(),
        (width, height),
        filter,
        bounds,
    )?;
    GrayAlphaImage::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::ValueError("resize_la: failed to create output buffer".into()))
}
