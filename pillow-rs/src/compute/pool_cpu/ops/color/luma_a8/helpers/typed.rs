//! Native LA to Pillow I/F scalar conversion from the archived typed lanes.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{GrayAlphaImage, Mode};
use wide::{f32x8, u32x8};

pub(crate) fn to_i32(source: &GrayAlphaImage) -> Result<(Vec<u8>, u64), PilError> {
    let (pixels, output_bytes) = checked_layout(source)?;
    let mut output = Vec::with_capacity(output_bytes);
    let mut vector_blocks = 0u64;
    for start in (0..pixels).step_by(8) {
        let active = (pixels - start).min(8);
        let lanes = std::array::from_fn(|lane| {
            if lane < active {
                u32::from(source.as_raw()[(start + lane) * 2])
            } else {
                0
            }
        });
        for value in u32x8::new(lanes).to_array().into_iter().take(active) {
            output.extend_from_slice(&(value as i32).to_le_bytes());
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    finish(output, output_bytes, vector_blocks, Mode::ImageI32)
}

pub(crate) fn to_f32(source: &GrayAlphaImage) -> Result<(Vec<u8>, u64), PilError> {
    let (pixels, output_bytes) = checked_layout(source)?;
    let mut output = Vec::with_capacity(output_bytes);
    let mut vector_blocks = 0u64;
    for start in (0..pixels).step_by(8) {
        let active = (pixels - start).min(8);
        let lanes = std::array::from_fn(|lane| {
            if lane < active {
                f32::from(source.as_raw()[(start + lane) * 2])
            } else {
                0.0
            }
        });
        for value in f32x8::new(lanes).to_array().into_iter().take(active) {
            output.extend_from_slice(&value.to_le_bytes());
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    finish(output, output_bytes, vector_blocks, Mode::ImageF32)
}

fn checked_layout(source: &GrayAlphaImage) -> Result<(usize, usize), PilError> {
    let dims = CheckedDims::new_allow_empty(source.width(), source.height(), 2)?;
    if source.as_raw().len() != dims.total_bytes() {
        return Err(PilError::InternalError(
            "LA Convert source storage did not match its dimensions".into(),
        ));
    }
    let output_bytes = dims
        .total_pixels()
        .checked_mul(4)
        .ok_or_else(|| PilError::DimensionError("typed Convert output size overflowed".into()))?;
    Ok((dims.total_pixels(), output_bytes))
}

fn finish(
    output: Vec<u8>,
    expected_bytes: usize,
    vector_blocks: u64,
    mode: Mode,
) -> Result<(Vec<u8>, u64), PilError> {
    if output.len() != expected_bytes {
        return Err(PilError::InternalError(format!(
            "LA Convert {} output shape mismatch",
            mode.pillow_name().unwrap_or("unknown")
        )));
    }
    Ok((output, vector_blocks))
}
