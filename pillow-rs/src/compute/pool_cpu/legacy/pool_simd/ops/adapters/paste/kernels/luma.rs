//! L, LA, and RGBA-with-L-mask Paste row kernels.

use super::{native_paste_blend_vector8, native_paste_blend_vector16};

/// Blend a clipped native-L row with the same-width L mask. This single-byte
/// kernel avoids rebuilding per-lane mask addresses in the generic channel
/// adapter and is independent across destination rows.
#[inline]
pub(super) fn native_paste_l_masked_row(
    source: &[u8],
    destination: &mut [u8],
    mask: &[u8],
    allow_short_masked_tail: bool,
) -> bool {
    debug_assert_eq!(source.len(), destination.len());
    debug_assert_eq!(source.len(), mask.len());

    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        let neon_prefix = native_paste_l_masked_row_neon(source, destination, mask);
        if neon_prefix != 0 {
            // Pillow rounds the weighted sum before dividing by 255. Keep the
            // scalar tail on that same exact integer formula; it is bounded
            // to fewer than sixteen bytes and cannot read beyond the clipped
            // row.
            for index in neon_prefix..source.len() {
                let weight = u16::from(mask[index]);
                let inverse = 255 - weight;
                let weighted = u16::from(source[index]) * weight
                    + u16::from(destination[index]) * inverse
                    + 127;
                destination[index] = (weighted / 255) as u8;
            }
            return true;
        }
    }

    let vector_len16 = source.len() / 16 * 16;
    for start in (0..vector_len16).step_by(16) {
        let source_block = <[u8; 16]>::try_from(&source[start..start + 16])
            .expect("validated L Paste row has a complete 16-byte block");
        let destination_block = <[u8; 16]>::try_from(&destination[start..start + 16])
            .expect("validated L Paste row has a complete 16-byte destination block");
        let mask_block = <[u8; 16]>::try_from(&mask[start..start + 16])
            .expect("validated L Paste mask row has a complete 16-byte block");
        let blended = native_paste_blend_vector16(source_block, destination_block, mask_block);
        destination[start..start + 16].copy_from_slice(&blended);
    }

    let vector_len8 = source.len() / 8 * 8;
    for start in (vector_len16..vector_len8).step_by(8) {
        let source_block = <[u8; 8]>::try_from(&source[start..start + 8])
            .expect("validated L Paste row has a complete 8-byte block");
        let destination_block = <[u8; 8]>::try_from(&destination[start..start + 8])
            .expect("validated L Paste row has a complete 8-byte destination block");
        let mask_block = <[u8; 8]>::try_from(&mask[start..start + 8])
            .expect("validated L Paste mask row has a complete 8-byte block");
        let blended = native_paste_blend_vector8(source_block, destination_block, mask_block);
        destination[start..start + 8].copy_from_slice(&blended);
    }

    if allow_short_masked_tail {
        let tail = source.len() - vector_len8;
        if tail != 0 {
            // Zero inactive lanes so the padded vector cannot read beyond any
            // clipped source, destination, or mask row.
            let mut source_block = [0u8; 8];
            let mut destination_block = [0u8; 8];
            let mut mask_block = [0u8; 8];
            source_block[..tail].copy_from_slice(&source[vector_len8..]);
            destination_block[..tail].copy_from_slice(&destination[vector_len8..]);
            mask_block[..tail].copy_from_slice(&mask[vector_len8..]);
            let blended = native_paste_blend_vector8(source_block, destination_block, mask_block);
            destination[vector_len8..].copy_from_slice(&blended[..tail]);
        }
    } else {
        for index in vector_len8..source.len() {
            let mask_value = u16::from(mask[index]);
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
    false
}

/// Blend one clipped native RGBA row with one L-mask sample per pixel.
/// Expanding the four mask samples into each vector block avoids the generic
/// adapter's per-byte channel division and repeated mask loads. The fourth
/// stored RGBA byte is alpha and follows the same exact Pillow blend.
#[inline]
pub(super) fn native_paste_rgba_l_masked_row(
    source: &[u8],
    destination: &mut [u8],
    mask: &[u8],
    allow_short_masked_tail: bool,
) -> (u64, u64, bool) {
    debug_assert_eq!(source.len(), destination.len());
    debug_assert_eq!(source.len() % 4, 0);
    debug_assert_eq!(source.len() / 4, mask.len());

    let vector_len16 = source.len() / 16 * 16;
    let mut vector_blocks = 0u64;
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    let neon_prefix = vector_len16;
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    let neon_prefix = 0usize;
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        let unrolled_len = neon_prefix / 64 * 64;
        for start in (0..unrolled_len).step_by(64) {
            native_paste_rgba_l_masked_neon_block(source, destination, mask, start);
            native_paste_rgba_l_masked_neon_block(source, destination, mask, start + 16);
            native_paste_rgba_l_masked_neon_block(source, destination, mask, start + 32);
            native_paste_rgba_l_masked_neon_block(source, destination, mask, start + 48);
        }
        for start in (unrolled_len..neon_prefix).step_by(16) {
            native_paste_rgba_l_masked_neon_block(source, destination, mask, start);
        }
        vector_blocks = vector_blocks.saturating_add((neon_prefix / 16) as u64);
    }

    for start in (neon_prefix..vector_len16).step_by(16) {
        let source_block = <[u8; 16]>::try_from(&source[start..start + 16])
            .expect("validated RGBA Paste row has a complete 16-byte block");
        let destination_block = <[u8; 16]>::try_from(&destination[start..start + 16])
            .expect("validated RGBA destination row has a complete 16-byte block");
        let pixel = start / 4;
        let m0 = mask[pixel];
        let m1 = mask[pixel + 1];
        let m2 = mask[pixel + 2];
        let m3 = mask[pixel + 3];
        let mask_block = [
            m0, m0, m0, m0, m1, m1, m1, m1, m2, m2, m2, m2, m3, m3, m3, m3,
        ];
        let blended = native_paste_blend_vector16(source_block, destination_block, mask_block);
        destination[start..start + 16].copy_from_slice(&blended);
        vector_blocks = vector_blocks.saturating_add(1);
    }

    let vector_len8 = source.len() / 8 * 8;
    for start in (vector_len16..vector_len8).step_by(8) {
        let source_block = <[u8; 8]>::try_from(&source[start..start + 8])
            .expect("validated RGBA Paste row has a complete 8-byte block");
        let destination_block = <[u8; 8]>::try_from(&destination[start..start + 8])
            .expect("validated RGBA destination row has a complete 8-byte block");
        let pixel = start / 4;
        let m0 = mask[pixel];
        let m1 = mask[pixel + 1];
        let mask_block = [m0, m0, m0, m0, m1, m1, m1, m1];
        let blended = native_paste_blend_vector8(source_block, destination_block, mask_block);
        destination[start..start + 8].copy_from_slice(&blended);
        vector_blocks = vector_blocks.saturating_add(1);
    }

    let tail = source.len() - vector_len8;
    let mut scalar_tail = 0u64;
    if tail != 0 && allow_short_masked_tail {
        // RGBA rows leave at most one complete four-byte pixel here. Pad its
        // second vector half with zero so the short row stays on SIMD without
        // reading past any input or writing outside the active pixel.
        let mut source_block = [0u8; 8];
        let mut destination_block = [0u8; 8];
        let mut mask_block = [0u8; 8];
        source_block[..tail].copy_from_slice(&source[vector_len8..]);
        destination_block[..tail].copy_from_slice(&destination[vector_len8..]);
        let mask_value = mask[vector_len8 / 4];
        mask_block[..tail].fill(mask_value);
        let blended = native_paste_blend_vector8(source_block, destination_block, mask_block);
        destination[vector_len8..].copy_from_slice(&blended[..tail]);
        vector_blocks = vector_blocks.saturating_add(1);
        scalar_tail = tail as u64;
    } else if tail != 0 {
        let mask_value = u16::from(mask[vector_len8 / 4]);
        let inverse = 255 - mask_value;
        for channel in 0..4 {
            let index = vector_len8 + channel;
            let weighted = u16::from(source[index]) * mask_value
                + u16::from(destination[index]) * inverse
                + 127;
            destination[index] = (weighted / 255) as u8;
            scalar_tail = scalar_tail.saturating_add(1);
        }
    }

    (vector_blocks, scalar_tail, neon_prefix != 0)
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[allow(unsafe_code)]
#[inline]
fn native_paste_rgba_l_masked_neon_block(
    source: &[u8],
    destination: &mut [u8],
    mask: &[u8],
    start: usize,
) {
    use core::arch::aarch64 as neon;

    macro_rules! blend_half {
        ($source:expr, $destination:expr, $mask:expr, $inverse:expr) => {{
            let weighted = neon::vmlal_u8(neon::vmull_u8($source, $mask), $destination, $inverse);
            let rounded = neon::vaddq_u16(weighted, neon::vdupq_n_u16(127));
            let incremented = neon::vaddq_u16(rounded, neon::vdupq_n_u16(1));
            let quotient = neon::vshrq_n_u16(
                neon::vaddq_u16(incremented, neon::vshrq_n_u16(incremented, 8)),
                8,
            );
            neon::vmovn_u16(quotient)
        }};
    }

    let pixel = start / 4;
    let m0 = mask[pixel];
    let m1 = mask[pixel + 1];
    let m2 = mask[pixel + 2];
    let m3 = mask[pixel + 3];
    let expanded_mask = [
        m0, m0, m0, m0, m1, m1, m1, m1, m2, m2, m2, m2, m3, m3, m3, m3,
    ];

    // SAFETY: The caller only enters this block for a complete sixteen-byte
    // prefix of the source and destination, which corresponds to four valid
    // mask bytes. The expanded local mask is exactly sixteen bytes. NEON
    // loads/stores permit unaligned pointers; Rust's borrows keep the output
    // disjoint from the immutable source and mask slices.
    unsafe {
        let source = neon::vld1q_u8(source.as_ptr().add(start));
        let destination_ptr = destination.as_mut_ptr().add(start);
        let destination = neon::vld1q_u8(destination_ptr);
        let mask = neon::vld1q_u8(expanded_mask.as_ptr());
        let inverse = neon::vsubq_u8(neon::vdupq_n_u8(255), mask);
        let output = neon::vcombine_u8(
            blend_half!(
                neon::vget_low_u8(source),
                neon::vget_low_u8(destination),
                neon::vget_low_u8(mask),
                neon::vget_low_u8(inverse)
            ),
            blend_half!(
                neon::vget_high_u8(source),
                neon::vget_high_u8(destination),
                neon::vget_high_u8(mask),
                neon::vget_high_u8(inverse)
            ),
        );
        neon::vst1q_u8(destination_ptr, output);
    }
}

/// Blend one complete sixteen-byte L row block with NEON's widening byte
/// multiplies. The identity `(x + 1 + ((x + 1) >> 8)) >> 8` is exact for the
/// rounded blend numerator (at most 65,152), so this preserves Pillow's
/// integer division by 255 while keeping all sixteen output lanes vectorized.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[allow(unsafe_code)]
#[inline]
fn native_paste_l_masked_neon_block(
    source: &[u8],
    destination: &mut [u8],
    mask: &[u8],
    start: usize,
) {
    use core::arch::aarch64 as neon;

    macro_rules! blend_half {
        ($source:expr, $destination:expr, $mask:expr, $inverse:expr) => {{
            let weighted = neon::vmlal_u8(neon::vmull_u8($source, $mask), $destination, $inverse);
            let rounded = neon::vaddq_u16(weighted, neon::vdupq_n_u16(127));
            let incremented = neon::vaddq_u16(rounded, neon::vdupq_n_u16(1));
            let quotient = neon::vshrq_n_u16(
                neon::vaddq_u16(incremented, neon::vshrq_n_u16(incremented, 8)),
                8,
            );
            neon::vmovn_u16(quotient)
        }};
    }

    // SAFETY: The caller bounds `start` to a complete sixteen-byte prefix of
    // all three equally sized slices. NEON's byte loads and store allow
    // unaligned pointers. The mutable destination slice is disjoint from the
    // immutable inputs by Rust's borrowing rules, and only the sixteen bytes
    // in this block are written.
    unsafe {
        let source = neon::vld1q_u8(source.as_ptr().add(start));
        let destination_ptr = destination.as_mut_ptr().add(start);
        let destination = neon::vld1q_u8(destination_ptr);
        let mask = neon::vld1q_u8(mask.as_ptr().add(start));
        let inverse = neon::vsubq_u8(neon::vdupq_n_u8(255), mask);
        let output = neon::vcombine_u8(
            blend_half!(
                neon::vget_low_u8(source),
                neon::vget_low_u8(destination),
                neon::vget_low_u8(mask),
                neon::vget_low_u8(inverse)
            ),
            blend_half!(
                neon::vget_high_u8(source),
                neon::vget_high_u8(destination),
                neon::vget_high_u8(mask),
                neon::vget_high_u8(inverse)
            ),
        );
        neon::vst1q_u8(destination_ptr, output);
    }
}

/// Process the largest complete sixteen-byte prefix of an L/mask row using
/// NEON, returning the first scalar-tail offset (or zero when no block fits).
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[allow(unsafe_code)]
#[inline]
fn native_paste_l_masked_row_neon(source: &[u8], destination: &mut [u8], mask: &[u8]) -> usize {
    debug_assert_eq!(source.len(), destination.len());
    debug_assert_eq!(source.len(), mask.len());

    let vector_len = source.len() / 16 * 16;
    if vector_len == 0 {
        return 0;
    }

    let unrolled_len = vector_len / 64 * 64;
    for start in (0..unrolled_len).step_by(64) {
        native_paste_l_masked_neon_block(source, destination, mask, start);
        native_paste_l_masked_neon_block(source, destination, mask, start + 16);
        native_paste_l_masked_neon_block(source, destination, mask, start + 32);
        native_paste_l_masked_neon_block(source, destination, mask, start + 48);
    }
    for start in (unrolled_len..vector_len).step_by(16) {
        native_paste_l_masked_neon_block(source, destination, mask, start);
    }
    vector_len
}

/// Blend a clipped native-LA row with one L-mask byte per pixel. The stored
/// luminance and alpha bytes share each mask value; rows are independent and
/// can use the same bounded parallel path as native L Paste.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[allow(unsafe_code)]
#[inline]
fn native_paste_la_masked_neon_block(
    source: &[u8],
    destination: &mut [u8],
    mask: &[u8],
    start: usize,
) {
    use core::arch::aarch64 as neon;

    macro_rules! blend {
        ($source:expr, $destination:expr, $mask:expr) => {{
            let inverse = neon::vsub_u8(neon::vdup_n_u8(255), $mask);
            let weighted = neon::vmlal_u8(neon::vmull_u8($source, $mask), $destination, inverse);
            // The byte blend rounds with +127 before exact DIV255. The
            // quotient correction contributes +1, so combine both constants.
            let incremented = neon::vaddq_u16(weighted, neon::vdupq_n_u16(128));
            let quotient = neon::vshrq_n_u16(
                neon::vaddq_u16(incremented, neon::vshrq_n_u16(incremented, 8)),
                8,
            );
            neon::vmovn_u16(quotient)
        }};
    }

    let mask_start = start / 2;
    // SAFETY: The caller bounds `start` to a complete sixteen-byte LA source
    // and destination block, corresponding to eight L-mask bytes. NEON's
    // interleaved byte loads/stores permit unaligned pointers; Rust's borrows
    // keep destination disjoint from the source and mask slices.
    unsafe {
        let source_pair = neon::vld2_u8(source.as_ptr().add(start));
        let destination_ptr = destination.as_mut_ptr().add(start);
        let destination_pair = neon::vld2_u8(destination_ptr);
        let mask = neon::vld1_u8(mask.as_ptr().add(mask_start));
        let output = neon::uint8x8x2_t(
            blend!(source_pair.0, destination_pair.0, mask),
            blend!(source_pair.1, destination_pair.1, mask),
        );
        neon::vst2_u8(destination_ptr, output);
    }
}

/// Blend complete interleaved LA blocks with NEON's deinterleaving byte loads.
/// Each L-mask lane is used once for luminance and once for alpha, avoiding
/// expansion into sixteen repeated mask bytes before the two channel blends.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[allow(unsafe_code)]
#[inline]
fn native_paste_la_masked_row_neon(
    source: &[u8],
    destination: &mut [u8],
    mask: &[u8],
    vector_len: usize,
) -> usize {
    let unrolled_len = vector_len / 64 * 64;
    for start in (0..unrolled_len).step_by(64) {
        native_paste_la_masked_neon_block(source, destination, mask, start);
        native_paste_la_masked_neon_block(source, destination, mask, start + 16);
        native_paste_la_masked_neon_block(source, destination, mask, start + 32);
        native_paste_la_masked_neon_block(source, destination, mask, start + 48);
    }
    for start in (unrolled_len..vector_len).step_by(16) {
        native_paste_la_masked_neon_block(source, destination, mask, start);
    }
    vector_len
}

#[inline]
pub(in crate::compute::pool_simd::ops::adapters) fn native_paste_la_masked_row(
    source: &[u8],
    destination: &mut [u8],
    mask: &[u8],
    allow_short_masked_tail: bool,
) -> bool {
    debug_assert_eq!(source.len(), destination.len());
    debug_assert_eq!(source.len() % 2, 0);
    debug_assert_eq!(source.len() / 2, mask.len());

    let vector_len16 = source.len() / 16 * 16;
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    let neon_prefix = native_paste_la_masked_row_neon(source, destination, mask, vector_len16);
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    let neon_prefix = 0;

    for start in (neon_prefix..vector_len16).step_by(16) {
        let source_block = <[u8; 16]>::try_from(&source[start..start + 16])
            .expect("validated LA Paste row has a complete 16-byte block");
        let destination_block = <[u8; 16]>::try_from(&destination[start..start + 16])
            .expect("validated LA Paste row has a complete 16-byte destination block");
        let mask_block = std::array::from_fn(|lane| mask[start / 2 + lane / 2]);
        let blended = native_paste_blend_vector16(source_block, destination_block, mask_block);
        destination[start..start + 16].copy_from_slice(&blended);
    }

    let vector_len8 = source.len() / 8 * 8;
    for start in (vector_len16..vector_len8).step_by(8) {
        let source_block = <[u8; 8]>::try_from(&source[start..start + 8])
            .expect("validated LA Paste row has a complete 8-byte block");
        let destination_block = <[u8; 8]>::try_from(&destination[start..start + 8])
            .expect("validated LA Paste row has a complete 8-byte destination block");
        let mask_block = std::array::from_fn(|lane| mask[start / 2 + lane / 2]);
        let blended = native_paste_blend_vector8(source_block, destination_block, mask_block);
        destination[start..start + 8].copy_from_slice(&blended);
    }

    let tail = source.len() - vector_len8;
    if tail == 0 {
        return neon_prefix != 0;
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
    neon_prefix != 0
}
