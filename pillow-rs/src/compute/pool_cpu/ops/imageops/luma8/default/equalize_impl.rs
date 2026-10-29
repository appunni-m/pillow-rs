//! Serial Equalize implementation for Pillow's native L mode.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage};

pub(crate) fn execute(input: &GrayImage, lut: &[u8]) -> Result<DynamicImage, PilError> {
    if super::super::super::helpers::histogram::lut_is_identity(lut, 1) {
        return Ok(DynamicImage::ImageLuma8(input.clone()));
    }
    let output = input
        .as_raw()
        .iter()
        .map(|&sample| lut[usize::from(sample)])
        .collect();
    let image = GrayImage::from_raw(input.width(), input.height(), output)
        .ok_or_else(|| PilError::InternalError("equalize L buffer shape mismatch".into()))?;
    Ok(DynamicImage::ImageLuma8(image))
}
