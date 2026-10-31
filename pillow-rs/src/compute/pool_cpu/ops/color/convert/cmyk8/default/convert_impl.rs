//! Direct destination kernels from native CMYK samples.

use crate::error::PilError;
use crate::pipeline::DitherMethod;
use crate::raster::{
    DynamicImage, GenericImageView, GrayAlphaImage, GrayImage, LumaA, Mode, Rgb, RgbImage,
    Rgba, RgbaImage,
};

pub(crate) fn execute(source: &RgbaImage, target: Mode) -> Result<DynamicImage, PilError> {
    match target {
        Mode::ImageLuma8 => Ok(DynamicImage::ImageLuma8(grayscale(source))),
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            super::super::super::helpers::luma16::from_luma8_samples(
                source.width(),
                source.height(),
                target,
                source.pixels().map(|pixel| {
                    let [red, green, blue] = crate::color::cmyk_pixel_to_rgb(pixel.0);
                    crate::color::rgb_to_luma_u8(red, green, blue)
                }),
            )
        }
        Mode::ImageLumaA8 => Ok(DynamicImage::ImageLumaA8(luma_alpha(source))),
        Mode::ImageRgb8 => Ok(DynamicImage::ImageRgb8(rgb(source))),
        Mode::ImageRgba8 => Ok(DynamicImage::ImageRgba8(rgba(source))),
        Mode::ImageCmyk8 => Ok(DynamicImage::ImageCmyk8(source.clone())),
        Mode::ImageYCbCr8 => Ok(DynamicImage::ImageYCbCr8(ycbcr(source))),
        Mode::ImageHsv8 => Ok(DynamicImage::ImageHsv8(hsv(source))),
        Mode::ImageI32 => Ok(DynamicImage::ImageI32(to_i32(source))),
        Mode::ImageF32 => Ok(DynamicImage::ImageF32(to_f32(source))),
        _ => Err(PilError::ValueError("image has wrong mode".into())),
    }
}

// NO_PILLOW_SUPPORT: Pillow's palette converter rejects native `CMYK` input.
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
        super::super::super::ops::ConvertWrapperMetadata::Drop
    };
    crate::compute::pool_cpu::ops::color::convert::helpers::palette::web_palette_from_pixels(
        source.width(),
        source.height(),
        source
            .pixels()
            .map(|pixel| crate::color::cmyk_pixel_to_rgb(pixel.0)),
        !matches!(dither, Some(DitherMethod::None)),
        target,
        metadata,
    )
}

fn grayscale(source: &RgbaImage) -> GrayImage {
    let mut output = GrayImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let [red, green, blue] = crate::color::cmyk_pixel_to_rgb(pixel.0);
        *destination = crate::raster::Luma([crate::color::rgb_to_luma_u8(red, green, blue)]);
    }
    output
}

fn luma_alpha(source: &RgbaImage) -> GrayAlphaImage {
    let mut output = GrayAlphaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let [red, green, blue] = crate::color::cmyk_pixel_to_rgb(pixel.0);
        *destination = LumaA([crate::color::rgb_to_luma_u8(red, green, blue), u8::MAX]);
    }
    output
}

fn rgb(source: &RgbaImage) -> RgbImage {
    let mut output = RgbImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        *destination = Rgb(crate::color::cmyk_pixel_to_rgb(pixel.0));
    }
    output
}

fn rgba(source: &RgbaImage) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let [red, green, blue] = crate::color::cmyk_pixel_to_rgb(pixel.0);
        *destination = Rgba([red, green, blue, u8::MAX]);
    }
    output
}

fn ycbcr(source: &RgbaImage) -> RgbImage {
    crate::color::rgb_pixels_to_ycbcr(
        source.width(),
        source.height(),
        source.pixels().map(|pixel| crate::color::cmyk_pixel_to_rgb(pixel.0)),
    )
}

fn hsv(source: &RgbaImage) -> RgbImage {
    crate::color::rgb_pixels_to_hsv(
        source.width(),
        source.height(),
        source.pixels().map(|pixel| crate::color::cmyk_pixel_to_rgb(pixel.0)),
    )
}

fn to_i32(source: &RgbaImage) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let [red, green, blue] = crate::color::cmyk_pixel_to_rgb(pixel.0);
        *destination = Rgba(crate::compute::pool_cpu::ops::color::convert::helpers::pixel::scalar_i32_rgb(
            red, green, blue,
        ));
    }
    output
}

fn to_f32(source: &RgbaImage) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let [red, green, blue] = crate::color::cmyk_pixel_to_rgb(pixel.0);
        *destination = Rgba(crate::compute::pool_cpu::ops::color::convert::helpers::pixel::scalar_f32_rgb(
            red, green, blue,
        ));
    }
    output
}
