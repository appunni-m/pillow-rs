//! Pillow-owned translation between codec images and exact raster identities.

use image_slash_star::{Decoded, DecodedImage, ImageMode};

use crate::error::PilError;
use crate::raster::{
    ColorType, DynamicImage, GrayAlphaImage, GrayImage, ImageBuffer, RgbImage, RgbaImage,
};

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
        image.color(),
    )
}

pub(crate) fn decoded_pixels_to_raster(d: &DecodedImage) -> Option<DynamicImage> {
    if d.mode != d.color.into() || d.palette.is_some() {
        return None;
    }
    let img = match d.color {
        ColorType::L8 => {
            DynamicImage::ImageLuma8(GrayImage::from_raw(d.width, d.height, d.pixels.clone())?)
        }
        ColorType::La8 => DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(
            d.width,
            d.height,
            d.pixels.clone(),
        )?),
        ColorType::Rgb8 => {
            DynamicImage::ImageRgb8(RgbImage::from_raw(d.width, d.height, d.pixels.clone())?)
        }
        ColorType::Rgba8 => {
            DynamicImage::ImageRgba8(RgbaImage::from_raw(d.width, d.height, d.pixels.clone())?)
        }
        ColorType::Cmyk8 => DynamicImage::ImageCmyk8(RgbaImage::from_raw(
            d.width,
            d.height,
            d.pixels.clone(),
        )?),
        ColorType::L16 => {
            let u16_data = native_u16_samples(&d.pixels);
            DynamicImage::ImageI16(ImageBuffer::from_raw(d.width, d.height, u16_data)?)
        }
        ColorType::La16 => {
            let u16_data = native_u16_samples(&d.pixels);
            DynamicImage::ImageLumaA16(ImageBuffer::from_raw(d.width, d.height, u16_data)?)
        }
        ColorType::Rgb16 => {
            let u16_data = native_u16_samples(&d.pixels);
            DynamicImage::ImageRgb16(ImageBuffer::from_raw(d.width, d.height, u16_data)?)
        }
        ColorType::Rgba16 => {
            let u16_data = native_u16_samples(&d.pixels);
            DynamicImage::ImageRgba16(ImageBuffer::from_raw(d.width, d.height, u16_data)?)
        }
        ColorType::Rgb32F => {
            let f32_data = native_f32_samples(&d.pixels);
            DynamicImage::ImageRgb32F(ImageBuffer::from_raw(d.width, d.height, f32_data)?)
        }
        ColorType::Rgba32F => {
            let f32_data = native_f32_samples(&d.pixels);
            DynamicImage::ImageRgba32F(ImageBuffer::from_raw(d.width, d.height, f32_data)?)
        }
        ColorType::L32F => DynamicImage::ImageF32(RgbaImage::from_raw(
            d.width,
            d.height,
            d.pixels.clone(),
        )?),
        ColorType::L32I => DynamicImage::ImageI32(RgbaImage::from_raw(
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
        mode => DynamicImage::from_decoded(content)
            .ok_or_else(|| PilError::ValueError(format!("unsupported decoded mode {mode:?}"))),
    }
}
