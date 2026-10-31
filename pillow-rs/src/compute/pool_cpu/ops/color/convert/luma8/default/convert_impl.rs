//! Serial conversions from native L samples without RGB expansion.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage, Mode};

pub(crate) fn execute(source: &GrayImage, target: Mode) -> Result<DynamicImage, PilError> {
    if target == Mode::ImageLuma8 {
        return Ok(DynamicImage::ImageLuma8(source.clone()));
    }
    super::super::super::helpers::luma8::execute(
        source.width(),
        source.height(),
        target,
        source.pixels().map(|pixel| pixel[0]),
    )
}
