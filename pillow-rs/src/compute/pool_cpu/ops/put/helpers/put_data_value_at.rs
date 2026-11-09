//! Shared storage helpers for mode-selected synchronous PutData writers.

use crate::error::PilError;
use crate::raster::{ImageBuffer, Pixel};

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
