//! Native channel extraction kernels shared by SIMD operation adapters.
//!
//! Keep packed-channel layout handling and architecture-specific deinterleave
//! kernels in this family module; operation adapters validate public inputs and
//! delegate the byte movement here.

use wide::u8x16;

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

/// Extract one selected byte from packed RGB with one NEON structure load.
/// `vld3q_u8` deinterleaves 16 RGB pixels directly into three byte vectors,
/// avoiding the temporary lane arrays and three separate swizzles in the
/// generic portable gather.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[allow(unsafe_code)]
#[inline]
fn gather_rgb_channel_neon_into<const CHANNEL: usize>(
    source: &[u8],
    output: &mut [u8],
) -> (u64, u64) {
    const CHANNELS: usize = 3;
    const LANES: usize = 16;
    let pixel_count = source.len() / CHANNELS;
    let vector_pixels = pixel_count / LANES * LANES;
    debug_assert_eq!(output.len(), pixel_count);

    macro_rules! extract_rgb_block {
        ($pixel:expr) => {{
            let input = core::arch::aarch64::vld3q_u8(source.as_ptr().add($pixel * CHANNELS));
            let channel = if CHANNEL == 0 {
                input.0
            } else if CHANNEL == 1 {
                input.1
            } else {
                input.2
            };
            core::arch::aarch64::vst1q_u8(output.as_mut_ptr().add($pixel), channel);
        }};
    }

    // SAFETY: Each structure load starts at `3 * pixel` and reads exactly
    // `3 * LANES` initialized source bytes. Both loops use aligned pixel
    // offsets within `vector_pixels`, which is a multiple of LANES and bounded
    // by `source.len() / 3`; each load and 16-byte store therefore fits its
    // backing allocation. The caller validates an output slice with exactly
    // `pixel_count` initialized writable bytes. The channel index is clamped by
    // `simd_extract_band_into` before this helper is called.
    unsafe {
        let unrolled_pixels = vector_pixels / (LANES * 4) * (LANES * 4);
        for pixel in (0..unrolled_pixels).step_by(LANES * 4) {
            extract_rgb_block!(pixel);
            extract_rgb_block!(pixel + LANES);
            extract_rgb_block!(pixel + LANES * 2);
            extract_rgb_block!(pixel + LANES * 3);
        }
        for pixel in (unrolled_pixels..vector_pixels).step_by(LANES) {
            extract_rgb_block!(pixel);
        }
    }

    for pixel in vector_pixels..pixel_count {
        output[pixel] = source[pixel * CHANNELS + CHANNEL];
    }
    (
        (vector_pixels / LANES) as u64,
        (pixel_count - vector_pixels) as u64,
    )
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[inline]
fn gather_rgb_channel_neon<const CHANNEL: usize>(source: &[u8]) -> (Vec<u8>, u64, u64) {
    let mut output = vec![0; source.len() / 3];
    let (vector_blocks, scalar_tail) = gather_rgb_channel_neon_into::<CHANNEL>(source, &mut output);
    (output, vector_blocks, scalar_tail)
}

/// Extract one selected byte from packed LA with one NEON structure load.
/// `vld2q_u8` deinterleaves sixteen luminance/alpha pixels directly into two
/// byte vectors, so channel extraction writes the result in one pass.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[allow(unsafe_code)]
#[inline]
fn gather_la_channel_neon_into<const CHANNEL: usize>(
    source: &[u8],
    output: &mut [u8],
) -> (u64, u64) {
    const CHANNELS: usize = 2;
    const LANES: usize = 16;
    let pixel_count = source.len() / CHANNELS;
    let vector_pixels = pixel_count / LANES * LANES;
    debug_assert_eq!(output.len(), pixel_count);

    macro_rules! extract_la_block {
        ($pixel:expr) => {{
            let input = core::arch::aarch64::vld2q_u8(source.as_ptr().add($pixel * CHANNELS));
            let channel = if CHANNEL == 0 { input.0 } else { input.1 };
            core::arch::aarch64::vst1q_u8(output.as_mut_ptr().add($pixel), channel);
        }};
    }

    // SAFETY: Each structure load starts at `2 * pixel` and reads exactly
    // `2 * LANES` initialized source bytes. The unrolled and remainder loops
    // visit sixteen-pixel-aligned offsets bounded by `vector_pixels`; each
    // load and 16-byte store fits its allocation. The caller validates an
    // output slice with exactly `pixel_count` writable bytes and selects only
    // channel 0 or 1.
    unsafe {
        let unrolled_pixels = vector_pixels / (LANES * 4) * (LANES * 4);
        for pixel in (0..unrolled_pixels).step_by(LANES * 4) {
            extract_la_block!(pixel);
            extract_la_block!(pixel + LANES);
            extract_la_block!(pixel + LANES * 2);
            extract_la_block!(pixel + LANES * 3);
        }
        for pixel in (unrolled_pixels..vector_pixels).step_by(LANES) {
            extract_la_block!(pixel);
        }
    }

    for pixel in vector_pixels..pixel_count {
        output[pixel] = source[pixel * CHANNELS + CHANNEL];
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
#[allow(unsafe_code)]
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

    macro_rules! extract_rgba_block {
        ($pixel:expr) => {{
            let input = core::arch::aarch64::vld4q_u8(source.as_ptr().add($pixel * CHANNELS));
            let channel = match CHANNEL {
                0 => input.0,
                1 => input.1,
                2 => input.2,
                _ => input.3,
            };
            core::arch::aarch64::vst1q_u8(output.as_mut_ptr().add($pixel), channel);
        }};
    }

    // SAFETY: Each structure load starts at `4 * pixel` and reads exactly
    // `4 * LANES` initialized source bytes. Both loops use sixteen-pixel
    // offsets bounded by `vector_pixels`, so every load and store stays within
    // its allocation. The caller validates an output slice with exactly
    // `pixel_count` writable bytes and clamps the channel to 0..4.
    unsafe {
        let unrolled_pixels = vector_pixels / (LANES * 4) * (LANES * 4);
        for pixel in (0..unrolled_pixels).step_by(LANES * 4) {
            extract_rgba_block!(pixel);
            extract_rgba_block!(pixel + LANES);
            extract_rgba_block!(pixel + LANES * 2);
            extract_rgba_block!(pixel + LANES * 3);
        }
        for pixel in (unrolled_pixels..vector_pixels).step_by(LANES) {
            extract_rgba_block!(pixel);
        }
    }

    for pixel in vector_pixels..pixel_count {
        output[pixel] = source[pixel * CHANNELS + CHANNEL];
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
