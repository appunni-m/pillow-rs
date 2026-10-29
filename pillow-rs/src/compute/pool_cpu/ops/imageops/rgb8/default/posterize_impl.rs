//! Serial Posterize implementation for native RGB samples.

use crate::error::PilError;
use crate::raster::{DynamicImage, RgbImage};

pub(crate) fn execute(input: &RgbImage, bits: u8) -> Result<DynamicImage, PilError> {
    let mask = !((1u8 << (8 - bits)) - 1);
    let bytes = input.as_raw().iter().map(|&sample| sample & mask).collect();
    let output = RgbImage::from_raw(input.width(), input.height(), bytes)
        .ok_or_else(|| PilError::InternalError("posterize RGB buffer mismatch".into()))?;
    Ok(DynamicImage::ImageRgb8(output))
}
