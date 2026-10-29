//! Native unsigned 16-bit reducer for exact ImageRgb16 storage.

use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) fn execute(
    input: &crate::raster::ImageBuffer<crate::raster::Rgb<u16>, Vec<u16>>,
    x_factor: u32,
    y_factor: u32,
) -> Result<DynamicImage, PilError> {
    if x_factor < 2 && y_factor < 2 {
        return Ok(DynamicImage::ImageRgb16(input.clone()));
    }
    super::super::super::helpers::native_samples::reduce_u16(input, x_factor, y_factor)
        .map(DynamicImage::ImageRgb16)
}
