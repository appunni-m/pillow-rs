//! Raw byte input for Rust-only high-depth raster identities.
//!
//! NO_PILLOW_SUPPORT: these exact-mode sample layouts extend the Pillow
//! `frombytes` contract and preserve the `DynamicImage` variant directly.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{DynamicImage, ImageBuffer, Mode};

fn bytes_per_pixel(mode: Mode) -> Option<u8> {
    match mode {
        Mode::ImageLumaA16 => Some(4),
        Mode::ImageRgb16 => Some(6),
        Mode::ImageRgba16 => Some(8),
        Mode::ImageRgb32F => Some(12),
        Mode::ImageRgba32F => Some(16),
        _ => None,
    }
}

fn expected_bytes(width: u32, height: u32, bytes_per_pixel: u8) -> Result<usize, PilError> {
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(usize::from(bytes_per_pixel)))
        .ok_or_else(|| PilError::DimensionError("image buffer size overflow".into()))
}

fn u16_samples(data: &[u8]) -> Vec<u16> {
    data.chunks_exact(2)
        .map(|sample| u16::from_ne_bytes([sample[0], sample[1]]))
        .collect()
}

fn f32_samples(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(4)
        .map(|sample| f32::from_ne_bytes([sample[0], sample[1], sample[2], sample[3]]))
        .collect()
}

fn buffer_error() -> PilError {
    PilError::ValueError("frombytes: buffer error".into())
}

/// Decodes direct native-endian samples for the five internal raster variants.
pub(super) fn decode(
    mode: Mode,
    (width, height): (u32, u32),
    data: &[u8],
) -> Result<Option<DynamicImage>, PilError> {
    let Some(bytes_per_pixel) = bytes_per_pixel(mode) else {
        return Ok(None);
    };
    let expected = expected_bytes(width, height, bytes_per_pixel)?;
    if data.len() < expected {
        return Err(PilError::ValueError("not enough image data".into()));
    }
    let _dimensions = if width == 0 || height == 0 {
        CheckedDims::new_allow_empty(width, height, bytes_per_pixel)?
    } else {
        CheckedDims::new(width, height, bytes_per_pixel)?
    };
    let data = &data[..expected];
    let image = match mode {
        Mode::ImageLumaA16 => DynamicImage::ImageLumaA16(
            ImageBuffer::from_raw(width, height, u16_samples(data)).ok_or_else(buffer_error)?,
        ),
        Mode::ImageRgb16 => DynamicImage::ImageRgb16(
            ImageBuffer::from_raw(width, height, u16_samples(data)).ok_or_else(buffer_error)?,
        ),
        Mode::ImageRgba16 => DynamicImage::ImageRgba16(
            ImageBuffer::from_raw(width, height, u16_samples(data)).ok_or_else(buffer_error)?,
        ),
        Mode::ImageRgb32F => DynamicImage::ImageRgb32F(
            ImageBuffer::from_raw(width, height, f32_samples(data)).ok_or_else(buffer_error)?,
        ),
        Mode::ImageRgba32F => DynamicImage::ImageRgba32F(
            ImageBuffer::from_raw(width, height, f32_samples(data)).ok_or_else(buffer_error)?,
        ),
        _ => return Ok(None),
    };
    Ok(Some(image))
}
