//! Identity-only reducer for exact ImagePaletteAlpha8 palette semantics.

use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) fn execute(
    input: &crate::raster::GrayAlphaImage,
    x_factor: u32,
    y_factor: u32,
) -> Result<DynamicImage, PilError> {
    if x_factor < 2 && y_factor < 2 {
        return Ok(DynamicImage::ImagePaletteAlpha8(input.clone()));
    }
    if x_factor.max(1) != 1 || y_factor.max(1) != 1 {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    Ok(DynamicImage::ImagePaletteAlpha8(input.clone()))
}
