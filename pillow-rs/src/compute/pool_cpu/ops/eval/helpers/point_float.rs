//! Shared native byte lookup kernel for gray-backed modes producing F samples.

use crate::error::PilError;
use crate::raster::{GrayImage, RgbaImage};

pub(crate) fn execute(input: &GrayImage, lut: &[f64]) -> Result<RgbaImage, PilError> {
    let mut raw = Vec::with_capacity(input.as_raw().len() * std::mem::size_of::<f32>());
    for &sample in input.as_raw() {
        raw.extend_from_slice(&(lut[usize::from(sample)] as f32).to_le_bytes());
    }
    RgbaImage::from_raw(input.width(), input.height(), raw)
        .ok_or_else(|| PilError::ValueError("invalid image data".into()))
}
