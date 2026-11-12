//! Direct conversions from premultiplied RGBa samples.

use crate::error::PilError;
use crate::pipeline::DitherMethod;
use crate::raster::{DynamicImage, GrayAlphaImage, LumaA, Mode, Rgb, RgbImage, Rgba, RgbaImage};

pub(crate) fn execute(source: &RgbaImage, target: Mode) -> Result<DynamicImage, PilError> {
    if target == Mode::ImageLab8 {
        return crate::compute::pool_cpu::ops::color::helpers::convert::lab_target::execute(
            source.width(),
            source.height(),
            source.pixels().map(straight_rgb),
        );
    }
    let (width, height) = source.dimensions();
    match target {
        Mode::ImageLuma8 => {
            let kernel = crate::compute::pool_cpu::ops::grayscale::rgb_premultiplied8::default::grayscale_impl::prepare(source)?;
            crate::compute::pool_cpu::ops::grayscale::rgb_premultiplied8::default::grayscale_impl::execute(kernel)
                .map(DynamicImage::ImageLuma8)
        }
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            crate::compute::pool_cpu::ops::color::helpers::convert::luma16::from_luma8_samples(
                width,
                height,
                target,
                source.pixels().map(|pixel| {
                    let red = crate::compute::pool_cpu::ops::color::helpers::convert::pixel::unpremultiply_byte(
                        pixel[0], pixel[3],
                    );
                    let green = crate::compute::pool_cpu::ops::color::helpers::convert::pixel::unpremultiply_byte(
                        pixel[1], pixel[3],
                    );
                    let blue = crate::compute::pool_cpu::ops::color::helpers::convert::pixel::unpremultiply_byte(
                        pixel[2], pixel[3],
                    );
                    crate::color::rgb_to_luma_u8(red, green, blue)
                }),
            )
        }
        Mode::ImageLumaA8 => {
            let mut output = GrayAlphaImage::new(width, height);
            for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
                let [red, green, blue] = straight_rgb(pixel);
                let luma = crate::color::rgb_to_luma_u8(red, green, blue);
                *destination = LumaA([luma, u8::MAX]);
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
        // Pillow unpremultiplies color and emits opaque RGB padding when alpha is dropped.
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
            crate::color::rgb_pixels_to_ycbcr(
                width,
                height,
                source.pixels().map(straight_rgb),
            ),
        )),
        Mode::ImageHsv8 => Ok(DynamicImage::ImageHsv8(
            crate::color::rgb_pixels_to_hsv(
                width,
                height,
                source.pixels().map(straight_rgb),
            ),
        )),
        Mode::ImageI32 => Ok(DynamicImage::ImageI32(scalar_i32(source))),
        Mode::ImageF32 => Ok(DynamicImage::ImageF32(scalar_f32(source))),
        _ => Err(PilError::ValueError("image has wrong mode".into())),
    }
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
    // Pillow's palette strategy consumes RGBa's stored color lanes directly;
    // PA output receives opaque per-pixel alpha.
    crate::compute::pool_cpu::ops::color::helpers::convert::palette::web_palette_from_pixels(
        source.width(),
        source.height(),
        source.pixels().map(|pixel| [pixel[0], pixel[1], pixel[2]]),
        !matches!(dither, Some(DitherMethod::None)),
        target,
        metadata,
    )
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
        let [red, green, blue] = straight_rgb(pixel);
        *destination = Rgba([
            u8::MAX.wrapping_sub(red),
            u8::MAX.wrapping_sub(green),
            u8::MAX.wrapping_sub(blue),
            0,
        ]);
    }
    output
}

fn scalar_i32(source: &RgbaImage) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let [red, green, blue] = straight_rgb(pixel);
        *destination = Rgba(
            crate::compute::pool_cpu::ops::color::helpers::convert::pixel::scalar_i32_rgb(
                red, green, blue,
            ),
        );
    }
    output
}

fn scalar_f32(source: &RgbaImage) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let [red, green, blue] = straight_rgb(pixel);
        *destination = Rgba(
            crate::compute::pool_cpu::ops::color::helpers::convert::pixel::scalar_f32_rgb(
                red, green, blue,
            ),
        );
    }
    output
}

#[inline]
fn unpremultiply(value: u8, alpha: u8) -> u8 {
    crate::compute::pool_cpu::ops::color::helpers::convert::pixel::unpremultiply_byte(value, alpha)
}

#[inline]
fn straight_rgb(pixel: &Rgba<u8>) -> [u8; 3] {
    [
        unpremultiply(pixel[0], pixel[3]),
        unpremultiply(pixel[1], pixel[3]),
        unpremultiply(pixel[2], pixel[3]),
    ]
}
