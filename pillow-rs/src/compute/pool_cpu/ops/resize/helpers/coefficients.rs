//! Shared coefficient preparation for native typed Resize strategies.

use super::typed_nearest::BoxBounds;
use crate::ops::pil_resize::{
    FilterCoeffs, FilterCoeffsF64, filter_from_resample, precompute_coeffs_f64,
    precompute_coeffs_f64_boxed,
};
use crate::pipeline::ResampleFilter;
use std::sync::Arc;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn for_resize(
    source: (u32, u32),
    destination: (u32, u32),
    filter: ResampleFilter,
    bounds: BoxBounds,
) -> (Arc<FilterCoeffsF64>, Arc<FilterCoeffsF64>) {
    let (kernel, support) = filter_from_resample(filter);
    match bounds {
        Some((left, top, right, bottom)) => (
            Arc::new(precompute_coeffs_f64_boxed(
                destination.0,
                source.0,
                left,
                right,
                filter,
            )),
            Arc::new(precompute_coeffs_f64_boxed(
                destination.1,
                source.1,
                top,
                bottom,
                filter,
            )),
        ),
        None => (
            precompute_coeffs_f64(destination.0, source.0, kernel, support),
            precompute_coeffs_f64(destination.1, source.1, kernel, support),
        ),
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn resample_pass_requirements(
    source: (u32, u32),
    destination: (u32, u32),
    bounds: BoxBounds,
) -> (bool, bool) {
    match bounds {
        Some((left, top, right, bottom)) => {
            let left = left as f32 as f64;
            let top = top as f32 as f64;
            let right = right as f32 as f64;
            let bottom = bottom as f32 as f64;
            (
                destination.0 != source.0 || left != 0.0 || right != f64::from(destination.0),
                destination.1 != source.1 || top != 0.0 || bottom != f64::from(destination.1),
            )
        }
        None => (destination.0 != source.0, destination.1 != source.1),
    }
}

/// Match Pillow's CPU resampler pass order for unboxed and boxed requests.
///
/// Pillow runs the vertical pass first when height is reduced by more than
/// twice the horizontal size change. The comparison uses image dimensions,
/// including when the source box is narrower than the full image.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn horizontal_first(source: (u32, u32), destination: (u32, u32)) -> bool {
    let horizontal_reduction = i64::from(source.0) - i64::from(destination.0);
    let vertical_reduction = i64::from(source.1) - i64::from(destination.1);
    !(vertical_reduction > 0 && vertical_reduction > horizontal_reduction * 2)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn byte_coefficients_fit_i32(coefficients: &FilterCoeffs) -> bool {
    if coefficients.count.len() != coefficients.offsets.len()
        || coefficients.xmin.len() != coefficients.offsets.len()
    {
        return false;
    }
    for index in 0..coefficients.count.len() {
        let Some(weights) = coefficients
            .offsets
            .get(index)
            .and_then(|&start| {
                start
                    .checked_add(*coefficients.count.get(index)?)
                    .map(|end| (start, end))
            })
            .and_then(|(start, end)| coefficients.weights.get(start..end))
        else {
            return false;
        };
        let Some(sum_abs) = weights.iter().try_fold(0_u64, |sum, &weight| {
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

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn byte_coefficients_fit_source(
    coefficients: &FilterCoeffs,
    source_extent: usize,
) -> bool {
    if coefficients.xmin.len() != coefficients.count.len()
        || coefficients.xmin.len() != coefficients.offsets.len()
    {
        return false;
    }
    coefficients
        .xmin
        .iter()
        .zip(&coefficients.count)
        .all(|(&first, &count)| {
            usize::try_from(first)
                .ok()
                .and_then(|first| first.checked_add(count))
                .is_some_and(|end| end <= source_extent)
        })
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
#[inline(always)]
pub(crate) fn fixed_point_to_u8_i32(sum: i32) -> u8 {
    ((sum + (1_i32 << 21)) >> 22).clamp(0, 255) as u8
}
