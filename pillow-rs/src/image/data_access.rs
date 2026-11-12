//! Bulk pixel data access and mutation methods for [`Image`].

use std::sync::Arc;

use bytes::Bytes;

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

use super::{
    FormattedImageData, Image, PutDataInput, PutDataValue, PutDataValueKind, materialization_cache,
};

pub(crate) enum ScalarImageSamples {
    Integer(Vec<i32>),
    Float(Vec<f64>),
}

pub(super) fn validate_scalar_storage(image: &DynamicImage, mode: Mode) -> Result<(), PilError> {
    if !matches!(mode, Mode::ImageI32 | Mode::ImageF32) {
        return Ok(());
    }
    // Pillow permits empty scalar images from `Image.frombytes`. CheckedDims
    // intentionally rejects zero dimensions for allocations, so validate the
    // already-existing empty buffer directly before using it for non-empty
    // images.
    let expected = if image.width() == 0 || image.height() == 0 {
        0
    } else {
        CheckedDims::new(image.width(), image.height(), 4)?.total_bytes()
    };
    if image.as_bytes().len() != expected {
        return Err(PilError::InternalError(format!(
            "{} image has invalid scalar storage",
            mode.pillow_name().unwrap_or("unknown")
        )));
    }
    Ok(())
}

fn putdata_clip_component(value: i128) -> u8 {
    value.clamp(0, 255) as u8
}

pub(super) const fn is_l16_identity(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N
    )
}

pub(super) const fn uses_native_value_at_putdata(mode: Mode) -> bool {
    is_l16_identity(mode)
        || matches!(
            mode,
            Mode::ImageLumaA16
                | Mode::ImageRgb16
                | Mode::ImageRgba16
                | Mode::ImageRgb32F
                | Mode::ImageRgba32F
        )
}

pub(super) const fn l16_identity_uses_big_endian(mode: Mode) -> bool {
    match mode {
        Mode::ImageI16B => true,
        Mode::ImageI16L => false,
        Mode::ImageI16 | Mode::ImageI16N => cfg!(target_endian = "big"),
        _ => false,
    }
}

fn putdata_byte_width(mode: Mode) -> Option<usize> {
    crate::compute::helpers::put_data::payload_bytes_per_pixel(mode)
}

fn unsupported_putdata_mode(mode: Mode) -> PilError {
    PilError::ValueError(format!(
        "unsupported putdata mode: {}",
        mode.pillow_name().unwrap_or("unknown")
    ))
}

fn append_numeric_value(
    mode: Mode,
    value: &PutDataValue,
    scale: f64,
    offset: f64,
    data: &mut Vec<u8>,
) -> Result<(), PilError> {
    let PutDataValue::Number(number) = value else {
        return Err(PilError::TypeError("sequence must be flattened".into()));
    };
    match mode {
        Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 => {
            data.push(crate::compute::helpers::put_data::clip_u8(
                number * scale + offset,
            ));
        }
        Mode::ImageI32 => {
            data.extend_from_slice(&((number * scale + offset) as i32).to_le_bytes());
        }
        Mode::ImageF32 => {
            data.extend_from_slice(&((number * scale + offset) as f32).to_le_bytes());
        }
        _ => return Err(unsupported_putdata_mode(mode)),
    }
    Ok(())
}

fn append_two_channel_value(value: &PutDataValue, data: &mut Vec<u8>) -> Result<(), PilError> {
    match value {
        PutDataValue::Packed(packed) => {
            let bytes = packed.to_le_bytes();
            data.extend_from_slice(&[bytes[0], bytes[3]]);
        }
        PutDataValue::Components(components)
            if (PutDataValueKind::Components { channels: 2 })
                .accepts_component_arity(components.len()) =>
        {
            data.extend(components.iter().copied().map(putdata_clip_component));
        }
        PutDataValue::Components(_) => {
            return Err(PilError::TypeError(
                "color must be int, or tuple of one or two elements".into(),
            ));
        }
        _ => return Err(PilError::TypeError("color must be int or tuple".into())),
    }
    Ok(())
}

fn append_three_channel_value(value: &PutDataValue, data: &mut Vec<u8>) -> Result<(), PilError> {
    match value {
        PutDataValue::Packed(packed) => data.extend_from_slice(&packed.to_le_bytes()[..3]),
        PutDataValue::Components(components)
            if (PutDataValueKind::Components { channels: 3 })
                .accepts_component_arity(components.len()) =>
        {
            data.extend(components[..3].iter().copied().map(putdata_clip_component));
        }
        PutDataValue::Components(_) => {
            return Err(PilError::TypeError(
                "color must be int, or tuple of one, three or four elements".into(),
            ));
        }
        _ => return Err(PilError::TypeError("color must be int or tuple".into())),
    }
    Ok(())
}

fn append_four_channel_value(value: &PutDataValue, data: &mut Vec<u8>) -> Result<(), PilError> {
    match value {
        PutDataValue::Packed(packed) => data.extend_from_slice(&packed.to_le_bytes()[..4]),
        PutDataValue::Components(components)
            if (PutDataValueKind::Components { channels: 4 })
                .accepts_component_arity(components.len()) =>
        {
            data.extend(components[..3].iter().copied().map(putdata_clip_component));
            data.push(
                components
                    .get(3)
                    .copied()
                    .map_or(255, putdata_clip_component),
            );
        }
        PutDataValue::Components(_) => {
            return Err(PilError::TypeError(
                "color must be int, or tuple of one, three or four elements".into(),
            ));
        }
        _ => return Err(PilError::TypeError("color must be int or tuple".into())),
    }
    Ok(())
}

fn append_putdata_value(
    mode: Mode,
    value: &PutDataValue,
    scale: f64,
    offset: f64,
    data: &mut Vec<u8>,
) -> Result<(), PilError> {
    match mode {
        Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 | Mode::ImageI32 | Mode::ImageF32 => {
            append_numeric_value(mode, value, scale, offset, data)
        }
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => {
            append_two_channel_value(value, data)
        }
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => {
            append_three_channel_value(value, data)
        }
        Mode::ImageRgba8 | Mode::ImageRgbPremultiplied8 | Mode::ImageRgbx8 | Mode::ImageCmyk8 => {
            append_four_channel_value(value, data)
        }
        _ => Err(unsupported_putdata_mode(mode)),
    }
}

fn putdata_bytes(
    mode: Mode,
    values: &[PutDataValue],
    scale: f64,
    offset: f64,
) -> Result<Vec<u8>, PilError> {
    let bytes_per_pixel = putdata_byte_width(mode).ok_or_else(|| unsupported_putdata_mode(mode))?;
    let capacity = values
        .len()
        .checked_mul(bytes_per_pixel)
        .ok_or_else(|| PilError::DimensionError("putdata byte count overflow".into()))?;
    let mut data = Vec::with_capacity(capacity);
    for value in values {
        append_putdata_value(mode, value, scale, offset, &mut data)?;
    }
    Ok(data)
}

/// Retains `_imaging.c::getink` bytes omitted from compact LA/La/PA and
/// RGB/YCbCr/LAB/HSV carriers.
/// `_putdata` stores all four bytes of that ink, even though public pixels
/// expose only lanes 0 and 3 for two-band modes or lanes 0 through 2 for
/// three-band modes. Pillow's `getink` selects those layouts by band count.
fn putdata_hidden_lanes(mode: Mode, value: &PutDataValue) -> Result<Option<[u8; 2]>, PilError> {
    match (mode, value) {
        (
            Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8,
            PutDataValue::Packed(packed),
        ) => {
            let bytes = packed.to_le_bytes();
            Ok(Some([bytes[1], bytes[2]]))
        }
        (
            Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8,
            PutDataValue::Components(components),
        ) if components.len() == 2 => {
            let luma = putdata_clip_component(components[0]);
            Ok(Some([luma, luma]))
        }
        (
            Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8,
            PutDataValue::Packed(packed),
        ) => Ok(Some([packed.to_le_bytes()[3], 0])),
        (
            Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8,
            PutDataValue::Components(components),
        ) if matches!(components.len(), 3 | 4) => Ok(Some([
            components
                .get(3)
                .copied()
                .map_or(u8::MAX, putdata_clip_component),
            0,
        ])),
        (Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8, _) => {
            Err(PilError::InternalError(
                "putdata LA hidden lanes did not match normalized value".into(),
            ))
        }
        (Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8, _) => {
            Err(PilError::InternalError(
                "putdata RGB hidden lane did not match normalized value".into(),
            ))
        }
        _ => Ok(None),
    }
}

fn putdata_hidden_lane_bytes(
    mode: Mode,
    values: &[PutDataValue],
) -> Result<Option<bytes::Bytes>, PilError> {
    let lane_count = match mode {
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => 2,
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => 1,
        _ => return Ok(None),
    };
    let capacity = values
        .len()
        .checked_mul(lane_count)
        .ok_or_else(|| PilError::DimensionError("putdata hidden lane size overflow".into()))?;
    let mut lanes = Vec::with_capacity(capacity);
    for value in values {
        let value = putdata_hidden_lanes(mode, value)?.ok_or_else(|| {
            PilError::InternalError("putdata hidden lane normalization was missing".into())
        })?;
        lanes.extend_from_slice(&value[..lane_count]);
    }
    Ok(Some(bytes::Bytes::from(lanes)))
}

pub(super) fn putpixel_numeric_bytes(mode: Mode, value: f64) -> Result<[u8; 4], PilError> {
    let value = value * 1.0 + 0.0;
    match mode {
        Mode::ImageI32 => Ok((value as i32).to_le_bytes()),
        Mode::ImageF32 => Ok((value as f32).to_le_bytes()),
        _ => Err(PilError::InternalError(
            "numeric putpixel bytes require I or F mode".into(),
        )),
    }
}

fn validated_band_index(band: i32, band_count: usize) -> Result<usize, PilError> {
    let band_index = usize::try_from(band)
        .map_err(|_| PilError::ValueError("band index out of range".into()))?;
    if band_index >= band_count {
        return Err(PilError::ValueError("band index out of range".into()));
    }
    Ok(band_index)
}

mod get;
mod put;
