//! PA, RGB, and CMYK Paste row kernels.

use super::{
    native_paste_blend_vector8, native_paste_blend_vector16, native_paste_rgb_l_mask_block8,
    native_paste_rgb_l_mask_block16,
};

/// Blend a native-PA row with an L mask. Palette indices and alpha remain two
/// independent byte samples; this is a PA-specific kernel, not an LA/RGBA
/// conversion or alpha-aware color blend.
#[inline]
pub(super) fn native_paste_pa_masked_row(
    source: &[u8],
    destination: &mut [u8],
    mask: &[u8],
    allow_short_masked_tail: bool,
) {
    debug_assert_eq!(source.len(), destination.len());
    debug_assert_eq!(source.len() % 2, 0);
    debug_assert_eq!(source.len() / 2, mask.len());

    let vector_len16 = source.len() / 16 * 16;
    for start in (0..vector_len16).step_by(16) {
        let source_block = <[u8; 16]>::try_from(&source[start..start + 16])
            .expect("validated PA Paste row has a complete 16-byte block");
        let destination_block = <[u8; 16]>::try_from(&destination[start..start + 16])
            .expect("validated PA Paste row has a complete 16-byte destination block");
        let mask_block = std::array::from_fn(|lane| mask[start / 2 + lane / 2]);
        let blended = native_paste_blend_vector16(source_block, destination_block, mask_block);
        destination[start..start + 16].copy_from_slice(&blended);
    }

    let vector_len8 = source.len() / 8 * 8;
    for start in (vector_len16..vector_len8).step_by(8) {
        let source_block = <[u8; 8]>::try_from(&source[start..start + 8])
            .expect("validated PA Paste row has a complete 8-byte block");
        let destination_block = <[u8; 8]>::try_from(&destination[start..start + 8])
            .expect("validated PA Paste row has a complete 8-byte destination block");
        let mask_block = std::array::from_fn(|lane| mask[start / 2 + lane / 2]);
        let blended = native_paste_blend_vector8(source_block, destination_block, mask_block);
        destination[start..start + 8].copy_from_slice(&blended);
    }

    native_paste_pa_masked_tail(
        source,
        destination,
        mask,
        vector_len8,
        allow_short_masked_tail,
    );
}

#[inline(always)]
fn native_paste_pa_masked_tail(
    source: &[u8],
    destination: &mut [u8],
    mask: &[u8],
    vector_len8: usize,
    allow_short_masked_tail: bool,
) {
    let tail = source.len() - vector_len8;
    if tail == 0 {
        return;
    }
    if allow_short_masked_tail {
        let mut source_block = [0u8; 8];
        let mut destination_block = [0u8; 8];
        source_block[..tail].copy_from_slice(&source[vector_len8..]);
        destination_block[..tail].copy_from_slice(&destination[vector_len8..]);
        let mask_block = std::array::from_fn(|lane| {
            if lane < tail {
                mask[vector_len8 / 2 + lane / 2]
            } else {
                0
            }
        });
        let blended = native_paste_blend_vector8(source_block, destination_block, mask_block);
        destination[vector_len8..].copy_from_slice(&blended[..tail]);
    } else {
        for index in vector_len8..source.len() {
            let mask_value = u16::from(mask[index / 2]);
            let source_value = u16::from(source[index]);
            let destination_value = u16::from(destination[index]);
            #[expect(
                clippy::arithmetic_side_effects,
                reason = "mask is a byte, so subtracting it from 255 cannot underflow"
            )]
            let inverse = 255 - mask_value;
            #[expect(
                clippy::arithmetic_side_effects,
                reason = "the convex byte blend is bounded by 255*255 plus its rounding bias"
            )]
            let weighted = source_value * mask_value + destination_value * inverse + 127;
            #[expect(
                clippy::cast_possible_truncation,
                reason = "DIV255 yields an 8-bit convex blend of byte channels"
            )]
            let blended = (weighted / 255) as u8;
            destination[index] = blended;
        }
    }
}

/// Blend an interleaved RGB row with an L mask without rebuilding mask
/// addresses in the generic channel adapter. Each mask sample covers three
/// adjacent byte lanes, including vector blocks that begin mid-pixel.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[allow(unsafe_code)]
#[inline]
fn native_paste_rgb_l_masked_neon_block(
    source: &[u8],
    destination: &mut [u8],
    mask: &[u8],
    start: usize,
) {
    use core::arch::aarch64 as neon;

    macro_rules! blend_channel {
        ($source:expr, $destination:expr, $mask:expr, $inverse:expr) => {{
            let source = $source;
            let destination = $destination;
            let mask = $mask;
            let inverse = $inverse;
            let weighted_low = neon::vmlal_u8(
                neon::vmull_u8(neon::vget_low_u8(source), neon::vget_low_u8(mask)),
                neon::vget_low_u8(destination),
                neon::vget_low_u8(inverse),
            );
            let weighted_high = neon::vmlal_u8(
                neon::vmull_u8(neon::vget_high_u8(source), neon::vget_high_u8(mask)),
                neon::vget_high_u8(destination),
                neon::vget_high_u8(inverse),
            );
            let rounded_low = neon::vaddq_u16(weighted_low, neon::vdupq_n_u16(128));
            let rounded_high = neon::vaddq_u16(weighted_high, neon::vdupq_n_u16(128));
            let blended_low = neon::vshrq_n_u16(
                neon::vaddq_u16(rounded_low, neon::vshrq_n_u16(rounded_low, 8)),
                8,
            );
            let blended_high = neon::vshrq_n_u16(
                neon::vaddq_u16(rounded_high, neon::vshrq_n_u16(rounded_high, 8)),
                8,
            );
            neon::vcombine_u8(neon::vmovn_u16(blended_low), neon::vmovn_u16(blended_high))
        }};
    }

    // SAFETY: the caller enables this block only for equal RGB source and
    // destination rows whose lengths are divisible by 48, with exactly one
    // L-mask byte per pixel. `start` advances by 48 within that validated
    // prefix, so each load reads 48 RGB bytes or 16 mask bytes and the store
    // writes the same 48-byte destination range. The safe slice borrows keep
    // source, destination, and mask access disjoint for this call.
    unsafe {
        let source_pixels = neon::vld3q_u8(source.as_ptr().add(start));
        let destination_ptr = destination.as_mut_ptr().add(start);
        let destination_pixels = neon::vld3q_u8(destination_ptr);
        let mask_vector = neon::vld1q_u8(mask.as_ptr().add(start / 3));
        let inverse_mask = neon::vsubq_u8(neon::vdupq_n_u8(255), mask_vector);
        let output = neon::uint8x16x3_t(
            blend_channel!(
                source_pixels.0,
                destination_pixels.0,
                mask_vector,
                inverse_mask
            ),
            blend_channel!(
                source_pixels.1,
                destination_pixels.1,
                mask_vector,
                inverse_mask
            ),
            blend_channel!(
                source_pixels.2,
                destination_pixels.2,
                mask_vector,
                inverse_mask
            ),
        );
        neon::vst3q_u8(destination_ptr, output);
    }
}

#[inline]
pub(in crate::compute::pool_simd::ops::adapters) fn native_paste_rgb_l_masked_row(
    source: &[u8],
    destination: &mut [u8],
    mask: &[u8],
    allow_short_masked_tail: bool,
) -> bool {
    debug_assert_eq!(source.len(), destination.len());
    debug_assert_eq!(source.len() % 3, 0);
    debug_assert_eq!(source.len() / 3, mask.len());

    let vector_len16 = source.len() / 16 * 16;
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    let neon_prefix = if source.len() == destination.len()
        && source.len().is_multiple_of(3)
        && source.len() / 3 == mask.len()
    {
        source.len() / 48 * 48
    } else {
        0
    };
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    let neon_prefix = 0usize;
    let used_neon = neon_prefix != 0;
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        let unrolled_len = neon_prefix / 192 * 192;
        for start in (0..unrolled_len).step_by(192) {
            native_paste_rgb_l_masked_neon_block(source, destination, mask, start);
            native_paste_rgb_l_masked_neon_block(source, destination, mask, start + 48);
            native_paste_rgb_l_masked_neon_block(source, destination, mask, start + 96);
            native_paste_rgb_l_masked_neon_block(source, destination, mask, start + 144);
        }
        for start in (unrolled_len..neon_prefix).step_by(48) {
            native_paste_rgb_l_masked_neon_block(source, destination, mask, start);
        }
    }

    for start in (neon_prefix..vector_len16).step_by(16) {
        let source_block = <[u8; 16]>::try_from(&source[start..start + 16])
            .expect("validated RGB Paste row has a complete 16-byte block");
        let destination_block = <[u8; 16]>::try_from(&destination[start..start + 16])
            .expect("validated RGB Paste destination row has a complete 16-byte block");
        let mask_block = native_paste_rgb_l_mask_block16(mask, start);
        let blended = native_paste_blend_vector16(source_block, destination_block, mask_block);
        destination[start..start + 16].copy_from_slice(&blended);
    }

    let vector_len8 = source.len() / 8 * 8;
    for start in (vector_len16.max(neon_prefix)..vector_len8).step_by(8) {
        let source_block = <[u8; 8]>::try_from(&source[start..start + 8])
            .expect("validated RGB Paste row has a complete 8-byte block");
        let destination_block = <[u8; 8]>::try_from(&destination[start..start + 8])
            .expect("validated RGB Paste destination row has a complete 8-byte block");
        let mask_block = native_paste_rgb_l_mask_block8(mask, start);
        let blended = native_paste_blend_vector8(source_block, destination_block, mask_block);
        destination[start..start + 8].copy_from_slice(&blended);
    }

    let tail = source.len() - vector_len8;
    if tail == 0 {
        return used_neon;
    }
    if allow_short_masked_tail {
        let mut source_block = [0u8; 8];
        let mut destination_block = [0u8; 8];
        source_block[..tail].copy_from_slice(&source[vector_len8..]);
        destination_block[..tail].copy_from_slice(&destination[vector_len8..]);
        let mask_block = std::array::from_fn(|lane| {
            if lane < tail {
                mask[(vector_len8 + lane) / 3]
            } else {
                0
            }
        });
        let blended = native_paste_blend_vector8(source_block, destination_block, mask_block);
        destination[vector_len8..].copy_from_slice(&blended[..tail]);
    } else {
        for index in vector_len8..source.len() {
            let mask_value = u16::from(mask[index / 3]);
            let source_value = u16::from(source[index]);
            let destination_value = u16::from(destination[index]);
            #[expect(
                clippy::arithmetic_side_effects,
                reason = "mask is a byte, so subtracting it from 255 cannot underflow"
            )]
            let inverse = 255 - mask_value;
            #[expect(
                clippy::arithmetic_side_effects,
                reason = "the convex byte blend is bounded by 255*255 plus its rounding bias"
            )]
            let weighted = source_value * mask_value + destination_value * inverse + 127;
            #[expect(
                clippy::cast_possible_truncation,
                reason = "DIV255 yields an 8-bit convex blend of byte channels"
            )]
            let blended = (weighted / 255) as u8;
            destination[index] = blended;
        }
    }
    used_neon
}

/// Blend native CMYK pixels with one L-mask byte per pixel. The fourth source
/// lane is K, not alpha; all four stored samples use the same byte blend.
#[inline]
fn native_paste_cmyk_l_masked_tail(
    source: &[u8],
    destination: &mut [u8],
    mask: &[u8],
    vector_len8: usize,
    allow_short_masked_tail: bool,
) -> (u64, u64) {
    let tail = source.len() - vector_len8;
    if tail == 0 {
        return (0, 0);
    }
    if allow_short_masked_tail {
        let mut source_block = [0u8; 8];
        let mut destination_block = [0u8; 8];
        source_block[..tail].copy_from_slice(&source[vector_len8..]);
        destination_block[..tail].copy_from_slice(&destination[vector_len8..]);
        let mask_block = std::array::from_fn(|lane| {
            if lane < tail {
                mask[(vector_len8 + lane) / 4]
            } else {
                0
            }
        });
        let blended = native_paste_blend_vector8(source_block, destination_block, mask_block);
        destination[vector_len8..].copy_from_slice(&blended[..tail]);
        (1, tail as u64)
    } else {
        let mut scalar_tail = 0u64;
        for index in vector_len8..source.len() {
            let mask_value = u16::from(mask[index / 4]);
            let source_value = u16::from(source[index]);
            let destination_value = u16::from(destination[index]);
            #[expect(
                clippy::arithmetic_side_effects,
                reason = "mask is a byte, so subtracting it from 255 cannot underflow"
            )]
            let inverse = 255 - mask_value;
            #[expect(
                clippy::arithmetic_side_effects,
                reason = "the convex byte blend is bounded by 255*255 plus its rounding bias"
            )]
            let weighted = source_value * mask_value + destination_value * inverse + 127;
            #[expect(
                clippy::cast_possible_truncation,
                reason = "DIV255 yields an 8-bit convex blend of byte channels"
            )]
            let blended = (weighted / 255) as u8;
            destination[index] = blended;
            scalar_tail = scalar_tail.saturating_add(1);
        }
        (0, scalar_tail)
    }
}

#[inline]
pub(in crate::compute::pool_simd::ops::adapters) fn native_paste_cmyk_l_masked_row(
    source: &[u8],
    destination: &mut [u8],
    mask: &[u8],
    allow_short_masked_tail: bool,
) -> (u64, u64, bool) {
    debug_assert_eq!(source.len(), destination.len());
    debug_assert_eq!(source.len() % 4, 0);
    debug_assert_eq!(source.len() / 4, mask.len());

    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    let vector_len16 = source.len() / 16 * 16;
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    let neon_prefix = if source.len() == destination.len()
        && source.len().is_multiple_of(4)
        && source.len() / 4 == mask.len()
    {
        source.len() / 64 * 64
    } else {
        0
    };
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    let neon_prefix = 0usize;
    let used_neon = neon_prefix != 0;
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        let unrolled_len = neon_prefix / 256 * 256;
        for start in (0..unrolled_len).step_by(256) {
            native_paste_cmyk_l_masked_neon_block(source, destination, mask, start);
            native_paste_cmyk_l_masked_neon_block(source, destination, mask, start + 64);
            native_paste_cmyk_l_masked_neon_block(source, destination, mask, start + 128);
            native_paste_cmyk_l_masked_neon_block(source, destination, mask, start + 192);
        }
        for start in (unrolled_len..neon_prefix).step_by(64) {
            native_paste_cmyk_l_masked_neon_block(source, destination, mask, start);
        }
        // Each structure block performs four sixteen-lane channel blends.
        vector_blocks = vector_blocks.saturating_add((neon_prefix / 16) as u64);
    }

    for start in (neon_prefix..vector_len16).step_by(16) {
        let source_block = <[u8; 16]>::try_from(&source[start..start + 16])
            .expect("validated CMYK Paste row has a complete 16-byte block");
        let destination_block = <[u8; 16]>::try_from(&destination[start..start + 16])
            .expect("validated CMYK Paste destination row has a complete 16-byte block");
        let first_mask = mask[start / 4];
        let second_mask = mask[start / 4 + 1];
        let third_mask = mask[start / 4 + 2];
        let fourth_mask = mask[start / 4 + 3];
        let mask_block = [
            first_mask,
            first_mask,
            first_mask,
            first_mask,
            second_mask,
            second_mask,
            second_mask,
            second_mask,
            third_mask,
            third_mask,
            third_mask,
            third_mask,
            fourth_mask,
            fourth_mask,
            fourth_mask,
            fourth_mask,
        ];
        let blended = native_paste_blend_vector16(source_block, destination_block, mask_block);
        destination[start..start + 16].copy_from_slice(&blended);
        vector_blocks = vector_blocks.saturating_add(1);
    }

    let vector_len8 = source.len() / 8 * 8;
    for start in (vector_len16..vector_len8).step_by(8) {
        let source_block = <[u8; 8]>::try_from(&source[start..start + 8])
            .expect("validated CMYK Paste row has a complete 8-byte block");
        let destination_block = <[u8; 8]>::try_from(&destination[start..start + 8])
            .expect("validated CMYK Paste destination row has a complete 8-byte block");
        let first_mask = mask[start / 4];
        let second_mask = mask[start / 4 + 1];
        let mask_block = [
            first_mask,
            first_mask,
            first_mask,
            first_mask,
            second_mask,
            second_mask,
            second_mask,
            second_mask,
        ];
        let blended = native_paste_blend_vector8(source_block, destination_block, mask_block);
        destination[start..start + 8].copy_from_slice(&blended);
        vector_blocks = vector_blocks.saturating_add(1);
    }

    let (tail_blocks, tail_bytes) = native_paste_cmyk_l_masked_tail(
        source,
        destination,
        mask,
        vector_len8,
        allow_short_masked_tail,
    );
    vector_blocks = vector_blocks.saturating_add(tail_blocks);
    scalar_tail = scalar_tail.saturating_add(tail_bytes);

    (vector_blocks, scalar_tail, used_neon)
}

/// Blend sixteen native CMYK pixels from one L mask block with NEON.
///
/// The mask stays one byte per pixel while `vld4q_u8` deinterleaves each
/// C/M/Y/K channel into sixteen lanes; K is processed like the other stored
/// samples and is never treated as alpha.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[allow(unsafe_code)]
#[inline]
fn native_paste_cmyk_l_masked_neon_block(
    source: &[u8],
    destination: &mut [u8],
    mask: &[u8],
    start: usize,
) {
    use core::arch::aarch64 as neon;

    macro_rules! blend_half {
        ($source:expr, $destination:expr, $mask:expr, $inverse:expr) => {{
            let weighted = neon::vmlal_u8(neon::vmull_u8($source, $mask), $destination, $inverse);
            // +127 is Pillow's blend rounding; +1 folds in the exact DIV255
            // correction used by the corresponding byte-vector path.
            let incremented = neon::vaddq_u16(weighted, neon::vdupq_n_u16(128));
            let quotient = neon::vshrq_n_u16(
                neon::vaddq_u16(incremented, neon::vshrq_n_u16(incremented, 8)),
                8,
            );
            neon::vmovn_u16(quotient)
        }};
    }
    macro_rules! blend_channel {
        ($source:expr, $destination:expr, $mask:expr, $inverse:expr) => {{
            neon::vcombine_u8(
                blend_half!(
                    neon::vget_low_u8($source),
                    neon::vget_low_u8($destination),
                    neon::vget_low_u8($mask),
                    neon::vget_low_u8($inverse)
                ),
                blend_half!(
                    neon::vget_high_u8($source),
                    neon::vget_high_u8($destination),
                    neon::vget_high_u8($mask),
                    neon::vget_high_u8($inverse)
                ),
            )
        }};
    }

    // SAFETY: `native_paste_cmyk_l_masked_row` enables this loop only after
    // checking equal source/destination lengths, four-byte pixels, and one
    // mask byte per pixel. Its `neon_prefix` is the largest complete 64-byte
    // prefix, and every call advances by 64 bytes within that prefix, so this
    // block accesses exactly 64 bytes from each image and sixteen mask bytes.
    // NEON permits unaligned byte loads and stores. Safe slice borrows keep
    // the mutable destination disjoint from both immutable inputs.
    unsafe {
        let source_pixels = neon::vld4q_u8(source.as_ptr().add(start));
        let destination_ptr = destination.as_mut_ptr().add(start);
        let destination_pixels = neon::vld4q_u8(destination_ptr);
        let mask = neon::vld1q_u8(mask.as_ptr().add(start / 4));
        let inverse = neon::vsubq_u8(neon::vdupq_n_u8(255), mask);
        let output = neon::uint8x16x4_t(
            blend_channel!(source_pixels.0, destination_pixels.0, mask, inverse),
            blend_channel!(source_pixels.1, destination_pixels.1, mask, inverse),
            blend_channel!(source_pixels.2, destination_pixels.2, mask, inverse),
            blend_channel!(source_pixels.3, destination_pixels.3, mask, inverse),
        );
        neon::vst4q_u8(destination_ptr, output);
    }
}
