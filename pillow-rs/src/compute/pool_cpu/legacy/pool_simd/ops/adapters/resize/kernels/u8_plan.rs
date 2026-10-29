//! Coefficient validation and horizontal lane plans for native-byte Resize.

use super::super::SIMD_RESIZE_LANES;
use crate::ops::pil_resize::FilterCoeffs;

#[inline]
pub(in crate::compute::pool_simd::ops::adapters) fn resize_coeff_slice(
    coeffs: &FilterCoeffs,
    index: usize,
) -> Option<&[i64]> {
    let start = *coeffs.offsets.get(index)?;
    let count = *coeffs.count.get(index)?;
    coeffs.weights.get(start..start.checked_add(count)?)
}

/// Prove that a vertical output has no nonzero intermediate row in its tap
/// window. False positives only keep extra work; false negatives could omit a
/// contribution, so the check conservatively treats every tap as active.
pub(in crate::compute::pool_simd::ops::adapters) fn resize_vertical_row_touches_nonzero_source(
    coeffs: &FilterCoeffs,
    source_height: usize,
    nonzero_rows: &[bool],
    output_y: usize,
) -> Option<bool> {
    let taps = resize_coeff_slice(coeffs, output_y)?;
    let first_source_y = usize::try_from(*coeffs.xmin.get(output_y)?).ok()?;
    for tap in 0..taps.len() {
        let source_y = first_source_y.checked_add(tap)?;
        if source_y >= source_height {
            return None;
        }
        if *nonzero_rows.get(source_y)? {
            return Some(true);
        }
    }
    Some(false)
}

/// Prove that every ordered partial sum for a byte sample row fits in i32,
/// including Pillow's positive fixed-point rounding bias. `wide::i32x8`
/// arithmetic wraps, so the specialized Cover path uses it only after both
/// resize-axis coefficient tables pass this proof.
pub(in crate::compute::pool_simd::ops::adapters) fn resize_u8_coefficients_fit_i32(
    coeffs: &FilterCoeffs,
) -> bool {
    if coeffs.count.len() != coeffs.offsets.len() {
        return false;
    }
    for index in 0..coeffs.count.len() {
        let Some(weights) = resize_coeff_slice(coeffs, index) else {
            return false;
        };
        let Some(sum_abs) = weights.iter().try_fold(0u64, |sum, &weight| {
            i32::try_from(weight).ok()?;
            sum.checked_add(weight.unsigned_abs())
        }) else {
            return false;
        };
        let Some(bound) = sum_abs
            .checked_mul(255)
            .and_then(|bound| bound.checked_add(1 << 21))
        else {
            return false;
        };
        if bound > i32::MAX as u64 {
            return false;
        }
    }
    true
}

#[derive(Clone)]
pub(in crate::compute::pool_simd::ops::adapters) struct ResizeHorizontalTapPlan {
    pub(in crate::compute::pool_simd::ops::adapters) source_bases: [usize; SIMD_RESIZE_LANES],
    pub(in crate::compute::pool_simd::ops::adapters) weights: [i32; SIMD_RESIZE_LANES],
}

pub(in crate::compute::pool_simd::ops::adapters) struct ResizeHorizontalBlockPlan {
    pub(in crate::compute::pool_simd::ops::adapters) taps: Vec<ResizeHorizontalTapPlan>,
}

pub(in crate::compute::pool_simd::ops::adapters) struct ResizeHorizontalPlan {
    pub(in crate::compute::pool_simd::ops::adapters) vector_width: usize,
    pub(in crate::compute::pool_simd::ops::adapters) blocks: Vec<ResizeHorizontalBlockPlan>,
}

/// Precompute lane-specific source offsets and fixed-point weights once per
/// resize call. They are independent of the source row, so rebuilding them
/// inside every row needlessly repeats checked arithmetic and coefficient
/// conversion.
pub(in crate::compute::pool_simd::ops::adapters) fn build_resize_horizontal_plan(
    coeffs: &FilterCoeffs,
    output_width: usize,
    channels: usize,
    vectorize_tail: bool,
) -> Option<ResizeHorizontalPlan> {
    let vector_width = if output_width < SIMD_RESIZE_LANES {
        SIMD_RESIZE_LANES
    } else if vectorize_tail {
        output_width
            .div_ceil(SIMD_RESIZE_LANES)
            .checked_mul(SIMD_RESIZE_LANES)?
    } else {
        output_width / SIMD_RESIZE_LANES * SIMD_RESIZE_LANES
    };
    let mut blocks = Vec::with_capacity(vector_width.div_ceil(SIMD_RESIZE_LANES));
    for output_x in (0..vector_width).step_by(SIMD_RESIZE_LANES) {
        let max_count = (0..SIMD_RESIZE_LANES)
            .filter_map(|lane| coeffs.count.get(output_x + lane))
            .copied()
            .max()
            .unwrap_or(0);
        let mut taps = Vec::with_capacity(max_count);
        for tap in 0..max_count {
            let mut source_bases = [0usize; SIMD_RESIZE_LANES];
            let mut weights = [0i32; SIMD_RESIZE_LANES];
            for lane in 0..SIMD_RESIZE_LANES {
                let index = output_x + lane;
                if let (Some(&count), Some(&xmin), Some(&offset)) = (
                    coeffs.count.get(index),
                    coeffs.xmin.get(index),
                    coeffs.offsets.get(index),
                ) && tap < count
                {
                    let source_x = usize::try_from(xmin).ok()?.checked_add(tap)?;
                    let source_base = source_x.checked_mul(channels)?;
                    let weight_index = offset.checked_add(tap)?;
                    source_bases[lane] = source_base;
                    weights[lane] = *coeffs.weights.get(weight_index)? as i32;
                }
            }
            taps.push(ResizeHorizontalTapPlan {
                source_bases,
                weights,
            });
        }
        blocks.push(ResizeHorizontalBlockPlan { taps });
    }
    Some(ResizeHorizontalPlan {
        vector_width,
        blocks,
    })
}
