//! integer for GPU resize exactness.

use super::super::*;

/// Round an exact signed binary sum to Pillow's INT32 `ROUND_UP` result.
///
/// `sum * 2^scale_exp` is kept as an integer/rational pair so the admission
/// proof does not rely on a host f64 conversion at a half-integer boundary.
/// The returned value is deliberately restricted to the signed i32 range;
/// overflowing rows stay on exact host semantic control rather than relying
/// on device or Rust cast saturation.
pub(in crate::compute::pool_gpu) fn gpu_i_f64_integer_to_i32(
    sum: i128,
    scale_exp: i32,
) -> Option<i32> {
    if sum == 0 {
        return Some(0);
    }
    let negative = sum < 0;
    let magnitude = if negative {
        sum.checked_neg()? as u128
    } else {
        sum as u128
    };
    let rounded = if scale_exp >= 0 {
        magnitude.checked_shl(u32::try_from(scale_exp).ok()?)?
    } else {
        let shift = u32::try_from(scale_exp.checked_neg()?).ok()?;
        if shift >= 128 {
            0
        } else if shift == 0 {
            magnitude
        } else {
            let mut quotient = magnitude >> shift;
            let remainder = magnitude & ((1u128 << shift) - 1);
            let halfway = 1u128 << (shift - 1);
            // ROUND_UP is away from zero, so exact half values increment too.
            if remainder >= halfway {
                quotient = quotient.checked_add(1)?;
            }
            quotient
        }
    };
    if negative {
        if rounded > (1u128 << 31) {
            return None;
        }
        if rounded == (1u128 << 31) {
            Some(i32::MIN)
        } else {
            Some(-(i32::try_from(rounded).ok()?))
        }
    } else {
        Some(i32::try_from(rounded).ok()?)
    }
}

#[derive(Clone, Copy)]
pub(in crate::compute::pool_gpu) struct GpuIResizeOutputPlan {
    pub(in crate::compute::pool_gpu) source_start: usize,
    pub(in crate::compute::pool_gpu) tap_start: usize,
    pub(in crate::compute::pool_gpu) tap_count: usize,
    pub(in crate::compute::pool_gpu) minimum_exponent: Option<i32>,
    pub(in crate::compute::pool_gpu) aligned: bool,
}

pub(in crate::compute::pool_gpu) struct GpuIResizePassPlan {
    pub(in crate::compute::pool_gpu) outputs: Vec<GpuIResizeOutputPlan>,
    pub(in crate::compute::pool_gpu) aligned_coefficients: Vec<i64>,
}

/// Align each row's dyadic weights once for the complete pass. The plan is
/// local to the I-mode proof and leaves the GPU kernel's coefficient table
/// and arithmetic unchanged.
pub(in crate::compute::pool_gpu) fn gpu_i_resize_f64_pass_plan(
    coeffs: &FilterCoeffsF64,
    source_axis: usize,
) -> Option<GpuIResizePassPlan> {
    if coeffs.xmin.len() != coeffs.count.len() || coeffs.xmin.len() != coeffs.weights.len() {
        return None;
    }
    let total_taps = coeffs
        .count
        .iter()
        .try_fold(0usize, |total, count| total.checked_add(*count))?;
    let mut outputs = Vec::new();
    outputs.try_reserve_exact(coeffs.xmin.len()).ok()?;
    let mut aligned_coefficients = Vec::new();
    aligned_coefficients.try_reserve_exact(total_taps).ok()?;

    for output_index in 0..coeffs.xmin.len() {
        let source_start = usize::try_from(coeffs.xmin[output_index]).ok()?;
        let weights = &coeffs.weights[output_index];
        let tap_count = coeffs.count[output_index];
        if tap_count != weights.len() || source_start.checked_add(tap_count)? > source_axis {
            return None;
        }
        let mut minimum_exponent = None;
        for &weight in weights {
            let parts = gpu_f64_integer_parts(weight)?;
            if parts.mantissa != 0 {
                minimum_exponent = Some(
                    minimum_exponent
                        .map_or(parts.exponent, |minimum: i32| minimum.min(parts.exponent)),
                );
            }
        }
        let tap_start = aligned_coefficients.len();
        let mut aligned = true;
        for &weight in weights {
            let parts = gpu_f64_integer_parts(weight)?;
            let coefficient = if parts.mantissa == 0 {
                Some(0)
            } else {
                minimum_exponent.and_then(|minimum| {
                    let shift = u32::try_from(parts.exponent.checked_sub(minimum)?).ok()?;
                    let factor = if shift >= 127 {
                        None
                    } else {
                        Some(1i128 << shift)
                    }?;
                    let magnitude = i128::from(parts.mantissa).checked_mul(factor)?;
                    let signed = if parts.negative {
                        magnitude.checked_neg()?
                    } else {
                        magnitude
                    };
                    i64::try_from(signed).ok()
                })
            };
            match coefficient {
                Some(coefficient) => aligned_coefficients.push(coefficient),
                None => {
                    aligned = false;
                    aligned_coefficients.push(0);
                }
            }
        }
        if !aligned {
            aligned_coefficients[tap_start..].fill(0);
        }
        outputs.push(GpuIResizeOutputPlan {
            source_start,
            tap_start,
            tap_count,
            minimum_exponent,
            aligned,
        });
    }
    Some(GpuIResizePassPlan {
        outputs,
        aligned_coefficients,
    })
}

/// Evaluate an aligned I-mode coefficient row with an exact integer reducer.
///
/// Each integer coefficient is the original dyadic f64 coefficient scaled to
/// the row's minimum binary exponent. This keeps the exact sum unchanged while
/// removing per-pixel exponent alignment from the admission proof.
pub(in crate::compute::pool_gpu) fn gpu_i_resize_f64_sample_aligned(
    samples: &[i32],
    source_width: usize,
    source_start: usize,
    weights: &[f64],
    aligned_coefficients: &[i64],
    minimum_exponent: Option<i32>,
    horizontal: bool,
    line: usize,
) -> Option<i32> {
    if weights.len() != aligned_coefficients.len() {
        return None;
    }
    let Some(minimum_exponent) = minimum_exponent else {
        return Some(0);
    };
    let mut pixel = if horizontal {
        line.checked_mul(source_width)?.checked_add(source_start)?
    } else {
        source_start.checked_mul(source_width)?.checked_add(line)?
    };
    let pixel_step = if horizontal { 1 } else { source_width };

    let mut exact_sum = 0i128;
    let mut ordered_f64 = 0.0f64;
    for (tap_index, (&weight, &coefficient)) in weights.iter().zip(aligned_coefficients).enumerate()
    {
        let sample = *samples.get(pixel)?;
        ordered_f64 = weight.mul_add(f64::from(sample), ordered_f64);
        if coefficient != 0 && sample != 0 {
            let term = i128::from(sample).checked_mul(i128::from(coefficient))?;
            exact_sum = exact_sum.checked_add(term)?;
        }
        if tap_index + 1 < weights.len() {
            pixel = pixel.checked_add(pixel_step)?;
        }
    }
    if !ordered_f64.is_finite() {
        return None;
    }
    let expected = round_up(ordered_f64);
    if !expected.is_finite() {
        return None;
    }
    let expected = i32::try_from(expected as i64).ok()?;
    let actual = gpu_i_f64_integer_to_i32(exact_sum, minimum_exponent)?;
    (actual == expected).then_some(actual)
}

/// Keep the general exact reducer for coefficient rows whose common integer
/// scale does not fit in the compact aligned plan.
pub(in crate::compute::pool_gpu) fn gpu_i_resize_f64_sample_slow(
    samples: &[i32],
    source_dimensions: (u32, u32),
    coeffs: &FilterCoeffsF64,
    output_index: usize,
    horizontal: bool,
    line: usize,
) -> Option<i32> {
    let source_w = usize::try_from(source_dimensions.0).ok()?;
    let source_h = usize::try_from(source_dimensions.1).ok()?;
    let source_start = usize::try_from(*coeffs.xmin.get(output_index)?).ok()?;
    let weights = coeffs.weights.get(output_index)?;
    if *coeffs.count.get(output_index)? != weights.len()
        || line >= if horizontal { source_h } else { source_w }
    {
        return None;
    }
    let source_pixel = |tap: usize| -> Option<usize> {
        let coordinate = source_start.checked_add(tap)?;
        if horizontal {
            (coordinate < source_w).then_some(line.checked_mul(source_w)?.checked_add(coordinate)?)
        } else {
            (coordinate < source_h).then_some(coordinate.checked_mul(source_w)?.checked_add(line)?)
        }
    };
    let sample_at = |tap: usize| -> Option<i32> {
        let pixel = source_pixel(tap)?;
        Some(*samples.get(pixel)?)
    };

    let mut minimum_exponent = None;
    let mut exact_sum = 0i128;
    let mut ordered_f64 = 0.0f64;
    for (tap, &weight) in weights.iter().enumerate() {
        let coeff = gpu_f64_integer_parts(weight)?;
        let sample = sample_at(tap)?;
        ordered_f64 = weight.mul_add(f64::from(sample), ordered_f64);
        if coeff.mantissa == 0 || sample == 0 {
            continue;
        }

        let exponent = coeff.exponent;
        let shift = match minimum_exponent {
            Some(minimum) if exponent < minimum => {
                let shift = u32::try_from(minimum.checked_sub(exponent)?).ok()?;
                if shift >= 127 {
                    if exact_sum != 0 {
                        return None;
                    }
                } else {
                    exact_sum = exact_sum.checked_mul(1i128 << shift)?;
                }
                minimum_exponent = Some(exponent);
                0
            }
            Some(minimum) => u32::try_from(exponent.checked_sub(minimum)?).ok()?,
            None => {
                minimum_exponent = Some(exponent);
                0
            }
        };
        let magnitude = if sample < 0 {
            i128::from(sample).checked_neg()?
        } else {
            i128::from(sample)
        };
        let product = magnitude.checked_mul(i128::try_from(coeff.mantissa).ok()?)?;
        let term = if shift >= 127 {
            return None;
        } else {
            product.checked_mul(1i128 << shift)?
        };
        let term = if sample < 0 {
            term.checked_neg()?
        } else {
            term
        };
        let term = if coeff.negative {
            term.checked_neg()?
        } else {
            term
        };
        exact_sum = exact_sum.checked_add(term)?;
    }
    let Some(minimum_exponent) = minimum_exponent else {
        return Some(0);
    };
    if !ordered_f64.is_finite() {
        return None;
    }
    let expected = round_up(ordered_f64);
    if !expected.is_finite() {
        return None;
    }
    let expected = i32::try_from(expected as i64).ok()?;
    let actual = gpu_i_f64_integer_to_i32(exact_sum, minimum_exponent)?;
    (actual == expected).then_some(actual)
}

pub(in crate::compute::pool_gpu) fn gpu_i_resize_f64_sample(
    samples: &[i32],
    source_dimensions: (u32, u32),
    coeffs: &FilterCoeffsF64,
    output_index: usize,
    plan: &GpuIResizePassPlan,
    horizontal: bool,
    line: usize,
) -> Option<i32> {
    let source_width = usize::try_from(source_dimensions.0).ok()?;
    let source_height = usize::try_from(source_dimensions.1).ok()?;
    if line
        >= if horizontal {
            source_height
        } else {
            source_width
        }
    {
        return None;
    }
    let output = *plan.outputs.get(output_index)?;
    if !output.aligned {
        return gpu_i_resize_f64_sample_slow(
            samples,
            source_dimensions,
            coeffs,
            output_index,
            horizontal,
            line,
        );
    }
    let weights = coeffs.weights.get(output_index)?;
    if weights.len() != output.tap_count {
        return None;
    }
    let tap_end = output.tap_start.checked_add(output.tap_count)?;
    let aligned_coefficients = plan.aligned_coefficients.get(output.tap_start..tap_end)?;
    let fast = gpu_i_resize_f64_sample_aligned(
        samples,
        source_width,
        output.source_start,
        weights,
        aligned_coefficients,
        output.minimum_exponent,
        horizontal,
        line,
    );
    fast.or_else(|| {
        gpu_i_resize_f64_sample_slow(
            samples,
            source_dimensions,
            coeffs,
            output_index,
            horizontal,
            line,
        )
    })
}

/// Evaluate every word produced by one exact I-mode resize pass. The result
/// order matches the packed image rows consumed by the next shader pass.
pub(in crate::compute::pool_gpu) fn gpu_i_resize_f64_pass_values(
    samples: &[i32],
    source_dimensions: (u32, u32),
    coeffs: &FilterCoeffsF64,
    plan: &GpuIResizePassPlan,
    horizontal: bool,
) -> Option<Vec<i32>> {
    let output_count = plan.outputs.len();
    let source_w = usize::try_from(source_dimensions.0).ok()?;
    let source_axis = if horizontal {
        source_w
    } else {
        usize::try_from(source_dimensions.1).ok()?
    };
    let line_count = if horizontal {
        usize::try_from(source_dimensions.1).ok()?
    } else {
        usize::try_from(source_dimensions.0).ok()?
    };
    let word_count = output_count.checked_mul(line_count)?;
    if word_count > GPU_BUFFER_CAPACITY as usize {
        return None;
    }
    if output_count == source_axis {
        if samples.len() != word_count {
            return None;
        }
        return Some(samples.to_vec());
    }
    let mut result = Vec::new();
    result.try_reserve(word_count).ok()?;
    if horizontal {
        for line in 0..line_count {
            for output_index in 0..output_count {
                result.push(gpu_i_resize_f64_sample(
                    samples,
                    source_dimensions,
                    coeffs,
                    output_index,
                    plan,
                    true,
                    line,
                )?);
            }
        }
    } else {
        for output_index in 0..output_count {
            for line in 0..line_count {
                result.push(gpu_i_resize_f64_sample(
                    samples,
                    source_dimensions,
                    coeffs,
                    output_index,
                    plan,
                    false,
                    line,
                )?);
            }
        }
    }
    Some(result)
}

/// Prove one pure filtered I resize for the typed GPU reducer. Horizontal
/// results are rounded to INT32 before the vertical pass, matching Pillow's
/// `ImagingResampleHorizontal_32bpc`/`Vertical_32bpc` storage boundary.
pub(in crate::compute::pool_gpu) fn gpu_i_resize_f64_is_exact(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if logical_mode != Some("I") || ops.len() != 1 || !matches!(image, DynamicImage::ImageRgba8(_))
    {
        return false;
    }
    let PipelineOp::Resize { w, h, filter } = &ops[0] else {
        return false;
    };
    if matches!(filter, ResampleFilter::Nearest) || *w == 0 || *h == 0 {
        return false;
    }
    let source_dimensions = image.dimensions();
    if source_dimensions.0 == 0 || source_dimensions.1 == 0 {
        return false;
    }
    let Ok(source_width) = usize::try_from(source_dimensions.0) else {
        return false;
    };
    let Ok(source_height) = usize::try_from(source_dimensions.1) else {
        return false;
    };
    let DynamicImage::ImageRgba8(pixels) = image else {
        return false;
    };
    let expected = CheckedDims::new(source_dimensions.0, source_dimensions.1, 4)
        .ok()
        .map(|dims| dims.total_bytes());
    if expected != Some(pixels.as_raw().len())
        || CheckedDims::new(*w, *h, 1)
            .ok()
            .map(|dims| dims.total_pixels())
            .is_none_or(|count| count > GPU_BUFFER_CAPACITY as usize)
    {
        return false;
    }

    let (kernel, support) = filter_from_resample(*filter);
    let horizontal = precompute_coeffs_f64(*w, source_dimensions.0, kernel, support);
    let vertical = precompute_coeffs_f64(*h, source_dimensions.1, kernel, support);
    let Some(horizontal_plan) = gpu_i_resize_f64_pass_plan(&horizontal, source_width) else {
        return false;
    };
    let Some(vertical_plan) = gpu_i_resize_f64_pass_plan(&vertical, source_height) else {
        return false;
    };

    let bytes = pixels.as_raw();
    let samples = i32_samples_from_le_bytes(bytes, bytes.len() / 4);
    if samples.len().checked_mul(4) != Some(bytes.len()) {
        return false;
    }
    let horizontal_values = gpu_i_resize_f64_pass_values(
        &samples,
        source_dimensions,
        &horizontal,
        &horizontal_plan,
        true,
    );
    let Some(horizontal_values) = horizontal_values else {
        return false;
    };
    gpu_i_resize_f64_pass_values(
        &horizontal_values,
        (*w, source_dimensions.1),
        &vertical,
        &vertical_plan,
        false,
    )
    .is_some()
}
