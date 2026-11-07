//! Portable wide-vector layouts for RGBA and RGBX source pixels.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{RgbImage, RgbaImage};
use wide::u8x16;

const DROP_FOURTH_BYTE_LANES: u8x16 =
    u8x16::new([0, 1, 2, 4, 5, 6, 8, 9, 10, 12, 13, 14, 0, 0, 0, 0]);
const IDENTITY_LANES: u8x16 =
    u8x16::new([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);

pub(crate) fn drop_fourth_byte(
    source: &RgbaImage,
    mode_name: &'static str,
) -> Result<(RgbImage, u64, usize), PilError> {
    let (pixels, _) = checked_source(source, mode_name)?;
    let output_dims = CheckedDims::new_allow_empty(source.width(), source.height(), 3)?;
    let mut output = Vec::with_capacity(output_dims.total_bytes());
    let mut vector_blocks = 0u64;
    for start in (0..pixels).step_by(4) {
        let active = (pixels - start).min(4);
        let padded = padded_four_byte_block(source, start, active);
        let block = u8x16::new(padded)
            .shuffle(DROP_FOURTH_BYTE_LANES)
            .to_array();
        output.extend_from_slice(&block[..active * 3]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let allocation_bytes = output.capacity();
    let output = RgbImage::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| {
            PilError::InternalError(format!("{mode_name} Convert RGB output shape mismatch"))
        })?;
    Ok((output, vector_blocks, allocation_bytes))
}

pub(crate) fn copy_four_byte_pixels(
    source: &RgbaImage,
) -> Result<(RgbaImage, u64, usize), PilError> {
    let (pixels, source_bytes) = checked_source(source, "RGBA")?;
    let mut output = Vec::with_capacity(source_bytes);
    let mut vector_blocks = 0u64;
    for start in (0..pixels).step_by(4) {
        let active = (pixels - start).min(4);
        let active_bytes = active * 4;
        let padded = padded_four_byte_block(source, start, active);
        let block = u8x16::new(padded).shuffle(IDENTITY_LANES).to_array();
        output.extend_from_slice(&block[..active_bytes]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let allocation_bytes = output.capacity();
    let output = RgbaImage::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| {
            PilError::InternalError("RGBA Convert identity output shape mismatch".into())
        })?;
    Ok((output, vector_blocks, allocation_bytes))
}

fn checked_source(
    source: &RgbaImage,
    mode_name: &'static str,
) -> Result<(usize, usize), PilError> {
    let dimensions = CheckedDims::new_allow_empty(source.width(), source.height(), 4)?;
    if source.as_raw().len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            format!("{mode_name} Convert source storage did not match its dimensions"),
        ));
    }
    Ok((dimensions.total_pixels(), dimensions.total_bytes()))
}

fn padded_four_byte_block(source: &RgbaImage, start: usize, active: usize) -> [u8; 16] {
    let mut padded = [0u8; 16];
    let input_start = start * 4;
    let active_bytes = active * 4;
    padded[..active_bytes]
        .copy_from_slice(&source.as_raw()[input_start..input_start + active_bytes]);
    padded
}
