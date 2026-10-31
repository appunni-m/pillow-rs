//! Serial RGB8 source conversions over the native triplet samples.

use crate::error::PilError;
use crate::pipeline::DitherMethod;
use crate::raster::{
    DynamicImage, GenericImageView, GrayAlphaImage, LumaA, Mode, RgbImage, Rgba, RgbaImage,
};

pub(crate) fn execute(source: &RgbImage, target: Mode) -> Result<DynamicImage, PilError> {
    match target {
        Mode::ImageLuma8 => {
            let kernel = crate::compute::pool_cpu::ops::grayscale::rgb8::default::grayscale_impl::prepare(source)?;
            crate::compute::pool_cpu::ops::grayscale::rgb8::default::grayscale_impl::execute(kernel)
                .map(DynamicImage::ImageLuma8)
        }
        Mode::ImageLumaA8 => convert_luma_alpha(source, Mode::ImageLumaA8),
        // NO_PILLOW_SUPPORT: The archived Pillow Convert contract rejects RGB→La.
        // The core extension writes native L with opaque alpha directly.
        Mode::ImageLaPremultiplied8 => {
            convert_luma_alpha(source, Mode::ImageLaPremultiplied8)
        }
        Mode::ImageRgb8 => Ok(DynamicImage::ImageRgb8(source.clone())),
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            super::super::super::helpers::luma16::from_luma8_samples(
                source.width(),
                source.height(),
                target,
                source.pixels().map(|pixel| {
                    crate::color::rgb_to_luma_u8(pixel[0], pixel[1], pixel[2])
                }),
            )
        }
        Mode::ImageRgba8 => Ok(DynamicImage::ImageRgba8(RgbaImage::from_fn(
            source.width(),
            source.height(),
            |x, y| {
                let pixel = source.get_pixel(x, y);
                Rgba([pixel[0], pixel[1], pixel[2], 255])
            },
        ))),
        // NO_PILLOW_SUPPORT: The archived Pillow Convert contract rejects RGB→RGBa.
        // Opaque alpha makes native RGB samples valid premultiplied samples.
        Mode::ImageRgbPremultiplied8 => Ok(DynamicImage::ImageRgbPremultiplied8(
            RgbaImage::from_fn(source.width(), source.height(), |x, y| {
                let pixel = source.get_pixel(x, y);
                Rgba([pixel[0], pixel[1], pixel[2], 255])
            }),
        )),
        // NO_PILLOW_SUPPORT: The archived Pillow Convert contract rejects RGB→RGBX.
        // The core extension supplies an opaque padding byte to native RGB samples.
        Mode::ImageRgbx8 => Ok(DynamicImage::ImageRgbx8(RgbaImage::from_fn(
            source.width(),
            source.height(),
            |x, y| {
                let pixel = source.get_pixel(x, y);
                Rgba([pixel[0], pixel[1], pixel[2], 255])
            },
        ))),
        Mode::ImageCmyk8 => Ok(DynamicImage::ImageCmyk8(
            crate::color::rgb_to_cmyk_inverse(source),
        )),
        Mode::ImageYCbCr8 => Ok(DynamicImage::ImageYCbCr8(
            crate::color::rgb_buffer_to_ycbcr(source),
        )),
        Mode::ImageHsv8 => Ok(DynamicImage::ImageHsv8(
            crate::color::rgb_buffer_to_hsv(source),
        )),
        Mode::ImageI32 => Ok(crate::compute::pool_cpu::ops::color::convert::helpers::pixel::scalar_i32_rgb8(source)),
        Mode::ImageF32 => Ok(crate::compute::pool_cpu::ops::color::convert::helpers::pixel::scalar_f32_rgb8(source)),
        _ => Err(PilError::ValueError("image has wrong mode".into())),
    }
}

pub(crate) fn execute_palette_target(
    source: &RgbImage,
    target: Mode,
    dither: Option<DitherMethod>,
) -> Result<super::super::super::ops::PreparedConvertOutput, PilError> {
    if !matches!(target, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8) {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    let dither = !matches!(dither, Some(DitherMethod::None));
    if target == Mode::ImagePaletteAlpha8 {
        return crate::compute::pool_cpu::ops::color::convert::helpers::palette::web_alpha_from_rgb_samples(
            source.width(),
            source.height(),
            source.as_raw(),
            dither,
            |_| u8::MAX,
        );
    }
    let (indices, palette) = crate::ops::quantize::web_palette_quantize(
        source.as_raw(),
        source.width(),
        source.height(),
        dither,
    )?;
    let indices = crate::raster::GrayImage::from_raw(source.width(), source.height(), indices)
        .ok_or_else(|| PilError::ValueError("palette conversion failed".into()))?;
    Ok(super::super::super::ops::PreparedConvertOutput {
        image: DynamicImage::ImagePalette8(indices),
        palette: Some(palette),
        palette_alpha: None,
        wrapper_metadata: Some(super::super::super::ops::ConvertWrapperMetadata::PreserveSource),
    })
}

fn convert_luma_alpha(source: &RgbImage, target: Mode) -> Result<DynamicImage, PilError> {
    let source_dims = crate::checked_dims::CheckedDims::new_allow_empty(
        source.width(),
        source.height(),
        3,
    )?;
    let output_dims = crate::checked_dims::CheckedDims::new_allow_empty(
        source.width(),
        source.height(),
        2,
    )?;
    if source.as_raw().len() != source_dims.total_bytes() {
        return Err(PilError::InternalError(
            "RGB Convert source storage did not match its dimensions".into(),
        ));
    }

    let mut bytes = Vec::with_capacity(output_dims.total_bytes());
    for pixel in source.as_raw().chunks_exact(3).take(source_dims.total_pixels()) {
        bytes.extend_from_slice(&[
            crate::color::rgb_to_luma_u8(pixel[0], pixel[1], pixel[2]),
            u8::MAX,
        ]);
    }
    if bytes.len() != output_dims.total_bytes() {
        return Err(PilError::InternalError(
            "RGB Convert luma-alpha output shape mismatch".into(),
        ));
    }
    if bytes.capacity() != 0 {
        crate::compute::record_pipeline_allocation(bytes.capacity());
    }
    let output = GrayAlphaImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("RGB Convert luma-alpha output shape mismatch".into()))?;

    match target {
        Mode::ImageLumaA8 => Ok(DynamicImage::ImageLumaA8(output)),
        Mode::ImageLaPremultiplied8 => Ok(DynamicImage::ImageLaPremultiplied8(output)),
        _ => Err(PilError::InternalError("invalid RGB8 alpha target".into())),
    }
}
