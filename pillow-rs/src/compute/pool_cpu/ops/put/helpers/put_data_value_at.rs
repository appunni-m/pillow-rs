//! Native synchronous writes used while the host coerces PutData values.

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) enum ValueAtInput {
    Bytes(Vec<u8>),
    Luma16(u16),
}

pub(crate) fn execute(
    image: &mut DynamicImage,
    mode: Mode,
    pixel_index: usize,
    input: ValueAtInput,
) -> Result<(), PilError> {
    match input {
        ValueAtInput::Bytes(bytes) => write_bytes(image, mode, pixel_index, &bytes),
        ValueAtInput::Luma16(sample) => write_luma16(image, mode, pixel_index, sample),
    }
}

fn byte_storage_mismatch(mode: Mode) -> &'static str {
    match mode {
        Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 => "putdata L storage mismatch",
        Mode::ImageLumaA8 | Mode::ImagePaletteAlpha8 => "putdata LA storage mismatch",
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageHsv8 => {
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
) -> Result<(), PilError> {
    let mismatch = byte_storage_mismatch(mode);
    if image.mode() != mode {
        return Err(PilError::InternalError(mismatch.into()));
    }
    let samples = image
        .as_bytes_mut()
        .ok_or_else(|| PilError::InternalError(mismatch.into()))?;
    super::put_pixel::write_native_bytes(samples, pixel_index, bytes)
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
