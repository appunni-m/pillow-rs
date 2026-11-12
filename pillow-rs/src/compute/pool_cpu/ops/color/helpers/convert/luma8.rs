//! Direct byte-luma source conversions shared by One and L mode strategies.

use crate::error::PilError;
use crate::raster::{
    DynamicImage, GrayAlphaImage, GrayImage, LumaA, Mode, Rgb, RgbImage, Rgba, RgbaImage,
};

pub(crate) fn execute<I>(
    width: u32,
    height: u32,
    target: Mode,
    samples: I,
) -> Result<DynamicImage, PilError>
where
    I: Iterator<Item = u8>,
{
    match target {
        Mode::ImageLuma8 => Ok(DynamicImage::ImageLuma8(to_luma8(width, height, samples))),
        Mode::ImageLumaA8 => Ok(DynamicImage::ImageLumaA8(to_luma_a8(
            width, height, samples,
        ))),
        Mode::ImageRgb8 => Ok(DynamicImage::ImageRgb8(to_rgb8(width, height, samples))),
        Mode::ImageRgba8 => Ok(DynamicImage::ImageRgba8(to_rgba8(width, height, samples))),
        Mode::ImageRgbx8 => Ok(DynamicImage::ImageRgbx8(to_rgbx8(width, height, samples))),
        Mode::ImageCmyk8 => Ok(DynamicImage::ImageCmyk8(to_cmyk8(width, height, samples))),
        Mode::ImageYCbCr8 => Ok(DynamicImage::ImageYCbCr8(to_ycbcr8(width, height, samples))),
        Mode::ImageHsv8 => Ok(DynamicImage::ImageHsv8(to_hsv8(width, height, samples))),
        Mode::ImageI32 => Ok(DynamicImage::ImageI32(to_i32(width, height, samples))),
        Mode::ImageF32 => Ok(DynamicImage::ImageF32(to_f32(width, height, samples))),
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            super::luma16::from_luma8_samples(width, height, target, samples)
        }
        _ => Err(PilError::ValueError("image has wrong mode".into())),
    }
}

pub(crate) fn execute_palette_target<I>(
    width: u32,
    height: u32,
    target: Mode,
    samples: I,
) -> Result<super::super::super::ops::PreparedConvertOutput, PilError>
where
    I: Iterator<Item = u8>,
{
    let identity_palette =
        crate::compute::pool_cpu::helpers::quantization::identity_grayscale_palette();
    match target {
        Mode::ImagePalette8 => {
            let indices = to_luma8(width, height, samples);
            Ok(super::super::super::ops::PreparedConvertOutput {
                image: DynamicImage::ImagePalette8(indices),
                palette: Some(identity_palette),
                palette_alpha: None,
                wrapper_metadata: Some(
                    super::super::super::ops::ConvertWrapperMetadata::PreserveSource,
                ),
            })
        }
        Mode::ImagePaletteAlpha8 => {
            let mut output = GrayAlphaImage::new(width, height);
            for (destination, sample) in output.pixels_mut().zip(samples) {
                *destination = LumaA([sample, u8::MAX]);
            }
            Ok(super::super::super::ops::PreparedConvertOutput {
                image: DynamicImage::ImagePaletteAlpha8(output),
                palette: Some(identity_palette),
                palette_alpha: None,
                wrapper_metadata: Some(super::super::super::ops::ConvertWrapperMetadata::ExifOnly),
            })
        }
        _ => Err(PilError::ValueError("image has wrong mode".into())),
    }
}

fn to_luma8<I: Iterator<Item = u8>>(width: u32, height: u32, samples: I) -> GrayImage {
    let mut output = GrayImage::new(width, height);
    for (pixel, sample) in output.pixels_mut().zip(samples) {
        pixel[0] = sample;
    }
    output
}

fn to_luma_a8<I: Iterator<Item = u8>>(width: u32, height: u32, samples: I) -> GrayAlphaImage {
    let mut output = GrayAlphaImage::new(width, height);
    for (pixel, sample) in output.pixels_mut().zip(samples) {
        *pixel = LumaA([sample, 255]);
    }
    if output.as_raw().capacity() != 0 {
        crate::compute::record_pipeline_allocation(output.as_raw().capacity());
    }
    output
}

fn to_rgb8<I: Iterator<Item = u8>>(width: u32, height: u32, samples: I) -> RgbImage {
    let mut output = RgbImage::new(width, height);
    for (pixel, sample) in output.pixels_mut().zip(samples) {
        *pixel = Rgb([sample, sample, sample]);
    }
    output
}

fn to_rgba8<I: Iterator<Item = u8>>(width: u32, height: u32, samples: I) -> RgbaImage {
    let mut output = RgbaImage::new(width, height);
    for (pixel, sample) in output.pixels_mut().zip(samples) {
        *pixel = Rgba([sample, sample, sample, 255]);
    }
    output
}

fn to_rgbx8<I: Iterator<Item = u8>>(width: u32, height: u32, samples: I) -> RgbaImage {
    let mut output = RgbaImage::new(width, height);
    for (pixel, sample) in output.pixels_mut().zip(samples) {
        *pixel = Rgba([sample, sample, sample, 255]);
    }
    output
}

fn to_cmyk8<I: Iterator<Item = u8>>(width: u32, height: u32, samples: I) -> RgbaImage {
    let mut output = RgbaImage::new(width, height);
    for (pixel, sample) in output.pixels_mut().zip(samples) {
        *pixel = Rgba([0, 0, 0, 255u8.wrapping_sub(sample)]);
    }
    output
}

fn to_ycbcr8<I: Iterator<Item = u8>>(width: u32, height: u32, samples: I) -> RgbImage {
    let mut output = RgbImage::new(width, height);
    for (pixel, sample) in output.pixels_mut().zip(samples) {
        *pixel = Rgb([sample, 128, 128]);
    }
    output
}

fn to_hsv8<I: Iterator<Item = u8>>(width: u32, height: u32, samples: I) -> RgbImage {
    let mut output = RgbImage::new(width, height);
    for (pixel, sample) in output.pixels_mut().zip(samples) {
        *pixel = Rgb([0, 0, sample]);
    }
    output
}

fn to_i32<I: Iterator<Item = u8>>(width: u32, height: u32, samples: I) -> RgbaImage {
    let mut output = RgbaImage::new(width, height);
    for (pixel, sample) in output.pixels_mut().zip(samples) {
        *pixel = Rgba(i32::from(sample).to_le_bytes());
    }
    output
}

fn to_f32<I: Iterator<Item = u8>>(width: u32, height: u32, samples: I) -> RgbaImage {
    let mut output = RgbaImage::new(width, height);
    for (pixel, sample) in output.pixels_mut().zip(samples) {
        *pixel = Rgba(f32::from(sample).to_le_bytes());
    }
    output
}
