//! Native straight-alpha conversion shared by LA and RGBA carrier modes.

use crate::compute::pool_cpu::ops::color::convert::helpers::wide::pack_u16x16;
use wide::{u8x16, u16x16, u32x16};

pub(crate) fn unpremultiply_block<const CHANNELS: usize>(input: u8x16) -> u8x16 {
    debug_assert!(CHANNELS == 2 || CHANNELS == 4);
    let alpha = input.shuffle(u8x16::new(alpha_indices::<CHANNELS>()));
    let alpha_values = alpha.to_array();
    let reciprocals = u32x16::new(std::array::from_fn(|lane| {
        65_536_u32.checked_div(u32::from(alpha_values[lane])).unwrap_or_default()
    }));
    let alpha_wide = u32x16::from(u16x16::from(alpha));
    let numerator = u32x16::from(u16x16::from(input)) * u32x16::splat(255);
    let estimate = (numerator * reciprocals) >> 16u32;
    let next = (estimate + u32x16::splat(1)) * alpha_wide;
    let round_up = next.simd_le(numerator) & alpha_wide.simd_gt(u32x16::ZERO);
    let exact = estimate + round_up.select(u32x16::splat(1), u32x16::ZERO);
    let bounded = exact
        .simd_gt(u32x16::splat(255))
        .select(u32x16::splat(255), exact);
    let values = u16x16::new(bounded.to_array().map(|value| value as u16));
    let converted = pack_u16x16(values);
    let preserve_alpha = alpha_lane_mask::<CHANNELS>().simd_ne(u8x16::ZERO);
    let zero_alpha = alpha.simd_eq(u8x16::ZERO);
    preserve_alpha.select(input, zero_alpha.select(input, converted))
}

fn alpha_indices<const CHANNELS: usize>() -> [u8; 16] {
    std::array::from_fn(|lane| (lane / CHANNELS * CHANNELS + CHANNELS - 1) as u8)
}

fn alpha_lane_mask<const CHANNELS: usize>() -> u8x16 {
    u8x16::new(std::array::from_fn(|lane| {
        if lane % CHANNELS == CHANNELS - 1 {
            u8::MAX
        } else {
            0
        }
    }))
}
