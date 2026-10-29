//! luma16 for GPU resize exactness.

use super::super::*;

/// Evaluate one I;16 f64-coefficient row with the same f32-normalized integer
/// reducer used by the shader, then compare its independently-clipped u16
/// bytes with Pillow's ordered f64 accumulation.  The source u16 is converted
/// to an f32 word before it reaches WGSL, so the proof must account for that
/// significand/exponent representation (and reject rows whose aligned sum
/// exceeds the device reducer's four-limb envelope).
pub(in crate::compute::pool_gpu) fn gpu_luma16_f64_sample(
    samples: &[u16],
    source_dimensions: (u32, u32),
    coeffs: &FilterCoeffsF64,
    output_index: usize,
    horizontal: bool,
    line: usize,
    big_endian: bool,
) -> Option<u16> {
    let source_w = usize::try_from(source_dimensions.0).ok()?;
    let source_h = usize::try_from(source_dimensions.1).ok()?;
    let source_start = usize::try_from(*coeffs.xmin.get(output_index)?).ok()?;
    let weights = coeffs.weights.get(output_index)?;
    let expected_count = *coeffs.count.get(output_index)?;
    if expected_count != weights.len() {
        return None;
    }
    if line >= if horizontal { source_h } else { source_w } {
        return None;
    }
    let sample_at = |tap: usize| -> Option<u16> {
        let coordinate = source_start.checked_add(tap)?;
        let index = if horizontal {
            if coordinate >= source_w {
                return None;
            }
            line.checked_mul(source_w)?.checked_add(coordinate)?
        } else {
            if coordinate >= source_h {
                return None;
            }
            coordinate.checked_mul(source_w)?.checked_add(line)?
        };
        let sample = *samples.get(index)?;
        Some(luma16_resample_read(sample, big_endian))
    };

    let mut minimum_exponent = None;
    for (tap, &weight) in weights.iter().enumerate() {
        let coeff = gpu_f64_integer_parts(weight)?;
        if coeff.mantissa == 0 {
            continue;
        }
        let sample = sample_at(tap)?;
        if sample == 0 {
            continue;
        }
        let sample_parts = gpu_f32_f64_integer_parts(f32::from(sample).to_bits())?;
        if sample_parts.mantissa == 0 {
            continue;
        }
        let exponent = sample_parts
            .exponent
            .checked_sub(23)?
            .checked_add(coeff.exponent)?;
        minimum_exponent =
            Some(minimum_exponent.map_or(exponent, |minimum: i32| minimum.min(exponent)));
    }
    let Some(minimum_exponent) = minimum_exponent else {
        return Some(0);
    };

    let mut sum = 0i128;
    let mut f64_accumulator = 0.0f64;
    for (tap, &weight) in weights.iter().enumerate() {
        let coeff = gpu_f64_integer_parts(weight)?;
        let sample = sample_at(tap)?;
        // Keep this in the same ordered multiply/add form as the native
        // luma16 path (`horizontal_pass_luma16`/`vertical_pass_luma16`).
        f64_accumulator += f64::from(sample) * weight;
        if coeff.mantissa == 0 || sample == 0 {
            continue;
        }
        let sample_parts = gpu_f32_f64_integer_parts(f32::from(sample).to_bits())?;
        if sample_parts.mantissa == 0 {
            continue;
        }
        let exponent = sample_parts
            .exponent
            .checked_sub(23)?
            .checked_add(coeff.exponent)?;
        let shift = u32::try_from(exponent.checked_sub(minimum_exponent)?).ok()?;
        let product = u128::from(sample_parts.mantissa).checked_mul(u128::from(coeff.mantissa))?;
        let term = i128::try_from(product.checked_shl(shift)?).ok()?;
        sum = if coeff.negative {
            sum.checked_sub(term)?
        } else {
            sum.checked_add(term)?
        };
    }

    let expected = luma16_resample_write(f64_accumulator, big_endian);
    // The device reducer materializes an f32 before the I;16 byte-level
    // round/clip step.  That conversion can move a half-integer boundary even
    // though the exact integer sum agrees with the f64 accumulator, so prove
    // the same intermediate word rather than admitting on the exact sum
    // alone.
    let shader_bits = gpu_f64_integer_to_f32(sum, minimum_exponent)?;
    let shader_value = f32::from_bits(shader_bits);
    if !shader_value.is_finite() {
        return None;
    }
    let actual = luma16_resample_write(f64::from(shader_value), big_endian);
    (actual == expected).then_some(actual)
}

pub(in crate::compute::pool_gpu) fn gpu_luma16_f64_pass(
    samples: &[u16],
    source_dimensions: (u32, u32),
    coeffs: &FilterCoeffsF64,
    horizontal: bool,
    big_endian: bool,
) -> Option<Vec<u16>> {
    let output_count = coeffs.xmin.len();
    let source_axis = if horizontal {
        usize::try_from(source_dimensions.0).ok()?
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
    // Pillow's same-size typed pass is an identity after its byte-level
    // read/round/write boundary.  Mirror the shader's packed-word copy here
    // instead of feeding tiny Lanczos/Bicubic tail coefficients into the
    // integer reducer (those tails are numerically zero at u16 precision but
    // can otherwise consume the reducer's exponent budget).
    if output_count == source_axis {
        return (samples.len() == word_count).then(|| samples.to_vec());
    }
    let mut result = Vec::new();
    result.try_reserve(word_count).ok()?;
    if horizontal {
        for line in 0..line_count {
            for output_index in 0..output_count {
                result.push(gpu_luma16_f64_sample(
                    samples,
                    source_dimensions,
                    coeffs,
                    output_index,
                    true,
                    line,
                    big_endian,
                )?);
            }
        }
    } else {
        for output_index in 0..output_count {
            for line in 0..line_count {
                result.push(gpu_luma16_f64_sample(
                    samples,
                    source_dimensions,
                    coeffs,
                    output_index,
                    false,
                    line,
                    big_endian,
                )?);
            }
        }
    }
    Some(result)
}

/// Prove a single filtered I;16 resize for the typed packed-word shader.
/// Pillow materializes a native u16 intermediate after the horizontal pass,
/// so the proof performs that same byte-order-aware round/clip step before
/// validating the vertical reducer. Chains and mixed operation batches stay
/// on exact host semantic control until their intermediate contracts are
/// separately established.
pub(in crate::compute::pool_gpu) fn gpu_luma16_resize_f64_is_exact(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if !(logical_mode.is_none()
        || matches!(logical_mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N")))
        || ops.len() != 1
    {
        return false;
    }
    let PipelineOp::Resize { w, h, filter } = &ops[0] else {
        return false;
    };
    if matches!(filter, ResampleFilter::Nearest) || *w == 0 || *h == 0 {
        return false;
    }
    let DynamicImage::ImageLuma16(pixels) = image else {
        return false;
    };
    let source_dimensions = image.dimensions();
    if source_dimensions.0 == 0 || source_dimensions.1 == 0 {
        return false;
    }
    let expected = CheckedDims::new(source_dimensions.0, source_dimensions.1, 1)
        .ok()
        .map(|dims| dims.total_pixels());
    if expected != Some(pixels.as_raw().len()) {
        return false;
    }
    if CheckedDims::new(*w, *h, 1)
        .ok()
        .map(|dims| dims.total_pixels())
        .is_none_or(|count| count > GPU_BUFFER_CAPACITY as usize)
    {
        return false;
    }

    let (kernel, support) = filter_from_resample(*filter);
    let horizontal = precompute_coeffs_f64(*w, source_dimensions.0, kernel, support);
    let vertical = precompute_coeffs_f64(*h, source_dimensions.1, kernel, support);

    let big_endian = luma16_resample_big_endian(logical_mode);
    let Some(horizontal_samples) = gpu_luma16_f64_pass(
        pixels.as_raw(),
        source_dimensions,
        &horizontal,
        true,
        big_endian,
    ) else {
        return false;
    };
    gpu_luma16_f64_pass(
        &horizontal_samples,
        (*w, source_dimensions.1),
        &vertical,
        false,
        big_endian,
    )
    .is_some()
}
