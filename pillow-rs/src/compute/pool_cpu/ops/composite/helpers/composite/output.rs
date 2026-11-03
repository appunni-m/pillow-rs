//! Exact-mode output construction for native typed Composite samples.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{
    DynamicImage, ImageBuffer, Luma, LumaA, Mode, Rgb, Rgb32FImage, Rgba, Rgba32FImage,
};

pub(super) fn u16_image(
    width: u32,
    height: u32,
    mode: Mode,
    bytes: Vec<u8>,
    channels: u8,
) -> Result<DynamicImage, PilError> {
    let dimensions = CheckedDims::new_allow_empty(width, height, channels * 2)?;
    if bytes.len() != dimensions.total_bytes() || bytes.len() % 2 != 0 {
        return Err(invalid_shape("u16"));
    }
    let samples = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_ne_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    match mode {
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            let image = ImageBuffer::<Luma<u16>, Vec<u16>>::from_raw(width, height, samples)
                .ok_or_else(|| invalid_shape("I;16"))?;
            match mode {
                Mode::ImageI16 => Ok(DynamicImage::ImageI16(image)),
                Mode::ImageI16L => Ok(DynamicImage::ImageI16L(image)),
                Mode::ImageI16B => Ok(DynamicImage::ImageI16B(image)),
                Mode::ImageI16N => Ok(DynamicImage::ImageI16N(image)),
                _ => Err(invalid_shape("I;16")),
            }
        }
        Mode::ImageLumaA16 => ImageBuffer::<LumaA<u16>, Vec<u16>>::from_raw(width, height, samples)
            .map(DynamicImage::ImageLumaA16)
            .ok_or_else(|| invalid_shape("LA16")),
        Mode::ImageRgb16 => ImageBuffer::<Rgb<u16>, Vec<u16>>::from_raw(width, height, samples)
            .map(DynamicImage::ImageRgb16)
            .ok_or_else(|| invalid_shape("RGB16")),
        Mode::ImageRgba16 => ImageBuffer::<Rgba<u16>, Vec<u16>>::from_raw(width, height, samples)
            .map(DynamicImage::ImageRgba16)
            .ok_or_else(|| invalid_shape("RGBA16")),
        _ => Err(invalid_shape("u16")),
    }
}

pub(super) fn f32_image(
    width: u32,
    height: u32,
    mode: Mode,
    bytes: Vec<u8>,
    channels: u8,
) -> Result<DynamicImage, PilError> {
    let dimensions = CheckedDims::new_allow_empty(width, height, channels * 4)?;
    if bytes.len() != dimensions.total_bytes() || bytes.len() % 4 != 0 {
        return Err(invalid_shape("f32"));
    }
    let samples = bytes
        .chunks_exact(4)
        .map(|sample| f32::from_ne_bytes([sample[0], sample[1], sample[2], sample[3]]))
        .collect::<Vec<_>>();
    match mode {
        Mode::ImageRgb32F => Rgb32FImage::from_raw(width, height, samples)
            .map(DynamicImage::ImageRgb32F)
            .ok_or_else(|| invalid_shape("RGB32F")),
        Mode::ImageRgba32F => Rgba32FImage::from_raw(width, height, samples)
            .map(DynamicImage::ImageRgba32F)
            .ok_or_else(|| invalid_shape("RGBA32F")),
        _ => Err(invalid_shape("f32")),
    }
}

fn invalid_shape(mode: &'static str) -> PilError {
    PilError::InternalError(format!("Composite output shape mismatch for {mode}"))
}
