//! Archived native vector loops shared by RGB, LA, and four-byte color strategies.
//!
//! These helpers receive a compile-time channel layout. They do not infer mode,
//! select architecture, inspect backend state, or schedule rows.

use wide::{u8x16, u16x16};

pub(super) use crate::compute::pool_cpu::helpers::channel::gather::gather_interleaved_channel;

pub(super) use crate::compute::pool_cpu::helpers::channel::wide::simd_pack_u16x16;

#[inline(always)]
fn grayscale_block<const CHANNELS: usize>(source: &[u8]) -> [u8; 16] {
    let load = |offset| {
        u8x16::new(
            source[offset..offset + 16]
                .try_into()
                .expect("complete grayscale vector"),
        )
    };
    let blocks = [
        load(0),
        load(16),
        if CHANNELS >= 3 {
            load(32)
        } else {
            u8x16::splat(0)
        },
        if CHANNELS == 4 {
            load(48)
        } else {
            u8x16::splat(0)
        },
    ];
    let red = gather_interleaved_channel::<CHANNELS, 0>(&blocks);
    if CHANNELS == 2 {
        return red.to_array();
    }
    let r = u16x16::from(red);
    let g = u16x16::from(gather_interleaved_channel::<CHANNELS, 1>(&blocks));
    let b = u16x16::from(gather_interleaved_channel::<CHANNELS, 2>(&blocks));
    // Decompose the exact coefficients at bit 8, retaining their residual:
    // 19595=77*256-117, 38470=150*256+70, 7471=29*256+47.
    // The biased residual is 2933..62603; wrapping u16 operations therefore
    // produce its exact positive value. Base plus carry stays <=65524.
    // Exhaustive live-Pillow verification covers all 2^24 RGB inputs.
    // Force compile-time vector constants: runtime splat construction on
    // Apple ARM emitted seven memset_pattern16 calls per sixteen pixels.
    let base = r * const { u16x16::splat(77) }
        + g * const { u16x16::splat(150) }
        + b * const { u16x16::splat(29) };
    let residual = g * const { u16x16::splat(70) } + b * const { u16x16::splat(47) }
        - r * const { u16x16::splat(117) }
        + const { u16x16::splat(32768) };
    simd_pack_u16x16((base + (residual >> 8u32)) >> 8u32).to_array()
}

pub(crate) fn grayscale_interleaved<const CHANNELS: usize>(source: &[u8]) -> Vec<u8> {
    debug_assert!(matches!(CHANNELS, 2 | 3 | 4));
    debug_assert_eq!(source.len() % CHANNELS, 0);
    let mut inputs = source.chunks_exact(16 * CHANNELS);
    let mut output = Vec::with_capacity(source.len() / CHANNELS);
    for input in inputs.by_ref() {
        output.extend_from_slice(&grayscale_block::<CHANNELS>(input));
    }
    let remainder = inputs.remainder();
    let active_pixels = remainder.len() / CHANNELS;
    if active_pixels != 0 {
        let mut padded = [0u8; 64];
        padded[..remainder.len()].copy_from_slice(remainder);
        let block = grayscale_block::<CHANNELS>(&padded);
        output.extend_from_slice(&block[..active_pixels]);
    }
    output
}

