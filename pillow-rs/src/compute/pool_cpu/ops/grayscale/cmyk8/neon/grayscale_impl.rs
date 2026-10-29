//! Original CMYK vector arithmetic with its NEON token injected at preparation.

use crate::raster::RgbaImage;
use super::super::default::grayscale_impl as scalar;
use fearless_simd::prelude::{SimdBase, SimdInterleaved, SimdNarrow, SimdWiden};
use fearless_simd::{Neon, u8x16 as SafeU8x16, u16x8 as SafeU16x8};

pub(crate) struct Kernel<'a> {
    input: &'a RgbaImage,
    neon: Neon,
}

pub(crate) fn prepare(input: scalar::Kernel<'_>, neon: Neon) -> Kernel<'_> {
    Kernel { input: input.input, neon }
}

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
fn block(neon: Neon, source: &[u8]) -> [u8; 16] {
    let source = source.try_into().expect("complete CMYK grayscale vector block");
    cmyk_grayscale_block_neon(neon, source)
}

pub(crate) fn dimensions(input: &Kernel<'_>) -> (u32, u32) {
    input.input.dimensions()
}

pub(crate) fn execute(input: Kernel<'_>) -> (Vec<u8>, u64, u64) {
    let source = input.input.as_raw();
    let pixels = source.len() / 4;
    let mut output = vec![0u8; pixels];
    let mut vector_blocks = 0u64;
    let mut input_blocks = source.chunks_exact(64);
    let mut output_blocks = output.chunks_exact_mut(16);
    for (source, output) in input_blocks.by_ref().zip(output_blocks.by_ref()) {
        output.copy_from_slice(&block(input.neon, source));
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let tail = output_blocks.into_remainder();
    if !tail.is_empty() {
        let mut padded = [0u8; 64];
        padded[..input_blocks.remainder().len()].copy_from_slice(input_blocks.remainder());
        tail.copy_from_slice(&block(input.neon, &padded)[..tail.len()]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    (output, vector_blocks, 0)
}
