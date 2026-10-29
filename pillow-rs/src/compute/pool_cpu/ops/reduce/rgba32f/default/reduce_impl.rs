//! Native float reducer for exact ImageRgba32F storage.

use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) fn execute(
    input: &crate::raster::Rgba32FImage,
    x_factor: u32,
    y_factor: u32,
) -> Result<DynamicImage, PilError> {
    if x_factor < 2 && y_factor < 2 {
        return Ok(DynamicImage::ImageRgba32F(input.clone()));
    }
    super::super::super::helpers::native_samples::reduce_f32(input, x_factor, y_factor)
        .map(DynamicImage::ImageRgba32F)
}
