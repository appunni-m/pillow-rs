//! Direct destination kernels from native Y, Cb, Cr samples.

use crate::error::PilError;
use crate::pipeline::DitherMethod;
use crate::raster::{
    DynamicImage, GenericImageView, GrayAlphaImage, GrayImage, Luma, LumaA, Mode, RgbImage,
    Rgba, RgbaImage,
};

pub(crate) fn execute(source: &RgbImage, target: Mode) -> Result<DynamicImage, PilError> {
    match target {
        Mode::ImageLuma8 => Ok(DynamicImage::ImageLuma8(luma(source))),
        Mode::ImageLaPremultiplied8 | Mode::ImageRgbPremultiplied8 => {
            super::super::super::helpers::premultiplied::from_rgb_samples(
                source.width(),
                source.height(),
                source.pixels().map(|pixel| {
                    crate::color::ycbcr_pixel_to_rgb(pixel[0], pixel[1], pixel[2])
                }),
                target,
            )
        }
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            super::super::super::helpers::luma16::from_luma8_samples(
                source.width(),
                source.height(),
                target,
                source.pixels().map(|pixel| {
                    let [red, green, blue] =
                        crate::color::ycbcr_pixel_to_rgb(pixel[0], pixel[1], pixel[2]);
                    crate::color::rgb_to_luma_u8(red, green, blue)
                }),
            )
        }
        Mode::ImageLumaA8 => Ok(DynamicImage::ImageLumaA8(luma_alpha(source))),
        Mode::ImageRgb8 => Ok(DynamicImage::ImageRgb8(rgb(source))),
        Mode::ImageRgbx8 => Ok(DynamicImage::ImageRgbx8(rgbx(source))),
        Mode::ImageRgba8 => Ok(DynamicImage::ImageRgba8(rgba(source))),
        Mode::ImageCmyk8 => Ok(DynamicImage::ImageCmyk8(cmyk(source))),
        Mode::ImageYCbCr8 => Ok(DynamicImage::ImageYCbCr8(source.clone())),
        Mode::ImageHsv8 => Ok(DynamicImage::ImageHsv8(hsv(source))),
        Mode::ImageI32 => Ok(DynamicImage::ImageI32(to_i32(source))),
        Mode::ImageF32 => Ok(DynamicImage::ImageF32(to_f32(source))),
        _ => Err(PilError::ValueError("image has wrong mode".into())),
    }
}

// YCbCr palette output uses Pillow's RGB-derived colors; map native YCbCr
// samples directly into the requested palette representation.
pub(crate) fn execute_palette_target(
    source: &RgbImage,
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
        source.pixels().map(|pixel| {
            crate::color::ycbcr_pixel_to_rgb(pixel[0], pixel[1], pixel[2])
        }),
        !matches!(dither, Some(DitherMethod::None)),
        target,
        metadata,
    )
}

fn luma(source: &RgbImage) -> GrayImage {
    let mut output = GrayImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        *destination = Luma([pixel[0]]);
    }
    output
}

fn luma_alpha(source: &RgbImage) -> GrayAlphaImage {
    let mut output = GrayAlphaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        *destination = LumaA([pixel[0], u8::MAX]);
    }
    output
}

fn rgb(source: &RgbImage) -> RgbImage {
    crate::color::ycbcr_buffer_to_rgb(source)
}

fn rgba(source: &RgbImage) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let [red, green, blue] = crate::color::ycbcr_pixel_to_rgb(pixel[0], pixel[1], pixel[2]);
        *destination = Rgba([red, green, blue, u8::MAX]);
    }
    output
}

fn rgbx(source: &RgbImage) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let [red, green, blue] = crate::color::ycbcr_pixel_to_rgb(pixel[0], pixel[1], pixel[2]);
        *destination = Rgba([red, green, blue, u8::MAX]);
    }
    output
}

fn cmyk(source: &RgbImage) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let [red, green, blue] = crate::color::ycbcr_pixel_to_rgb(pixel[0], pixel[1], pixel[2]);
        *destination = Rgba([
            u8::MAX - red,
            u8::MAX - green,
            u8::MAX - blue,
            0,
        ]);
    }
    output
}

fn hsv(source: &RgbImage) -> RgbImage {
    crate::color::rgb_pixels_to_hsv(
        source.width(),
        source.height(),
        source.pixels().map(|pixel| {
            crate::color::ycbcr_pixel_to_rgb(pixel[0], pixel[1], pixel[2])
        }),
    )
}

fn to_i32(source: &RgbImage) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let [red, green, blue] = crate::color::ycbcr_pixel_to_rgb(pixel[0], pixel[1], pixel[2]);
        *destination = Rgba(crate::compute::pool_cpu::ops::color::convert::helpers::pixel::scalar_i32_rgb(
            red, green, blue,
        ));
    }
    output
}

fn to_f32(source: &RgbImage) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let [red, green, blue] = crate::color::ycbcr_pixel_to_rgb(pixel[0], pixel[1], pixel[2]);
        *destination = Rgba(crate::compute::pool_cpu::ops::color::convert::helpers::pixel::scalar_f32_rgb(
            red, green, blue,
        ));
    }
    output
}
