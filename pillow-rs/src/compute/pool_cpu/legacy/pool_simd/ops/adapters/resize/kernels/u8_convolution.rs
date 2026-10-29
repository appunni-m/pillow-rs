//! Shared native-byte separable convolution rows for Resize and Thumbnail.

use super::super::{SIMD_RESIZE_LANES, simd_div255_u16x8};
use super::u8_cover_i32::resize_horizontal_cover_i32_vector_row;
use super::u8_plan::{ResizeHorizontalPlan, resize_coeff_slice};
use crate::ops::pil_resize::FilterCoeffs;
use wide::{i32x8, i64x8, u16x8};

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
#[inline(always)]
fn resize_fixed_point_i64_to_u8(sum: i64) -> u8 {
    let value = (sum + (1_i64 << 21)) >> 22;
    value.clamp(0, 255) as u8
}

#[inline]
pub(super) fn resize_premultiply_u8(value: u8, alpha: u8) -> u8 {
    ((u16::from(value) * u16::from(alpha) + 127) / 255) as u8
}

fn resize_premultiply_alpha_row(source: &[u8], channels: usize, output: &mut [u8]) -> Option<()> {
    if !matches!(channels, 2 | 4) || source.len() != output.len() {
        return None;
    }
    let alpha_channel = channels - 1;
    for (source_pixel, output_pixel) in source
        .chunks_exact(channels)
        .zip(output.chunks_exact_mut(channels))
    {
        let alpha = source_pixel[alpha_channel];
        for channel in 0..alpha_channel {
            output_pixel[channel] = resize_premultiply_u8(source_pixel[channel], alpha);
        }
        output_pixel[alpha_channel] = alpha;
    }
    Some(())
}

#[inline]
pub(super) fn resize_unpremultiply_u8(value: u8, alpha: u8) -> u8 {
    crate::ops::pil_resize::unpremultiply_channel(value, alpha)
}

#[inline]
pub(super) fn resize_horizontal_scalar(
    source_row: &[u8],
    channels: usize,
    coeffs: &FilterCoeffs,
    output_x: usize,
    channel: usize,
    premultiplied_alpha: bool,
) -> Option<u8> {
    let x0 = usize::try_from(*coeffs.xmin.get(output_x)?).ok()?;
    let weights = resize_coeff_slice(coeffs, output_x)?;
    let alpha_channel = channels - 1;
    let mut sum = 0.0;
    for (tap, &weight) in weights.iter().enumerate() {
        let source_x = x0.checked_add(tap)?;
        let source_index = source_x.checked_mul(channels)?.checked_add(channel)?;
        let value = *source_row.get(source_index)?;
        let value = if premultiplied_alpha && channel != alpha_channel {
            resize_premultiply_u8(value, *source_row.get(source_x * channels + alpha_channel)?)
        } else {
            value
        };
        sum += f64::from(value) * weight as f64;
    }
    Some(resize_fixed_point_to_u8(sum))
}

pub(in crate::compute::pool_simd::ops::adapters) fn resize_horizontal_vector_row(
    source_row: &[u8],
    channels: usize,
    coeffs: &FilterCoeffs,
    plan: &ResizeHorizontalPlan,
    output_width: usize,
    output_row: &mut [u8],
    premultiplied_alpha: bool,
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
    let alpha_channel = channels - 1;
    for (block_index, output_x) in (0..plan.vector_width)
        .step_by(SIMD_RESIZE_LANES)
        .enumerate()
    {
        let mut result = [[0u8; SIMD_RESIZE_LANES]; 4];
        let mut sums = [i64x8::splat(0); 4];
        let block = plan.blocks.get(block_index)?;
        for tap in &block.taps {
            let weight_vector = i32x8::new(tap.weights);
            for channel in 0..channels {
                let mut samples = [0u8; SIMD_RESIZE_LANES];
                for lane in 0..SIMD_RESIZE_LANES {
                    // Missing lanes have weight zero and base zero. Reading
                    // one valid source sample avoids a branch per lane/tap.
                    samples[lane] = *source_row.get(tap.source_bases[lane] + channel)?;
                }
                let samples = if premultiplied_alpha && channel != alpha_channel {
                    let mut alphas = [0u8; SIMD_RESIZE_LANES];
                    for lane in 0..SIMD_RESIZE_LANES {
                        alphas[lane] = *source_row.get(tap.source_bases[lane] + alpha_channel)?;
                    }
                    let values = u16x8::new(samples.map(u16::from))
                        * u16x8::new(alphas.map(u16::from))
                        + u16x8::splat(127);
                    simd_div255_u16x8(values).to_array().map(i32::from)
                } else {
                    samples.map(i32::from)
                };
                sums[channel] += i32x8::new(samples).widening_mul(weight_vector);
            }
        }
        for channel in 0..channels {
            result[channel] = sums[channel].to_array().map(resize_fixed_point_i64_to_u8);
        }
        for lane in 0..SIMD_RESIZE_LANES {
            if output_x + lane >= output_width {
                continue;
            }
            let output_start = (output_x + lane).checked_mul(channels)?;
            for channel in 0..channels {
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
        let output_start = output_x.checked_mul(channels)?;
        for channel in 0..channels {
            *output_row.get_mut(output_start + channel)? = resize_horizontal_scalar(
                source_row,
                channels,
                coeffs,
                output_x,
                channel,
                premultiplied_alpha,
            )?;
        }
        scalar_tail = scalar_tail.saturating_add(1);
    }
    Some((vector_blocks, scalar_tail))
}

/// Dispatch one horizontal row. For alpha-bearing native byte modes, premultiply
/// each source pixel once into a reusable row buffer, then let the convolution
/// taps consume those native interleaved samples without repeating the alpha
/// multiply for every destination pixel and filter tap.
pub(in crate::compute::pool_simd::ops::adapters) fn resize_horizontal_convolution_row(
    source_row: &[u8],
    channels: usize,
    coeffs: &FilterCoeffs,
    plan: &ResizeHorizontalPlan,
    output_width: usize,
    output_row: &mut [u8],
    premultiplied_alpha: bool,
    cover_byte_i32: bool,
    premultiplied_row: &mut [u8],
) -> Option<(u64, u64)> {
    let precompute_la_luma = premultiplied_alpha && cover_byte_i32 && channels == 2;
    let (source_row, premultiply_in_kernel) =
        if premultiplied_alpha && (!cover_byte_i32 || channels == 2 || channels == 4) {
            resize_premultiply_alpha_row(source_row, channels, premultiplied_row)?;
            (&*premultiplied_row, false)
        } else {
            (source_row, premultiplied_alpha)
        };

    if cover_byte_i32 {
        resize_horizontal_cover_i32_vector_row(
            source_row,
            channels,
            coeffs,
            plan,
            output_width,
            output_row,
            premultiplied_alpha,
            precompute_la_luma,
        )
    } else {
        resize_horizontal_vector_row(
            source_row,
            channels,
            coeffs,
            plan,
            output_width,
            output_row,
            premultiply_in_kernel,
        )
    }
}

pub(in crate::compute::pool_simd::ops::adapters) fn resize_vertical_vector_row(
    intermediate: &[u8],
    output_width: usize,
    source_height: usize,
    channels: usize,
    coeffs: &FilterCoeffs,
    output_y: usize,
    output_row: &mut [u8],
    premultiplied_alpha: bool,
    vectorize_tail: bool,
) -> Option<(u64, u64)> {
    let weights = resize_coeff_slice(coeffs, output_y)?;
    let y0 = usize::try_from(*coeffs.xmin.get(output_y)?).ok()?;
    // A short output row is handled as one padded vector block. Inactive
    // lanes are zero-filled and skipped when the valid prefix is stored.
    let vector_width = if output_width < SIMD_RESIZE_LANES {
        SIMD_RESIZE_LANES
    } else if vectorize_tail {
        output_width
            .div_ceil(SIMD_RESIZE_LANES)
            .checked_mul(SIMD_RESIZE_LANES)?
    } else {
        output_width / SIMD_RESIZE_LANES * SIMD_RESIZE_LANES
    };
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    let alpha_channel = channels - 1;
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
                .checked_mul(channels)?;
            let source_end = source_start.checked_add(count.checked_mul(channels)?)?;
            let source_pixels = intermediate.get(source_start..source_end)?;
            let weight_vector = i32x8::splat(weight as i32);
            for channel in 0..channels {
                let mut samples = [0i32; SIMD_RESIZE_LANES];
                for (lane, pixel) in source_pixels.chunks_exact(channels).enumerate() {
                    samples[lane] = i32::from(pixel[channel]);
                }
                sums[channel] += i32x8::new(samples).widening_mul(weight_vector);
            }
        }
        for channel in 0..channels {
            result[channel] = sums[channel].to_array().map(resize_fixed_point_i64_to_u8);
        }
        if premultiplied_alpha {
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
            let output_start = (output_x + lane).checked_mul(channels)?;
            for channel in 0..channels {
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
        let output_start = output_x.checked_mul(channels)?;
        let mut result = [0u8; 4];
        for channel in 0..channels {
            let mut sum = 0.0;
            for (tap, &weight) in weights.iter().enumerate() {
                let source_y = y0.checked_add(tap)?;
                let source_index = source_y
                    .checked_mul(output_width)?
                    .checked_add(output_x)?
                    .checked_mul(channels)?
                    .checked_add(channel)?;
                sum += f64::from(*intermediate.get(source_index)?) * weight as f64;
            }
            result[channel] = resize_fixed_point_to_u8(sum);
        }
        if premultiplied_alpha {
            let alpha = result[alpha_channel];
            for channel in 0..alpha_channel {
                result[channel] = resize_unpremultiply_u8(result[channel], alpha);
            }
        }
        for channel in 0..channels {
            *output_row.get_mut(output_start + channel)? = result[channel];
        }
        scalar_tail = scalar_tail.saturating_add(1);
    }
    Some((vector_blocks, scalar_tail))
}
