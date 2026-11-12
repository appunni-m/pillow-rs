//! Archived native vector path for RGBA-to-RGBX opaque padding.

use crate::error::PilError;
use crate::raster::RgbaImage;
use wide::u8x16;

fn alpha_lane_mask(channels: usize) -> u8x16 {
    u8x16::new(std::array::from_fn(|lane| {
        if lane % channels == channels - 1 {
            u8::MAX
        } else {
            0
        }
    }))
}

fn opaque_alpha(input: u8x16, channels: usize) -> u8x16 {
    alpha_lane_mask(channels)
        .simd_ne(u8x16::ZERO)
        .select(u8x16::splat(u8::MAX), input)
}

pub(crate) fn execute(source: &RgbaImage) -> Result<RgbaImage, PilError> {
    let pixels = (source.width() as usize)
        .checked_mul(source.height() as usize)
        .ok_or_else(|| PilError::DimensionError("RGBA Convert dimensions overflowed".into()))?;
    let expected_bytes = pixels
        .checked_mul(4)
        .ok_or_else(|| PilError::DimensionError("RGBA Convert source size overflowed".into()))?;
    if source.as_raw().len() != expected_bytes {
        return Err(PilError::InternalError(
            "RGBA Convert source storage did not match its dimensions".into(),
        ));
    }

    let input = source.as_raw();
    let mut output = Vec::with_capacity(pixels.saturating_mul(4));
    let mut vector_blocks = 0u64;
    for start in (0..input.len()).step_by(16) {
        let active_bytes = (input.len() - start).min(16);
        let active_pixels = active_bytes / 4;
        let source_lanes =
            std::array::from_fn(|lane| input.get(start + lane).copied().unwrap_or(0));
        let lanes = opaque_alpha(u8x16::new(source_lanes), 4).to_array();
        for pixel in lanes.chunks_exact(4).take(active_pixels) {
            output.extend_from_slice(&[pixel[0], pixel[1], pixel[2], u8::MAX]);
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }

    if pixels == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
    } else {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(0);
        crate::compute::record_pipeline_operation_path("vector");
    }
    RgbaImage::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| PilError::InternalError("RGBX Convert output shape mismatch".into()))
}
