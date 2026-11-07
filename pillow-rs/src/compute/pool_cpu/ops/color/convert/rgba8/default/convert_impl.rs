//! Direct conversions from straight RGBA samples.

use crate::error::PilError;
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
            let kernel = crate::compute::pool_cpu::ops::grayscale::rgba8::default::grayscale_impl::prepare(source)?;
            crate::compute::pool_cpu::ops::grayscale::rgba8::default::grayscale_impl::execute(kernel)
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
                let luma = crate::color::rgb_to_luma_u8(pixel[0], pixel[1], pixel[2]);
                // Pillow retries RGBA→La through RGB, which drops the source
                // alpha before RGB→La and therefore emits opaque alpha.
                *destination = LumaA([luma, u8::MAX]);
            }
            Ok(DynamicImage::ImageLaPremultiplied8(output))
        }
        Mode::ImageRgb8 => {
            let mut output = RgbImage::new(width, height);
            for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
                *destination = Rgb([pixel[0], pixel[1], pixel[2]]);
            }
            Ok(DynamicImage::ImageRgb8(output))
        }
        Mode::ImageRgba8 => Ok(DynamicImage::ImageRgba8(source.clone())),
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
        Mode::ImageRgbPremultiplied8 => {
            let mut output = RgbaImage::new(width, height);
            for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
                *destination = Rgba([
                    crate::compute::pool_cpu::ops::color::convert::helpers::pixel::premultiply_byte(
                        pixel[0], pixel[3],
                    ),
                    crate::compute::pool_cpu::ops::color::convert::helpers::pixel::premultiply_byte(
                        pixel[1], pixel[3],
                    ),
                    crate::compute::pool_cpu::ops::color::convert::helpers::pixel::premultiply_byte(
                        pixel[2], pixel[3],
                    ),
                    pixel[3],
                ]);
            }
            Ok(DynamicImage::ImageRgbPremultiplied8(output))
        }
        Mode::ImageRgbx8 => {
            let mut output = RgbaImage::new(width, height);
            for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
                *destination = Rgba([pixel[0], pixel[1], pixel[2], u8::MAX]);
            }
            Ok(DynamicImage::ImageRgbx8(output))
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

pub(crate) fn execute_palette_target(
    source: &RgbaImage,
    target: Mode,
) -> Result<super::super::super::ops::PreparedConvertOutput, PilError> {
    if !matches!(target, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8) {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    let width = source.width();
    let height = source.height();
    if target == Mode::ImagePalette8 {
        let (indices, palette, palette_alpha) =
            crate::compute::pool_cpu::helpers::quantization::quantize_rgba_samples(
                source.as_raw(), width, height, 256,
            )?;
        return crate::compute::pool_cpu::ops::color::convert::helpers::palette::palette_index_output(
            width,
            height,
            indices,
            palette,
            Some(palette_alpha),
            super::super::super::ops::ConvertWrapperMetadata::Drop,
        );
    }
    let pixel_count = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| PilError::ValueError("palette conversion failed".into()))?;
    let rgb_length = pixel_count
        .checked_mul(3)
        .ok_or_else(|| PilError::ValueError("palette conversion failed".into()))?;
    let rgba_length = pixel_count
        .checked_mul(4)
        .ok_or_else(|| PilError::ValueError("palette conversion failed".into()))?;
    let input = source
        .as_raw()
        .get(..rgba_length)
        .ok_or_else(|| PilError::ValueError("palette conversion failed".into()))?;
    // Pillow's RGBA-to-PA route median-cuts RGB samples and merges the
    // original alpha afterward; quantizing RGBA changes observable indices.
    let mut rgb_samples = Vec::with_capacity(rgb_length);
    for pixel in input.chunks_exact(4) {
        rgb_samples.extend_from_slice(&pixel[..3]);
    }
    let (indices, palette) =
        crate::compute::pool_cpu::helpers::quantization::median_cut_quantize_rgb(
            &rgb_samples,
            256,
        );
    let palette: Vec<u8> = palette
        .into_iter()
        .flat_map(|color| color.into_iter())
        .collect();
    crate::compute::pool_cpu::ops::color::convert::helpers::palette::palette_alpha_output(
        width,
        height,
        indices,
        palette,
        |position| input[position * 4 + 3],
        super::super::super::ops::ConvertWrapperMetadata::ExifOnly,
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
