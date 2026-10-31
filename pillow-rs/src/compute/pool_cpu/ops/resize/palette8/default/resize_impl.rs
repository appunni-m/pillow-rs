//! Raw palette-index sampling without color-table expansion.

use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::GrayImage;

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
    GrayImage::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::ValueError("resize_p: failed to create output buffer".into()))
}
