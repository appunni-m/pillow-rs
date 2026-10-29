//! Shared SIMD arithmetic for native sample operations.

use wide::{i16x8, u8x16, u16x8, u16x16};

pub(super) fn simd_div255(value: u16x16) -> u16x16 {
    // For 0 <= value <= 65025, this is exactly floor(value / 255), including
    // both endpoints.  It replaces an integer divide without changing the
    // intermediate truncation required by ImageChops.multiply/screen.
    // A runtime splat can emit memset_pattern16 in every vector iteration.
    const ONE: u16x16 = u16x16::splat(1);
    let incremented = value + ONE;
    (incremented + (incremented >> 8u32)) >> 8u32
}

#[inline]
pub(super) fn simd_div255_u16x8(value: u16x8) -> u16x8 {
    const ONE: u16x8 = u16x8::splat(1);
    let incremented = value + ONE;
    (incremented + (incremented >> 8u32)) >> 8u32
}

#[inline]
pub(super) fn simd_pack_u16x16(value: u16x16) -> u8x16 {
    let [low, high]: [u16x8; 2] = bytemuck::cast(value);
    let low: i16x8 = bytemuck::cast(low);
    let high: i16x8 = bytemuck::cast(high);
    u8x16::narrow_i16x8(low, high)
}
