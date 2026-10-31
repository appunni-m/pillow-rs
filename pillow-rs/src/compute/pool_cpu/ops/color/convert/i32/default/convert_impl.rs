//! Direct conversions from little-endian signed I samples.

use crate::error::PilError;
use crate::raster::{
    DynamicImage, GenericImageView, GrayAlphaImage, LumaA, Mode, Rgb, RgbImage, Rgba,
    RgbaImage,
};

pub(crate) fn execute(source: &RgbaImage, target: Mode) -> Result<DynamicImage, PilError> {
    match target {
        Mode::ImageLuma8 => Ok(DynamicImage::ImageLuma8(
            crate::compute::pool_cpu::ops::grayscale::i32::default::grayscale_impl::execute(source),
        )),
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            convert_luma16(source, target)
        }
        Mode::ImageLumaA8 => Ok(DynamicImage::ImageLumaA8(luma_alpha(source))),
        Mode::ImageRgb8 => Ok(DynamicImage::ImageRgb8(rgb(source))),
        Mode::ImageRgba8 => Ok(DynamicImage::ImageRgba8(rgba(source))),
        // RGBX uses the existing scalar→RGB values and an opaque padding byte.
        Mode::ImageRgbx8 => Ok(DynamicImage::ImageRgbx8(rgba(source))),
        Mode::ImageCmyk8 => Ok(DynamicImage::ImageCmyk8(cmyk(source))),
        Mode::ImageYCbCr8 => Ok(DynamicImage::ImageYCbCr8(ycbcr(source))),
        Mode::ImageHsv8 => Ok(DynamicImage::ImageHsv8(hsv(source))),
        Mode::ImageI32 => Ok(DynamicImage::ImageI32(source.clone())),
        Mode::ImageF32 => Ok(DynamicImage::ImageF32(to_f32(source))),
        _ => Err(PilError::ValueError("image has wrong mode".into())),
    }
}

fn convert_luma16(source: &RgbaImage, target: Mode) -> Result<DynamicImage, PilError> {
    if target == Mode::ImageI16N {
        return super::super::super::helpers::luma16::from_luma8_samples(
            source.width(),
            source.height(),
            target,
            source.pixels().map(|pixel| sample(pixel.0)),
        );
    }
    super::super::super::helpers::luma16::from_u16_samples(
        source.width(),
        source.height(),
        target,
        source.pixels().map(|pixel| {
            i32::from_le_bytes(pixel.0).clamp(0, i32::from(u16::MAX)) as u16
        }),
    )
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

fn to_f32(source: &RgbaImage) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let value = i32::from_le_bytes(pixel.0) as f32;
        *destination = Rgba(value.to_le_bytes());
    }
    output
}

// NO_PILLOW_SUPPORT: Pillow's palette converter rejects scalar `I` input.
pub(crate) fn execute_palette_target(
    source: &RgbaImage,
    target: Mode,
) -> Result<super::super::super::ops::PreparedConvertOutput, PilError> {
    let indices = match target {
        Mode::ImagePalette8 => crate::compute::pool_cpu::ops::grayscale::i32::default::grayscale_impl::execute(source),
        Mode::ImagePaletteAlpha8 => {
            let mut output = GrayAlphaImage::new(source.width(), source.height());
            for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
                *destination = LumaA([sample(pixel.0), u8::MAX]);
            }
            return Ok(super::super::super::ops::PreparedConvertOutput {
                image: DynamicImage::ImagePaletteAlpha8(output),
                palette: Some(crate::ops::quantize::identity_grayscale_palette()),
                palette_alpha: None,
                wrapper_metadata: Some(super::super::super::ops::ConvertWrapperMetadata::ExifOnly),
            });
        }
        _ => return Err(PilError::ValueError("image has wrong mode".into())),
    };
    Ok(super::super::super::ops::PreparedConvertOutput {
        image: DynamicImage::ImagePalette8(indices),
        palette: Some(crate::ops::quantize::identity_grayscale_palette()),
        palette_alpha: None,
        wrapper_metadata: Some(super::super::super::ops::ConvertWrapperMetadata::Drop),
    })
}

#[inline]
fn sample(bytes: [u8; 4]) -> u8 {
    i32::from_le_bytes(bytes).clamp(0, i32::from(u8::MAX)) as u8
}
