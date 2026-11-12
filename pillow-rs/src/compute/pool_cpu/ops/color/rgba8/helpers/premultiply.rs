//! Native RGBA-to-RGBa premultiplication from the archived alpha kernel.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::RgbaImage;
use wide::{i16x8, u8x16, u16x8, u16x16};

pub(crate) fn execute(source: &RgbaImage) -> Result<(RgbaImage, u64, usize), PilError> {
    let dims = CheckedDims::new_allow_empty(source.width(), source.height(), 4)?;
    if source.as_raw().len() != dims.total_bytes() {
        return Err(PilError::InternalError(
            "RGBA Convert source storage did not match its dimensions".into(),
        ));
    }

    let mut output = Vec::with_capacity(dims.total_bytes());
    let mut vector_blocks = 0u64;
    for block in source.as_raw().chunks(16) {
        let active = block.len();
        let mut padded = [0u8; 16];
        padded[..active].copy_from_slice(block);
        let converted = premultiply_block(u8x16::new(padded)).to_array();
        output.extend_from_slice(&converted[..active]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let allocation_bytes = output.capacity();
    let output = RgbaImage::from_raw(source.width(), source.height(), output).ok_or_else(|| {
        PilError::InternalError("RGBA Convert premultiply output shape mismatch".into())
    })?;
    Ok((output, vector_blocks, allocation_bytes))
}

fn alpha_indices() -> [u8; 16] {
    std::array::from_fn(|lane| (lane / 4 * 4 + 3) as u8)
}

fn alpha_lane_mask() -> u8x16 {
    u8x16::new(std::array::from_fn(
        |lane| {
            if lane % 4 == 3 { u8::MAX } else { 0 }
        },
    ))
}

fn premultiply_block(input: u8x16) -> u8x16 {
    let alpha = input.shuffle(u8x16::new(alpha_indices()));
    let values = u16x16::from(input);
    let alpha = u16x16::from(alpha);
    let scaled = div255(values * alpha + u16x16::splat(127));
    let converted = pack_u16(scaled);
    alpha_lane_mask()
        .simd_eq(u8x16::ZERO)
        .select(converted, input)
}

fn div255(value: u16x16) -> u16x16 {
    let incremented = value + u16x16::splat(1);
    (incremented + (incremented >> 8u32)) >> 8u32
}

fn pack_u16(value: u16x16) -> u8x16 {
    let [low, high]: [u16x8; 2] = bytemuck::cast(value);
    let low: i16x8 = bytemuck::cast(low);
    let high: i16x8 = bytemuck::cast(high);
    u8x16::narrow_i16x8(low, high)
}
