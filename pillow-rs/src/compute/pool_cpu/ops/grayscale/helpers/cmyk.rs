//! Shared serial traversal of the original wide CMYK vector algorithm.

use super::interleaved::{gather_interleaved_channel, simd_pack_u16x16};
use wide::{u8x16, u16x16};

#[inline(always)]
fn block(source: &[u8]) -> [u8; 16] {
    let load = |offset| {
        u8x16::new(
            source[offset..offset + 16]
                .try_into()
                .expect("complete CMYK grayscale vector"),
        )
    };
    let blocks = [load(0), load(16), load(32), load(48)];
    let c = u16x16::from(gather_interleaved_channel::<4, 0>(&blocks));
    let m = u16x16::from(gather_interleaved_channel::<4, 1>(&blocks));
    let y = u16x16::from(gather_interleaved_channel::<4, 2>(&blocks));
    let k = u16x16::from(gather_interleaved_channel::<4, 3>(&blocks));
    let nk = u16x16::splat(255) - k;
    let muldiv255 = |value: u16x16| {
        let value = value + u16x16::splat(128);
        ((value >> 8u32) + value) >> 8u32
    };
    let r = nk - muldiv255(c * nk);
    let g = nk - muldiv255(m * nk);
    let b = nk - muldiv255(y * nk);

    // Retain the exact coefficient decomposition and u16 arithmetic order.
    let base = r * const { u16x16::splat(77) }
        + g * const { u16x16::splat(150) }
        + b * const { u16x16::splat(29) };
    let residual = g * const { u16x16::splat(70) } + b * const { u16x16::splat(47) }
        - r * const { u16x16::splat(117) }
        + const { u16x16::splat(32768) };
    simd_pack_u16x16((base + (residual >> 8u32)) >> 8u32).to_array()
}

pub(crate) fn execute(source: &[u8]) -> (Vec<u8>, u64, u64) {
    let mut output = vec![0u8; source.len() / 4];
    let mut vector_blocks = 0u64;
    let mut input_blocks = source.chunks_exact(64);
    let mut output_blocks = output.chunks_exact_mut(16);
    for (input, output) in input_blocks.by_ref().zip(output_blocks.by_ref()) {
        output.copy_from_slice(&block(input));
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let tail = output_blocks.into_remainder();
    if !tail.is_empty() {
        let mut padded = [0u8; 64];
        padded[..input_blocks.remainder().len()].copy_from_slice(input_blocks.remainder());
        tail.copy_from_slice(&block(&padded)[..tail.len()]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    (output, vector_blocks, 0)
}
