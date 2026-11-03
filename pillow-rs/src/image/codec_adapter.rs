//! Pillow-owned translation between codec images and exact raster identities.

use image_slash_star::{
    ColorType as CodecColorType, Decoded, DecodedImage, ImageMode as CodecImageMode,
    Mode as CodecMode,
};

use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{
    ColorType, DynamicImage, GrayAlphaImage, GrayImage, ImageBuffer, Mode, RgbImage, RgbaImage,
};

/// Translates the header-selected codec identity into Pillow's raster identity.
///
/// The codec selects mode while inspecting headers; this exhaustive boundary
/// translation keeps raster ownership and pixel semantics in Pillow.
pub(super) const fn raster_mode_from_codec_mode(mode: CodecMode) -> Mode {
    match mode {
        CodecMode::One => Mode::One,
        CodecMode::ImageLuma8 => Mode::ImageLuma8,
        CodecMode::ImageLumaA8 => Mode::ImageLumaA8,
        // NO_PILLOW_SUPPORT: Pillow 12.2 does not open associated-LA TIFF.
        CodecMode::ImageLaPremultiplied8 => Mode::ImageLaPremultiplied8,
        CodecMode::ImagePalette8 => Mode::ImagePalette8,
        CodecMode::ImagePaletteAlpha8 => Mode::ImagePaletteAlpha8,
        CodecMode::ImageRgb8 => Mode::ImageRgb8,
        // TODO(pillow-rs#3ptb): verify TIFF associated-alpha RGBA after the
        // codec selects Mode from decoded transfer semantics. Its contiguous
        // and compressed paths unassociate samples; raw separate uncompressed
        // input retains them. Do not infer the raster identity here.
        CodecMode::ImageRgba8 => Mode::ImageRgba8,
        CodecMode::ImageRgbPremultiplied8 => Mode::ImageRgbPremultiplied8,
        CodecMode::ImageRgbx8 => Mode::ImageRgbx8,
        CodecMode::ImageCmyk8 => Mode::ImageCmyk8,
        CodecMode::ImageYCbCr8 => Mode::ImageYCbCr8,
        CodecMode::ImageLab8 => Mode::ImageLab8,
        CodecMode::ImageHsv8 => Mode::ImageHsv8,
        CodecMode::ImageI32 => Mode::ImageI32,
        CodecMode::ImageF32 => Mode::ImageF32,
        CodecMode::ImageI16 => Mode::ImageI16,
        CodecMode::ImageI16L => Mode::ImageI16L,
        CodecMode::ImageI16B => Mode::ImageI16B,
        CodecMode::ImageI16N => Mode::ImageI16N,
        CodecMode::ImageLumaA16 => Mode::ImageLumaA16,
        CodecMode::ImageRgb16 => Mode::ImageRgb16,
        CodecMode::ImageRgba16 => Mode::ImageRgba16,
        CodecMode::ImageRgb32F => Mode::ImageRgb32F,
        CodecMode::ImageRgba32F => Mode::ImageRgba32F,
    }
}

fn little_endian_u16_samples(bytes: &[u8]) -> Vec<u16> {
    bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect()
}

fn native_f32_samples(bytes: &[u8]) -> Vec<f32> {
    bytes.chunks_exact(4).map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]])).collect()
}

pub(crate) fn raster_to_decoded(image: DynamicImage) -> Result<DecodedImage, PilError> {
    let width = image.width();
    let height = image.height();
    match image {
        DynamicImage::One(image) => {
            let packed = pack_one_samples(&image)?;
            Ok(DecodedImage::with_mode(
                width,
                height,
                packed,
                CodecImageMode::L1,
            ))
        }
        DynamicImage::ImageI32(image) => Ok(DecodedImage::with_mode(
            width,
            height,
            image.into_raw(),
            CodecImageMode::I32,
        )),
        DynamicImage::ImageF32(image) => Ok(DecodedImage::with_mode(
            width,
            height,
            image.into_raw(),
            CodecImageMode::F32,
        )),
        DynamicImage::ImageI16(image)
        | DynamicImage::ImageI16L(image)
        | DynamicImage::ImageI16B(image)
        | DynamicImage::ImageI16N(image) => Ok(DecodedImage::new(
            width,
            height,
            little_endian_u16_bytes(image.as_raw())?,
            CodecColorType::L16,
        )),
        DynamicImage::ImageLumaA16(image) => Ok(DecodedImage::new(
            width,
            height,
            little_endian_u16_bytes(image.as_raw())?,
            CodecColorType::La16,
        )),
        DynamicImage::ImageRgb16(image) => Ok(DecodedImage::new(
            width,
            height,
            little_endian_u16_bytes(image.as_raw())?,
            CodecColorType::Rgb16,
        )),
        DynamicImage::ImageRgba16(image) => Ok(DecodedImage::new(
            width,
            height,
            little_endian_u16_bytes(image.as_raw())?,
            CodecColorType::Rgba16,
        )),
        image => {
            // TODO(image-slash-star#nype): add exact codec transfer mappings
            // for premultiplied alpha, RGBX, and three-byte color spaces,
            // including format-specific support and rejection behavior.
            // Append newly found mode/format cases to that task. Keep pixel
            // translation in the codec boundary, not as a Pillow-side
            // conversion.
            // TODO(pillow-rs#3ptb): update this adapter when those codec
            // contracts land and verify Pillow-visible output and errors.
            Ok(DecodedImage::new(
                width,
                height,
                image.as_bytes().to_vec(),
                codec_color_type(image.color()),
            ))
        }
    }
}

fn little_endian_u16_bytes(samples: &[u16]) -> Result<Vec<u8>, PilError> {
    let byte_len = samples
        .len()
        .checked_mul(2)
        .ok_or_else(|| PilError::DimensionError("16-bit image byte size overflows".into()))?;
    let mut bytes = Vec::with_capacity(byte_len);
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    Ok(bytes)
}

fn pack_one_samples(image: &crate::raster::GrayImage) -> Result<Vec<u8>, PilError> {
    let width = image.width() as usize;
    let height = image.height() as usize;
    let row_bytes = width.div_ceil(8);
    let packed_len = row_bytes
        .checked_mul(height)
        .ok_or_else(|| PilError::DimensionError("packed bilevel image size overflows".into()))?;
    let mut packed = vec![0; packed_len];
    if width == 0 {
        return Ok(packed);
    }
    for (index, &sample) in image.as_raw().iter().enumerate() {
        if sample == 0 {
            continue;
        }
        let y = index / width;
        let x = index % width;
        packed[y * row_bytes + x / 8] |= 1 << (7 - x % 8);
    }
    Ok(packed)
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
            let u16_data = little_endian_u16_samples(&d.pixels);
            DynamicImage::ImageI16(ImageBuffer::from_raw(d.width, d.height, u16_data)?)
        }
        CodecColorType::La16 => {
            let u16_data = little_endian_u16_samples(&d.pixels);
            DynamicImage::ImageLumaA16(ImageBuffer::from_raw(d.width, d.height, u16_data)?)
        }
        CodecColorType::Rgb16 => {
            let u16_data = little_endian_u16_samples(&d.pixels);
            DynamicImage::ImageRgb16(ImageBuffer::from_raw(d.width, d.height, u16_data)?)
        }
        CodecColorType::Rgba16 => {
            let u16_data = little_endian_u16_samples(&d.pixels);
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

pub(super) fn decoded_to_dynamic(
    decoded: &Decoded<DecodedImage>,
    expected_codec_mode: CodecImageMode,
    header_mode: Mode,
) -> Result<DynamicImage, PilError> {
    let content = &decoded.content;
    if content.mode != expected_codec_mode {
        return Err(PilError::InternalError(format!(
            "encoded header mode {expected_codec_mode:?} differs from decoded mode {:?}",
            content.mode
        )));
    }
    let image = match content.mode {
        CodecImageMode::P8 => crate::raster::GrayImage::from_raw(
            content.width,
            content.height,
            content.pixels.clone(),
        )
        .map(DynamicImage::ImagePalette8)
        .ok_or_else(|| PilError::DimensionError("invalid indexed buffer".to_owned())),
        CodecImageMode::L1 => {
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
        CodecImageMode::Cmyk8 => crate::raster::RgbaImage::from_raw(
            content.width,
            content.height,
            content.pixels.clone(),
        )
        .map(DynamicImage::ImageCmyk8)
        .ok_or_else(|| PilError::DimensionError("invalid four-byte buffer".to_owned())),
        CodecImageMode::F32 => crate::raster::RgbaImage::from_raw(
            content.width,
            content.height,
            content.pixels.clone(),
        )
        .map(DynamicImage::ImageF32)
        .ok_or_else(|| PilError::DimensionError("invalid four-byte buffer".to_owned())),
        CodecImageMode::I32 => crate::raster::RgbaImage::from_raw(
            content.width,
            content.height,
            content.pixels.clone(),
        )
        .map(DynamicImage::ImageI32)
        .ok_or_else(|| PilError::DimensionError("invalid four-byte buffer".to_owned())),
        mode => decoded_pixels_to_raster(content)
            .ok_or_else(|| PilError::ValueError(format!("unsupported decoded mode {mode:?}"))),
    }?;
    // Codec transfer layouts can be less specific than Pillow identity. Keep
    // the header-selected mode while reusing only the exact same decoded
    // carrier; no sample conversion or DynamicImage materialization happens
    // before this decode boundary.
    let mut image = retag_decoded_image(image, header_mode)?;
    attach_decoded_hidden_lanes(&mut image, header_mode)?;
    Ok(image)
}

pub(crate) fn attach_decoded_hidden_lanes(
    image: &mut DynamicImage,
    mode: Mode,
) -> Result<(), PilError> {
    let pixel_count = (image.width() as usize)
        .checked_mul(image.height() as usize)
        .ok_or_else(|| PilError::DimensionError("physical pixel lane count overflows".into()))?;
    let attached = match (mode, image) {
        (Mode::ImageLumaA8, DynamicImage::ImageLumaA8(buffer))
        | (Mode::ImageLaPremultiplied8, DynamicImage::ImageLaPremultiplied8(buffer))
        | (Mode::ImagePaletteAlpha8, DynamicImage::ImagePaletteAlpha8(buffer)) => {
            let lane_bytes = pixel_count.checked_mul(2).ok_or_else(|| {
                PilError::DimensionError("decoded LA lane size overflows".into())
            })?;
            let mut values = Vec::with_capacity(lane_bytes);
            for pixel in buffer.pixels() {
                values.extend_from_slice(&[pixel.0[0], pixel.0[0]]);
            }
            let lanes = HiddenPixelLanes::per_pixel(2, pixel_count, values)
                .ok_or_else(|| PilError::InternalError("invalid decoded LA lane layout".into()))?;
            buffer.replace_hidden_pixel_lanes(Some(lanes))
        }
        (Mode::ImageRgb8, DynamicImage::ImageRgb8(buffer))
        | (Mode::ImageYCbCr8, DynamicImage::ImageYCbCr8(buffer))
        | (Mode::ImageLab8, DynamicImage::ImageLab8(buffer))
        | (Mode::ImageHsv8, DynamicImage::ImageHsv8(buffer)) => {
            let lanes = HiddenPixelLanes::constant(1, pixel_count, [u8::MAX, 0])
                .ok_or_else(|| PilError::InternalError("invalid decoded RGB lane layout".into()))?;
            buffer.replace_hidden_pixel_lanes(Some(lanes))
        }
        _ => return Ok(()),
    };
    if !attached {
        return Err(PilError::InternalError(
            "decoded mode did not match its physical pixel layout".into(),
        ));
    }
    Ok(())
}

fn retag_decoded_image(image: DynamicImage, header_mode: Mode) -> Result<DynamicImage, PilError> {
    if image.mode() == header_mode {
        return Ok(image);
    }
    let image = match (header_mode, image) {
        (Mode::ImageLaPremultiplied8, DynamicImage::ImageLumaA8(image)) => {
            DynamicImage::ImageLaPremultiplied8(image)
        }
        (Mode::ImagePaletteAlpha8, DynamicImage::ImageLumaA8(image)) => {
            DynamicImage::ImagePaletteAlpha8(image)
        }
        (Mode::ImageRgbPremultiplied8, DynamicImage::ImageRgba8(image)) => {
            DynamicImage::ImageRgbPremultiplied8(image)
        }
        (Mode::ImageRgbx8, DynamicImage::ImageRgba8(image)) => {
            DynamicImage::ImageRgbx8(image)
        }
        (Mode::ImageYCbCr8, DynamicImage::ImageRgb8(image)) => {
            DynamicImage::ImageYCbCr8(image)
        }
        (Mode::ImageLab8, DynamicImage::ImageRgb8(image)) => {
            DynamicImage::ImageLab8(image)
        }
        (Mode::ImageHsv8, DynamicImage::ImageRgb8(image)) => {
            DynamicImage::ImageHsv8(image)
        }
        (Mode::ImageI16L, DynamicImage::ImageI16(image)) => DynamicImage::ImageI16L(image),
        (Mode::ImageI16B, DynamicImage::ImageI16(image)) => DynamicImage::ImageI16B(image),
        (Mode::ImageI16N, DynamicImage::ImageI16(image)) => DynamicImage::ImageI16N(image),
        (header_mode, _) => {
            return Err(PilError::InternalError(format!(
                "encoded header mode {header_mode:?} does not match decoded pixel storage"
            )));
        }
    };
    Ok(image)
}
