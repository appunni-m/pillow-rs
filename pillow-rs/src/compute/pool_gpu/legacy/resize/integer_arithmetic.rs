//! integer arithmetic for GPU resize exactness.

use super::super::*;

#[derive(Clone, Copy)]
pub(in crate::compute::pool_gpu) struct F32IntegerParts {
    pub(in crate::compute::pool_gpu) negative: bool,
    pub(in crate::compute::pool_gpu) mantissa: u32,
    pub(in crate::compute::pool_gpu) exponent: i32,
}

/// Decode a finite normal f32 into the integer significand/exponent form used
/// by the exact marker-6 accumulator. Positive zero is represented by a zero
/// mantissa; negative zero, subnormals, and non-finite words are rejected.
pub(in crate::compute::pool_gpu) fn gpu_f32_integer_parts(bits: u32) -> Option<F32IntegerParts> {
    if bits == (-0.0f32).to_bits() {
        return None;
    }
    let exponent_bits = (bits >> 23) & 0xff;
    let fraction = bits & 0x7f_ff_ff;
    if exponent_bits == 0xff || (exponent_bits == 0 && fraction != 0) {
        return None;
    }
    if exponent_bits == 0 {
        return Some(F32IntegerParts {
            negative: false,
            mantissa: 0,
            exponent: 0,
        });
    }
    Some(F32IntegerParts {
        negative: bits & (1 << 31) != 0,
        mantissa: fraction | 0x80_0000,
        exponent: exponent_bits as i32 - 127,
    })
}

/// Decode every finite f32 word into an integer significand and binary scale
/// for the marker-9 f64 reducer.  Normal words use the implicit leading bit;
/// subnormals use their explicit fraction at the fixed `2^-149` scale.  Keep
/// signed zero here because Pillow's ordered f64 accumulation can observe it;
/// the final proof preserves a signed-zero result when the exact reducer
/// agrees.  The older marker-6 proof intentionally keeps its stricter
/// normal-only decoder above.
pub(in crate::compute::pool_gpu) fn gpu_f32_f64_integer_parts(
    bits: u32,
) -> Option<F32IntegerParts> {
    let exponent_bits = (bits >> 23) & 0xff;
    let fraction = bits & 0x7f_ff_ff;
    if exponent_bits == 0xff {
        return None;
    }
    if exponent_bits == 0 {
        return Some(F32IntegerParts {
            negative: bits & (1 << 31) != 0,
            mantissa: fraction,
            exponent: -126,
        });
    }
    Some(F32IntegerParts {
        negative: bits & (1 << 31) != 0,
        mantissa: fraction | 0x80_0000,
        exponent: exponent_bits as i32 - 127,
    })
}

/// Evaluate one horizontal or vertical fixed-point sample with the exact
/// integer representation of the f32 inputs. A source word is
/// `M * 2^(E-23)` and a fixed coefficient is `P / 2^22`, so the complete row
/// is an integer sum scaled by `2^(E-45)`. Keeping every sequential partial
/// sum within 53 bits proves that Pillow's f64 accumulation is exact before
/// its final f32 store, including cancellation from signed samples or
/// ringing-filter coefficients.
pub(in crate::compute::pool_gpu) fn gpu_f_resize_integer_sample_bits(
    bytes: &[u8],
    source_dimensions: (u32, u32),
    coeffs: &FilterCoeffs,
    output_index: usize,
    horizontal: bool,
    line: usize,
) -> Option<u32> {
    let (source_w, source_h) = source_dimensions;
    let source_w = match usize::try_from(source_w) {
        Ok(value) => value,
        Err(_) => return None,
    };
    let source_h = match usize::try_from(source_h) {
        Ok(value) => value,
        Err(_) => return None,
    };
    let &source_start = coeffs.xmin.get(output_index)?;
    let &count = coeffs.count.get(output_index)?;
    let &weight_start = coeffs.offsets.get(output_index)?;
    let weights = coeffs
        .weights
        .get(weight_start..weight_start.saturating_add(count))?;
    let source_start = match usize::try_from(source_start) {
        Ok(value) => value,
        Err(_) => return None,
    };
    let word_at = |pixel: usize| -> Option<F32IntegerParts> {
        let offset = pixel.checked_mul(4)?;
        let end = offset.checked_add(4)?;
        let word = bytes.get(offset..end)?;
        gpu_f32_integer_parts(u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
    };
    let source_pixel = |line: usize, tap: usize| -> Option<usize> {
        let coordinate = source_start.checked_add(tap)?;
        if horizontal {
            (coordinate < source_w).then_some(line.checked_mul(source_w)?.checked_add(coordinate)?)
        } else {
            (coordinate < source_h).then_some(coordinate.checked_mul(source_w)?.checked_add(line)?)
        }
    };

    if line >= if horizontal { source_h } else { source_w } {
        return None;
    }

    let mut minimum_exponent = None;
    for (tap, &weight) in weights.iter().enumerate() {
        if weight == 0 {
            continue;
        }
        let pixel = source_pixel(line, tap).and_then(word_at)?;
        if pixel.mantissa != 0 {
            minimum_exponent = Some(
                minimum_exponent.map_or(pixel.exponent, |minimum: i32| minimum.min(pixel.exponent)),
            );
        }
    }
    let Some(minimum_exponent) = minimum_exponent else {
        // A row whose selected samples are all positive zero has an exact
        // positive-zero result. Negative zero is rejected by the source-word
        // proof because Pillow's filtered path canonicalizes its zero output.
        return Some(0);
    };

    let mut sum = 0i128;
    for (tap, &weight) in weights.iter().enumerate() {
        if weight == 0 {
            continue;
        }
        let pixel = source_pixel(line, tap).and_then(word_at)?;
        if pixel.mantissa == 0 {
            continue;
        }
        let shift = u32::try_from(pixel.exponent - minimum_exponent).ok()?;
        let product = i128::from(pixel.mantissa).checked_mul(i128::from(weight))?;
        let product = if pixel.negative {
            product.checked_neg()?
        } else {
            product
        };
        // `checked_shl` only validates the shift count; it intentionally
        // permits high bits to be truncated. That is not an exact integer
        // reduction, so reject a term whose aligned magnitude would exceed
        // the signed accumulator before shifting it.
        let product_magnitude = product.checked_abs()?;
        let product_bits = 128 - product_magnitude.leading_zeros();
        if product_bits.saturating_add(shift) > 127 {
            return None;
        }
        let term = product.checked_shl(shift)?;
        let next = sum.checked_add(term)?;
        let magnitude = if next < 0 { next.checked_neg()? } else { next };
        // f64 represents every integer exactly through 2^53. Check each
        // sequential partial, not only the final cancellation result.
        if magnitude > (1i128 << 53) {
            return None;
        }
        sum = next;
    }
    if sum == 0 {
        return Some(0);
    }

    let magnitude = if sum < 0 { sum.checked_neg()? } else { sum };
    let magnitude = (magnitude as f64) * 2f64.powi(minimum_exponent - 45);
    let value = if sum < 0 {
        -(magnitude as f32)
    } else {
        magnitude as f32
    };
    let bits = value.to_bits();
    // The marker's conversion routine deliberately stays one exponent below
    // overflow and never emits a subnormal, which also avoids adapter flush-to
    // zero behavior at the device boundary.
    (value.is_normal() && (((bits >> 23) & 0xff) as i32 - 127) <= 126).then_some(bits)
}

/// Check one horizontal or vertical fixed-point row using the exact integer
/// representation of the f32 inputs.
pub(in crate::compute::pool_gpu) fn gpu_f_resize_integer_row_is_exact(
    bytes: &[u8],
    source_dimensions: (u32, u32),
    coeffs: &FilterCoeffs,
    output_index: usize,
    horizontal: bool,
) -> bool {
    let line_count = if horizontal {
        source_dimensions.1
    } else {
        source_dimensions.0
    };
    let Ok(line_count) = usize::try_from(line_count) else {
        return false;
    };
    (0..line_count).all(|line| {
        gpu_f_resize_integer_sample_bits(
            bytes,
            source_dimensions,
            coeffs,
            output_index,
            horizontal,
            line,
        )
        .is_some()
    })
}

/// Evaluate every word produced by one exact fixed-point resize pass. The
/// resulting words are fed into the second pass proof so a two-axis resize
/// preserves Pillow's f32 intermediate store, rather than proving both axes
/// against the original source independently.
pub(in crate::compute::pool_gpu) fn gpu_f_resize_integer_pass_bits(
    bytes: &[u8],
    source_dimensions: (u32, u32),
    coeffs: &FilterCoeffs,
    horizontal: bool,
) -> Option<Vec<u32>> {
    let output_count = coeffs.xmin.len();
    let line_count = if horizontal {
        usize::try_from(source_dimensions.1).ok()?
    } else {
        usize::try_from(source_dimensions.0).ok()?
    };
    let word_count = output_count.checked_mul(line_count)?;
    let byte_count = word_count.checked_mul(4)?;
    if byte_count > (GPU_BUFFER_CAPACITY as usize).saturating_mul(4) {
        return None;
    }
    let mut result = Vec::new();
    result.try_reserve(word_count).ok()?;
    if horizontal {
        for line in 0..line_count {
            for output_index in 0..output_count {
                result.push(gpu_f_resize_integer_sample_bits(
                    bytes,
                    source_dimensions,
                    coeffs,
                    output_index,
                    true,
                    line,
                )?);
            }
        }
    } else {
        for output_index in 0..output_count {
            for line in 0..line_count {
                result.push(gpu_f_resize_integer_sample_bits(
                    bytes,
                    source_dimensions,
                    coeffs,
                    output_index,
                    false,
                    line,
                )?);
            }
        }
    }
    Some(result)
}

#[derive(Clone, Copy)]
pub(in crate::compute::pool_gpu) struct F64IntegerParts {
    pub(in crate::compute::pool_gpu) negative: bool,
    pub(in crate::compute::pool_gpu) mantissa: u64,
    pub(in crate::compute::pool_gpu) exponent: i32,
}

/// Decode a finite normal f64 coefficient into an integer significand and a
/// binary scale.  Pillow's f64 coefficient table is dyadic, so this keeps the
/// device path entirely integer while preserving the coefficient bits that
/// the generic f32 shader previously discarded.  Zero is represented with a
/// zero mantissa; subnormal and non-finite coefficients are rejected because
/// the bounded WGSL conversion below only promises normal values.
pub(in crate::compute::pool_gpu) fn gpu_f64_integer_parts(value: f64) -> Option<F64IntegerParts> {
    let bits = value.to_bits();
    let exponent_bits = (bits >> 52) & 0x7ff;
    let fraction = bits & ((1u64 << 52) - 1);
    if exponent_bits == 0x7ff || (exponent_bits == 0 && fraction != 0) {
        return None;
    }
    if exponent_bits == 0 {
        return Some(F64IntegerParts {
            negative: false,
            mantissa: 0,
            exponent: 0,
        });
    }
    Some(F64IntegerParts {
        negative: bits & (1u64 << 63) != 0,
        mantissa: fraction | (1u64 << 52),
        exponent: exponent_bits as i32 - 1023 - 52,
    })
}

/// Round a signed integer scaled by `2^scale_exp` directly to an f32 bit
/// pattern.  This is the same round-to-nearest-even operation that a final
/// `f64 as f32` conversion performs, but it avoids an intermediate host f64
/// conversion so the result can be compared with the integer WGSL reducer.
pub(in crate::compute::pool_gpu) fn gpu_f64_integer_to_f32(
    sum: i128,
    scale_exp: i32,
) -> Option<u32> {
    if sum == 0 {
        return Some(0);
    }
    let negative = sum < 0;
    let magnitude = sum.checked_abs()? as u128;
    let bit_length = 128 - magnitude.leading_zeros();
    let mut exponent = scale_exp.checked_add(bit_length as i32 - 1)?;

    // Values below the normal range are rounded in units of 2^-149, exactly
    // as an f64-to-f32 cast rounds a finite result.  Keeping this conversion
    // integer-only lets marker 9 carry source subnormals without relying on
    // adapter floating-point denormal handling.  A rounded 2^23 fraction is
    // the smallest normal f32 and is represented by exponent field one.
    if exponent < -126 {
        let target_shift = scale_exp.checked_add(149)?;
        let subnormal = if target_shift >= 0 {
            magnitude.checked_shl(u32::try_from(target_shift).ok()?)?
        } else {
            let shift = u32::try_from(target_shift.checked_neg()?).ok()?;
            if shift >= 128 {
                0
            } else if shift == 0 {
                magnitude
            } else {
                let mut rounded = magnitude >> shift;
                let remainder = magnitude & ((1u128 << shift) - 1);
                let halfway = 1u128 << (shift - 1);
                if remainder > halfway || (remainder == halfway && rounded & 1 != 0) {
                    rounded = rounded.checked_add(1)?;
                }
                rounded
            }
        };
        if subnormal >= (1u128 << 23) {
            return Some(if negative { 0x8080_0000 } else { 0x0080_0000 });
        } else {
            let mantissa = u32::try_from(subnormal).ok()?;
            let bits = mantissa;
            return Some(if negative { bits | 0x8000_0000 } else { bits });
        }
    }
    if exponent > 127 {
        return Some(if negative { 0xff80_0000 } else { 0x7f80_0000 });
    }
    if !(-126..=127).contains(&exponent) {
        return None;
    }
    let mut mantissa = if bit_length > 24 {
        let shift = bit_length - 24;
        let mut rounded = magnitude >> shift;
        let remainder = magnitude & ((1u128 << shift) - 1);
        let halfway = 1u128 << (shift - 1);
        if remainder > halfway || (remainder == halfway && rounded & 1 != 0) {
            rounded = rounded.checked_add(1)?;
        }
        rounded
    } else {
        magnitude.checked_shl(24 - bit_length)?
    };
    if mantissa >= (1u128 << 24) {
        mantissa >>= 1;
        exponent = exponent.checked_add(1)?;
        if exponent > 127 {
            return Some(if negative { 0xff80_0000 } else { 0x7f80_0000 });
        }
    }
    let mantissa = u32::try_from(mantissa & 0x7f_ff_ff).ok()?;
    let exponent_bits = u32::try_from(exponent + 127).ok()?;
    let bits = (exponent_bits << 23) | mantissa;
    Some(if negative { bits | 0x8000_0000 } else { bits })
}

/// Round the shader's unsigned 128-bit magnitude plus sign to an f32 word.
///
/// Marker 9 uses four 32-bit limbs on the device, so its exact reducer can
/// retain one more magnitude bit than a signed host `i128`.  The older helper
/// above remains for the typed luma16 proof; this variant mirrors
/// `f64_sum_to_f32`'s U128 conversion and lets the F proof model the same
/// bounded truncation at the device boundary.
pub(in crate::compute::pool_gpu) fn gpu_f64_u128_integer_to_f32(
    magnitude: u128,
    negative: bool,
    scale_exp: i32,
) -> Option<u32> {
    if magnitude == 0 {
        return Some(if negative { 0x8000_0000 } else { 0 });
    }
    let bit_length = 128 - magnitude.leading_zeros();
    let mut exponent = scale_exp.checked_add(bit_length as i32 - 1)?;

    if exponent < -126 {
        let target_shift = scale_exp.checked_add(149)?;
        let subnormal = if target_shift >= 0 {
            magnitude.checked_shl(u32::try_from(target_shift).ok()?)?
        } else {
            let shift = u32::try_from(target_shift.checked_neg()?).ok()?;
            if shift >= 128 {
                0
            } else if shift == 0 {
                magnitude
            } else {
                let mut rounded = magnitude >> shift;
                let remainder = magnitude & ((1u128 << shift) - 1);
                let halfway = 1u128 << (shift - 1);
                if remainder > halfway || (remainder == halfway && rounded & 1 != 0) {
                    rounded = rounded.checked_add(1)?;
                }
                rounded
            }
        };
        if subnormal >= (1u128 << 23) {
            return Some(if negative { 0x8080_0000 } else { 0x0080_0000 });
        }
        let mantissa = u32::try_from(subnormal).ok()?;
        return Some(if negative {
            mantissa | 0x8000_0000
        } else {
            mantissa
        });
    }
    if exponent > 127 {
        return Some(if negative { 0xff80_0000 } else { 0x7f80_0000 });
    }
    if !(-126..=127).contains(&exponent) {
        return None;
    }

    let mut mantissa = if bit_length > 24 {
        let shift = bit_length - 24;
        let mut rounded = magnitude >> shift;
        let remainder = magnitude & ((1u128 << shift) - 1);
        let halfway = 1u128 << (shift - 1);
        if remainder > halfway || (remainder == halfway && rounded & 1 != 0) {
            rounded = rounded.checked_add(1)?;
        }
        rounded
    } else {
        magnitude.checked_shl(24 - bit_length)?
    };
    if mantissa >= (1u128 << 24) {
        mantissa >>= 1;
        exponent = exponent.checked_add(1)?;
        if exponent > 127 {
            return Some(if negative { 0xff80_0000 } else { 0x7f80_0000 });
        }
    }
    let mantissa = u32::try_from(mantissa & 0x7f_ff_ff).ok()?;
    let exponent_bits = u32::try_from(exponent + 127).ok()?;
    let bits = (exponent_bits << 23) | mantissa;
    Some(if negative { bits | 0x8000_0000 } else { bits })
}
