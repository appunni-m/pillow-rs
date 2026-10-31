//! Default Posterize strategy over native palette indices.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage};

pub(crate) fn execute(input: &GrayImage, bits: u8) -> Result<DynamicImage, PilError> {
    let mask = !((1u8 << (8 - bits)) - 1);
    let bytes = input.as_raw().iter().map(|&sample| sample & mask).collect();
    let output = GrayImage::from_raw(input.width(), input.height(), bytes)
        .ok_or_else(|| PilError::InternalError("posterize P buffer mismatch".into()))?;
    Ok(DynamicImage::ImagePalette8(output))
}
