//! Pillow-owned translation between codec images and exact raster identities.

use image_slash_star::{ColorType as CodecColorType, Decoded, DecodedImage, ImageMode};

use crate::error::PilError;
use crate::raster::{
    ColorType, DynamicImage, GrayAlphaImage, GrayImage, ImageBuffer, Mode, RgbImage, RgbaImage,
};

/// Projects codec transfer metadata onto the exact Pillow-owned raster carrier.
///
/// The 16-bit and float RGB/alpha layouts are internal raster identities; they
/// must not be collapsed to their byte-mode counterparts before materializing.
pub(super) const fn raster_mode_from_image_mode(mode: ImageMode) -> Option<Mode> {
    match mode {
        ImageMode::L1 => Some(Mode::One),
        ImageMode::P8 => Some(Mode::ImagePalette8),
        ImageMode::L8 => Some(Mode::ImageLuma8),
        ImageMode::La8 => Some(Mode::ImageLumaA8),
        ImageMode::La16 => Some(Mode::ImageLumaA16),
        ImageMode::Rgb8 => Some(Mode::ImageRgb8),
        ImageMode::Rgba8 => Some(Mode::ImageRgba8),
        ImageMode::Cmyk8 => Some(Mode::ImageCmyk8),
        ImageMode::L16 => Some(Mode::ImageI16),
        ImageMode::Rgb16 => Some(Mode::ImageRgb16),
        ImageMode::Rgba16 => Some(Mode::ImageRgba16),
        ImageMode::Rgb32F => Some(Mode::ImageRgb32F),
        ImageMode::Rgba32F => Some(Mode::ImageRgba32F),
        ImageMode::F32 => Some(Mode::ImageF32),
        ImageMode::I32 => Some(Mode::ImageI32),
        _ => None,
    }
}

fn native_u16_samples(bytes: &[u8]) -> Vec<u16> {
    bytes.chunks_exact(2).map(|c| u16::from_ne_bytes([c[0], c[1]])).collect()
}

fn native_f32_samples(bytes: &[u8]) -> Vec<f32> {
    bytes.chunks_exact(4).map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]])).collect()
}

pub(crate) fn raster_to_decoded(image: DynamicImage) -> DecodedImage {
    DecodedImage::new(
        image.width(),
        image.height(),
        image.as_bytes().to_vec(),
        codec_color_type(image.color()),
    )
}

fn codec_color_type(color: ColorType) -> CodecColorType {
    match color {
        ColorType::L8 => CodecColorType::L8,
        ColorType::La8 => CodecColorType::La8,
        ColorType::Rgb8 => CodecColorType::Rgb8,
        ColorType::Rgba8 => CodecColorType::Rgba8,
        ColorType::Cmyk8 => CodecColorType::Cmyk8,
        ColorType::L16 => CodecColorType::L16,
        ColorType::La16 => CodecColorType::La16,
        ColorType::Rgb16 => CodecColorType::Rgb16,
        ColorType::Rgba16 => CodecColorType::Rgba16,
        ColorType::Rgb32F => CodecColorType::Rgb32F,
        ColorType::Rgba32F => CodecColorType::Rgba32F,
        ColorType::L32F => CodecColorType::L32F,
        ColorType::L32I => CodecColorType::L32I,
    }
}

pub(crate) fn decoded_pixels_to_raster(d: &DecodedImage) -> Option<DynamicImage> {
    if d.mode != d.color.into() || d.palette.is_some() {
        return None;
    }
    let img = match d.color {
        CodecColorType::L8 => {
            DynamicImage::ImageLuma8(GrayImage::from_raw(d.width, d.height, d.pixels.clone())?)
        }
        CodecColorType::La8 => DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(
            d.width,
            d.height,
            d.pixels.clone(),
        )?),
        CodecColorType::Rgb8 => {
            DynamicImage::ImageRgb8(RgbImage::from_raw(d.width, d.height, d.pixels.clone())?)
        }
        CodecColorType::Rgba8 => {
            DynamicImage::ImageRgba8(RgbaImage::from_raw(d.width, d.height, d.pixels.clone())?)
        }
        CodecColorType::Cmyk8 => DynamicImage::ImageCmyk8(RgbaImage::from_raw(
            d.width,
            d.height,
            d.pixels.clone(),
        )?),
        CodecColorType::L16 => {
            let u16_data = native_u16_samples(&d.pixels);
            DynamicImage::ImageI16(ImageBuffer::from_raw(d.width, d.height, u16_data)?)
        }
        CodecColorType::La16 => {
            let u16_data = native_u16_samples(&d.pixels);
            DynamicImage::ImageLumaA16(ImageBuffer::from_raw(d.width, d.height, u16_data)?)
        }
        CodecColorType::Rgb16 => {
            let u16_data = native_u16_samples(&d.pixels);
            DynamicImage::ImageRgb16(ImageBuffer::from_raw(d.width, d.height, u16_data)?)
        }
        CodecColorType::Rgba16 => {
            let u16_data = native_u16_samples(&d.pixels);
            DynamicImage::ImageRgba16(ImageBuffer::from_raw(d.width, d.height, u16_data)?)
        }
        CodecColorType::Rgb32F => {
            let f32_data = native_f32_samples(&d.pixels);
            DynamicImage::ImageRgb32F(ImageBuffer::from_raw(d.width, d.height, f32_data)?)
        }
        CodecColorType::Rgba32F => {
            let f32_data = native_f32_samples(&d.pixels);
            DynamicImage::ImageRgba32F(ImageBuffer::from_raw(d.width, d.height, f32_data)?)
        }
        CodecColorType::L32F => DynamicImage::ImageF32(RgbaImage::from_raw(
            d.width,
            d.height,
            d.pixels.clone(),
        )?),
        CodecColorType::L32I => DynamicImage::ImageI32(RgbaImage::from_raw(
            d.width,
            d.height,
            d.pixels.clone(),
        )?),
        _ => return None,
    };
    Some(img)
}

pub(super) fn decoded_to_dynamic(decoded: &Decoded<DecodedImage>) -> Result<DynamicImage, PilError> {
    let content = &decoded.content;
    match content.mode {
        ImageMode::P8 => crate::raster::GrayImage::from_raw(
            content.width,
            content.height,
            content.pixels.clone(),
        )
        .map(DynamicImage::ImagePalette8)
        .ok_or_else(|| PilError::DimensionError("invalid indexed buffer".to_owned())),
        ImageMode::L1 => {
            let row_bytes = content.width.div_ceil(8) as usize;
            let mut unpacked = Vec::with_capacity(content.width as usize * content.height as usize);
            for row in content.pixels.chunks_exact(row_bytes) {
                for x in 0..content.width as usize {
                    let bit = (row[x / 8] >> (7 - (x % 8))) & 1;
                    unpacked.push(if bit == 0 { 0 } else { 255 });
                }
            }
            crate::raster::GrayImage::from_raw(content.width, content.height, unpacked)
                .map(DynamicImage::One)
                .ok_or_else(|| PilError::DimensionError("invalid packed bilevel buffer".to_owned()))
        }
        ImageMode::Cmyk8 => crate::raster::RgbaImage::from_raw(
            content.width,
            content.height,
            content.pixels.clone(),
        )
        .map(DynamicImage::ImageCmyk8)
        .ok_or_else(|| PilError::DimensionError("invalid four-byte buffer".to_owned())),
        ImageMode::F32 => crate::raster::RgbaImage::from_raw(
            content.width,
            content.height,
            content.pixels.clone(),
        )
        .map(DynamicImage::ImageF32)
        .ok_or_else(|| PilError::DimensionError("invalid four-byte buffer".to_owned())),
        ImageMode::I32 => crate::raster::RgbaImage::from_raw(
            content.width,
            content.height,
            content.pixels.clone(),
        )
        .map(DynamicImage::ImageI32)
        .ok_or_else(|| PilError::DimensionError("invalid four-byte buffer".to_owned())),
        mode => decoded_pixels_to_raster(content)
            .ok_or_else(|| PilError::ValueError(format!("unsupported decoded mode {mode:?}"))),
    }
}
