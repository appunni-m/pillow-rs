//! Direct conversions from native `LA` samples.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage, LumaA, Mode, Rgb, RgbImage, Rgba, RgbaImage};

pub(crate) fn execute(source: &GrayAlphaImage, target: Mode) -> Result<DynamicImage, PilError> {
    if target == Mode::ImageLab8 {
        return super::super::super::helpers::lab_target::execute(
            source.width(),
            source.height(),
            source.pixels().map(|pixel| [pixel[0]; 3]),
        );
    }
    let (width, height) = source.dimensions();
    match target {
        Mode::ImageLuma8 => {
            let kernel = crate::compute::pool_cpu::ops::grayscale::luma_a8::default::grayscale_impl::prepare(source)?;
            crate::compute::pool_cpu::ops::grayscale::luma_a8::default::grayscale_impl::execute(
                kernel,
            )
            .map(DynamicImage::ImageLuma8)
        }
        Mode::ImageLaPremultiplied8 => {
            let mut output = GrayAlphaImage::new(width, height);
            for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
                *destination = LumaA([
                    crate::compute::pool_cpu::ops::color::convert::helpers::pixel::premultiply_byte(
                        pixel[0], pixel[1],
                    ),
                    pixel[1],
                ]);
            }
            Ok(DynamicImage::ImageLaPremultiplied8(output))
        }
        Mode::ImageRgb8 => {
            let mut output = RgbImage::new(width, height);
            for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
                *destination = Rgb([pixel[0], pixel[0], pixel[0]]);
            }
            Ok(DynamicImage::ImageRgb8(output))
        }
        Mode::ImageRgba8 => {
            let mut output = RgbaImage::new(width, height);
            for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
                *destination = Rgba([pixel[0], pixel[0], pixel[0], pixel[1]]);
            }
            Ok(DynamicImage::ImageRgba8(output))
        }
        Mode::ImageRgbx8 => {
            let mut output = RgbaImage::new(width, height);
            for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
                *destination = Rgba([pixel[0], pixel[0], pixel[0], pixel[1]]);
            }
            Ok(DynamicImage::ImageRgbx8(output))
        }
        Mode::ImageCmyk8 => {
            let mut output = RgbaImage::new(width, height);
            for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
                *destination = Rgba([0, 0, 0, u8::MAX.wrapping_sub(pixel[0])]);
            }
            Ok(DynamicImage::ImageCmyk8(output))
        }
        // Pillow's LA-to-YCbCr conversion emits opaque padding, not source alpha.
        Mode::ImageYCbCr8 => {
            let mut output = RgbImage::new(width, height);
            for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
                *destination = Rgb([pixel[0], 128, 128]);
            }
            Ok(DynamicImage::ImageYCbCr8(output))
        }
        Mode::ImageHsv8 => {
            let mut output = RgbImage::new(width, height);
            for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
                *destination = Rgb([0, 0, pixel[0]]);
            }
            Ok(DynamicImage::ImageHsv8(output))
        }
        Mode::ImageI32 => {
            let mut output = RgbaImage::new(width, height);
            for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
                *destination = Rgba(i32::from(pixel[0]).to_le_bytes());
            }
            Ok(DynamicImage::ImageI32(output))
        }
        Mode::ImageF32 => {
            let mut output = RgbaImage::new(width, height);
            for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
                *destination = Rgba(f32::from(pixel[0]).to_le_bytes());
            }
            Ok(DynamicImage::ImageF32(output))
        }
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            super::super::super::helpers::luma16::from_luma8_samples(
                width,
                height,
                target,
                source.pixels().map(|pixel| pixel[0]),
            )
        }
        _ => Err(PilError::ValueError("image has wrong mode".into())),
    }
}

// The LA source/target matrix uses luma-only palette values; write the final
// palette output from the luma lane without constructing an intermediate L raster.
pub(crate) fn execute_palette_target(
    source: &GrayAlphaImage,
    target: Mode,
) -> Result<super::super::super::ops::PreparedConvertOutput, PilError> {
    super::super::super::helpers::luma8::execute_palette_target(
        source.width(),
        source.height(),
        target,
        source.pixels().map(|pixel| pixel[0]),
    )
}
