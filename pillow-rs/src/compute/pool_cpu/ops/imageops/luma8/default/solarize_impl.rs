//! Serial Solarize implementation for Pillow's native L mode.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage};

pub(crate) fn execute(input: &GrayImage, threshold: u8) -> Result<DynamicImage, PilError> {
    let bytes = input
        .as_raw()
        .iter()
        .map(|&sample| if sample >= threshold { u8::MAX - sample } else { sample })
        .collect();
    let output = GrayImage::from_raw(input.width(), input.height(), bytes)
        .ok_or_else(|| PilError::InternalError("solarize L buffer mismatch".into()))?;
    Ok(DynamicImage::ImageLuma8(output))
}
