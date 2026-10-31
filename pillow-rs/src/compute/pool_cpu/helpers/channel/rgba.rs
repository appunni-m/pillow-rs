//! Portable SIMD shuffles for native RGBA-related byte layouts.

use wide::u8x16;

const RGB_TO_RGBA_SHUFFLE: [u8; 16] = [
    0, 1, 2, 12, 3, 4, 5, 13, 6, 7, 8, 14, 9, 10, 11, 15,
];
const RGBA_TO_RGB_SHUFFLE: [u8; 16] = [
    0, 1, 2, 4, 5, 6, 8, 9, 10, 12, 13, 14, 0, 0, 0, 0,
];

/// Inserts an opaque or supplied alpha byte into one four-pixel RGB block.
#[inline(always)]
pub(crate) fn rgb_to_rgba_block(source: &[u8], alpha: u8) -> [u8; 16] {
    debug_assert_eq!(source.len(), 12);
    let mut padded = [alpha; 16];
    padded[..12].copy_from_slice(source);
    u8x16::new(padded)
        .swizzle_relaxed(u8x16::new(RGB_TO_RGBA_SHUFFLE))
        .to_array()
}

/// Drops alpha from up to four native RGBA pixels into one RGB block.
#[inline(always)]
pub(crate) fn rgba_to_rgb_block(source: &[u8]) -> [u8; 12] {
    debug_assert!(source.len() <= 16 && source.len() % 4 == 0);
    let mut padded = [0u8; 16];
    padded[..source.len()].copy_from_slice(source);
    let shuffled = u8x16::new(padded)
        .swizzle_relaxed(u8x16::new(RGBA_TO_RGB_SHUFFLE))
        .to_array();
    let mut output = [0u8; 12];
    output.copy_from_slice(&shuffled[..12]);
    output
}
