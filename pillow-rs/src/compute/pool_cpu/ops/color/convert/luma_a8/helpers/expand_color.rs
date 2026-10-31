//! Archived native LA expansion kernels for RGBA and RGBX destinations.

use crate::error::PilError;
use crate::raster::{GrayAlphaImage, RgbaImage};
use wide::u8x16;

const LA_TO_RGBA_0: u8x16 = u8x16::new([0, 0, 0, 1, 2, 2, 2, 3, 4, 4, 4, 5, 6, 6, 6, 7]);
const LA_TO_RGBA_1: u8x16 = u8x16::new([8, 8, 8, 9, 10, 10, 10, 11, 12, 12, 12, 13, 14, 14, 14, 15]);

pub(crate) fn to_rgba(source: &GrayAlphaImage) -> Result<RgbaImage, PilError> {
    let pixels = checked_pixels(source)?;
    let output_bytes = pixels
        .checked_mul(4)
        .ok_or_else(|| PilError::DimensionError("RGBA Convert output size overflowed".into()))?;
    let mut output = Vec::with_capacity(output_bytes);
    for source_block in source.as_raw().chunks(32) {
        let active_pixels = (pixels - output.len() / 4).min(16);
        let mut padded = [0u8; 32];
        padded[..source_block.len()].copy_from_slice(source_block);
        let low_lanes: [u8; 16] = padded[..16]
            .try_into()
            .map_err(|_| PilError::InternalError("LA vector low extent mismatch".into()))?;
        let high_lanes: [u8; 16] = padded[16..]
            .try_into()
            .map_err(|_| PilError::InternalError("LA vector high extent mismatch".into()))?;
        let low = u8x16::new(low_lanes);
        let high = u8x16::new(high_lanes);
        let vectors = [
            low.swizzle_relaxed(LA_TO_RGBA_0).to_array(),
            low.swizzle_relaxed(LA_TO_RGBA_1).to_array(),
            high.swizzle_relaxed(LA_TO_RGBA_0).to_array(),
            high.swizzle_relaxed(LA_TO_RGBA_1).to_array(),
        ];
        for (block_index, vector) in vectors.into_iter().enumerate() {
            let block_pixels = active_pixels.saturating_sub(block_index * 4).min(4);
            output.extend_from_slice(&vector[..block_pixels * 4]);
        }
    }
    crate::compute::record_pipeline_operation_vector_blocks(pixels.div_ceil(4) as u64);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::compute::record_pipeline_operation_path("vector");
    RgbaImage::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| PilError::InternalError("RGBA Convert output shape mismatch".into()))
}

pub(crate) fn to_rgbx(source: &GrayAlphaImage) -> Result<RgbaImage, PilError> {
    let pixels = checked_pixels(source)?;
    let input = source.as_raw();
    let mut output = Vec::with_capacity(input.len().saturating_mul(2));
    let mut vector_blocks = 0u64;
    for start in (0..input.len()).step_by(16) {
        let active_bytes = (input.len() - start).min(16);
        let active_pixels = active_bytes / 2;
        let lanes = std::array::from_fn(|lane| input.get(start + lane).copied().unwrap_or(0));
        let source_block = u8x16::new(lanes);
        let first = source_block.swizzle_relaxed(LA_TO_RGBA_0).to_array();
        let second = source_block.swizzle_relaxed(LA_TO_RGBA_1).to_array();
        let first_pixels = active_pixels.min(4);
        output.extend_from_slice(&first[..first_pixels * 4]);
        let second_pixels = active_pixels.saturating_sub(4).min(4);
        output.extend_from_slice(&second[..second_pixels * 4]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    debug_assert_eq!(output.len(), pixels.saturating_mul(4));
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::compute::record_pipeline_operation_path("vector");
    RgbaImage::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| PilError::InternalError("RGBX Convert output shape mismatch".into()))
}

fn checked_pixels(source: &GrayAlphaImage) -> Result<usize, PilError> {
    let pixels = (source.width() as usize)
        .checked_mul(source.height() as usize)
        .ok_or_else(|| PilError::DimensionError("LA Convert dimensions overflowed".into()))?;
    let expected_bytes = pixels
        .checked_mul(2)
        .ok_or_else(|| PilError::DimensionError("LA Convert source size overflowed".into()))?;
    if source.as_raw().len() != expected_bytes {
        return Err(PilError::InternalError(
            "LA Convert source storage did not match its dimensions".into(),
        ));
    }
    Ok(pixels)
}
