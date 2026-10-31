//! Coefficient validation and horizontal lane plans for native-byte Resize.

use super::constants::SIMD_RESIZE_LANES;
use crate::ops::pil_resize::FilterCoeffs;

#[inline]
pub(super) fn resize_coeff_slice(
    coeffs: &FilterCoeffs,
    index: usize,
) -> Option<&[i64]> {
    let start = *coeffs.offsets.get(index)?;
    let count = *coeffs.count.get(index)?;
    coeffs.weights.get(start..start.checked_add(count)?)
}

#[derive(Clone)]
pub(super) struct ResizeHorizontalTapPlan {
    pub(super) source_bases: [usize; SIMD_RESIZE_LANES],
    pub(super) weights: [i32; SIMD_RESIZE_LANES],
}

pub(super) struct ResizeHorizontalBlockPlan {
    pub(super) taps: Vec<ResizeHorizontalTapPlan>,
}

pub(super) struct ResizeHorizontalPlan {
    pub(super) vector_width: usize,
    pub(super) blocks: Vec<ResizeHorizontalBlockPlan>,
}

/// Precompute lane-specific source offsets and fixed-point weights once per
/// resize call. They are independent of the source row, so rebuilding them
/// inside every row needlessly repeats checked arithmetic and coefficient
/// conversion.
pub(super) fn build_resize_horizontal_plan<const CHANNELS: usize>(
    coeffs: &FilterCoeffs,
    output_width: usize,
) -> Option<ResizeHorizontalPlan> {
    let vector_width = if output_width < SIMD_RESIZE_LANES {
        SIMD_RESIZE_LANES
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
                    let source_base = source_x.checked_mul(CHANNELS)?;
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
