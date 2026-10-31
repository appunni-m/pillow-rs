//! Native RGBA-to-LA vector kernel retained from the archived Convert adapter.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::compute::pool_cpu::helpers::channel::gather::gather_interleaved_channel;
use crate::raster::{GrayAlphaImage, RgbaImage};
use wide::{i16x8, u8x16, u16x8, u16x16};

const LA_DUPLICATE_0: u8x16 = u8x16::new([0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7]);
const LA_DUPLICATE_1: u8x16 = u8x16::new([8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14, 14, 15, 15]);
const LA_LUMA_LANES: u8x16 = u8x16::new([
    u8::MAX, 0, u8::MAX, 0, u8::MAX, 0, u8::MAX, 0, u8::MAX, 0, u8::MAX, 0, u8::MAX, 0, u8::MAX, 0,
]);
const LA_ALPHA_LANES: u8x16 = u8x16::new([
    0, u8::MAX, 0, u8::MAX, 0, u8::MAX, 0, u8::MAX, 0, u8::MAX, 0, u8::MAX, 0, u8::MAX, 0, u8::MAX,
]);

pub(crate) fn execute(source: &RgbaImage) -> Result<(GrayAlphaImage, u64, usize), PilError> {
    let source_dims = CheckedDims::new_allow_empty(source.width(), source.height(), 4)?;
    let output_dims = CheckedDims::new_allow_empty(source.width(), source.height(), 2)?;
    if source.as_raw().len() != source_dims.total_bytes() {
        return Err(PilError::InternalError(
            "RGBA Convert source storage did not match its dimensions".into(),
        ));
    }

    let mut output = Vec::with_capacity(output_dims.total_bytes());
    for source_block in source.as_raw().chunks(64) {
        let active_pixels = source_block.len() / 4;
        let mut padded = [0u8; 64];
        padded[..source_block.len()].copy_from_slice(source_block);
        let blocks = [
            load_block(&padded, 0),
            load_block(&padded, 16),
            load_block(&padded, 32),
            load_block(&padded, 48),
        ];
        let red = u16x16::from(gather_interleaved_channel::<4, 0>(&blocks));
        let green = u16x16::from(gather_interleaved_channel::<4, 1>(&blocks));
        let blue = u16x16::from(gather_interleaved_channel::<4, 2>(&blocks));
        let base = red * u16x16::splat(77) + green * u16x16::splat(150) + blue * u16x16::splat(29);
        let residual = green * u16x16::splat(70) + blue * u16x16::splat(47)
            - red * u16x16::splat(117)
            + u16x16::splat(32768);
        let luma = pack_u16x16((base + (residual >> 8u32)) >> 8u32);
        let alpha = gather_interleaved_channel::<4, 3>(&blocks);
        append_la(&mut output, luma, alpha, active_pixels);
    }
    if output.len() != output_dims.total_bytes() {
        return Err(PilError::InternalError(
            "RGBA Convert luma-alpha output shape mismatch".into(),
        ));
    }
    let allocation_bytes = output.capacity();
    let output = GrayAlphaImage::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| PilError::InternalError("RGBA Convert luma-alpha output shape mismatch".into()))?;
    let vector_blocks = source_dims.total_pixels().div_ceil(8) as u64;
    Ok((output, vector_blocks, allocation_bytes))
}

fn load_block(bytes: &[u8; 64], offset: usize) -> u8x16 {
    let lanes = bytes[offset..offset + 16]
        .try_into()
        .expect("RGBA-to-LA block has sixteen bytes");
    u8x16::new(lanes)
}

fn append_la(output: &mut Vec<u8>, luma: u8x16, alpha: u8x16, active_pixels: usize) {
    let vectors = [LA_DUPLICATE_0, LA_DUPLICATE_1].map(|indices| {
        ((luma.swizzle_relaxed(indices) & LA_LUMA_LANES)
            | (alpha.swizzle_relaxed(indices) & LA_ALPHA_LANES))
            .to_array()
    });
    let first_pixels = active_pixels.min(8);
    output.extend_from_slice(&vectors[0][..first_pixels * 2]);
    let second_pixels = active_pixels.saturating_sub(8).min(8);
    output.extend_from_slice(&vectors[1][..second_pixels * 2]);
}

fn pack_u16x16(value: u16x16) -> u8x16 {
    let [low, high]: [u16x8; 2] = bytemuck::cast(value);
    let low: i16x8 = bytemuck::cast(low);
    let high: i16x8 = bytemuck::cast(high);
    u8x16::narrow_i16x8(low, high)
}
