//! Native RGB-to-RGBA vector conversion for the exact RGB8 source mode.

use crate::error::PilError;
use crate::raster::{RgbImage, RgbaImage};
use wide::u8x16;

const RGB_TO_RGBA_BYTES: u8x16 =
    u8x16::new([0, 1, 2, 15, 3, 4, 5, 15, 6, 7, 8, 15, 9, 10, 11, 15]);

pub(crate) fn execute(source: &RgbImage) -> Result<RgbaImage, PilError> {
    let pixels = (source.width() as usize)
        .checked_mul(source.height() as usize)
        .ok_or_else(|| PilError::DimensionError("RGB Convert dimensions overflowed".into()))?;
    let source_bytes = pixels
        .checked_mul(3)
        .ok_or_else(|| PilError::DimensionError("RGB Convert source size overflowed".into()))?;
    let output_bytes = pixels
        .checked_mul(4)
        .filter(|&bytes| bytes <= isize::MAX as usize)
        .ok_or_else(|| PilError::DimensionError("RGBA Convert output size overflowed".into()))?;
    if source.as_raw().len() != source_bytes {
        return Err(PilError::InternalError(
            "RGB Convert source storage did not match its dimensions".into(),
        ));
    }

    let vector_pixels = pixels / 4 * 4;
    let mut output = Vec::with_capacity(output_bytes);
    for pixel in (0..vector_pixels).step_by(4) {
        output.extend_from_slice(&rgb_to_rgba_block(source.as_raw(), pixel, 4));
    }
    for pixel in vector_pixels..pixels {
        let offset = pixel * 3;
        output.extend_from_slice(&[
            source.as_raw()[offset],
            source.as_raw()[offset + 1],
            source.as_raw()[offset + 2],
            u8::MAX,
        ]);
    }

    crate::compute::record_pipeline_operation_vector_blocks((vector_pixels / 4) as u64);
    crate::compute::record_pipeline_operation_scalar_tail((pixels - vector_pixels) as u64);
    crate::compute::record_pipeline_operation_path("vector");
    RgbaImage::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| PilError::InternalError("RGBA Convert output shape mismatch".into()))
}

#[inline]
fn rgb_to_rgba_block(source: &[u8], start_pixel: usize, active_pixels: usize) -> [u8; 16] {
    debug_assert!((1..=4).contains(&active_pixels));
    let source_start = start_pixel * 3;
    let active_bytes = active_pixels * 3;
    let mut input = [0u8; 16];
    input[..active_bytes].copy_from_slice(&source[source_start..source_start + active_bytes]);
    input[15] = u8::MAX;
    u8x16::new(input)
        .swizzle_relaxed(RGB_TO_RGBA_BYTES)
        .to_array()
}
