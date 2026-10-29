//! Native CMYK-to-luma kernel.

#[cfg(feature = "parallel")]
use super::super::super::simd_rows_mut_serial;
#[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
use super::simd_pack_u16x16;
#[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
use crate::compute::pool_simd::ops::native_channel::gather_interleaved_channel;
use crate::raster::DynamicImage;
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
use fearless_simd::prelude::{SimdBase, SimdInterleaved, SimdNarrow, SimdWiden};
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
use fearless_simd::{Level, Neon, u8x16 as SafeU8x16, u16x8 as SafeU16x8};
#[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
use wide::{u8x16, u16x16};

/// Convert sixteen native C/M/Y/K pixels directly to Pillow-compatible luma.
/// This fuses the exact integer CMYK-to-RGB conversion with the luma formula,
/// so the path never materializes the intermediate RGB image.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[inline(always)]
fn cmyk_muldiv255_neon(
    neon: Neon,
    value: SafeU8x16<Neon>,
    ink: SafeU8x16<Neon>,
) -> SafeU8x16<Neon> {
    let (value_low, value_high) = value.widen();
    let (ink_low, ink_high) = ink.widen();
    let round = |product: SafeU16x8<Neon>| {
        let value = product + SafeU16x8::<Neon>::splat(neon, 128);
        ((value >> 8u32) + value) >> 8u32
    };
    round(value_low * ink_low).narrow(round(value_high * ink_high))
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[inline(always)]
fn cmyk_luma_neon(
    neon: Neon,
    red: SafeU8x16<Neon>,
    green: SafeU8x16<Neon>,
    blue: SafeU8x16<Neon>,
) -> SafeU8x16<Neon> {
    let (red_low, red_high) = red.widen();
    let (green_low, green_high) = green.widen();
    let (blue_low, blue_high) = blue.widen();
    let luma_half = |red: SafeU16x8<Neon>, green: SafeU16x8<Neon>, blue: SafeU16x8<Neon>| {
        // Pillow's exact coefficients split at bit 8. The biased residual
        // stays in 2,933..62,603, and base plus carry stays within u16.
        let base = red * SafeU16x8::<Neon>::splat(neon, 77)
            + green * SafeU16x8::<Neon>::splat(neon, 150)
            + blue * SafeU16x8::<Neon>::splat(neon, 29);
        let residual = green * SafeU16x8::<Neon>::splat(neon, 70)
            + blue * SafeU16x8::<Neon>::splat(neon, 47)
            - red * SafeU16x8::<Neon>::splat(neon, 117)
            + SafeU16x8::<Neon>::splat(neon, 32_768);
        (base + (residual >> 8u32)) >> 8u32
    };
    luma_half(red_low, green_low, blue_low).narrow(luma_half(red_high, green_high, blue_high))
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
fearless_simd::kernel!(
    #[inline(always)]
    fn cmyk_grayscale_block_neon(neon: Neon, source: &[u8; 64]) -> [u8; 16] {
        let [cyan, magenta, yellow, black] = SafeU8x16::<Neon>::load_four_interleaved(neon, source);
        let ink = SafeU8x16::<Neon>::splat(neon, 255) - black;
        let red = ink - cmyk_muldiv255_neon(neon, cyan, ink);
        let green = ink - cmyk_muldiv255_neon(neon, magenta, ink);
        let blue = ink - cmyk_muldiv255_neon(neon, yellow, ink);
        let luma = cmyk_luma_neon(neon, red, green, blue);
        let mut output = [0u8; 16];
        luma.store_array(&mut output);
        output
    }
);

#[inline(always)]
fn cmyk_grayscale_block(source: &[u8]) -> [u8; 16] {
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        let neon = Level::new()
            .as_neon()
            .expect("AArch64 NEON target configuration has a NEON execution token");
        let source = source
            .try_into()
            .expect("complete CMYK grayscale vector block");
        cmyk_grayscale_block_neon(neon, source)
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
