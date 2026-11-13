//! Shared native-byte separable convolution rows for Resize.

use super::constants::SIMD_RESIZE_LANES;
use super::plan::{ResizeHorizontalPlan, resize_coeff_slice};
use crate::ops::pil_resize::FilterCoeffs;
use wide::{i32x8, i64x8};

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
#[inline]
pub(super) fn resize_fixed_point_to_u8(sum: f64) -> u8 {
    let value = (sum as i64 + (1_i64 << 21)) >> 22;
    value.clamp(0, 255) as u8
}

/// Convert one exact Pillow fixed-point accumulator to a byte.
///
/// `FilterCoeffs` stores weights at `2^22` precision. Samples and weights are
/// therefore accumulated as integers in the native SIMD path; this helper
/// preserves the scalar rounding without a floating-point conversion.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
#[inline(always)]
fn resize_fixed_point_i64_to_u8(sum: i64) -> u8 {
    let value = (sum + (1_i64 << 21)) >> 22;
    value.clamp(0, 255) as u8
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
#[inline]
pub(super) fn resize_premultiply_u8(value: u8, alpha: u8) -> u8 {
    ((u16::from(value) * u16::from(alpha) + 127) / 255) as u8
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(super) fn resize_premultiply_alpha_row<const CHANNELS: usize>(
    source: &[u8],
    output: &mut [u8],
) -> Option<()> {
    if !matches!(CHANNELS, 2 | 4) || source.len() != output.len() {
        return None;
    }
    let alpha_channel = CHANNELS - 1;
    for (source_pixel, output_pixel) in source
        .chunks_exact(CHANNELS)
        .zip(output.chunks_exact_mut(CHANNELS))
    {
        let alpha = source_pixel[alpha_channel];
        for channel in 0..alpha_channel {
            output_pixel[channel] = resize_premultiply_u8(source_pixel[channel], alpha);
        }
        output_pixel[alpha_channel] = alpha;
    }
    Some(())
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
#[inline]
pub(super) fn resize_unpremultiply_u8(value: u8, alpha: u8) -> u8 {
    crate::ops::pil_resize::unpremultiply_channel(value, alpha)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
#[inline]
pub(super) fn resize_horizontal_scalar<const CHANNELS: usize>(
    source_row: &[u8],
    coeffs: &FilterCoeffs,
    output_x: usize,
    channel: usize,
) -> Option<u8> {
    let x0 = usize::try_from(*coeffs.xmin.get(output_x)?).ok()?;
    let weights = resize_coeff_slice(coeffs, output_x)?;
    let mut sum = 0.0;
    for (tap, &weight) in weights.iter().enumerate() {
        let source_x = x0.checked_add(tap)?;
        let source_index = source_x.checked_mul(CHANNELS)?.checked_add(channel)?;
        let value = *source_row.get(source_index)?;
        sum += f64::from(value) * weight as f64;
    }
    Some(resize_fixed_point_to_u8(sum))
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(super) fn resize_horizontal_vector_row<const CHANNELS: usize>(
    source_row: &[u8],
    coeffs: &FilterCoeffs,
    plan: &ResizeHorizontalPlan,
    output_width: usize,
    output_row: &mut [u8],
) -> Option<(u64, u64)> {
    if source_row.is_empty() {
        // Empty source rows contribute only zero-weight padding lanes. Keep
        // their established zero result without entering the gather loop.
        output_row.fill(0);
        let vector_blocks = u64::try_from(plan.blocks.len()).ok()?;
        let scalar_start = if output_width < SIMD_RESIZE_LANES {
            output_width
        } else {
            plan.vector_width
        };
        let scalar_tail = u64::try_from(output_width.saturating_sub(scalar_start)).ok()?;
        return Some((vector_blocks, scalar_tail));
    }
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for (block_index, output_x) in (0..plan.vector_width)
        .step_by(SIMD_RESIZE_LANES)
        .enumerate()
    {
        let mut result = [[0u8; SIMD_RESIZE_LANES]; 4];
        let mut sums = [i64x8::splat(0); 4];
        let block = plan.blocks.get(block_index)?;
        for tap in &block.taps {
            let weight_vector = i32x8::new(tap.weights);
            for channel in 0..CHANNELS {
                let mut samples = [0u8; SIMD_RESIZE_LANES];
                for lane in 0..SIMD_RESIZE_LANES {
                    // Missing lanes have weight zero and base zero. Reading
                    // one valid source sample avoids a branch per lane/tap.
                    samples[lane] = *source_row.get(tap.source_bases[lane] + channel)?;
                }
                sums[channel] += i32x8::new(samples.map(i32::from)).widening_mul(weight_vector);
            }
        }
        for channel in 0..CHANNELS {
            result[channel] = sums[channel].to_array().map(resize_fixed_point_i64_to_u8);
        }
        for lane in 0..SIMD_RESIZE_LANES {
            if output_x + lane >= output_width {
                continue;
            }
            let output_start = (output_x + lane).checked_mul(CHANNELS)?;
            for channel in 0..CHANNELS {
                *output_row.get_mut(output_start + channel)? = result[channel][lane];
            }
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let scalar_start = if output_width < SIMD_RESIZE_LANES {
        output_width
    } else {
        plan.vector_width
    };
    for output_x in scalar_start..output_width {
        let output_start = output_x.checked_mul(CHANNELS)?;
        for channel in 0..CHANNELS {
            *output_row.get_mut(output_start + channel)? =
                resize_horizontal_scalar::<CHANNELS>(source_row, coeffs, output_x, channel)?;
        }
        scalar_tail = scalar_tail.saturating_add(1);
    }
    Some((vector_blocks, scalar_tail))
}

/// Dispatch one horizontal row. For alpha-bearing native byte modes, premultiply
/// each source pixel once into a reusable row buffer, then let the convolution
/// taps consume those native interleaved samples without repeating the alpha
/// multiply for every destination pixel and filter tap.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(super) fn resize_horizontal_convolution_row<
    const CHANNELS: usize,
    const PREMULTIPLIED_ALPHA: bool,
    const USE_BOUNDED_I32: bool,
>(
    source_row: &[u8],
    coeffs: &FilterCoeffs,
    plan: &ResizeHorizontalPlan,
    output_width: usize,
    output_row: &mut [u8],
    premultiplied_row: &mut [u8],
) -> Option<(u64, u64)> {
    let source_row = if PREMULTIPLIED_ALPHA && (!USE_BOUNDED_I32 || CHANNELS == 2 || CHANNELS == 4)
    {
        resize_premultiply_alpha_row::<CHANNELS>(source_row, premultiplied_row)?;
        &*premultiplied_row
    } else {
        source_row
    };

    if USE_BOUNDED_I32 {
        super::bounded_i32::resize_horizontal_bounded_i32_vector_row::<CHANNELS, PREMULTIPLIED_ALPHA>(
            source_row,
            coeffs,
            plan,
            output_width,
            output_row,
        )
    } else {
        resize_horizontal_vector_row::<CHANNELS>(source_row, coeffs, plan, output_width, output_row)
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(super) fn resize_vertical_vector_row<const CHANNELS: usize, const PREMULTIPLIED_ALPHA: bool>(
    intermediate: &[u8],
    output_width: usize,
    source_height: usize,
    coeffs: &FilterCoeffs,
    output_y: usize,
    output_row: &mut [u8],
) -> Option<(u64, u64)> {
    let weights = resize_coeff_slice(coeffs, output_y)?;
    let y0 = usize::try_from(*coeffs.xmin.get(output_y)?).ok()?;
    // A short output row is handled as one padded vector block. Inactive
    // lanes are zero-filled and skipped when the valid prefix is stored.
    let vector_width = if output_width < SIMD_RESIZE_LANES {
        SIMD_RESIZE_LANES
    } else {
        output_width / SIMD_RESIZE_LANES * SIMD_RESIZE_LANES
    };
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    let alpha_channel = CHANNELS - 1;
    for output_x in (0..vector_width).step_by(SIMD_RESIZE_LANES) {
        let mut result = [[0u8; SIMD_RESIZE_LANES]; 4];
        let mut sums = [i64x8::splat(0); 4];
        for (tap, &weight) in weights.iter().enumerate() {
            let source_y = y0.checked_add(tap)?;
            if source_y >= source_height {
                return None;
            }
            // Vertical taps read adjacent pixels from one intermediate row.
            // Validate that contiguous span once instead of rebuilding eight
            // absolute addresses and active flags for every tap and channel.
            // Only the final short vector is padded; its missing samples keep
            // the same zero contribution as the previous masked gather.
            let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
            let source_start = source_y
                .checked_mul(output_width)?
                .checked_add(output_x)?
                .checked_mul(CHANNELS)?;
            let source_end = source_start.checked_add(count.checked_mul(CHANNELS)?)?;
            let source_pixels = intermediate.get(source_start..source_end)?;
            let weight_vector = i32x8::splat(weight as i32);
            for channel in 0..CHANNELS {
                let mut samples = [0i32; SIMD_RESIZE_LANES];
                for (lane, pixel) in source_pixels.chunks_exact(CHANNELS).enumerate() {
                    samples[lane] = i32::from(pixel[channel]);
                }
                sums[channel] += i32x8::new(samples).widening_mul(weight_vector);
            }
        }
        for channel in 0..CHANNELS {
            result[channel] = sums[channel].to_array().map(resize_fixed_point_i64_to_u8);
        }
        if PREMULTIPLIED_ALPHA {
            let alpha = result[alpha_channel];
            for channel in 0..alpha_channel {
                for lane in 0..SIMD_RESIZE_LANES {
                    result[channel][lane] =
                        resize_unpremultiply_u8(result[channel][lane], alpha[lane]);
                }
            }
        }
        for lane in 0..SIMD_RESIZE_LANES {
            if output_x + lane >= output_width {
                continue;
            }
            let output_start = (output_x + lane).checked_mul(CHANNELS)?;
            for channel in 0..CHANNELS {
                *output_row.get_mut(output_start + channel)? = result[channel][lane];
            }
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let scalar_start = if output_width < SIMD_RESIZE_LANES {
        output_width
    } else {
        vector_width
    };
    for output_x in scalar_start..output_width {
        let output_start = output_x.checked_mul(CHANNELS)?;
        let mut result = [0u8; 4];
        for channel in 0..CHANNELS {
            let mut sum = 0.0;
            for (tap, &weight) in weights.iter().enumerate() {
                let source_y = y0.checked_add(tap)?;
                let source_index = source_y
                    .checked_mul(output_width)?
                    .checked_add(output_x)?
                    .checked_mul(CHANNELS)?
                    .checked_add(channel)?;
                sum += f64::from(*intermediate.get(source_index)?) * weight as f64;
            }
            result[channel] = resize_fixed_point_to_u8(sum);
        }
        if PREMULTIPLIED_ALPHA {
            let alpha = result[alpha_channel];
            for channel in 0..alpha_channel {
                result[channel] = resize_unpremultiply_u8(result[channel], alpha);
            }
        }
        for channel in 0..CHANNELS {
            *output_row.get_mut(output_start + channel)? = result[channel];
        }
        scalar_tail = scalar_tail.saturating_add(1);
    }
    Some((vector_blocks, scalar_tail))
}
