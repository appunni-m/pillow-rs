//! Bulk pixel data access and mutation methods for [`Image`].

use std::sync::Arc;

use bytes::Bytes;

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::pipeline::{PipelineOp, PixelMode};
use crate::raster::DynamicImage;

use super::{
    FormattedImageData, Image, PutDataInput, PutDataValue, PutDataValueKind, image_mode_name,
    materialization_cache, select_interleaved_band, select_interleaved_band_into, select_lab_band,
    select_lab_band_into,
};

pub(crate) enum ScalarImageSamples {
    Integer(Vec<i32>),
    Float(Vec<f64>),
}

pub(super) fn decode_integer_samples(raw: &[u8]) -> Vec<i32> {
    raw.chunks_exact(4)
        .map(|sample| i32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]))
        .collect()
}

pub(super) fn decode_float_samples(raw: &[u8]) -> Vec<f64> {
    raw.chunks_exact(4)
        .map(|sample| f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]) as f64)
        .collect()
}

pub(super) fn validate_scalar_storage(image: &DynamicImage, mode: &str) -> Result<(), PilError> {
    if !matches!(mode, "I" | "F") {
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
            "{mode} image has invalid scalar storage"
        )));
    }
    Ok(())
}

pub(super) fn putdata_clip_u8(value: f64) -> u8 {
    if value.is_nan() || value >= 255.0 {
        255
    } else if value <= 0.0 {
        0
    } else {
        value as u8
    }
}

fn putdata_clip_component(value: i128) -> u8 {
    value.clamp(0, 255) as u8
}

pub(super) fn is_l16_mode(mode: &str) -> bool {
    matches!(mode, "I;16" | "I;16L" | "I;16B" | "I;16N")
}

pub(super) fn l16_uses_big_endian(mode: &str) -> bool {
    match mode {
        "I;16B" => true,
        "I;16L" => false,
        "I;16" | "I;16N" => cfg!(target_endian = "big"),
        _ => false,
    }
}

fn putdata_l16_sample(value: &PutDataValue, scale: f64, offset: f64) -> Result<u16, PilError> {
    match value {
        // _imaging.c:_putdata treats I;16 as an unsigned 16-bit sample
        // destination. Rust's float-to-integer cast provides the same
        // bounded conversion for the public numeric path.
        PutDataValue::Number(number) => Ok((number * scale + offset) as u16),
        // Pillow's I;16 decoder reaches the flattened-sequence guard before
        // numeric coercion for nested/non-numeric values. Keep that public
        // error text exact instead of exposing the generic scalar path.
        _ => Err(PilError::TypeError("sequence must be flattened".to_owned())),
    }
}

fn putdata_bytes(
    mode: crate::pipeline::PixelMode,
    values: &[PutDataValue],
    scale: f64,
    offset: f64,
) -> Result<Vec<u8>, PilError> {
    let capacity = values
        .len()
        .checked_mul(mode.channels())
        .ok_or_else(|| PilError::DimensionError("putdata byte count overflow".into()))?;
    let mut data = Vec::with_capacity(capacity);

    for value in values {
        match (mode, value) {
            (
                crate::pipeline::PixelMode::Mode1
                | crate::pipeline::PixelMode::L
                | crate::pipeline::PixelMode::P,
                PutDataValue::Number(number),
            ) => data.push(putdata_clip_u8(number * scale + offset)),
            (crate::pipeline::PixelMode::I, PutDataValue::Number(number)) => {
                data.extend_from_slice(&((number * scale + offset) as i32).to_le_bytes());
            }
            (crate::pipeline::PixelMode::F, PutDataValue::Number(number)) => {
                data.extend_from_slice(&((number * scale + offset) as f32).to_le_bytes());
            }
            (
                crate::pipeline::PixelMode::LA | crate::pipeline::PixelMode::PA,
                PutDataValue::Packed(packed),
            ) => {
                let bytes = packed.to_le_bytes();
                data.extend_from_slice(&[bytes[0], bytes[3]]);
            }
            (
                crate::pipeline::PixelMode::RGB
                | crate::pipeline::PixelMode::YCbCr
                | crate::pipeline::PixelMode::HSV,
                PutDataValue::Packed(packed),
            ) => data.extend_from_slice(&packed.to_le_bytes()[..3]),
            (
                crate::pipeline::PixelMode::RGBA | crate::pipeline::PixelMode::CMYK,
                PutDataValue::Packed(packed),
            ) => data.extend_from_slice(&packed.to_le_bytes()[..4]),
            (
                crate::pipeline::PixelMode::LA | crate::pipeline::PixelMode::PA,
                PutDataValue::Components(components),
            ) if components.len() == 2 => {
                data.extend(components.iter().copied().map(putdata_clip_component));
            }
            (
                crate::pipeline::PixelMode::RGB
                | crate::pipeline::PixelMode::YCbCr
                | crate::pipeline::PixelMode::HSV,
                PutDataValue::Components(components),
            ) if matches!(components.len(), 3 | 4) => {
                data.extend(components[..3].iter().copied().map(putdata_clip_component));
            }
            (
                crate::pipeline::PixelMode::RGBA | crate::pipeline::PixelMode::CMYK,
                PutDataValue::Components(components),
            ) if matches!(components.len(), 3 | 4) => {
                data.extend(components[..3].iter().copied().map(putdata_clip_component));
                data.push(
                    components
                        .get(3)
                        .copied()
                        .map_or(255, putdata_clip_component),
                );
            }
            (
                crate::pipeline::PixelMode::LA | crate::pipeline::PixelMode::PA,
                PutDataValue::Components(_),
            ) => {
                return Err(PilError::TypeError(
                    "color must be int, or tuple of one or two elements".into(),
                ));
            }
            (
                crate::pipeline::PixelMode::RGB
                | crate::pipeline::PixelMode::RGBA
                | crate::pipeline::PixelMode::CMYK
                | crate::pipeline::PixelMode::YCbCr
                | crate::pipeline::PixelMode::HSV,
                PutDataValue::Components(_),
            ) => {
                return Err(PilError::TypeError(
                    "color must be int, or tuple of one, three or four elements".into(),
                ));
            }
            (
                crate::pipeline::PixelMode::Mode1
                | crate::pipeline::PixelMode::L
                | crate::pipeline::PixelMode::P
                | crate::pipeline::PixelMode::I
                | crate::pipeline::PixelMode::F,
                _,
            ) => {
                return Err(PilError::TypeError("sequence must be flattened".into()));
            }
            _ => {
                return Err(PilError::TypeError("color must be int or tuple".into()));
            }
        }
    }

    Ok(data)
}

pub(super) fn pillow_band_count(mode: &str) -> usize {
    match mode {
        "L" | "1" | "P" | "I" | "F" | "I;16" | "I;16L" | "I;16B" | "I;16N" => 1,
        "LA" | "La" | "PA" => 2,
        "RGB" | "YCbCr" | "HSV" | "LAB" => 3,
        _ => 4,
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

#[cfg(test)]
#[path = "data_access/putdata_shared_bytes_tests.rs"]
mod putdata_shared_bytes_tests;
