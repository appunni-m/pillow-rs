//! AArch64 NEON implementation of LA's exact 3x3 binomial filter.

/// Filter packed LA samples using eight vector pixels and a scalar tail.
///
/// The source and output retain their native two-byte pixel layout. Borders
/// are unchanged, and the integer arithmetic matches the default strategy.
#[allow(unsafe_code)]
pub(crate) fn filter_3x3_binomial_la_rows(
    raw: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
) -> Option<(u64, u64)> {
    use core::arch::aarch64 as neon;

    let row_samples = width.checked_mul(2)?;
    let expected_len = row_samples.checked_mul(height)?;
    if raw.len() != expected_len || output.len() != expected_len {
        return None;
    }
    if width < 3 || height < 3 {
        return Some((0, 0));
    }

    let horizontal = |source_y: usize, x: usize| {
        // SAFETY: The caller checks the full raster length, `source_y` comes
        // from the validated three-row window, and `x + 14 < width` in the
        // vector loop keeps all 16 interleaved pixels inside this source row.
        let samples = unsafe {
            neon::vld2q_u8(raw.as_ptr().add(source_y * row_samples + (x - 1) * 2))
        };
        let filter_channel = |samples: neon::uint8x16_t| {
            // SAFETY: This module is reachable only on AArch64 with NEON
            // enabled; the values are initialized NEON registers and the
            // intrinsics operate on those register values.
            unsafe {
                let low = neon::vget_low_u8(samples);
                let high = neon::vget_high_u8(samples);
                let left = low;
                let center = neon::vext_u8(low, high, 1);
                let right = neon::vext_u8(low, high, 2);
                neon::vaddq_u16(neon::vaddl_u8(left, right), neon::vaddl_u8(center, center))
            }
        };
        (filter_channel(samples.0), filter_channel(samples.1))
    };

    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for y in 1..height - 1 {
        let output_row = y * row_samples;
        let mut x = 1usize;
        while x + 14 < width {
            let above = horizontal(y - 1, x);
            let center = horizontal(y, x);
            let below = horizontal(y + 1, x);
            let filter_vertical = |above: neon::uint16x8_t,
                                   center: neon::uint16x8_t,
                                   below: neon::uint16x8_t| {
                // SAFETY: The enclosing module is target-gated for AArch64
                // NEON; each operand is an initialized vector from the row loads.
                unsafe {
                    let sum = neon::vaddq_u16(
                        neon::vaddq_u16(above, below),
                        neon::vaddq_u16(center, center),
                    );
                    let rounded =
                        neon::vshrq_n_u16(neon::vaddq_u16(sum, neon::vdupq_n_u16(8)), 4);
                    neon::vqmovn_u16(rounded)
                }
            };
            let result = neon::uint8x8x2_t(
                filter_vertical(above.0, center.0, below.0),
                filter_vertical(above.1, center.1, below.1),
            );
            // SAFETY: The vector-loop bound guarantees eight interior output
            // pixels fit in the row. The checked raster length covers this
            // destination range, and safe borrowing keeps input and output disjoint.
            unsafe {
                neon::vst2_u8(output.as_mut_ptr().add(output_row + x * 2), result);
            }
            x += 8;
            vector_blocks += 1;
        }

        for pixel_x in x..width - 1 {
            for channel in 0..2 {
                let horizontal_sum = |source_y: usize| {
                    let center = source_y * row_samples + pixel_x * 2 + channel;
                    u16::from(raw[center - 2])
                        + u16::from(raw[center]) * 2
                        + u16::from(raw[center + 2])
                };
                let sum = u32::from(horizontal_sum(y - 1))
                    + u32::from(horizontal_sum(y)) * 2
                    + u32::from(horizontal_sum(y + 1));
                output[output_row + pixel_x * 2 + channel] = ((sum + 8) >> 4) as u8;
            }
            scalar_tail += 2;
        }
    }
    Some((vector_blocks, scalar_tail))
}
