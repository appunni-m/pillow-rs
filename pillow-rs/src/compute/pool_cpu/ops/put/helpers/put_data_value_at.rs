//! Shared storage helpers for mode-selected synchronous PutData writers.

use crate::error::PilError;
use crate::raster::{ImageBuffer, Mode, Pixel};

/// Normalizes one numeric I;16 putdata item through Pillow's byte stores.
///
/// Pillow casts the scaled value to a C `int`, clips its low byte and shifted
/// high byte independently, then stores those bytes in mode byte order. The
/// exact-mode typed carrier represents the resulting logical sample directly.
pub(crate) fn luma16_sample(
    value: &crate::image::PutDataValue,
    scale: f64,
    offset: f64,
) -> Result<u16, PilError> {
    let crate::image::PutDataValue::Number(number) = value else {
        return Err(PilError::TypeError("sequence must be flattened".to_owned()));
    };
    let scaled = if scale == 1.0 && offset == 0.0 {
        *number
    } else {
        *number * scale + offset
    };
    // NO_PILLOW_SUPPORT: C double-to-int conversion is undefined for NaN,
    // infinities, and values outside i32; Rust's cast defines this extension.
    // TODO(pillow-rs#3ptb): freeze these deterministic Rust-only edge values in
    // the CPU parity extension lane; Pillow has no defined oracle result.
    let integer = scaled as i32;
    let low = (integer % 256).clamp(0, 255) as u16;
    let high = (integer >> 8).clamp(0, 255) as u16;
    Ok((high << 8) | low)
}

pub(crate) enum ValueAtInput {
    Bytes(Vec<u8>, Option<[u8; 2]>),
    Luma16(u16),
    LumaA16([u16; 2]),
    Rgb16([u16; 3]),
    Rgba16([u16; 4]),
    Rgb32F([f32; 3]),
    Rgba32F([f32; 4]),
}

/// Normalizes CoreOnly multiband tuples directly into their native sample type.
pub(crate) fn native_value_at_input(
    mode: Mode,
    value: &crate::image::PutDataValue,
) -> Result<ValueAtInput, PilError> {
    match mode {
        Mode::ImageLumaA16 => native_u16_components::<2>(value, 2, u16::MAX)
            .map(ValueAtInput::LumaA16),
        Mode::ImageRgb16 => native_u16_components::<3>(value, 3, u16::MAX)
            .map(ValueAtInput::Rgb16),
        Mode::ImageRgba16 => native_u16_components::<4>(value, 4, u16::MAX)
            .map(ValueAtInput::Rgba16),
        Mode::ImageRgb32F => native_f32_components::<3>(value, 3).map(ValueAtInput::Rgb32F),
        Mode::ImageRgba32F => native_f32_components::<4>(value, 4).map(ValueAtInput::Rgba32F),
        _ => Err(PilError::InternalError(
            "native putdata input requested for a non-native mode".into(),
        )),
    }
}

#[inline]
pub(crate) fn native_u16_components<const CHANNELS: usize>(
    value: &crate::image::PutDataValue,
    channels: usize,
    alpha_default: u16,
) -> Result<[u16; CHANNELS], PilError> {
    match value {
        crate::image::PutDataValue::Packed(packed) => {
            let bytes = packed.to_le_bytes();
            Ok(std::array::from_fn(|index| {
                u16::from_le_bytes([bytes[index * 2], bytes[index * 2 + 1]])
            }))
        }
        crate::image::PutDataValue::Components(components)
            if components.len() == channels
                || (channels >= 3 && matches!(components.len(), 3 | 4)) =>
        {
            Ok(std::array::from_fn(|index| {
                if index == 3 && components.len() == 3 {
                    alpha_default
                } else {
                    components[index].clamp(0, i128::from(u16::MAX)) as u16
                }
            }))
        }
        crate::image::PutDataValue::Components(_) => Err(PilError::TypeError(
            "color tuple has the wrong number of native samples".into(),
        )),
        _ => Err(PilError::TypeError(
            "color must be an integer or a tuple of native samples".into(),
        )),
    }
}

#[inline]
pub(crate) fn native_f32_components<const CHANNELS: usize>(
    value: &crate::image::PutDataValue,
    channels: usize,
) -> Result<[f32; CHANNELS], PilError> {
    match value {
        crate::image::PutDataValue::FloatComponents(components)
            if components.len() == channels
                || (channels == 4 && components.len() == 3)
                || (channels == 3 && components.len() == 4) =>
        {
            Ok(std::array::from_fn(|index| {
                if index == 3 && components.len() == 3 {
                    1.0
                } else {
                    components[index] as f32
                }
            }))
        }
        crate::image::PutDataValue::FloatComponents(_) => Err(PilError::TypeError(
            "color tuple has the wrong number of native float samples".into(),
        )),
        _ => Err(PilError::TypeError(
            "color must be a tuple of native float samples".into(),
        )),
    }
}

// TODO(pillow-rs#3ptb): verify callback-visible partial writes and faults,
// compact hidden lanes, and selected/fallback receipts for all 20 modes in
// frozen CPU parity; retain the immediate write-before-next-coercion order.
pub(crate) fn write_plain_bytes(
    samples: &mut [u8],
    pixel_index: usize,
    bytes: &[u8],
    expected_bytes: usize,
    hidden_lanes: Option<[u8; 2]>,
    mismatch: &'static str,
) -> Result<(), PilError> {
    if bytes.len() != expected_bytes {
        return Err(PilError::InternalError(mismatch.into()));
    }
    if hidden_lanes.is_some() {
        return Err(PilError::InternalError(
            "putdata hidden lanes did not match its raster mode".into(),
        ));
    }
    super::put_pixel::write_native_bytes(samples, pixel_index, bytes)
}

pub(crate) fn write_compact_bytes<P, F, D>(
    image: &mut ImageBuffer<P, Vec<u8>>,
    pixel_index: usize,
    bytes: &[u8],
    expected_bytes: usize,
    lane_count: usize,
    hidden_lanes: Option<[u8; 2]>,
    mismatch: &'static str,
    missing_lanes: &'static str,
    default_lanes: D,
    fallback: F,
) -> Result<(), PilError>
where
    P: Pixel<Subpixel = u8>,
    F: Fn(&P) -> [u8; 2],
    D: Fn(&[u8]) -> [u8; 2],
{
    if bytes.len() != expected_bytes {
        return Err(PilError::InternalError(mismatch.into()));
    }
    let Some(lane_values) = hidden_lanes else {
        return Err(PilError::InternalError(missing_lanes.into()));
    };

    let default = default_lanes(bytes);
    let mut retained_lanes = image.hidden_pixel_lanes().cloned();
    super::put_pixel::write_native_bytes(image.as_mut(), pixel_index, bytes)?;
    super::put_pixel::update_hidden_lanes(
        image,
        &mut retained_lanes,
        pixel_index,
        lane_count,
        lane_values,
        default,
        &fallback,
    )?;
    if !image.replace_hidden_pixel_lanes(retained_lanes) {
        return Err(PilError::InternalError(
            "putdata hidden lane extent did not match its image".into(),
        ));
    }
    Ok(())
}

pub(crate) fn write_luma16(
    samples: &mut [u16],
    pixel_index: usize,
    sample: u16,
) -> Result<(), PilError> {
    let pixel = samples
        .get_mut(pixel_index)
        .ok_or_else(|| PilError::InternalError("putdata I;16 offset out of bounds".into()))?;
    *pixel = sample;
    Ok(())
}

pub(crate) fn write_native_pixel<T: Copy, const CHANNELS: usize>(
    samples: &mut [T],
    pixel_index: usize,
    pixel: [T; CHANNELS],
) -> Result<(), PilError> {
    let start = pixel_index
        .checked_mul(CHANNELS)
        .ok_or_else(|| PilError::DimensionError("putdata native offset overflow".into()))?;
    let end = start
        .checked_add(CHANNELS)
        .ok_or_else(|| PilError::DimensionError("putdata native offset overflow".into()))?;
    let destination = samples
        .get_mut(start..end)
        .ok_or_else(|| PilError::InternalError("putdata native offset out of bounds".into()))?;
    destination.copy_from_slice(&pixel);
    Ok(())
}

/// Writes a sequence of already mode-selected values into native sample lanes.
///
/// The first value is normalized before materialization by the caller. Later
/// values are normalized immediately before their write, preserving partial
/// writes when a later value is invalid.
#[inline]
pub(crate) fn write_native_values<T: Copy, const CHANNELS: usize, F>(
    samples: &mut [T],
    values: &[crate::image::PutDataValue],
    first: [T; CHANNELS],
    mut normalize: F,
    completed: &mut usize,
) -> Result<(), PilError>
where
    F: FnMut(&crate::image::PutDataValue) -> Result<[T; CHANNELS], PilError>,
{
    if CHANNELS == 0 {
        return Err(PilError::InternalError(
            "putdata native channel count was zero".into(),
        ));
    }
    let mut destinations = samples.chunks_exact_mut(CHANNELS);
    for (index, value) in values.iter().enumerate() {
        let pixel = if index == 0 { first } else { normalize(value)? };
        let destination = destinations.next().ok_or_else(|| {
            PilError::InternalError("putdata native offset out of bounds".into())
        })?;
        destination.copy_from_slice(&pixel);
        *completed += 1;
    }
    Ok(())
}
