//! Packed RGB/LA/RGBA channel and luma kernels.

use super::simd_pack_u16x16;
use crate::compute::pool_simd::ops::native_channel::gather_interleaved_channel;
use crate::compute::{Architecture, ExecutionContext};
use crate::raster::DynamicImage;
use wide::{u8x16, u16x16};

/// Apply the exact RGB-to-luma integer formula to sixteen packed RGB pixels
/// using NEON's structure load to deinterleave the channels in hardware.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[allow(unsafe_code)]
#[inline]
fn grayscale_rgb_luma_vector_neon(source: &[u8]) -> core::arch::aarch64::uint8x16_t {
    use core::arch::aarch64 as neon;

    debug_assert!(source.len() >= 48);
    macro_rules! luma_half {
        ($r:expr, $g:expr, $b:expr) => {{
            let r = neon::vmovl_u8($r);
            let g = neon::vmovl_u8($g);
            let b = neon::vmovl_u8($b);
            // 19595=77*256-117, 38470=150*256+70,
            // 7471=29*256+47. Wrapping u16 arithmetic on this biased
            // residual yields the exact rounded luma result.
            let base = neon::vaddq_u16(
                neon::vaddq_u16(neon::vmulq_n_u16(r, 77), neon::vmulq_n_u16(g, 150)),
                neon::vmulq_n_u16(b, 29),
            );
            let residual = neon::vaddq_u16(
                neon::vaddq_u16(
                    neon::vsubq_u16(neon::vmulq_n_u16(g, 70), neon::vmulq_n_u16(r, 117)),
                    neon::vmulq_n_u16(b, 47),
                ),
                neon::vdupq_n_u16(32768),
            );
            let carry = neon::vshrq_n_u16(residual, 8);
            let rounded = neon::vshrq_n_u16(neon::vaddq_u16(base, carry), 8);
            neon::vmovn_u16(rounded)
        }};
    }

    // SAFETY: `grayscale_block::<3>` passes complete 48-byte RGB groups, or a
    // zero-padded 64-byte tail buffer. `vld3q_u8` reads exactly 48 initialized
    // bytes. The returned vector contains all sixteen computed luma values.
    unsafe {
        let rgb = neon::vld3q_u8(source.as_ptr());
        let low = luma_half!(
            neon::vget_low_u8(rgb.0),
            neon::vget_low_u8(rgb.1),
            neon::vget_low_u8(rgb.2)
        );
        let high = luma_half!(
            neon::vget_high_u8(rgb.0),
            neon::vget_high_u8(rgb.1),
            neon::vget_high_u8(rgb.2)
        );
        neon::vcombine_u8(low, high)
    }
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[allow(unsafe_code)]
#[inline]
fn grayscale_rgb_block_neon(source: &[u8]) -> [u8; 16] {
    use core::arch::aarch64 as neon;

    let values = grayscale_rgb_luma_vector_neon(source);
    let mut output = [0u8; 16];
    // SAFETY: The local array has sixteen writable bytes and the NEON vector
    // contains exactly sixteen luma samples.
    unsafe { neon::vst1q_u8(output.as_mut_ptr(), values) };
    output
}

/// Store complete NEON RGB blocks directly into the final grayscale vector.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[allow(unsafe_code)]
fn grayscale_rgb_neon_bytes(source: &[u8], pixels: usize) -> (Vec<u8>, u64) {
    use core::arch::aarch64 as neon;

    let vector_pixels = pixels / 16 * 16;
    let mut output = Vec::<u8>::with_capacity(pixels);
    for pixel in (0..vector_pixels).step_by(16) {
        let source_start = pixel * 3;
        let values = grayscale_rgb_luma_vector_neon(&source[source_start..source_start + 48]);
        // SAFETY: `output` reserves at least `pixels` bytes, and each write
        // initializes sixteen bytes at a vector-aligned pixel offset below
        // `vector_pixels`. The source slice above proves the full 48-byte
        // interleaved block exists. `set_len` runs only after all full blocks
        // have been initialized; the partial block is appended afterward.
        unsafe { neon::vst1q_u8(output.as_mut_ptr().add(pixel), values) };
    }
    // SAFETY: Every byte in this vector prefix was initialized by the stores
    // above; `vector_pixels` never exceeds the reserved pixel capacity.
    unsafe { output.set_len(vector_pixels) };

    if vector_pixels < pixels {
        let source_start = vector_pixels * 3;
        let remainder = &source[source_start..];
        let mut padded = [0u8; 48];
        padded[..remainder.len()].copy_from_slice(remainder);
        let tail = grayscale_rgb_block_neon(&padded);
        output.extend_from_slice(&tail[..pixels - vector_pixels]);
    }

    (output, pixels.div_ceil(16) as u64)
}

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

fn grayscale_interleaved<const CHANNELS: usize>(source: &[u8]) -> Vec<u8> {
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

/// Convert admitted native bytes to L. Complete groups use direct vector
/// loads and deinterleaving; only the final partial group needs padding.
pub(in crate::compute::pool_simd::ops::adapters) fn native_grayscale_bytes(
    context: &ExecutionContext,
    img: &DynamicImage,
    channels: usize,
) -> Option<(Vec<u8>, u64, u64)> {
    if !(1..=4).contains(&channels) {
        return None;
    }
    let pixels = (img.width() as usize).checked_mul(img.height() as usize)?;
    let source = img.as_bytes();
    if source.len() != pixels.checked_mul(channels)? {
        return None;
    }
    let (output, vector_blocks) = match channels {
        1 => (source.to_vec(), 0),
        2 => (
            grayscale_interleaved::<2>(source),
            pixels.div_ceil(16) as u64,
        ),
        3 => grayscale_rgb_bytes_for_architecture(context, source, pixels),
        4 => (
            grayscale_interleaved::<4>(source),
            pixels.div_ceil(16) as u64,
        ),
        _ => return None,
    };
    Some((output, vector_blocks, 0))
}

fn grayscale_rgb_bytes_for_architecture(
    context: &ExecutionContext,
    source: &[u8],
    pixels: usize,
) -> (Vec<u8>, u64) {
    match context.architecture() {
        Architecture::Avx2 => {
            #[cfg(target_arch = "x86_64")]
            if let Some(result) = super::rgb_avx2::grayscale_rgb_avx2(context, source, pixels) {
                return result;
            }
        }
        Architecture::Neon => {
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            return grayscale_rgb_neon_bytes(source, pixels);
        }
        _ => {}
    }
    (
        grayscale_interleaved::<3>(source),
        pixels.div_ceil(16) as u64,
    )
}
