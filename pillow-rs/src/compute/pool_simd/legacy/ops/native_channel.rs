//! Native channel extraction kernels shared by SIMD operation adapters.
//!
//! Keep packed-channel layout handling and architecture-specific deinterleave
//! kernels in this family module; operation adapters validate public inputs and
//! delegate the byte movement here.

use wide::u8x16;

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
use fearless_simd::prelude::{SimdBase, SimdInterleaved};
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
use fearless_simd::{Level, Neon, u8x16 as SafeU8x16};

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

#[cfg(target_arch = "x86_64")]
#[path = "native_channel/cmyk_ssse3.rs"]
mod cmyk_ssse3;

#[cfg(target_arch = "x86_64")]
pub(crate) use cmyk_ssse3::deinterleave_cmyk_16_ssse3;

/// Gather one byte channel from a small set of interleaved SIMD blocks.
///
/// This helper is shared by grayscale, alpha, and CMYK kernels. It performs
/// only the lane selection; callers own the operation's arithmetic and output.
#[inline(always)]
pub(crate) fn gather_interleaved_channel<const CHANNELS: usize, const CHANNEL: usize>(
    blocks: &[u8x16; 4],
) -> u8x16 {
    let mut samples = u8x16::splat(0);
    for (block, bytes) in blocks.iter().enumerate().take(CHANNELS) {
        let select = u8x16::new(std::array::from_fn(|lane| {
            let index = lane * CHANNELS + CHANNEL;
            if index / 16 == block {
                (index % 16) as u8
            } else {
                0x80
            }
        }));
        samples |= bytes.swizzle_relaxed(select);
    }
    samples
}

#[inline(always)]
pub(crate) fn gather_channel_into<const CHANNELS: usize, const CHANNEL: usize>(
    source: &[u8],
    output: &mut [u8],
) -> (u64, u64) {
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    if CHANNELS == 2 {
        return gather_la_channel_neon_into::<CHANNEL>(source, output);
    }
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    if CHANNELS == 3 {
        return gather_rgb_channel_neon_into::<CHANNEL>(source, output);
    }
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    if CHANNELS == 4 {
        return gather_rgba_channel_neon_into::<CHANNEL>(source, output);
    }
    #[cfg(target_arch = "x86_64")]
    if let Some((vector_blocks, scalar_tail)) =
        crate::color::gather_interleaved_channel_ssse3_into(source, output, CHANNELS, CHANNEL)
    {
        return (vector_blocks, scalar_tail);
    }
    let (gathered, vector_blocks, scalar_tail) =
        crate::compute::pool_cpu::ops::imageops::gather_native_channel::<CHANNELS, CHANNEL>(source);
    output.copy_from_slice(&gathered);
    (vector_blocks, scalar_tail)
}

#[inline(always)]
pub(crate) fn gather_channel<const CHANNELS: usize, const CHANNEL: usize>(
    source: &[u8],
) -> (Vec<u8>, u64, u64) {
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    if CHANNELS == 2 {
        return gather_la_channel_neon::<CHANNEL>(source);
    }
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    if CHANNELS == 3 {
        return gather_rgb_channel_neon::<CHANNEL>(source);
    }
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    if CHANNELS == 4 {
        return gather_rgba_channel_neon::<CHANNEL>(source);
    }
    crate::compute::pool_cpu::ops::imageops::gather_native_channel::<CHANNELS, CHANNEL>(source)
}

/// Return one RGB channel in newly allocated, fully initialized storage.
/// Const-specialized dispatch keeps channel selection outside the pixel loop.
pub(crate) fn extract_rgb_channel_bytes(
    source: &[u8],
    channel: usize,
) -> Option<(Vec<u8>, u64, u64)> {
    match channel {
        0 => Some(gather_channel::<3, 0>(source)),
        1 => Some(gather_channel::<3, 1>(source)),
        2 => Some(gather_channel::<3, 2>(source)),
        _ => None,
    }
}

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

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[inline]
fn gather_rgb_channel_neon<const CHANNEL: usize>(source: &[u8]) -> (Vec<u8>, u64, u64) {
    let pixel_count = source.len() / 3;
    let mut output = vec![0; pixel_count];
    let (vector_blocks, scalar_tail) = gather_rgb_channel_neon_into::<CHANNEL>(source, &mut output);
    (output, vector_blocks, scalar_tail)
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

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[inline]
fn gather_la_channel_neon<const CHANNEL: usize>(source: &[u8]) -> (Vec<u8>, u64, u64) {
    let mut output = vec![0; source.len() / 2];
    let (vector_blocks, scalar_tail) = gather_la_channel_neon_into::<CHANNEL>(source, &mut output);
    (output, vector_blocks, scalar_tail)
}

/// Extract one selected byte from packed four-channel pixels with one NEON
/// structure load. `vld4q_u8` deinterleaves sixteen pixels directly into four
/// byte vectors, including CMYK and other four-byte native carriers.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[inline]
fn gather_rgba_channel_neon_into<const CHANNEL: usize>(
    source: &[u8],
    output: &mut [u8],
) -> (u64, u64) {
    const CHANNELS: usize = 4;
    const LANES: usize = 16;
    let pixel_count = source.len() / CHANNELS;
    let vector_pixels = pixel_count / LANES * LANES;
    debug_assert_eq!(output.len(), pixel_count);
    let Some(neon) = Level::new().as_neon() else {
        for (pixel, output_sample) in source.chunks_exact(CHANNELS).zip(output) {
            *output_sample = pixel[CHANNEL];
        }
        return (0, pixel_count as u64);
    };
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

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[inline]
fn gather_rgba_channel_neon<const CHANNEL: usize>(source: &[u8]) -> (Vec<u8>, u64, u64) {
    let mut output = vec![0; source.len() / 4];
    let (vector_blocks, scalar_tail) =
        gather_rgba_channel_neon_into::<CHANNEL>(source, &mut output);
    (output, vector_blocks, scalar_tail)
}

/// Dispatch one validated native channel extraction to its const-specialized
/// kernel. The adapter checks layout, channel index, and buffer lengths first.
pub(crate) fn extract_channel_into(
    source: &[u8],
    output: &mut [u8],
    channels: usize,
    channel: usize,
) -> (u64, u64) {
    match (channels, channel) {
        (1, _) => {
            output.copy_from_slice(source);
            (0, 0)
        }
        (2, 0) => gather_channel_into::<2, 0>(source, output),
        (2, 1) => gather_channel_into::<2, 1>(source, output),
        (3, 0) => gather_channel_into::<3, 0>(source, output),
        (3, 1) => gather_channel_into::<3, 1>(source, output),
        (3, 2) => gather_channel_into::<3, 2>(source, output),
        (4, 0) => gather_channel_into::<4, 0>(source, output),
        (4, 1) => gather_channel_into::<4, 1>(source, output),
        (4, 2) => gather_channel_into::<4, 2>(source, output),
        (4, 3) => gather_channel_into::<4, 3>(source, output),
        _ => unreachable!("native extraction validates channel layout and index"),
    }
}

#[cfg(test)]
mod tests {
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    #[test]
    fn extract_band_rgb_neon_deinterleave_covers_blocks_and_tail() {
        let pixel_count = 37;
        let raw: Vec<u8> = (0..pixel_count * 3)
            .map(|index| u8::try_from((index * 73 + 19) % 256).expect("sample byte fits"))
            .collect();

        for channel in 0..3 {
            let (output, vector_blocks, scalar_tail) = match channel {
                0 => super::gather_rgb_channel_neon::<0>(&raw),
                1 => super::gather_rgb_channel_neon::<1>(&raw),
                2 => super::gather_rgb_channel_neon::<2>(&raw),
                _ => unreachable!(),
            };
            let expected: Vec<u8> = raw.chunks_exact(3).map(|pixel| pixel[channel]).collect();

            assert_eq!(output, expected, "RGB channel {channel}");
            assert_eq!(vector_blocks, 2, "RGB channel {channel}");
            assert_eq!(scalar_tail, 5, "RGB channel {channel}");
        }
    }
}
