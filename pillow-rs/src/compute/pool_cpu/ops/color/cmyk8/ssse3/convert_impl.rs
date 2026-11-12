//! SSSE3-selected CMYK-to-RGB vector implementation.

use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};

pub(crate) fn execute(source: &RgbaImage) -> Result<DynamicImage, PilError> {
    super::super::helpers::to_rgb::execute_ssse3(source).map(DynamicImage::ImageRgb8)
}
