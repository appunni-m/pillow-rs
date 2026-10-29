//! Native float reducer for exact ImageRgb32F storage.

use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) fn execute(
    input: &crate::raster::Rgb32FImage,
    x_factor: u32,
    y_factor: u32,
) -> Result<DynamicImage, PilError> {
    if x_factor < 2 && y_factor < 2 {
        return Ok(DynamicImage::ImageRgb32F(input.clone()));
    }
    super::super::super::helpers::native_samples::reduce_f32(input, x_factor, y_factor)
        .map(DynamicImage::ImageRgb32F)
}
