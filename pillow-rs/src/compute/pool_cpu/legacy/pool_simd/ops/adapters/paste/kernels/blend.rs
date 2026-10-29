//! Shared masked-byte blend primitives for Paste kernels.

use super::super::NativePasteMaskLayout;
use crate::compute::pool_simd::ops::adapters::{simd_div255, simd_div255_u16x8, simd_pack_u16x16};
use wide::{u8x16, u16x8, u16x16};

/// Gather one mask value per stored source byte. `wide` has no portable
/// byte-gather primitive, so this address calculation is scalar control work;
/// the source/destination arithmetic remains in the vector kernel below.
#[inline]
pub(super) fn native_paste_mask_block<const N: usize>(
    mask: &[u8],
    mask_row_stride: usize,
    source_y: usize,
    source_left: usize,
    byte_start: usize,
    source_channels: usize,
    mask_layout: NativePasteMaskLayout,
) -> [u8; N] {
    if source_channels == 1 && mask_layout.channels == 1 && mask_layout.value_index == 0 {
        let start = source_y * mask_row_stride + source_left + byte_start;
        return mask[start..start + N]
            .try_into()
            .expect("validated one-byte mask vector has the requested width");
    }
    std::array::from_fn(|lane| {
        let source_byte = byte_start + lane;
        let source_pixel = source_byte / source_channels;
        mask[source_y * mask_row_stride
            + (source_left + source_pixel) * mask_layout.channels
            + mask_layout.value_index]
    })
}

#[inline]
pub(super) fn native_paste_mask_block_active<const N: usize>(
    mask: &[u8],
    mask_row_stride: usize,
    source_y: usize,
    source_left: usize,
    byte_start: usize,
    active_bytes: usize,
    source_channels: usize,
    mask_layout: NativePasteMaskLayout,
) -> [u8; N] {
    if source_channels == 1 && mask_layout.channels == 1 && mask_layout.value_index == 0 {
        let start = source_y * mask_row_stride + source_left + byte_start;
        let active_lanes = active_bytes.saturating_sub(byte_start).min(N);
        return std::array::from_fn(|lane| {
            if lane < active_lanes {
                mask[start + lane]
            } else {
                0
            }
        });
    }
    std::array::from_fn(|lane| {
        let source_byte = byte_start + lane;
        if source_byte >= active_bytes {
            return 0;
        }
        let source_pixel = source_byte / source_channels;
        mask[source_y * mask_row_stride
            + (source_left + source_pixel) * mask_layout.channels
            + mask_layout.value_index]
    })
}

#[inline]
pub(super) fn native_paste_blend_vector16(
    source: [u8; 16],
    destination: [u8; 16],
    mask: [u8; 16],
) -> [u8; 16] {
    const MAX: u16x16 = u16x16::splat(255);
    const ROUND_BIAS: u16x16 = u16x16::splat(127);
    let source = u16x16::from(u8x16::new(source));
    let destination = u16x16::from(u8x16::new(destination));
    let mask = u16x16::from(u8x16::new(mask));
    let weighted = source * mask + destination * (MAX - mask) + ROUND_BIAS;
    simd_pack_u16x16(simd_div255(weighted)).to_array()
}

#[inline]
pub(super) fn native_paste_binary_copy_vector16(
    source: [u8; 16],
    destination: [u8; 16],
    mask: [u8; 16],
) -> [u8; 16] {
    u8x16::new(mask)
        .simd_eq(u8x16::splat(0))
        .bitselect(u8x16::new(destination), u8x16::new(source))
        .to_array()
}

#[inline]
pub(super) fn native_paste_blend_vector8(
    source: [u8; 8],
    destination: [u8; 8],
    mask: [u8; 8],
) -> [u8; 8] {
    const MAX: u16x8 = u16x8::splat(255);
    const ROUND_BIAS: u16x8 = u16x8::splat(127);
    let source = u16x8::new(source.map(u16::from));
    let destination = u16x8::new(destination.map(u16::from));
    let mask = u16x8::new(mask.map(u16::from));
    let weighted = source * mask + destination * (MAX - mask) + ROUND_BIAS;
    simd_div255_u16x8(weighted)
        .to_array()
        .map(|value| value as u8)
}

#[inline]
pub(super) fn native_paste_preblend_vector16(
    source: [u8; 16],
    destination: [u8; 16],
    mask: [u8; 16],
) -> [u8; 16] {
    const MAX: u16x16 = u16x16::splat(255);
    const ROUND_BIAS: u16x16 = u16x16::splat(128);
    let source = u16x16::from(u8x16::new(source));
    let destination = u16x16::from(u8x16::new(destination));
    let mask = u16x16::from(u8x16::new(mask));
    let scaled = destination * (MAX - mask) + ROUND_BIAS;
    (simd_div255(scaled) + source)
        .to_array()
        .map(|value| value as u8)
}

#[inline]
pub(super) fn native_paste_preblend_vector8(
    source: [u8; 8],
    destination: [u8; 8],
    mask: [u8; 8],
) -> [u8; 8] {
    const MAX: u16x8 = u16x8::splat(255);
    const ROUND_BIAS: u16x8 = u16x8::splat(128);
    let source = u16x8::new(source.map(u16::from));
    let destination = u16x8::new(destination.map(u16::from));
    let mask = u16x8::new(mask.map(u16::from));
    let scaled = destination * (MAX - mask) + ROUND_BIAS;
    (simd_div255_u16x8(scaled) + source)
        .to_array()
        .map(|value| value as u8)
}

#[inline]
pub(super) fn native_paste_rgb_l_mask_block16(mask: &[u8], byte_start: usize) -> [u8; 16] {
    let pixel_start = byte_start / 3;
    let m0 = mask[pixel_start];
    let m1 = mask[pixel_start + 1];
    let m2 = mask[pixel_start + 2];
    let m3 = mask[pixel_start + 3];
    let m4 = mask[pixel_start + 4];
    let m5 = mask[pixel_start + 5];
    match byte_start % 3 {
        0 => [
            m0, m0, m0, m1, m1, m1, m2, m2, m2, m3, m3, m3, m4, m4, m4, m5,
        ],
        1 => [
            m0, m0, m1, m1, m1, m2, m2, m2, m3, m3, m3, m4, m4, m4, m5, m5,
        ],
        2 => [
            m0, m1, m1, m1, m2, m2, m2, m3, m3, m3, m4, m4, m4, m5, m5, m5,
        ],
        _ => unreachable!("byte phase modulo three is in 0..3"),
    }
}

#[inline]
pub(super) fn native_paste_rgb_l_mask_block8(mask: &[u8], byte_start: usize) -> [u8; 8] {
    let pixel_start = byte_start / 3;
    let phase = byte_start % 3;
    let m0 = mask[pixel_start];
    let m1 = mask[pixel_start + 1];
    let m2 = mask[pixel_start + 2];
    let m3 = if phase == 2 { mask[pixel_start + 3] } else { 0 };
    match phase {
        0 => [m0, m0, m0, m1, m1, m1, m2, m2],
        1 => [m0, m0, m1, m1, m1, m2, m2, m2],
        2 => [m0, m1, m1, m1, m2, m2, m2, m3],
        _ => unreachable!("byte phase modulo three is in 0..3"),
    }
}
