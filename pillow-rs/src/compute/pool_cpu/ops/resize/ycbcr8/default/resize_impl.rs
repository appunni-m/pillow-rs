//! YCbCr8 resampling over its native three-byte sample layout.

use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::RgbImage;

pub(crate) fn execute(
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
    RgbImage::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::ValueError("resize_ycbcr: failed to create output buffer".into()))
}
