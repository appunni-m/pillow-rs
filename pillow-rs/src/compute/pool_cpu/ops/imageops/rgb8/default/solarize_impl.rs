//! Serial Solarize implementation for native RGB samples.

use crate::error::PilError;
use crate::raster::{DynamicImage, RgbImage};

pub(crate) fn execute(input: &RgbImage, threshold: u8) -> Result<DynamicImage, PilError> {
    let bytes = input
        .as_raw()
        .iter()
        .map(|&sample| if sample >= threshold { u8::MAX - sample } else { sample })
        .collect();
    let output = RgbImage::from_raw(input.width(), input.height(), bytes)
        .ok_or_else(|| PilError::InternalError("solarize RGB buffer mismatch".into()))?;
    Ok(DynamicImage::ImageRgb8(output))
}
