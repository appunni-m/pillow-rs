//! Identity-only reducer for exact ImagePalette8 palette semantics.

use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) fn execute(input: &crate::raster::GrayImage) -> Result<DynamicImage, PilError> {
    Ok(DynamicImage::ImagePalette8(input.clone()))
}
