//! Native CMYK-to-luma kernel.

#[cfg(feature = "parallel")]
use super::super::super::simd_rows_mut_serial;
#[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
use super::simd_pack_u16x16;
#[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
use crate::compute::pool_simd::ops::native_channel::gather_interleaved_channel;
use crate::raster::DynamicImage;
#[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
use wide::{u8x16, u16x16};

/// Convert sixteen native C/M/Y/K pixels directly to Pillow-compatible luma.
/// This fuses the exact integer CMYK-to-RGB conversion with the luma formula,
/// so the path never materializes the intermediate RGB image.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[allow(unsafe_code)]
#[inline(always)]
fn cmyk_grayscale_block_neon(source: &[u8]) -> [u8; 16] {
    use core::arch::aarch64 as neon;

    macro_rules! muldiv255 {
        ($sample:expr, $ink:expr) => {{
            let product = neon::vaddq_u16(neon::vmulq_u16($sample, $ink), neon::vdupq_n_u16(128));
            let biased = neon::vaddq_u16(neon::vshrq_n_u16(product, 8), product);
            neon::vshrq_n_u16(biased, 8)
        }};
    }
    macro_rules! luma_half {
        ($cyan:expr, $magenta:expr, $yellow:expr, $black:expr $(,)?) => {{
            let cyan = neon::vmovl_u8($cyan);
            let magenta = neon::vmovl_u8($magenta);
            let yellow = neon::vmovl_u8($yellow);
            let black = neon::vmovl_u8($black);
            let ink = neon::vsubq_u16(neon::vdupq_n_u16(255), black);
            let red = neon::vsubq_u16(ink, muldiv255!(cyan, ink));
            let green = neon::vsubq_u16(ink, muldiv255!(magenta, ink));
            let blue = neon::vsubq_u16(ink, muldiv255!(yellow, ink));

            // Pillow's exact luma coefficients split at bit 8. The u16
            // residual uses wrapping subtraction, but its final biased value
            // is 2,933..62,603; base plus carry is also within u16. These
            // lanes therefore need neither widening nor saturation.
            let base = neon::vaddq_u16(
                neon::vaddq_u16(neon::vmulq_n_u16(red, 77), neon::vmulq_n_u16(green, 150)),
                neon::vmulq_n_u16(blue, 29),
            );
            let residual = neon::vaddq_u16(
                neon::vaddq_u16(
                    neon::vsubq_u16(neon::vmulq_n_u16(green, 70), neon::vmulq_n_u16(red, 117)),
                    neon::vmulq_n_u16(blue, 47),
                ),
                neon::vdupq_n_u16(32_768),
            );
            let carry = neon::vshrq_n_u16(residual, 8);
            let rounded = neon::vshrq_n_u16(neon::vaddq_u16(base, carry), 8);
            neon::vmovn_u16(rounded)
        }};
    }

    // SAFETY: This private helper is called only with a complete 64-byte
    // `chunks_exact(64)` block or a 64-byte zero-padded tail. `vld4q_u8` reads
    // exactly those 64 initialized bytes and deinterleaves them into four
    // sixteen-sample channels. The local output array holds sixteen writable
    // bytes for the matching `vst1q_u8` store.
    unsafe {
        let channels = neon::vld4q_u8(source.as_ptr());
        let low = luma_half!(
            neon::vget_low_u8(channels.0),
            neon::vget_low_u8(channels.1),
            neon::vget_low_u8(channels.2),
            neon::vget_low_u8(channels.3),
        );
        let high = luma_half!(
            neon::vget_high_u8(channels.0),
            neon::vget_high_u8(channels.1),
            neon::vget_high_u8(channels.2),
            neon::vget_high_u8(channels.3),
        );
        let pixels = neon::vcombine_u8(low, high);
        let mut output = [0u8; 16];
        neon::vst1q_u8(output.as_mut_ptr(), pixels);
        output
    }
}

#[inline(always)]
fn cmyk_grayscale_block(source: &[u8]) -> [u8; 16] {
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        cmyk_grayscale_block_neon(source)
    }

    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    {
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

        // Exact Pillow luma coefficients, decomposed at bit 8 to keep all
        // intermediates within u16 lanes.
        let base = r * const { u16x16::splat(77) }
            + g * const { u16x16::splat(150) }
            + b * const { u16x16::splat(29) };
        let residual = g * const { u16x16::splat(70) } + b * const { u16x16::splat(47) }
            - r * const { u16x16::splat(117) }
            + const { u16x16::splat(32768) };
        simd_pack_u16x16((base + (residual >> 8u32)) >> 8u32).to_array()
    }
}

#[cfg(feature = "parallel")]
#[inline]
fn cmyk_grayscale_row(source: &[u8], output: &mut [u8]) {
    let mut input_blocks = source.chunks_exact(64);
    let mut output_blocks = output.chunks_exact_mut(16);
    for (input, output) in input_blocks.by_ref().zip(output_blocks.by_ref()) {
        output.copy_from_slice(&cmyk_grayscale_block(input));
    }
    let tail = output_blocks.into_remainder();
    if !tail.is_empty() {
        let mut padded = [0u8; 64];
        padded[..input_blocks.remainder().len()].copy_from_slice(input_blocks.remainder());
        tail.copy_from_slice(&cmyk_grayscale_block(&padded)[..tail.len()]);
    }
}

#[cfg(feature = "parallel")]
const SIMD_CMYK_GRAYSCALE_PARALLEL_PIXEL_THRESHOLD: usize = 512 * 512;

/// Fused native-layout CMYK-to-luma conversion. Complete pixels use sixteen
/// SIMD lanes; the final partial block is padded and writes only active pixels.
pub(in crate::compute::pool_simd::ops::adapters) fn native_cmyk_grayscale_bytes(
    img: &DynamicImage,
) -> Option<(Vec<u8>, u64, u64)> {
    let DynamicImage::ImageRgba8(source) = img else {
        return None;
    };
    let pixels = (img.width() as usize).checked_mul(img.height() as usize)?;
    if source.as_raw().len() != pixels.checked_mul(4)? {
        return None;
    }
    let mut output = vec![0u8; pixels];
    #[cfg(feature = "parallel")]
    if pixels >= SIMD_CMYK_GRAYSCALE_PARALLEL_PIXEL_THRESHOLD {
        let width = img.width() as usize;
        let source_stride = width.checked_mul(4)?;
        let height = img.height() as usize;
        simd_rows_mut_serial!(
            &mut output,
            width,
            height,
            |_row_start, _row_end, y, row| {
                let source_start = y as usize * source_stride;
                cmyk_grayscale_row(
                    &source.as_raw()[source_start..source_start + source_stride],
                    row,
                );
            }
        );
        let blocks_per_row = width.div_ceil(16);
        let vector_blocks = (blocks_per_row as u64).checked_mul(height as u64)?;
        return Some((output, vector_blocks, 0));
    }
    let mut vector_blocks = 0u64;
    let mut input_blocks = source.as_raw().chunks_exact(64);
    let mut output_blocks = output.chunks_exact_mut(16);
    for (input, output) in input_blocks.by_ref().zip(output_blocks.by_ref()) {
        output.copy_from_slice(&cmyk_grayscale_block(input));
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let tail = output_blocks.into_remainder();
    if !tail.is_empty() {
        let mut padded = [0u8; 64];
        padded[..input_blocks.remainder().len()].copy_from_slice(input_blocks.remainder());
        tail.copy_from_slice(&cmyk_grayscale_block(&padded)[..tail.len()]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    Some((output, vector_blocks, 0))
}
