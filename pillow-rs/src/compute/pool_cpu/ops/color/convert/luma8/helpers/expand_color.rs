//! Archived native Luma8 expansion kernels for RGB and RGBA destinations.

use crate::error::PilError;
use crate::raster::{GrayImage, RgbImage, RgbaImage};
use wide::u8x16;

const LUMA_TO_RGB_0: u8x16 =
    u8x16::new([0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5]);
const LUMA_TO_RGB_1: u8x16 =
    u8x16::new([5, 5, 6, 6, 6, 7, 7, 7, 8, 8, 8, 9, 9, 9, 10, 10]);
const LUMA_TO_RGB_2: u8x16 = u8x16::new([
    10, 11, 11, 11, 12, 12, 12, 13, 13, 13, 14, 14, 14, 15, 15, 15,
]);
const LUMA_TO_RGBA_0: u8x16 =
    u8x16::new([0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3]);
const LUMA_TO_RGBA_1: u8x16 =
    u8x16::new([4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6, 7, 7, 7, 7]);
const LUMA_TO_RGBA_2: u8x16 =
    u8x16::new([8, 8, 8, 8, 9, 9, 9, 9, 10, 10, 10, 10, 11, 11, 11, 11]);
const LUMA_TO_RGBA_3: u8x16 = u8x16::new([
    12, 12, 12, 12, 13, 13, 13, 13, 14, 14, 14, 14, 15, 15, 15, 15,
]);
const RGBA_ALPHA_LANES: u8x16 =
    u8x16::new([0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255]);
const RGBA_COLOR_LANES: u8x16 = u8x16::new([
    255, 255, 255, 0, 255, 255, 255, 0, 255, 255, 255, 0, 255, 255, 255, 0,
]);

pub(crate) fn to_rgb(source: &GrayImage) -> Result<RgbImage, PilError> {
    let pixels = checked_pixels(source)?;
    let output_bytes = pixels
        .checked_mul(3)
        .ok_or_else(|| PilError::DimensionError("RGB Convert output size overflowed".into()))?;
    let mut blocks = Vec::<[[u8; 16]; 3]>::with_capacity(pixels.div_ceil(16));
    let mut vector_blocks = 0u64;
    for start in (0..pixels).step_by(16) {
        let active_pixels = (pixels - start).min(16);
        let mut lanes = [0u8; 16];
        lanes[..active_pixels].copy_from_slice(&source.as_raw()[start..start + active_pixels]);
        let input = u8x16::new(lanes);
        blocks.push([
            input.swizzle_relaxed(LUMA_TO_RGB_0).to_array(),
            input.swizzle_relaxed(LUMA_TO_RGB_1).to_array(),
            input.swizzle_relaxed(LUMA_TO_RGB_2).to_array(),
        ]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let mut output = blocks.into_flattened().into_flattened();
    output.truncate(output_bytes);
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::compute::record_pipeline_operation_path("vector");
    RgbImage::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| PilError::InternalError("RGB Convert output shape mismatch".into()))
}

pub(crate) fn to_rgba(source: &GrayImage) -> Result<RgbaImage, PilError> {
    let pixels = checked_pixels(source)?;
    let output_bytes = pixels
        .checked_mul(4)
        .ok_or_else(|| PilError::DimensionError("RGBA Convert output size overflowed".into()))?;
    let mut blocks = Vec::<[[u8; 16]; 4]>::with_capacity(pixels.div_ceil(16));
    let mut vector_blocks = 0u64;
    for start in (0..pixels).step_by(16) {
        let active_pixels = (pixels - start).min(16);
        let mut lanes = [0u8; 16];
        lanes[..active_pixels].copy_from_slice(&source.as_raw()[start..start + active_pixels]);
        let input = u8x16::new(lanes);
        let vectors = [
            input.swizzle_relaxed(LUMA_TO_RGBA_0).to_array(),
            input.swizzle_relaxed(LUMA_TO_RGBA_1).to_array(),
            input.swizzle_relaxed(LUMA_TO_RGBA_2).to_array(),
            input.swizzle_relaxed(LUMA_TO_RGBA_3).to_array(),
        ];
        blocks.push(vectors.map(|vector| {
            ((u8x16::new(vector) & RGBA_COLOR_LANES) | RGBA_ALPHA_LANES).to_array()
        }));
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let mut output = blocks.into_flattened().into_flattened();
    output.truncate(output_bytes);
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::compute::record_pipeline_operation_path("vector");
    RgbaImage::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| PilError::InternalError("RGBA Convert output shape mismatch".into()))
}

fn checked_pixels(source: &GrayImage) -> Result<usize, PilError> {
    let pixels = (source.width() as usize)
        .checked_mul(source.height() as usize)
        .ok_or_else(|| PilError::DimensionError("Luma Convert dimensions overflowed".into()))?;
    if source.as_raw().len() != pixels {
        return Err(PilError::InternalError(
            "Luma Convert source storage did not match its dimensions".into(),
        ));
    }
    Ok(pixels)
}
