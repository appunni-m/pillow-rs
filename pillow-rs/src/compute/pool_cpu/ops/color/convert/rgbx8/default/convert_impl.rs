//! Direct conversions from RGBX samples; the fourth byte is never alpha.

use crate::error::PilError;
use crate::pipeline::DitherMethod;
use crate::raster::{
    DynamicImage, GrayAlphaImage, LumaA, Mode, Rgb, RgbImage, Rgba,
    RgbaImage,
};

pub(crate) fn execute(source: &RgbaImage, target: Mode) -> Result<DynamicImage, PilError> {
    if target == Mode::ImageLab8 {
        return super::super::super::helpers::lab_target::execute(
            source.width(),
            source.height(),
            source.pixels().map(|pixel| [pixel[0], pixel[1], pixel[2]]),
        );
    }
    let (width, height) = source.dimensions();
    match target {
        Mode::ImageLuma8 => {
            let kernel = crate::compute::pool_cpu::ops::grayscale::rgbx8::default::grayscale_impl::prepare(source)?;
            crate::compute::pool_cpu::ops::grayscale::rgbx8::default::grayscale_impl::execute(kernel)
                .map(DynamicImage::ImageLuma8)
        }
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            super::super::super::helpers::luma16::from_luma8_samples(
                width,
                height,
                target,
                source.pixels().map(|pixel| {
                    crate::color::rgb_to_luma_u8(pixel[0], pixel[1], pixel[2])
                }),
            )
        }
        Mode::ImageLumaA8 => {
            Ok(DynamicImage::ImageLumaA8(opaque_luma_alpha(source)))
        }
        // Compute RGB-derived luma and opaque alpha directly from RGBX lanes.
        Mode::ImageLaPremultiplied8 => {
            Ok(DynamicImage::ImageLaPremultiplied8(opaque_luma_alpha(source)))
        }
        // Pillow discards RGBX's X byte and emits opaque RGB padding.
        Mode::ImageRgb8 => {
            let mut output = RgbImage::new(width, height);
            for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
                *destination = Rgb([pixel[0], pixel[1], pixel[2]]);
            }
            Ok(DynamicImage::ImageRgb8(output))
        }
        Mode::ImageRgba8 => {
            Ok(DynamicImage::ImageRgba8(opaque_rgba(source)))
        }
        // An opaque RGB sample needs no premultiplication arithmetic; discard
        // X and write the final carrier directly from native RGBX lanes.
        Mode::ImageRgbPremultiplied8 => {
            Ok(DynamicImage::ImageRgbPremultiplied8(opaque_rgba(source)))
        }
        Mode::ImageCmyk8 => {
            let mut output = RgbaImage::new(width, height);
            for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
                *destination = Rgba([
                    u8::MAX.wrapping_sub(pixel[0]),
                    u8::MAX.wrapping_sub(pixel[1]),
                    u8::MAX.wrapping_sub(pixel[2]),
                    0,
                ]);
            }
            Ok(DynamicImage::ImageCmyk8(output))
        }
        Mode::ImageYCbCr8 => Ok(DynamicImage::ImageYCbCr8(
            crate::color::rgb_bytes_to_ycbcr::<4>(source.as_raw(), width, height),
        )),
        Mode::ImageHsv8 => Ok(DynamicImage::ImageHsv8(
            crate::color::rgb_bytes_to_hsv::<4>(source.as_raw(), width, height),
        )),
        Mode::ImageI32 => Ok(DynamicImage::ImageI32(scalar_i32(source))),
        Mode::ImageF32 => Ok(DynamicImage::ImageF32(scalar_f32(source))),
        _ => Err(PilError::ValueError("image has wrong mode".into())),
    }
}

fn opaque_luma_alpha(source: &RgbaImage) -> GrayAlphaImage {
    let (width, height) = source.dimensions();
    let mut output = GrayAlphaImage::new(width, height);
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let luma = crate::color::rgb_to_luma_u8(pixel[0], pixel[1], pixel[2]);
        *destination = LumaA([luma, u8::MAX]);
    }
    output
}

fn opaque_rgba(source: &RgbaImage) -> RgbaImage {
    let (width, height) = source.dimensions();
    let mut output = RgbaImage::new(width, height);
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        *destination = Rgba([pixel[0], pixel[1], pixel[2], u8::MAX]);
    }
    output
}

pub(crate) fn execute_palette_target(
    source: &RgbaImage,
    target: Mode,
    dither: Option<DitherMethod>,
) -> Result<super::super::super::ops::PreparedConvertOutput, PilError> {
    if !matches!(target, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8) {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    let metadata = if target == Mode::ImagePaletteAlpha8 {
        super::super::super::ops::ConvertWrapperMetadata::ExifOnly
    } else {
        super::super::super::ops::ConvertWrapperMetadata::PreserveSource
    };
    crate::compute::pool_cpu::ops::color::convert::helpers::palette::web_palette_from_pixels(
        source.width(),
        source.height(),
        source
            .pixels()
            .map(|pixel| [pixel[0], pixel[1], pixel[2]]),
        !matches!(dither, Some(DitherMethod::None)),
        target,
        metadata,
    )
}

fn scalar_i32(source: &RgbaImage) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        *destination = Rgba(
            crate::compute::pool_cpu::ops::color::convert::helpers::pixel::scalar_i32_rgb(
                pixel[0], pixel[1], pixel[2],
            ),
        );
    }
    output
}

fn scalar_f32(source: &RgbaImage) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        *destination = Rgba(
            crate::compute::pool_cpu::ops::color::convert::helpers::pixel::scalar_f32_rgb(
                pixel[0], pixel[1], pixel[2],
            ),
        );
    }
    output
}
