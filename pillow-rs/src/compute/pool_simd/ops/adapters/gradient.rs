//! SIMD adapters for eager linear and radial gradient constructors.

use super::{DynamicImage, PilError, simd_unsupported};
use wide::u8x16;

/// Generate a linear gradient through native vector stores.
///
/// The public `Image.linear_gradient` constructor is eager, so it does not
/// enter the ordinary `PipelineOp` dispatcher.  Its data contract is still a
/// straightforward row fill: every pixel in row `y` carries the scalar value
/// `y`, with the mode-specific sample width retained in the native bytes.  A
/// `u8x16` store is therefore the complete data plane for all supported modes;
/// only the row value and its four-byte encoding are scalar control work.
pub(crate) fn simd_linear_gradient_generate(mode: &str) -> Result<DynamicImage, PilError> {
    let channels = match mode {
        "1" | "L" | "P" => 1,
        "I" | "F" => 4,
        _ => return Err(simd_unsupported("LinearGradient")),
    };
    let row_bytes = 256usize
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("SIMD LinearGradient row length overflow".into()))?;
    let output_len = row_bytes
        .checked_mul(256)
        .ok_or_else(|| PilError::ValueError("SIMD LinearGradient output length overflow".into()))?;
    if mode == "L" {
        let mut output = Vec::with_capacity(output_len);
        let mut vector_blocks = 0u64;
        for y in 0..256usize {
            let block = u8x16::splat(y as u8).to_array();
            let mut row = [0u8; 256];
            for chunk in row.chunks_exact_mut(block.len()) {
                chunk.copy_from_slice(&block);
                vector_blocks = vector_blocks.saturating_add(1);
            }
            output.extend_from_slice(&row);
        }
        crate::compute::record_pipeline_operation_path("vector");
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(0);
        return crate::image_utils::raw_bytes_to_image(256, 256, output, 1);
    }
    let mut output = vec![0u8; output_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;

    for (y, row) in output.chunks_exact_mut(row_bytes).enumerate() {
        let block = match mode {
            "I" => {
                let bytes = (y as i32).to_le_bytes();
                u8x16::new([
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[0], bytes[1], bytes[2], bytes[3],
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[0], bytes[1], bytes[2], bytes[3],
                ])
            }
            "F" => {
                let bytes = (y as f32).to_le_bytes();
                u8x16::new([
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[0], bytes[1], bytes[2], bytes[3],
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[0], bytes[1], bytes[2], bytes[3],
                ])
            }
            "1" if y != 0 => u8x16::splat(0xff),
            _ => u8x16::splat(y as u8),
        };
        let block = block.to_array();
        let vector_len = row.len() / 16 * 16;
        for chunk in row[..vector_len].chunks_exact_mut(16) {
            chunk.copy_from_slice(&block);
            vector_blocks = vector_blocks.saturating_add(1);
        }
        let tail = row.len() - vector_len;
        if tail != 0 {
            row[vector_len..].copy_from_slice(&block[..tail]);
            vector_blocks = vector_blocks.saturating_add(1);
            scalar_tail = scalar_tail.saturating_add(tail as u64);
        }
    }

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(256, 256, output, channels)
}

/// Copy native-L radial-gradient pixels in 16-byte vector blocks.
///
/// The public output is a fixed native-L raster, so generation is compile-time
/// and this backend performs the final image copy in 16-byte vector
/// blocks rather than recomputing each radius on every call.
pub(crate) fn simd_radial_gradient_generate() -> Result<DynamicImage, PilError> {
    let template = &crate::ops::module_fns::RADIAL_GRADIENT_L_TEMPLATE;
    let mut output = Vec::with_capacity(template.len());
    let mut vector_blocks = 0u64;

    for chunk in template.chunks_exact(16) {
        let mut lanes = [0u8; 16];
        lanes.copy_from_slice(chunk);
        let vector = u8x16::new(lanes);
        output.extend_from_slice(&vector.to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::image_utils::raw_bytes_to_image(256, 256, output, 1)
}
