//! Native LAB8 identity conversion; cross-space conversion is explicitly pending.

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbImage};

pub(crate) fn execute(source: &RgbImage, target: Mode) -> Result<DynamicImage, PilError> {
    match target {
        Mode::ImageLab8 => Ok(DynamicImage::ImageLab8(source.clone())),
        _ => Err(PilError::ValueError("image has wrong mode".into())),
    }
}
