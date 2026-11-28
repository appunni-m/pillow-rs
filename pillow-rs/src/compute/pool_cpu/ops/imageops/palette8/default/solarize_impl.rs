//! Default Solarize strategy over native palette indices.

// NO_PILLOW_SUPPORT: Pillow transforms P palette entries; this Rust route transforms indices.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage};

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn execute(input: &GrayImage, threshold: u8) -> Result<DynamicImage, PilError> {
    let bytes = input
        .as_raw()
        .iter()
        .map(|&sample| {
            if sample >= threshold {
                u8::MAX - sample
            } else {
                sample
            }
        })
        .collect();
    let output = GrayImage::from_raw(input.width(), input.height(), bytes)
        .ok_or_else(|| PilError::InternalError("solarize P buffer mismatch".into()))?;
    Ok(DynamicImage::ImagePalette8(output))
}
