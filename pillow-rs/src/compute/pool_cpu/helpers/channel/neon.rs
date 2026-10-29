//! Original native NEON gathers with the capability token supplied by preparation.

use wide::u8x16;
use fearless_simd::prelude::{SimdBase, SimdInterleaved};
use fearless_simd::{Neon, u8x16 as SafeU8x16};
use super::gather::gather_interleaved_channel;

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
fearless_simd::kernel!(
    #[inline(always)]
    fn store_rgba_channel_neon(neon: Neon, channel: usize, source: &[u8], output: &mut [u8]) {
        let (source_blocks, source_tail) = source.as_chunks::<64>();
        let (output_blocks, output_tail) = output.as_chunks_mut::<16>();
        debug_assert!(source_tail.is_empty());
        debug_assert!(output_tail.is_empty());
        for (source_block, output_block) in source_blocks.iter().zip(output_blocks) {
            let channels = SafeU8x16::<Neon>::load_four_interleaved(neon, source_block);
            let selected = match channel {
                0 => channels[0],
                1 => channels[1],
                2 => channels[2],
                _ => channels[3],
            };
            selected.store_array(output_block);
        }
    }
);

/// Extract one selected byte from packed RGB with safe SIMD lane selection.
/// Each vector block gathers sixteen samples from three contiguous source
/// vectors, and the scalar tail handles only the remaining pixels.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[inline]
fn gather_rgb_channel_neon_into<const CHANNEL: usize>(
    source: &[u8],
    output: &mut [u8],
) -> (u64, u64) {
    const LANES: usize = 16;
    const CHANNELS: usize = 3;
    let pixel_count = source.len() / 3;
    debug_assert_eq!(output.len(), pixel_count);
    debug_assert_eq!(source.len() % CHANNELS, 0);
    debug_assert!(CHANNEL < CHANNELS);

    let (source_blocks, source_tail) = source.as_chunks::<48>();
    let (output_blocks, output_tail) = output.as_chunks_mut::<LANES>();
    for (source_block, output_block) in source_blocks.iter().zip(output_blocks) {
        let source_vectors = [
            u8x16::new(
                source_block[..LANES]
                    .try_into()
                    .expect("complete RGB vector"),
            ),
            u8x16::new(
                source_block[LANES..LANES * 2]
                    .try_into()
                    .expect("complete RGB vector"),
            ),
            u8x16::new(
                source_block[LANES * 2..]
                    .try_into()
                    .expect("complete RGB vector"),
            ),
            u8x16::splat(0),
        ];
        let channel = gather_interleaved_channel::<CHANNELS, CHANNEL>(&source_vectors);
        output_block.copy_from_slice(&channel.to_array());
    }

    for (pixel, output_sample) in source_tail.chunks_exact(CHANNELS).zip(output_tail) {
        *output_sample = pixel[CHANNEL];
    }
    (
        (source_blocks.len() * LANES) as u64 / LANES as u64,
        (pixel_count % LANES) as u64,
    )
}


/// Extract one selected byte from packed LA with safe SIMD lane selection.
/// Each vector block gathers sixteen luminance or alpha samples from two
/// contiguous source vectors.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[inline]
fn gather_la_channel_neon_into<const CHANNEL: usize>(
    source: &[u8],
    output: &mut [u8],
) -> (u64, u64) {
    const LANES: usize = 16;
    const LANES_BYTES: usize = LANES * 2;
    let pixel_count = source.len() / 2;
    let vector_pixels = pixel_count / LANES * LANES;
    debug_assert_eq!(output.len(), pixel_count);

    let (source_blocks, source_tail) = source.as_chunks::<LANES_BYTES>();
    let (output_blocks, output_tail) = output.as_chunks_mut::<LANES>();
    debug_assert!(source_tail.len() < LANES_BYTES);
    debug_assert!(output_tail.len() < LANES);
    for (source_block, output_block) in source_blocks.iter().zip(output_blocks) {
        let source_vectors = [
            u8x16::new(
                source_block[..LANES]
                    .try_into()
                    .expect("complete LA vector"),
            ),
            u8x16::new(
                source_block[LANES..]
                    .try_into()
                    .expect("complete LA vector"),
            ),
            u8x16::splat(0),
            u8x16::splat(0),
        ];
        let channel = gather_interleaved_channel::<2, CHANNEL>(&source_vectors);
        output_block.copy_from_slice(&channel.to_array());
    }

    for (pixel, output_sample) in source_tail.chunks_exact(2).zip(output_tail) {
        *output_sample = pixel[CHANNEL];
    }
    (
        (vector_pixels / LANES) as u64,
        (pixel_count - vector_pixels) as u64,
    )
}


/// Extract one selected byte from packed four-channel pixels with one NEON
/// structure load. `vld4q_u8` deinterleaves sixteen pixels directly into four
/// byte vectors, including CMYK and other four-byte native carriers.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[inline]
fn gather_rgba_channel_neon_into<const CHANNEL: usize>(
    neon: Neon,
    source: &[u8],
    output: &mut [u8],
) -> (u64, u64) {
    const CHANNELS: usize = 4;
    const LANES: usize = 16;
    let pixel_count = source.len() / CHANNELS;
    let vector_pixels = pixel_count / LANES * LANES;
    debug_assert_eq!(output.len(), pixel_count);
    store_rgba_channel_neon(
        neon,
        CHANNEL,
        &source[..vector_pixels * CHANNELS],
        &mut output[..vector_pixels],
    );

    for (pixel, output_sample) in source[vector_pixels * CHANNELS..]
        .chunks_exact(CHANNELS)
        .zip(&mut output[vector_pixels..])
    {
        *output_sample = pixel[CHANNEL];
    }
    (
        (vector_pixels / LANES) as u64,
        (pixel_count - vector_pixels) as u64,
    )
}


pub(crate) fn extract_channel_into<const CHANNELS: usize>(
    source: &[u8], output: &mut [u8], channel: usize, neon: Neon,
) -> (u64, u64) {
    match (CHANNELS, channel) {
        (2, 0) => gather_la_channel_neon_into::<0>(source, output),
        (2, 1) => gather_la_channel_neon_into::<1>(source, output),
        (3, 0) => gather_rgb_channel_neon_into::<0>(source, output),
        (3, 1) => gather_rgb_channel_neon_into::<1>(source, output),
        (3, 2) => gather_rgb_channel_neon_into::<2>(source, output),
        (4, 0) => gather_rgba_channel_neon_into::<0>(neon, source, output),
        (4, 1) => gather_rgba_channel_neon_into::<1>(neon, source, output),
        (4, 2) => gather_rgba_channel_neon_into::<2>(neon, source, output),
        (4, 3) => gather_rgba_channel_neon_into::<3>(neon, source, output),
        _ => unreachable!("native extraction validates channel layout and index"),
    }
}
