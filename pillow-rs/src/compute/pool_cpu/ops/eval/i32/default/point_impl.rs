//! Native I sample transforms for the Default architecture.

use crate::error::PilError;
use crate::raster::{GrayImage, RgbaImage};

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn affine(image: &mut RgbaImage, scale: f64, offset: f64) {
    for pixel in image.pixels_mut() {
        let value = i32::from_le_bytes(pixel.0);
        pixel.0 = ((f64::from(value) * scale + offset) as i32).to_le_bytes();
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn callable<F>(image: &mut RgbaImage, callback: &mut F) -> Result<(), PilError>
where
    F: FnMut(f64) -> Result<f64, PilError>,
{
    for pixel in image.pixels_mut() {
        let value = f64::from(i32::from_le_bytes(pixel.0));
        pixel.0 = (callback(value)? as i32).to_le_bytes();
    }
    Ok(())
}

/// Maps each signed I sample through Pillow's clamped I-to-L byte table.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn to_luma(input: &RgbaImage, lut: &[u8]) -> Result<GrayImage, PilError> {
    let pixel_count = (input.width() as usize)
        .checked_mul(input.height() as usize)
        .ok_or_else(|| PilError::InternalError("Point I32 output size overflowed".into()))?;
    let mut bytes = Vec::with_capacity(pixel_count);
    for pixel in input.pixels() {
        let sample = i32::from_le_bytes(pixel.0).clamp(0, 65_535) as usize;
        bytes.push(lut[sample]);
    }
    GrayImage::from_raw(input.width(), input.height(), bytes)
        .ok_or_else(|| PilError::InternalError("Point I32 output dimensions were invalid".into()))
}
