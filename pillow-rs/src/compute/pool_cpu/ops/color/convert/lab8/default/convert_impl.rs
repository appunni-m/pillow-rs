//! Serial Convert paths for native three-byte LAB source samples.

use crate::error::PilError;
use crate::pipeline::DitherMethod;
use crate::raster::{
    DynamicImage, GenericImageView, GrayAlphaImage, LumaA, Mode, RgbImage, Rgba, RgbaImage,
};

pub(crate) fn execute(source: &RgbImage, target: Mode) -> Result<DynamicImage, PilError> {
    // TODO(pillow-rs#3ptb): Apply the Rust inverse LAB profile before treating
    // LAB samples as RGB for color-space, luma, or CMYK target strategies.
    match target {
        Mode::ImageLuma8 => luma(source).map(DynamicImage::ImageLuma8),
        Mode::ImageLumaA8 => luma_alpha(source).map(DynamicImage::ImageLumaA8),
        Mode::ImageLaPremultiplied8 | Mode::ImageRgbPremultiplied8 => {
            super::super::super::helpers::premultiplied::from_rgb_samples(
                source.width(),
                source.height(),
                source.pixels().map(|pixel| pixel.0),
                target,
            )
        }
        Mode::ImageRgb8 => Ok(DynamicImage::ImageRgb8(source.clone())),
        Mode::ImageRgba8 => Ok(DynamicImage::ImageRgba8(RgbaImage::from_fn(
            source.width(),
            source.height(),
            |x, y| {
                let pixel = source.get_pixel(x, y);
                Rgba([pixel[0], pixel[1], pixel[2], u8::MAX])
            },
        ))),
        Mode::ImageLab8 => Ok(DynamicImage::ImageLab8(source.clone())),
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            super::super::super::helpers::luma16::from_luma8_samples(
                source.width(),
                source.height(),
                target,
                luma(source)?.pixels().map(|pixel| pixel[0]),
            )
        }
        Mode::ImageCmyk8 => Ok(DynamicImage::ImageCmyk8(
            crate::color::rgb_to_cmyk_inverse(source),
        )),
        Mode::ImageYCbCr8 => Ok(DynamicImage::ImageYCbCr8(
            crate::color::rgb_buffer_to_ycbcr(source),
        )),
        Mode::ImageHsv8 => Ok(DynamicImage::ImageHsv8(
            crate::color::rgb_buffer_to_hsv(source),
        )),
        Mode::ImageI32 => Ok(crate::compute::pool_cpu::ops::color::convert::helpers::pixel::
            scalar_i32_rgb8(source)),
        Mode::ImageF32 => Ok(crate::compute::pool_cpu::ops::color::convert::helpers::pixel::
            scalar_f32_rgb8(source)),
        _ => Err(PilError::ValueError("image has wrong mode".into())),
    }
}

fn luma(source: &RgbImage) -> Result<crate::raster::GrayImage, PilError> {
    let kernel =
        crate::compute::pool_cpu::ops::grayscale::lab8::default::grayscale_impl::prepare(source)?;
    crate::compute::pool_cpu::ops::grayscale::lab8::default::grayscale_impl::execute(kernel)
}

fn luma_alpha(source: &RgbImage) -> Result<GrayAlphaImage, PilError> {
    let luma = luma(source)?;
    let mut output = GrayAlphaImage::new(source.width(), source.height());
    for (target, pixel) in output.pixels_mut().zip(luma.pixels()) {
        *target = LumaA([pixel[0], u8::MAX]);
    }
    Ok(output)
}

// NO_PILLOW_SUPPORT: Pillow's palette converter rejects native `LAB` input.
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
        super::super::super::ops::ConvertWrapperMetadata::PreserveSource
    };
    crate::compute::pool_cpu::ops::color::convert::helpers::palette::web_palette_from_pixels(
        source.width(),
        source.height(),
        source.pixels().map(|pixel| pixel.0),
        !matches!(dither, Some(DitherMethod::None)),
        target,
        metadata,
    )
}
