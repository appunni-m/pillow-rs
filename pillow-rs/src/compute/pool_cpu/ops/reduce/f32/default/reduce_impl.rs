//! Native scalar reducer for exact ImageF32 storage.

use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) fn execute(
    input: &crate::raster::RgbaImage,
    x_factor: u32,
    y_factor: u32,
) -> Result<DynamicImage, PilError> {
    if x_factor < 2 && y_factor < 2 {
        return Ok(DynamicImage::ImageF32(input.clone()));
    }
    super::super::super::helpers::scalar32::reduce_f32(input, x_factor, y_factor)
        .map(DynamicImage::ImageF32)
}
