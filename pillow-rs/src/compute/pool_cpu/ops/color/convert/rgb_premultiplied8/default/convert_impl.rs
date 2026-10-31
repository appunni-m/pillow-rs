//! Direct conversions from premultiplied RGBa samples.

use crate::error::PilError;
use crate::raster::{
    DynamicImage, GenericImageView, GrayAlphaImage, LumaA, Mode, Rgb, RgbImage, Rgba,
    RgbaImage,
};

pub(crate) fn execute(source: &RgbaImage, target: Mode) -> Result<DynamicImage, PilError> {
    let (width, height) = source.dimensions();
    match target {
        Mode::ImageLuma8 => {
            let kernel = crate::compute::pool_cpu::ops::grayscale::rgb_premultiplied8::default::grayscale_impl::prepare(source)?;
            crate::compute::pool_cpu::ops::grayscale::rgb_premultiplied8::default::grayscale_impl::execute(kernel)
                .map(DynamicImage::ImageLuma8)
        }
        Mode::ImageLumaA8 => {
            let mut output = GrayAlphaImage::new(width, height);
            for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
                let luma = crate::color::rgb_to_luma_u8(pixel[0], pixel[1], pixel[2]);
                *destination = LumaA([luma, pixel[3]]);
            }
            Ok(DynamicImage::ImageLumaA8(output))
        }
        Mode::ImageLaPremultiplied8 => {
            let mut output = GrayAlphaImage::new(width, height);
            for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
                let red = unpremultiply(pixel[0], pixel[3]);
                let green = unpremultiply(pixel[1], pixel[3]);
                let blue = unpremultiply(pixel[2], pixel[3]);
                *destination = LumaA([
                    crate::color::rgb_to_luma_u8(red, green, blue),
                    u8::MAX,
                ]);
            }
            Ok(DynamicImage::ImageLaPremultiplied8(output))
        }
        Mode::ImageRgb8 => {
            let mut output = RgbImage::new(width, height);
            for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
                *destination = Rgb([
                    unpremultiply(pixel[0], pixel[3]),
                    unpremultiply(pixel[1], pixel[3]),
                    unpremultiply(pixel[2], pixel[3]),
                ]);
            }
            Ok(DynamicImage::ImageRgb8(output))
        }
        Mode::ImageRgba8 => Ok(DynamicImage::ImageRgba8(unpremultiply_rgba(source, false))),
        Mode::ImageRgbx8 => Ok(DynamicImage::ImageRgbx8(unpremultiply_rgba(source, true))),
        Mode::ImageCmyk8 => Ok(DynamicImage::ImageCmyk8(rgb_to_cmyk(source))),
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

fn unpremultiply_rgba(source: &RgbaImage, opaque_alpha: bool) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        *destination = Rgba([
            unpremultiply(pixel[0], pixel[3]),
            unpremultiply(pixel[1], pixel[3]),
            unpremultiply(pixel[2], pixel[3]),
            if opaque_alpha { u8::MAX } else { pixel[3] },
        ]);
    }
    output
}

fn rgb_to_cmyk(source: &RgbaImage) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        *destination = Rgba([
            u8::MAX.wrapping_sub(pixel[0]),
            u8::MAX.wrapping_sub(pixel[1]),
            u8::MAX.wrapping_sub(pixel[2]),
            0,
        ]);
    }
    output
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

#[inline]
fn unpremultiply(value: u8, alpha: u8) -> u8 {
    crate::compute::pool_cpu::ops::color::convert::helpers::pixel::unpremultiply_byte(value, alpha)
}
