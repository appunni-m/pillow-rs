//! Checked i32 accumulator rows for native-byte Resize.

use super::super::{SIMD_RESIZE_LANES, simd_div255_u16x8};
use super::u8_convolution::{
    resize_fixed_point_to_u8, resize_horizontal_scalar, resize_unpremultiply_u8,
};
use super::u8_plan::{ResizeHorizontalPlan, resize_coeff_slice};
use crate::ops::pil_resize::FilterCoeffs;
use wide::{f64x8, i32x8, u16x8};

#[inline(always)]
fn resize_fixed_point_i32_to_u8(sum: i32) -> u8 {
    let value = (sum + (1_i32 << 21)) >> 22;
    value.clamp(0, 255) as u8
}

/// L-only horizontal convolution with a checked, non-wrapping i32
/// accumulator. The regular SIMD kernel remains the fallback for coefficient
/// tables that do not satisfy the byte-domain bound.
fn resize_horizontal_luma_i32_vector_row(
    source_row: &[u8],
    coeffs: &FilterCoeffs,
    plan: &ResizeHorizontalPlan,
    output_width: usize,
    output_row: &mut [u8],
) -> Option<(u64, u64)> {
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for (block_index, output_x) in (0..plan.vector_width)
        .step_by(SIMD_RESIZE_LANES)
        .enumerate()
    {
        let mut sum = i32x8::splat(0);
        let block = plan.blocks.get(block_index)?;
        for tap in &block.taps {
            let mut samples = [0u8; SIMD_RESIZE_LANES];
            for lane in 0..SIMD_RESIZE_LANES {
                // A padded lane has a zero weight. Avoid reading its default
                // base while keeping the compact plan free of a second mask.
                if tap.weights[lane] != 0 {
                    samples[lane] = *source_row.get(tap.source_bases[lane])?;
                }
            }
            sum += i32x8::new(samples.map(i32::from)) * i32x8::new(tap.weights);
        }
        let result = sum.to_array().map(resize_fixed_point_i32_to_u8);
        for lane in 0..SIMD_RESIZE_LANES {
            if output_x + lane < output_width {
                *output_row.get_mut(output_x + lane)? = result[lane];
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
        *output_row.get_mut(output_x)? =
            resize_horizontal_scalar(source_row, 1, coeffs, output_x, 0, false)?;
        scalar_tail = scalar_tail.saturating_add(1);
    }
    Some((vector_blocks, scalar_tail))
}

/// L-only vertical convolution with a checked i32 accumulator. It preserves
/// the horizontal byte intermediate and the same fixed-point rounding at
/// each axis boundary as Pillow's two-pass resampler.
fn resize_vertical_luma_i32_vector_row(
    intermediate: &[u8],
    output_width: usize,
    source_height: usize,
    coeffs: &FilterCoeffs,
    output_y: usize,
    output_row: &mut [u8],
) -> Option<(u64, u64)> {
    let weights = resize_coeff_slice(coeffs, output_y)?;
    let y0 = usize::try_from(*coeffs.xmin.get(output_y)?).ok()?;
    let vector_width = if output_width < SIMD_RESIZE_LANES {
        SIMD_RESIZE_LANES
    } else {
        output_width / SIMD_RESIZE_LANES * SIMD_RESIZE_LANES
    };
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for output_x in (0..vector_width).step_by(SIMD_RESIZE_LANES) {
        let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
        let mut sum = i32x8::splat(0);
        for (tap, &weight) in weights.iter().enumerate() {
            let source_y = y0.checked_add(tap)?;
            if source_y >= source_height {
                return None;
            }
            let source_start = source_y.checked_mul(output_width)?.checked_add(output_x)?;
            let source_end = source_start.checked_add(count)?;
            let source_pixels = intermediate.get(source_start..source_end)?;
            let mut samples = [0i32; SIMD_RESIZE_LANES];
            for (lane, &sample) in source_pixels.iter().enumerate() {
                samples[lane] = i32::from(sample);
            }
            let weight = i32::try_from(weight).ok()?;
            sum += i32x8::new(samples) * i32x8::splat(weight);
        }
        let result = sum.to_array().map(resize_fixed_point_i32_to_u8);
        for lane in 0..count {
            *output_row.get_mut(output_x + lane)? = result[lane];
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let scalar_start = if output_width < SIMD_RESIZE_LANES {
        output_width
    } else {
        vector_width
    };
    for output_x in scalar_start..output_width {
        let mut sum = 0.0;
        for (tap, &weight) in weights.iter().enumerate() {
            let source_y = y0.checked_add(tap)?;
            let source_index = source_y.checked_mul(output_width)?.checked_add(output_x)?;
            sum += f64::from(*intermediate.get(source_index)?) * weight as f64;
        }
        *output_row.get_mut(output_x)? = resize_fixed_point_to_u8(sum);
        scalar_tail = scalar_tail.saturating_add(1);
    }
    Some((vector_blocks, scalar_tail))
}

/// LA horizontal convolution with checked i32 accumulators. Luma is
/// premultiplied by alpha before filtering, while alpha itself is filtered as
/// its own native channel. The coefficient guard used by the Cover dispatcher
/// bounds both accumulators because each sample remains in 0..=255.
fn resize_horizontal_la_i32_vector_row(
    source_row: &[u8],
    coeffs: &FilterCoeffs,
    plan: &ResizeHorizontalPlan,
    output_width: usize,
    output_row: &mut [u8],
    luma_is_premultiplied: bool,
) -> Option<(u64, u64)> {
    if source_row.is_empty() {
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
        let block = plan.blocks.get(block_index)?;
        let mut luma_sum = i32x8::splat(0);
        let mut alpha_sum = i32x8::splat(0);
        for tap in &block.taps {
            let mut luma = [0u8; SIMD_RESIZE_LANES];
            let mut alpha = [0u8; SIMD_RESIZE_LANES];
            for lane in 0..SIMD_RESIZE_LANES {
                if tap.weights[lane] != 0 {
                    let source_base = tap.source_bases[lane];
                    luma[lane] = *source_row.get(source_base)?;
                    alpha[lane] = *source_row.get(source_base.checked_add(1)?)?;
                }
            }
            let premultiplied_luma = if luma_is_premultiplied {
                luma.map(i32::from)
            } else {
                simd_div255_u16x8(
                    u16x8::new(luma.map(u16::from)) * u16x8::new(alpha.map(u16::from))
                        + u16x8::splat(127),
                )
                .to_array()
                .map(i32::from)
            };
            let weight = i32x8::new(tap.weights);
            luma_sum += i32x8::new(premultiplied_luma) * weight;
            alpha_sum += i32x8::new(alpha.map(i32::from)) * weight;
        }
        let luma = luma_sum.to_array().map(resize_fixed_point_i32_to_u8);
        let alpha = alpha_sum.to_array().map(resize_fixed_point_i32_to_u8);
        for lane in 0..SIMD_RESIZE_LANES {
            if output_x + lane >= output_width {
                continue;
            }
            let output_start = (output_x + lane).checked_mul(2)?;
            *output_row.get_mut(output_start)? = luma[lane];
            *output_row.get_mut(output_start.checked_add(1)?)? = alpha[lane];
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }

    let scalar_start = if output_width < SIMD_RESIZE_LANES {
        output_width
    } else {
        plan.vector_width
    };
    for output_x in scalar_start..output_width {
        let output_start = output_x.checked_mul(2)?;
        *output_row.get_mut(output_start)? =
            resize_horizontal_scalar(source_row, 2, coeffs, output_x, 0, !luma_is_premultiplied)?;
        *output_row.get_mut(output_start.checked_add(1)?)? =
            resize_horizontal_scalar(source_row, 2, coeffs, output_x, 1, false)?;
        scalar_tail = scalar_tail.saturating_add(1);
    }
    Some((vector_blocks, scalar_tail))
}

/// LA vertical convolution with checked i32 accumulators. Restore luma only
/// after filtering it and alpha separately, matching the generic premultiplied
/// path's fixed-point rounding at each pass boundary.
fn resize_vertical_la_i32_vector_row(
    intermediate: &[u8],
    output_width: usize,
    source_height: usize,
    coeffs: &FilterCoeffs,
    output_y: usize,
    output_row: &mut [u8],
) -> Option<(u64, u64)> {
    let weights = resize_coeff_slice(coeffs, output_y)?;
    let y0 = usize::try_from(*coeffs.xmin.get(output_y)?).ok()?;
    let vector_width = if output_width < SIMD_RESIZE_LANES {
        SIMD_RESIZE_LANES
    } else {
        output_width / SIMD_RESIZE_LANES * SIMD_RESIZE_LANES
    };
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for output_x in (0..vector_width).step_by(SIMD_RESIZE_LANES) {
        let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
        let mut luma_sum = i32x8::splat(0);
        let mut alpha_sum = i32x8::splat(0);
        for (tap, &weight) in weights.iter().enumerate() {
            let source_y = y0.checked_add(tap)?;
            if source_y >= source_height {
                return None;
            }
            let source_start = source_y
                .checked_mul(output_width)?
                .checked_add(output_x)?
                .checked_mul(2)?;
            let source_end = source_start.checked_add(count.checked_mul(2)?)?;
            let source_pixels = intermediate.get(source_start..source_end)?;
            let mut luma = [0i32; SIMD_RESIZE_LANES];
            let mut alpha = [0i32; SIMD_RESIZE_LANES];
            for (lane, pixel) in source_pixels.chunks_exact(2).enumerate() {
                luma[lane] = i32::from(pixel[0]);
                alpha[lane] = i32::from(pixel[1]);
            }
            let weight = i32::try_from(weight).ok()?;
            luma_sum += i32x8::new(luma) * i32x8::splat(weight);
            alpha_sum += i32x8::new(alpha) * i32x8::splat(weight);
        }
        let mut luma = luma_sum.to_array().map(resize_fixed_point_i32_to_u8);
        let alpha = alpha_sum.to_array().map(resize_fixed_point_i32_to_u8);
        let restored_luma = (f64x8::new(luma.map(f64::from)) * f64x8::splat(255.0)
            / f64x8::new(alpha.map(f64::from)).max(f64x8::splat(1.0)))
        .to_array();
        for lane in 0..count {
            if alpha[lane] != 0 {
                luma[lane] = restored_luma[lane] as u8;
            }
            let output_start = (output_x + lane).checked_mul(2)?;
            *output_row.get_mut(output_start)? = luma[lane];
            *output_row.get_mut(output_start.checked_add(1)?)? = alpha[lane];
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }

    let scalar_start = if output_width < SIMD_RESIZE_LANES {
        output_width
    } else {
        vector_width
    };
    for output_x in scalar_start..output_width {
        let mut luma_sum = 0.0;
        let mut alpha_sum = 0.0;
        for (tap, &weight) in weights.iter().enumerate() {
            let source_y = y0.checked_add(tap)?;
            if source_y >= source_height {
                return None;
            }
            let source_start = source_y
                .checked_mul(output_width)?
                .checked_add(output_x)?
                .checked_mul(2)?;
            luma_sum += f64::from(*intermediate.get(source_start)?) * weight as f64;
            alpha_sum +=
                f64::from(*intermediate.get(source_start.checked_add(1)?)?) * weight as f64;
        }
        let mut luma = resize_fixed_point_to_u8(luma_sum);
        let alpha = resize_fixed_point_to_u8(alpha_sum);
        if alpha != 0 {
            luma = (f64::from(luma) * 255.0 / f64::from(alpha)) as u8;
        }
        let output_start = output_x.checked_mul(2)?;
        *output_row.get_mut(output_start)? = luma;
        *output_row.get_mut(output_start.checked_add(1)?)? = alpha;
        scalar_tail = scalar_tail.saturating_add(1);
    }
    Some((vector_blocks, scalar_tail))
}

/// HSV is stored as three independent native bytes per pixel. When the
/// coefficient bound proves every ordered byte-sample sum fits in i32, use
/// native 32-bit SIMD accumulators instead of widening each channel multiply
/// to i64. This path is selected only for ImageOps.cover's HSV layout; unsafe
/// coefficient tables retain the general wide accumulator.
pub(in crate::compute::pool_simd::ops::adapters) fn resize_horizontal_hsv_i32_vector_row(
    source_row: &[u8],
    coeffs: &FilterCoeffs,
    plan: &ResizeHorizontalPlan,
    output_width: usize,
    output_row: &mut [u8],
) -> Option<(u64, u64)> {
    if source_row.is_empty() {
        output_row.fill(0);
        let vector_blocks = u64::try_from(plan.blocks.len()).ok()?;
        let scalar_start = if output_width < SIMD_RESIZE_LANES {
            output_width
        } else {
            plan.vector_width
        };
        return Some((
            vector_blocks,
            u64::try_from(output_width.saturating_sub(scalar_start)).ok()?,
        ));
    }

    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for (block_index, output_x) in (0..plan.vector_width)
        .step_by(SIMD_RESIZE_LANES)
        .enumerate()
    {
        let block = plan.blocks.get(block_index)?;
        let mut red_sum = i32x8::splat(0);
        let mut green_sum = i32x8::splat(0);
        let mut blue_sum = i32x8::splat(0);
        for tap in &block.taps {
            let mut red_samples = [0i32; SIMD_RESIZE_LANES];
            let mut green_samples = [0i32; SIMD_RESIZE_LANES];
            let mut blue_samples = [0i32; SIMD_RESIZE_LANES];
            for lane in 0..SIMD_RESIZE_LANES {
                if tap.weights[lane] != 0 {
                    let source_base = tap.source_bases[lane];
                    red_samples[lane] = i32::from(*source_row.get(source_base)?);
                    green_samples[lane] = i32::from(*source_row.get(source_base.checked_add(1)?)?);
                    blue_samples[lane] = i32::from(*source_row.get(source_base.checked_add(2)?)?);
                }
            }
            let weights = i32x8::new(tap.weights);
            red_sum += i32x8::new(red_samples) * weights;
            green_sum += i32x8::new(green_samples) * weights;
            blue_sum += i32x8::new(blue_samples) * weights;
        }
        let red = red_sum.to_array().map(resize_fixed_point_i32_to_u8);
        let green = green_sum.to_array().map(resize_fixed_point_i32_to_u8);
        let blue = blue_sum.to_array().map(resize_fixed_point_i32_to_u8);
        for lane in 0..SIMD_RESIZE_LANES {
            let output_x = output_x + lane;
            if output_x < output_width {
                let output_start = output_x.checked_mul(3)?;
                *output_row.get_mut(output_start)? = red[lane];
                *output_row.get_mut(output_start.checked_add(1)?)? = green[lane];
                *output_row.get_mut(output_start.checked_add(2)?)? = blue[lane];
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
        let output_start = output_x.checked_mul(3)?;
        for channel in 0..3 {
            *output_row.get_mut(output_start.checked_add(channel)?)? =
                resize_horizontal_scalar(source_row, 3, coeffs, output_x, channel, false)?;
        }
        scalar_tail = scalar_tail.saturating_add(1);
    }
    Some((vector_blocks, scalar_tail))
}

/// HSV vertical convolution paired with the checked 32-bit horizontal path.
/// The samples are read from contiguous triples without a mode conversion.
pub(in crate::compute::pool_simd::ops::adapters) fn resize_vertical_hsv_i32_vector_row(
    intermediate: &[u8],
    output_width: usize,
    source_height: usize,
    coeffs: &FilterCoeffs,
    output_y: usize,
    output_row: &mut [u8],
) -> Option<(u64, u64)> {
    let weights = resize_coeff_slice(coeffs, output_y)?;
    let y0 = usize::try_from(*coeffs.xmin.get(output_y)?).ok()?;
    let vector_width = if output_width < SIMD_RESIZE_LANES {
        SIMD_RESIZE_LANES
    } else {
        output_width / SIMD_RESIZE_LANES * SIMD_RESIZE_LANES
    };
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for output_x in (0..vector_width).step_by(SIMD_RESIZE_LANES) {
        let count = output_width.saturating_sub(output_x).min(SIMD_RESIZE_LANES);
        let mut red_sum = i32x8::splat(0);
        let mut green_sum = i32x8::splat(0);
        let mut blue_sum = i32x8::splat(0);
        for (tap, &weight) in weights.iter().enumerate() {
            let source_y = y0.checked_add(tap)?;
            if source_y >= source_height {
                return None;
            }
            let source_start = source_y
                .checked_mul(output_width)?
                .checked_add(output_x)?
                .checked_mul(3)?;
            let source_end = source_start.checked_add(count.checked_mul(3)?)?;
            let source_pixels = intermediate.get(source_start..source_end)?;
            let mut red_samples = [0i32; SIMD_RESIZE_LANES];
            let mut green_samples = [0i32; SIMD_RESIZE_LANES];
            let mut blue_samples = [0i32; SIMD_RESIZE_LANES];
            for (lane, pixel) in source_pixels.chunks_exact(3).enumerate() {
                red_samples[lane] = i32::from(pixel[0]);
                green_samples[lane] = i32::from(pixel[1]);
                blue_samples[lane] = i32::from(pixel[2]);
            }
            let weight = i32x8::splat(i32::try_from(weight).ok()?);
            red_sum += i32x8::new(red_samples) * weight;
            green_sum += i32x8::new(green_samples) * weight;
            blue_sum += i32x8::new(blue_samples) * weight;
        }
        let red = red_sum.to_array().map(resize_fixed_point_i32_to_u8);
        let green = green_sum.to_array().map(resize_fixed_point_i32_to_u8);
        let blue = blue_sum.to_array().map(resize_fixed_point_i32_to_u8);
        for lane in 0..count {
            let output_start = (output_x + lane).checked_mul(3)?;
            *output_row.get_mut(output_start)? = red[lane];
            *output_row.get_mut(output_start.checked_add(1)?)? = green[lane];
            *output_row.get_mut(output_start.checked_add(2)?)? = blue[lane];
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }

    let scalar_start = if output_width < SIMD_RESIZE_LANES {
        output_width
    } else {
        vector_width
    };
    for output_x in scalar_start..output_width {
        let output_start = output_x.checked_mul(3)?;
        for channel in 0..3 {
            let mut sum = 0i32;
            for (tap, &weight) in weights.iter().enumerate() {
                let source_y = y0.checked_add(tap)?;
                if source_y >= source_height {
                    return None;
                }
                let source_index = source_y
                    .checked_mul(output_width)?
                    .checked_add(output_x)?
                    .checked_mul(3)?
                    .checked_add(channel)?;
                sum += i32::from(*intermediate.get(source_index)?) * i32::try_from(weight).ok()?;
            }
            *output_row.get_mut(output_start.checked_add(channel)?)? =
                resize_fixed_point_i32_to_u8(sum);
        }
        scalar_tail = scalar_tail.saturating_add(1);
    }
    Some((vector_blocks, scalar_tail))
}

/// Accumulate four interleaved byte channels with bounded i32 sums. CMYK uses
/// four independent samples; RGBA reaches this kernel only after the caller
/// has premultiplied its native RGB channels by alpha.
#[allow(clippy::arithmetic_side_effects)] // The coefficient bound guarantees SIMD sums cannot wrap.
fn resize_horizontal_four_channel_i32_vector_row(
    source_row: &[u8],
    coeffs: &FilterCoeffs,
    plan: &ResizeHorizontalPlan,
    output_width: usize,
    output_row: &mut [u8],
) -> Option<(u64, u64)> {
    if source_row.is_empty() {
        output_row.fill(0);
        let vector_blocks = u64::try_from(plan.blocks.len()).ok()?;
        let scalar_start = if output_width < SIMD_RESIZE_LANES {
            output_width
        } else {
            plan.vector_width
        };
        return Some((
            vector_blocks,
            u64::try_from(output_width.saturating_sub(scalar_start)).ok()?,
        ));
    }

    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for (block_index, output_x) in (0..plan.vector_width)
        .step_by(SIMD_RESIZE_LANES)
        .enumerate()
    {
        let block = plan.blocks.get(block_index)?;
        let mut sums = [i32x8::splat(0); 4];
        for tap in &block.taps {
            let mut samples = [[0i32; SIMD_RESIZE_LANES]; 4];
            for lane in 0..SIMD_RESIZE_LANES {
                if tap.weights[lane] != 0 {
                    let source_start = tap.source_bases[lane];
                    let source_pixel =
                        source_row.get(source_start..source_start.checked_add(4)?)?;
                    samples[0][lane] = i32::from(source_pixel[0]);
                    samples[1][lane] = i32::from(source_pixel[1]);
                    samples[2][lane] = i32::from(source_pixel[2]);
                    samples[3][lane] = i32::from(source_pixel[3]);
                }
            }
            let weights = i32x8::new(tap.weights);
            for (sum, channel_samples) in sums.iter_mut().zip(samples) {
                *sum += i32x8::new(channel_samples) * weights;
            }
        }
        let result = sums.map(|sum| sum.to_array().map(resize_fixed_point_i32_to_u8));
        for lane in 0..SIMD_RESIZE_LANES {
            let x = output_x + lane;
            if x < output_width {
                let output_start = x.checked_mul(4)?;
                for (channel, channel_result) in result.iter().enumerate() {
                    *output_row.get_mut(output_start.checked_add(channel)?)? = channel_result[lane];
                }
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
        let output_start = output_x.checked_mul(4)?;
        for channel in 0..4 {
            *output_row.get_mut(output_start.checked_add(channel)?)? =
                resize_horizontal_scalar(source_row, 4, coeffs, output_x, channel, false)?;
        }
        scalar_tail = scalar_tail.saturating_add(1);
    }
    Some((vector_blocks, scalar_tail))
}

/// Vertical counterpart to the four-channel i32 horizontal pass. For RGBA,
/// `premultiplied_alpha` means RGB is restored after filtering; CMYK keeps all
/// four stored samples independent.
#[allow(clippy::arithmetic_side_effects)] // The coefficient bound guarantees SIMD sums cannot wrap.
fn resize_vertical_four_channel_i32_vector_row(
    intermediate: &[u8],
    output_width: usize,
    source_height: usize,
    coeffs: &FilterCoeffs,
    output_y: usize,
    output_row: &mut [u8],
    premultiplied_alpha: bool,
) -> Option<(u64, u64)> {
    let weights = resize_coeff_slice(coeffs, output_y)?;
    let y0 = usize::try_from(*coeffs.xmin.get(output_y)?).ok()?;
    let vector_width = if output_width < SIMD_RESIZE_LANES {
        SIMD_RESIZE_LANES
    } else {
        output_width / SIMD_RESIZE_LANES * SIMD_RESIZE_LANES
    };
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for output_x in (0..vector_width).step_by(SIMD_RESIZE_LANES) {
        let count = output_width.saturating_sub(output_x).min(SIMD_RESIZE_LANES);
        let mut sums = [i32x8::splat(0); 4];
        for (tap, &weight) in weights.iter().enumerate() {
            let source_y = y0.checked_add(tap)?;
            if source_y >= source_height {
                return None;
            }
            let source_start = source_y
                .checked_mul(output_width)?
                .checked_add(output_x)?
                .checked_mul(4)?;
            let source_end = source_start.checked_add(count.checked_mul(4)?)?;
            let source_pixels = intermediate.get(source_start..source_end)?;
            let mut samples = [[0i32; SIMD_RESIZE_LANES]; 4];
            for (lane, pixel) in source_pixels.chunks_exact(4).enumerate() {
                for channel in 0..4 {
                    samples[channel][lane] = i32::from(pixel[channel]);
                }
            }
            let weight = i32x8::splat(i32::try_from(weight).ok()?);
            for (sum, channel_samples) in sums.iter_mut().zip(samples) {
                *sum += i32x8::new(channel_samples) * weight;
            }
        }
        let mut result = sums.map(|sum| sum.to_array().map(resize_fixed_point_i32_to_u8));
        if premultiplied_alpha {
            for lane in 0..SIMD_RESIZE_LANES {
                let alpha = result[3][lane];
                for channel in &mut result[..3] {
                    channel[lane] = resize_unpremultiply_u8(channel[lane], alpha);
                }
            }
        }
        for lane in 0..count {
            let output_start = (output_x + lane).checked_mul(4)?;
            for (channel, channel_result) in result.iter().enumerate() {
                *output_row.get_mut(output_start.checked_add(channel)?)? = channel_result[lane];
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
        let output_start = output_x.checked_mul(4)?;
        let mut pixel = [0u8; 4];
        for (channel, output) in pixel.iter_mut().enumerate() {
            let mut sum = 0i32;
            for (tap, &weight) in weights.iter().enumerate() {
                let source_y = y0.checked_add(tap)?;
                if source_y >= source_height {
                    return None;
                }
                let source_index = source_y
                    .checked_mul(output_width)?
                    .checked_add(output_x)?
                    .checked_mul(4)?
                    .checked_add(channel)?;
                sum += i32::from(*intermediate.get(source_index)?) * i32::try_from(weight).ok()?;
            }
            *output = resize_fixed_point_i32_to_u8(sum);
        }
        if premultiplied_alpha {
            let alpha = pixel[3];
            for channel in &mut pixel[..3] {
                *channel = resize_unpremultiply_u8(*channel, alpha);
            }
        }
        output_row
            .get_mut(output_start..output_start.checked_add(4)?)?
            .copy_from_slice(&pixel);
        scalar_tail = scalar_tail.saturating_add(1);
    }
    Some((vector_blocks, scalar_tail))
}

pub(super) fn resize_horizontal_cover_i32_vector_row(
    source_row: &[u8],
    channels: usize,
    coeffs: &FilterCoeffs,
    plan: &ResizeHorizontalPlan,
    output_width: usize,
    output_row: &mut [u8],
    premultiplied_alpha: bool,
    luma_is_premultiplied: bool,
) -> Option<(u64, u64)> {
    match (channels, premultiplied_alpha) {
        (1, false) => resize_horizontal_luma_i32_vector_row(
            source_row,
            coeffs,
            plan,
            output_width,
            output_row,
        ),
        (2, true) => resize_horizontal_la_i32_vector_row(
            source_row,
            coeffs,
            plan,
            output_width,
            output_row,
            luma_is_premultiplied,
        ),
        (3, false) => {
            resize_horizontal_hsv_i32_vector_row(source_row, coeffs, plan, output_width, output_row)
        }
        (4, false) => resize_horizontal_four_channel_i32_vector_row(
            source_row,
            coeffs,
            plan,
            output_width,
            output_row,
        ),
        (4, true) => resize_horizontal_four_channel_i32_vector_row(
            source_row,
            coeffs,
            plan,
            output_width,
            output_row,
        ),
        _ => None,
    }
}

pub(super) fn resize_vertical_cover_i32_vector_row(
    intermediate: &[u8],
    output_width: usize,
    source_height: usize,
    channels: usize,
    coeffs: &FilterCoeffs,
    output_y: usize,
    output_row: &mut [u8],
    premultiplied_alpha: bool,
) -> Option<(u64, u64)> {
    match (channels, premultiplied_alpha) {
        (1, false) => resize_vertical_luma_i32_vector_row(
            intermediate,
            output_width,
            source_height,
            coeffs,
            output_y,
            output_row,
        ),
        (2, true) => resize_vertical_la_i32_vector_row(
            intermediate,
            output_width,
            source_height,
            coeffs,
            output_y,
            output_row,
        ),
        (3, false) => resize_vertical_hsv_i32_vector_row(
            intermediate,
            output_width,
            source_height,
            coeffs,
            output_y,
            output_row,
        ),
        (4, false) => resize_vertical_four_channel_i32_vector_row(
            intermediate,
            output_width,
            source_height,
            coeffs,
            output_y,
            output_row,
            false,
        ),
        (4, true) => resize_vertical_four_channel_i32_vector_row(
            intermediate,
            output_width,
            source_height,
            coeffs,
            output_y,
            output_row,
            true,
        ),
        _ => None,
    }
}
