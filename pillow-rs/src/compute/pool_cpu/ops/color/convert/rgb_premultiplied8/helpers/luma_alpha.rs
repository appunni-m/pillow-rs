//! Native RGBa-to-LA kernel retaining stored premultiplied color samples.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{GrayAlphaImage, RgbaImage};
use wide::{u8x16, u32x8};

const DUPLICATE: u8x16 = u8x16::new([0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7]);
const LUMA_LANES: u8x16 = u8x16::new([
    u8::MAX, 0, u8::MAX, 0, u8::MAX, 0, u8::MAX, 0, u8::MAX, 0, u8::MAX, 0, u8::MAX, 0, u8::MAX, 0,
]);
const ALPHA_LANES: u8x16 = u8x16::new([
    0, u8::MAX, 0, u8::MAX, 0, u8::MAX, 0, u8::MAX, 0, u8::MAX, 0, u8::MAX, 0, u8::MAX, 0, u8::MAX,
]);

pub(crate) fn execute(source: &RgbaImage) -> Result<(GrayAlphaImage, u64, usize), PilError> {
    let source_dims = CheckedDims::new_allow_empty(source.width(), source.height(), 4)?;
    let output_dims = CheckedDims::new_allow_empty(source.width(), source.height(), 2)?;
    if source.as_raw().len() != source_dims.total_bytes() {
        return Err(PilError::InternalError(
            "RGBa Convert source storage did not match its dimensions".into(),
        ));
    }

    let mut output = Vec::with_capacity(output_dims.total_bytes());
    let mut vector_blocks = 0u64;
    for start in (0..source_dims.total_pixels()).step_by(8) {
        let active = (source_dims.total_pixels() - start).min(8);
        let red = rgb_lane(source.as_raw(), start, active, 0);
        let green = rgb_lane(source.as_raw(), start, active, 1);
        let blue = rgb_lane(source.as_raw(), start, active, 2);
        let alpha = rgb_lane(source.as_raw(), start, active, 3);
        let luma = ((u32x8::new(red.map(u32::from)) * u32x8::splat(19595)
            + u32x8::new(green.map(u32::from)) * u32x8::splat(38470)
            + u32x8::new(blue.map(u32::from)) * u32x8::splat(7471)
            + u32x8::splat(32768))
            >> 16u32)
            .to_array()
            .map(|value| value.min(255) as u8);
        append_luma_alpha(&mut output, luma, alpha, active);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    if output.len() != output_dims.total_bytes() {
        return Err(PilError::InternalError(
            "RGBa Convert luma-alpha output shape mismatch".into(),
        ));
    }
    let allocation_bytes = output.capacity();
    let output = GrayAlphaImage::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| PilError::InternalError("RGBa Convert luma-alpha output shape mismatch".into()))?;
    Ok((output, vector_blocks, allocation_bytes))
}

fn rgb_lane(source: &[u8], start: usize, active: usize, channel: usize) -> [u8; 8] {
    std::array::from_fn(|lane| {
        if lane < active {
            source[(start + lane) * 4 + channel]
        } else {
            0
        }
    })
}

fn append_luma_alpha(output: &mut Vec<u8>, luma: [u8; 8], alpha: [u8; 8], active: usize) {
    let mut luma_lanes = [0u8; 16];
    luma_lanes[..8].copy_from_slice(&luma);
    let mut alpha_lanes = [0u8; 16];
    alpha_lanes[..8].copy_from_slice(&alpha);
    let interleaved = (u8x16::new(luma_lanes).swizzle_relaxed(DUPLICATE) & LUMA_LANES)
        | (u8x16::new(alpha_lanes).swizzle_relaxed(DUPLICATE) & ALPHA_LANES);
    output.extend_from_slice(&interleaved.to_array()[..active * 2]);
}
