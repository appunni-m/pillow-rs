//! Native synchronous writes used while the host coerces PutData values.

use crate::error::PilError;
use crate::raster::{DynamicImage, ImageBuffer, Mode, Pixel};

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

pub(crate) fn execute(
    image: &mut DynamicImage,
    mode: Mode,
    pixel_index: usize,
    input: ValueAtInput,
) -> Result<(), PilError> {
    match input {
        ValueAtInput::Bytes(bytes, hidden_lanes) => {
            write_bytes(image, mode, pixel_index, &bytes, hidden_lanes)
        }
        ValueAtInput::Luma16(sample) => write_luma16(image, mode, pixel_index, sample),
    }
}

fn byte_storage_mismatch(mode: Mode) -> &'static str {
    match mode {
        Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 => "putdata L storage mismatch",
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => {
            "putdata LA storage mismatch"
        }
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => {
            "putdata RGB storage mismatch"
        }
        Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageI32
        | Mode::ImageF32 => "putdata RGBA storage mismatch",
        _ => "putdata byte-addressable storage mismatch",
    }
}

fn write_bytes(
    image: &mut DynamicImage,
    mode: Mode,
    pixel_index: usize,
    bytes: &[u8],
    hidden_lanes: Option<[u8; 2]>,
) -> Result<(), PilError> {
    let mismatch = byte_storage_mismatch(mode);
    if image.mode() != mode {
        return Err(PilError::InternalError(mismatch.into()));
    }
    if mode.byte_carrier_pixel_width() != Some(bytes.len()) {
        return Err(PilError::InternalError(mismatch.into()));
    }

    match (mode, image) {
        (Mode::ImageLumaA8, DynamicImage::ImageLumaA8(buffer))
        | (Mode::ImageLaPremultiplied8, DynamicImage::ImageLaPremultiplied8(buffer))
        | (Mode::ImagePaletteAlpha8, DynamicImage::ImagePaletteAlpha8(buffer)) => {
            let lanes = hidden_lanes.ok_or_else(|| {
                PilError::InternalError("putdata LA hidden lanes were missing".into())
            })?;
            write_with_hidden_lanes(
                buffer,
                pixel_index,
                bytes,
                2,
                lanes,
                [bytes[0], bytes[0]],
                |pixel| [pixel.0[0], pixel.0[0]],
            )
        }
        (Mode::ImageRgb8, DynamicImage::ImageRgb8(buffer))
        | (Mode::ImageYCbCr8, DynamicImage::ImageYCbCr8(buffer))
        | (Mode::ImageLab8, DynamicImage::ImageLab8(buffer))
        | (Mode::ImageHsv8, DynamicImage::ImageHsv8(buffer)) => {
            let lane = hidden_lanes.ok_or_else(|| {
                PilError::InternalError("putdata RGB hidden lane was missing".into())
            })?;
            write_with_hidden_lanes(buffer, pixel_index, bytes, 1, lane, [u8::MAX, 0], |_| {
                [u8::MAX, 0]
            })
        }
        (_, image) => {
            if hidden_lanes.is_some() {
                return Err(PilError::InternalError(
                    "putdata hidden lanes did not match its raster mode".into(),
                ));
            }
            let samples = image
                .as_bytes_mut()
                .ok_or_else(|| PilError::InternalError(mismatch.into()))?;
            super::put_pixel::write_native_bytes(samples, pixel_index, bytes)
        }
    }
}

fn write_with_hidden_lanes<P, F>(
    image: &mut ImageBuffer<P, Vec<u8>>,
    pixel_index: usize,
    bytes: &[u8],
    lane_count: usize,
    lane_values: [u8; 2],
    default: [u8; 2],
    fallback: F,
) -> Result<(), PilError>
where
    P: Pixel<Subpixel = u8>,
    F: Fn(&P) -> [u8; 2],
{
    let mut hidden_lanes = image.hidden_pixel_lanes().cloned();
    let samples = image.as_mut();
    super::put_pixel::write_native_bytes(samples, pixel_index, bytes)?;
    super::put_pixel::update_hidden_lanes(
        image,
        &mut hidden_lanes,
        pixel_index,
        lane_count,
        lane_values,
        default,
        &fallback,
    )?;
    if !image.replace_hidden_pixel_lanes(hidden_lanes) {
        return Err(PilError::InternalError(
            "putdata hidden lane extent did not match its image".into(),
        ));
    }
    Ok(())
}

fn write_luma16(
    image: &mut DynamicImage,
    mode: Mode,
    pixel_index: usize,
    sample: u16,
) -> Result<(), PilError> {
    if image.mode() != mode {
        return Err(PilError::InternalError("putdata I;16 storage mismatch".into()));
    }
    let destination = image
        .as_mut_luma16()
        .ok_or_else(|| PilError::InternalError("putdata I;16 storage mismatch".into()))?;
    let pixel = destination
        .as_mut()
        .get_mut(pixel_index)
        .ok_or_else(|| PilError::InternalError("putdata I;16 offset out of bounds".into()))?;
    *pixel = sample;
    Ok(())
}
