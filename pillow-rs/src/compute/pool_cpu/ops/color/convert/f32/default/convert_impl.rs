//! Direct conversions from little-endian 32-bit float F samples.

use crate::error::PilError;
use crate::raster::{
    DynamicImage, GenericImageView, GrayAlphaImage, LumaA, Mode, Rgb, RgbImage, Rgba,
    RgbaImage,
};

pub(crate) fn execute(source: &RgbaImage, target: Mode) -> Result<DynamicImage, PilError> {
    match target {
        Mode::ImageLuma8 => Ok(DynamicImage::ImageLuma8(
            crate::compute::pool_cpu::ops::grayscale::f32::default::grayscale_impl::execute(source),
        )),
        Mode::ImageLumaA8 => Ok(DynamicImage::ImageLumaA8(luma_alpha(source))),
        Mode::ImageRgb8 => Ok(DynamicImage::ImageRgb8(rgb(source))),
        Mode::ImageRgba8 => Ok(DynamicImage::ImageRgba8(rgba(source))),
        Mode::ImageCmyk8 => Ok(DynamicImage::ImageCmyk8(cmyk(source))),
        Mode::ImageYCbCr8 => Ok(DynamicImage::ImageYCbCr8(ycbcr(source))),
        Mode::ImageHsv8 => Ok(DynamicImage::ImageHsv8(hsv(source))),
        Mode::ImageI32 => Ok(DynamicImage::ImageI32(to_i32(source))),
        Mode::ImageF32 => Ok(DynamicImage::ImageF32(source.clone())),
        _ => Err(PilError::ValueError("image has wrong mode".into())),
    }
}

fn luma_alpha(source: &RgbaImage) -> GrayAlphaImage {
    let mut output = GrayAlphaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        *destination = LumaA([sample(pixel.0), u8::MAX]);
    }
    output
}

fn rgb(source: &RgbaImage) -> RgbImage {
    let mut output = RgbImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let value = sample(pixel.0);
        *destination = Rgb([value, value, value]);
    }
    output
}

fn rgba(source: &RgbaImage) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let value = sample(pixel.0);
        *destination = Rgba([value, value, value, u8::MAX]);
    }
    output
}

fn cmyk(source: &RgbaImage) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        *destination = Rgba([0, 0, 0, u8::MAX.wrapping_sub(sample(pixel.0))]);
    }
    output
}

fn ycbcr(source: &RgbaImage) -> RgbImage {
    let mut output = RgbImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        *destination = Rgb([sample(pixel.0), 128, 128]);
    }
    output
}

fn hsv(source: &RgbaImage) -> RgbImage {
    let mut output = RgbImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        *destination = Rgb([0, 0, sample(pixel.0)]);
    }
    output
}

fn to_i32(source: &RgbaImage) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let value = f32::from_le_bytes(pixel.0) as i32;
        *destination = Rgba(value.to_le_bytes());
    }
    output
}

#[inline]
fn sample(bytes: [u8; 4]) -> u8 {
    f32::from_le_bytes(bytes).clamp(0.0, 255.0) as u8
}
