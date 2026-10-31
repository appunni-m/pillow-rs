//! Direct destination kernels from native palette indices and pixel alpha.

use crate::compute::registry::support::PaletteInput;
use crate::error::PilError;
use crate::raster::{
    DynamicImage, GenericImageView, GrayAlphaImage, GrayImage, Luma, LumaA, Mode, Rgb,
    RgbImage, Rgba, RgbaImage,
};

pub(crate) fn execute(
    source: &GrayAlphaImage,
    palette: PaletteInput<'_>,
    target: Mode,
) -> Result<DynamicImage, PilError> {
    match target {
        Mode::ImageLuma8 => Ok(DynamicImage::ImageLuma8(luma(source, palette))),
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            super::super::super::helpers::luma16::from_luma8_samples(
                source.width(),
                source.height(),
                target,
                source.pixels().map(|pixel| {
                    let [red, green, blue] =
                        super::super::super::helpers::palette::color(pixel[0], palette);
                    crate::color::rgb_to_luma_u8(red, green, blue)
                }),
            )
        }
        Mode::ImageLumaA8 => Ok(DynamicImage::ImageLumaA8(luma_alpha(source, palette))),
        Mode::ImageRgb8 => Ok(DynamicImage::ImageRgb8(rgb(source, palette))),
        Mode::ImageRgba8 => Ok(DynamicImage::ImageRgba8(rgba(source, palette))),
        Mode::ImageCmyk8 => Ok(DynamicImage::ImageCmyk8(cmyk(source, palette))),
        Mode::ImageYCbCr8 => Ok(DynamicImage::ImageYCbCr8(ycbcr(source, palette))),
        Mode::ImageHsv8 => Ok(DynamicImage::ImageHsv8(hsv(source, palette))),
        Mode::ImageI32 => Ok(DynamicImage::ImageI32(to_i32(source, palette))),
        Mode::ImageF32 => Ok(DynamicImage::ImageF32(to_f32(source, palette))),
        _ => Err(PilError::ValueError("image has wrong mode".into())),
    }
}

pub(crate) fn execute_palette_target(
    source: &GrayAlphaImage,
    target: Mode,
) -> Result<super::super::super::ops::PreparedConvertOutput, PilError> {
    if target != Mode::ImagePalette8 {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    let mut output = GrayImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        destination[0] = pixel[0];
    }
    Ok(super::super::super::ops::PreparedConvertOutput {
        image: DynamicImage::ImagePalette8(output),
        palette: None,
        palette_alpha: None,
        wrapper_metadata: Some(
            super::super::super::ops::ConvertWrapperMetadata::PreserveSourceLoaded,
        ),
    })
}

fn luma(source: &GrayAlphaImage, palette: PaletteInput<'_>) -> GrayImage {
    let mut output = GrayImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let [red, green, blue] = super::super::super::helpers::palette::color(pixel[0], palette);
        *destination = Luma([crate::color::rgb_to_luma_u8(red, green, blue)]);
    }
    output
}

fn luma_alpha(source: &GrayAlphaImage, palette: PaletteInput<'_>) -> GrayAlphaImage {
    let mut output = GrayAlphaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let [red, green, blue] = super::super::super::helpers::palette::color(pixel[0], palette);
        let luma = crate::color::rgb_to_luma_u8(red, green, blue);
        *destination = LumaA([luma, pixel[1]]);
    }
    output
}

fn rgb(source: &GrayAlphaImage, palette: PaletteInput<'_>) -> RgbImage {
    let mut output = RgbImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        *destination = Rgb(super::super::super::helpers::palette::color(pixel[0], palette));
    }
    output
}

fn rgba(source: &GrayAlphaImage, palette: PaletteInput<'_>) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let color = super::super::super::helpers::palette::color(pixel[0], palette);
        *destination = Rgba([color[0], color[1], color[2], pixel[1]]);
    }
    output
}

fn cmyk(source: &GrayAlphaImage, palette: PaletteInput<'_>) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let [red, green, blue] = super::super::super::helpers::palette::color(pixel[0], palette);
        *destination = Rgba([u8::MAX - red, u8::MAX - green, u8::MAX - blue, 0]);
    }
    output
}

fn ycbcr(source: &GrayAlphaImage, palette: PaletteInput<'_>) -> RgbImage {
    crate::color::rgb_pixels_to_ycbcr(
        source.width(),
        source.height(),
        source
            .pixels()
            .map(|pixel| super::super::super::helpers::palette::color(pixel[0], palette)),
    )
}

fn hsv(source: &GrayAlphaImage, palette: PaletteInput<'_>) -> RgbImage {
    crate::color::rgb_pixels_to_hsv(
        source.width(),
        source.height(),
        source
            .pixels()
            .map(|pixel| super::super::super::helpers::palette::color(pixel[0], palette)),
    )
}

fn to_i32(source: &GrayAlphaImage, palette: PaletteInput<'_>) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let [red, green, blue] = super::super::super::helpers::palette::color(pixel[0], palette);
        *destination = Rgba(crate::compute::pool_cpu::ops::color::convert::helpers::pixel::scalar_i32_rgb(
            red, green, blue,
        ));
    }
    output
}

fn to_f32(source: &GrayAlphaImage, palette: PaletteInput<'_>) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let [red, green, blue] = super::super::super::helpers::palette::color(pixel[0], palette);
        *destination = Rgba(crate::compute::pool_cpu::ops::color::convert::helpers::pixel::scalar_f32_rgb(
            red, green, blue,
        ));
    }
    output
}
