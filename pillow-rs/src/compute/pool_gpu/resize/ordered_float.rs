//! ordered float for GPU resize exactness.

use super::super::*;

#[derive(Clone, Copy)]
pub(in crate::compute::pool_gpu) struct F64SignedMagnitude {
    pub(in crate::compute::pool_gpu) magnitude: u128,
    pub(in crate::compute::pool_gpu) negative: bool,
}

// Pillow 12.2.0's arm64 FLOAT32 horizontal resampler uses scalar FMA for
// rows with at most 15 taps, then switches to a vector path that materializes
// each complete 16-tap block before the ordered additions; any tail remains
// on the scalar FMA loop. Its vertical FLOAT32 path stays on the scalar FMA
// loop. Keep this boundary explicit in every host/shader admission model; a
// single arithmetic model falsely admits some wide rows whose final f32 word
// differs by one ULP.
pub(in crate::compute::pool_gpu) const GPU_F_RESIZE_HORIZONTAL_FMA_MAX_TAPS: usize = 15;
pub(in crate::compute::pool_gpu) const GPU_F_RESIZE_VECTOR_WIDTH: usize = 16;
pub(in crate::compute::pool_gpu) const GPU_F_RESIZE_MARKER9_MAX_TAPS: usize = 32;
// Marker 12's single-range transport stops one tap below 8388608 because
// coefficient metadata/alignment would otherwise exceed a 128-MiB binding.
// This is not an arithmetic bound: markers 14/15 stream longer rows, retaining
// the ordered binary64 state on GPU up to the same GPU_BUFFER_CAPACITY image
// limit. Marker 13 keeps its compact repeated-coefficient Box encoding.
pub(in crate::compute::pool_gpu) const GPU_F_RESIZE_ORDERED_MAX_TAPS: usize = 8_388_607;
// `GpuInner::new` requests wgpu's default limits, whose storage-buffer
// binding limit is 128 MiB.  Keep f64 coefficient admission within that
// guaranteed limit even when the selected adapter reports a larger maximum;
// this also prevents a future adapter-specific range from turning a valid
// Pillow operation into a device validation error.
pub(in crate::compute::pool_gpu) const GPU_F_RESIZE_MAX_COEFFICIENT_BINDING_BYTES: usize =
    128 << 20;
pub(in crate::compute::pool_gpu) const GPU_F_RESIZE_COEFFICIENT_ALIGNMENT_BYTES: usize = 256;
// Long coefficient rows are streamed through this bounded device arena. The
// binary64 accumulator remains on GPU between chunks; the tap index is global
// so Pillow's horizontal vector-block/FMA-tail split survives each boundary.
pub(in crate::compute::pool_gpu) const GPU_F_RESIZE_TILE_TAPS: usize = 16_384;
pub(in crate::compute::pool_gpu) const GPU_F_RESIZE_TILE_ROWS: usize = 8;
pub(in crate::compute::pool_gpu) const GPU_F_RESIZE_TILE_PIXELS: usize = 32_768;

pub(in crate::compute::pool_gpu) fn gpu_f_resize_coefficients_need_tiles(
    coeffs: &FilterCoeffsF64,
) -> bool {
    coeffs
        .count
        .iter()
        .any(|&count| count > GPU_F_RESIZE_TILE_TAPS)
        || !gpu_f_resize_f64_coefficients_fit_binding(coeffs)
}

pub(in crate::compute::pool_gpu) fn gpu_f_resize_uses_separate_horizontal_product_add(
    horizontal: bool,
    tap_count: usize,
    tap: usize,
) -> bool {
    if !horizontal || tap_count <= GPU_F_RESIZE_HORIZONTAL_FMA_MAX_TAPS {
        return false;
    }
    // The arm64 vector loop consumes complete 16-tap blocks. Its scalar tail
    // remains an FMA loop, even when the row as a whole is wider than 15.
    tap < (tap_count / GPU_F_RESIZE_VECTOR_WIDTH) * GPU_F_RESIZE_VECTOR_WIDTH
}

pub(in crate::compute::pool_gpu) fn gpu_f_resize_accumulate_f64(
    accumulator: &mut f64,
    weight: f64,
    sample: f32,
    separate_product_add: bool,
) {
    let sample = f64::from(sample);
    if separate_product_add {
        // Keep the product out of the following add. LLVM may otherwise
        // contract this expression back into an FMA, defeating the arm64
        // wide-row contract that Pillow uses after 15 taps.
        let product = std::hint::black_box(weight * sample);
        *accumulator += product;
    } else {
        *accumulator = weight.mul_add(sample, *accumulator);
    }
}

/// Add one bounded U128 term with the same signed-magnitude ordering used by
/// the marker-9 WGSL reducer.  A same-sign overflow is rejected because the
/// shader wraps its four limbs there; only the representable exact domain is
/// safe to admit.
pub(in crate::compute::pool_gpu) fn gpu_f64_signed_u128_add(
    sum: F64SignedMagnitude,
    term: u128,
    term_negative: bool,
) -> Option<F64SignedMagnitude> {
    if term == 0 {
        return Some(sum);
    }
    if sum.magnitude == 0 {
        return Some(F64SignedMagnitude {
            magnitude: term,
            negative: term_negative,
        });
    }
    if sum.negative == term_negative {
        return Some(F64SignedMagnitude {
            magnitude: sum.magnitude.checked_add(term)?,
            negative: sum.negative,
        });
    }
    if sum.magnitude < term {
        Some(F64SignedMagnitude {
            magnitude: term - sum.magnitude,
            negative: term_negative,
        })
    } else {
        Some(F64SignedMagnitude {
            magnitude: sum.magnitude - term,
            negative: sum.negative,
        })
    }
}

/// Evaluate one f64-coefficient row as an exact integer sum, then compare its
/// final f32 bits with Pillow's ordered arm64 FLOAT32 accumulation. Horizontal
/// rows use scalar FMA through 15 taps, complete 16-tap vector product/add
/// blocks after that, and scalar FMA for any tail; vertical rows use scalar FMA
/// throughout. The shader uses the same exact-sum representation; rows where
/// an intermediate rounding would change the final f32 value are rejected.
/// Finite rows may end in a signed infinity when the exact result overflows
/// f32.  Special rows use an integer IEEE state machine for NaN/infinity
/// products and are admitted only when their final bits match Pillow's
/// ordered f64 result.  That prepass remains valid for rows wider than the
/// marker-9 finite bound; finite rows above the bound stay with marker 12 or
/// exact host semantic control.
pub(in crate::compute::pool_gpu) fn gpu_f_resize_f64_sample_bits(
    bytes: &[u8],
    source_dimensions: (u32, u32),
    coeffs: &FilterCoeffsF64,
    output_index: usize,
    horizontal: bool,
    line: usize,
) -> Option<u32> {
    let source_w = usize::try_from(source_dimensions.0).ok()?;
    let source_h = usize::try_from(source_dimensions.1).ok()?;
    let source_start = usize::try_from(*coeffs.xmin.get(output_index)?).ok()?;
    let weights = coeffs.weights.get(output_index)?;
    let expected_count = *coeffs.count.get(output_index)?;
    if expected_count != weights.len() || weights.len() > GPU_BUFFER_CAPACITY as usize {
        return None;
    }
    if line >= if horizontal { source_h } else { source_w } {
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
    let sample_bits_at = |tap: usize| -> Option<u32> {
        let pixel = source_pixel(tap)?;
        let offset = pixel.checked_mul(4)?;
        let word = bytes.get(offset..offset.checked_add(4)?)?;
        Some(u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
    };
    let sample_at = |tap: usize| -> Option<(u32, F32IntegerParts)> {
        let bits = sample_bits_at(tap)?;
        Some((bits, gpu_f32_f64_integer_parts(bits)?))
    };

    // Non-finite F words do not need floating-point arithmetic on the device:
    // a finite coefficient times NaN is NaN, a zero coefficient times an
    // infinity is the invalid NaN operation, and opposite signed infinities
    // cancel to NaN.  Preserve the first NaN payload in tap order (the same
    // payload Pillow's ordered `mul_add` path exposes). The host-side f64
    // result below still has to agree with this device state machine before
    // the row is admitted.
    let mut ordered_accumulator = 0.0f64;
    let mut first_nan = None;
    let mut positive_infinity = false;
    let mut negative_infinity = false;
    let mut has_special = false;
    let mut minimum_exponent = None;
    for (tap, &weight) in weights.iter().enumerate() {
        let coeff = gpu_f64_integer_parts(weight)?;
        let bits = sample_bits_at(tap)?;
        let sample = f32::from_bits(bits);
        let separate_product_add =
            gpu_f_resize_uses_separate_horizontal_product_add(horizontal, weights.len(), tap);
        gpu_f_resize_accumulate_f64(
            &mut ordered_accumulator,
            weight,
            sample,
            separate_product_add,
        );
        let exponent_bits = (bits >> 23) & 0xff;
        if exponent_bits != 0xff {
            if weights.len() <= GPU_F_RESIZE_MARKER9_MAX_TAPS && coeff.mantissa != 0 {
                let sample = gpu_f32_f64_integer_parts(bits)?;
                if sample.mantissa != 0 {
                    let exponent = sample
                        .exponent
                        .checked_sub(23)?
                        .checked_add(coeff.exponent)?;
                    minimum_exponent = Some(
                        minimum_exponent.map_or(exponent, |minimum: i32| minimum.min(exponent)),
                    );
                }
            }
            continue;
        }
        has_special = true;
        let fraction = bits & 0x7f_ff_ff;
        if fraction != 0 {
            first_nan.get_or_insert(bits | 0x0040_0000);
        } else if coeff.mantissa == 0 {
            // 0 * infinity is an invalid operation.  Pillow stores the
            // resulting quiet NaN with the canonical f32 payload.
            first_nan.get_or_insert(0x7fc0_0000);
        } else if (bits & 0x8000_0000) != 0 {
            if coeff.negative {
                positive_infinity = true;
            } else {
                negative_infinity = true;
            }
        } else if coeff.negative {
            negative_infinity = true;
        } else {
            positive_infinity = true;
        }
    }
    if has_special {
        let actual = if let Some(bits) = first_nan {
            bits
        } else if positive_infinity && negative_infinity {
            0x7fc0_0000
        } else if positive_infinity {
            0x7f80_0000
        } else if negative_infinity {
            0xff80_0000
        } else {
            // The special scan only reaches this branch for an impossible
            // coefficient/source combination; keep the admission conservative.
            return None;
        };
        let expected = (ordered_accumulator as f32).to_bits();
        return (actual == expected).then_some(actual);
    }

    // Finite rows wider than marker 9's historical domain must use the
    // bounded ordered reducer (marker 12), whose device state models Pillow's
    // arm64 product/add split. Special rows use the same per-row binding cap:
    // their prepass avoids arithmetic, but it still consumes the encoded
    // coefficient range and cannot bypass the adapter's storage limit.
    if weights.len() > GPU_F_RESIZE_MARKER9_MAX_TAPS {
        return None;
    }
    let Some(minimum_exponent) = minimum_exponent else {
        return Some(0);
    };

    // Keep the host model in the same unsigned-magnitude domain as the
    // shader's four-limb reducer.  Terms shifted beyond 128 bits are dropped
    // by WGSL's bounded `u128_shl`; compare the resulting word with Pillow's
    // f64 result below before admitting such a row.
    let mut sum = F64SignedMagnitude {
        magnitude: 0,
        negative: false,
    };
    for (tap, &weight) in weights.iter().enumerate() {
        let coeff = gpu_f64_integer_parts(weight)?;
        let (_, sample) = sample_at(tap)?;
        if coeff.mantissa == 0 || sample.mantissa == 0 {
            continue;
        }
        let exponent = sample
            .exponent
            .checked_sub(23)?
            .checked_add(coeff.exponent)?;
        let shift = u32::try_from(exponent.checked_sub(minimum_exponent)?).ok()?;
        let product = u128::from(sample.mantissa).checked_mul(u128::from(coeff.mantissa))?;
        let term = if shift < 128 {
            product.checked_shl(shift)?
        } else {
            // `u128_shl` in both resize shaders returns an all-zero limb
            // value once the shift reaches the width of the reducer.
            0
        };
        sum = gpu_f64_signed_u128_add(sum, term, sample.negative != coeff.negative)?;
    }

    let expected = (ordered_accumulator as f32).to_bits();
    let expected_exponent = expected & 0x7f80_0000;
    if (expected_exponent == 0x7f80_0000 && (expected & 0x007f_ffff) != 0)
        || (expected_exponent != 0x7f80_0000 && !ordered_accumulator.is_finite())
    {
        return None;
    }
    let actual = gpu_f64_u128_integer_to_f32(sum.magnitude, sum.negative, minimum_exponent)?;
    (actual == expected).then_some(actual)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::compute::pool_gpu) enum F64OrderedKind {
    Finite,
    Infinity,
    Nan,
}

#[derive(Clone, Copy)]
pub(in crate::compute::pool_gpu) struct F64OrderedState {
    /// A finite f64 value is `magnitude * 2^exponent`. Normal values have at
    /// most 53 significant bits; subnormal values use `exponent = -1074` and
    /// the explicit binary64 fraction. Infinity and NaN use the same words
    /// with `kind` set accordingly.
    pub(in crate::compute::pool_gpu) magnitude: u128,
    pub(in crate::compute::pool_gpu) exponent: i32,
    pub(in crate::compute::pool_gpu) negative: bool,
    pub(in crate::compute::pool_gpu) kind: F64OrderedKind,
}

pub(in crate::compute::pool_gpu) fn gpu_f64_round_shift_right(magnitude: u128, shift: u32) -> u128 {
    if shift == 0 {
        return magnitude;
    }
    if shift > 128 {
        return 0;
    }
    if shift == 128 {
        return u128::from(magnitude > (1u128 << 127));
    }
    let mut rounded = magnitude >> shift;
    let remainder = magnitude & ((1u128 << shift) - 1);
    let halfway = 1u128 << (shift - 1);
    if remainder > halfway || (remainder == halfway && rounded & 1 != 0) {
        rounded = rounded.saturating_add(1);
    }
    rounded
}

/// Round an exact signed binary integer to the binary64 state used by the
/// ordered FMA proof. The marker-12 shader performs this same round-to-nearest-
/// even step after every product+accumulator operation. Subnormal values,
/// signed infinities, and NaN-producing overflow transitions stay in the
/// explicit state machine instead of falling back to host execution.
pub(in crate::compute::pool_gpu) fn gpu_f64_ordered_round(
    sum: F64SignedMagnitude,
    scale_exp: i32,
) -> Option<F64OrderedState> {
    if sum.magnitude == 0 {
        return Some(F64OrderedState {
            magnitude: 0,
            exponent: 0,
            // Exact cancellation under round-to-nearest produces +0.0.
            // Preserve a negative sign only for a nonzero value that
            // underflows in the subnormal branch below.
            negative: false,
            kind: F64OrderedKind::Finite,
        });
    }
    let bit_length = 128 - sum.magnitude.leading_zeros();
    let mut exponent = scale_exp.checked_add(bit_length as i32 - 1)?;
    if exponent > 1023 {
        return Some(F64OrderedState {
            magnitude: 0,
            exponent: 0,
            negative: sum.negative,
            kind: F64OrderedKind::Infinity,
        });
    }
    if exponent < -1022 {
        // Binary64 subnormals are integer multiples of 2^-1074. Round the
        // exact integer directly at that scale; a carry becomes the smallest
        // normal value and keeps the same representation as every other
        // finite state.
        let target_shift = scale_exp.checked_add(1074)?;
        let mantissa = if target_shift >= 0 {
            sum.magnitude
                .checked_shl(u32::try_from(target_shift).ok()?)?
        } else {
            gpu_f64_round_shift_right(
                sum.magnitude,
                u32::try_from(target_shift.checked_neg()?).ok()?,
            )
        };
        if mantissa == 0 {
            return Some(F64OrderedState {
                magnitude: 0,
                exponent: 0,
                negative: sum.negative,
                kind: F64OrderedKind::Finite,
            });
        }
        if mantissa >= (1u128 << 52) {
            return Some(F64OrderedState {
                magnitude: 1u128 << 52,
                exponent: -1074,
                negative: sum.negative,
                kind: F64OrderedKind::Finite,
            });
        }
        return Some(F64OrderedState {
            magnitude: mantissa,
            exponent: -1074,
            negative: sum.negative,
            kind: F64OrderedKind::Finite,
        });
    }
    let mut mantissa = if bit_length > 53 {
        let shift = bit_length - 53;
        let mut rounded = sum.magnitude >> shift;
        let remainder = sum.magnitude & ((1u128 << shift) - 1);
        let halfway = 1u128 << (shift - 1);
        if remainder > halfway || (remainder == halfway && rounded & 1 != 0) {
            rounded = rounded.checked_add(1)?;
        }
        rounded
    } else {
        sum.magnitude.checked_shl(53 - bit_length)?
    };
    if mantissa >= (1u128 << 53) {
        mantissa >>= 1;
        exponent = exponent.checked_add(1)?;
        if exponent > 1023 {
            return Some(F64OrderedState {
                magnitude: 0,
                exponent: 0,
                negative: sum.negative,
                kind: F64OrderedKind::Infinity,
            });
        }
    }
    Some(F64OrderedState {
        magnitude: mantissa,
        exponent: exponent - 52,
        negative: sum.negative,
        kind: F64OrderedKind::Finite,
    })
}

pub(in crate::compute::pool_gpu) fn gpu_f64_ordered_nan() -> F64OrderedState {
    F64OrderedState {
        magnitude: 0,
        exponent: 0,
        negative: false,
        kind: F64OrderedKind::Nan,
    }
}

/// Add one exact f64-coefficient/f32-sample product to a rounded f64 state.
/// The product is supplied as an unsigned integer scaled by `product_exp`;
/// aligning it with the previous 53-bit state and rounding once models
/// `weight.mul_add(f64::from(sample), accumulator)` without device floats.
/// The horizontal arm64 vector path instead rounds the product first and then
/// adds it in tap order; `separate_product_add` selects that model.
pub(in crate::compute::pool_gpu) fn gpu_f64_ordered_add_product(
    state: F64OrderedState,
    product: u128,
    product_exp: i32,
    product_negative: bool,
    separate_product_add: bool,
) -> Option<F64OrderedState> {
    if state.kind == F64OrderedKind::Nan {
        return Some(state);
    }
    if product == 0 {
        return Some(state);
    }
    let rounded_product = if separate_product_add {
        Some(gpu_f64_ordered_round(
            F64SignedMagnitude {
                magnitude: product,
                negative: product_negative,
            },
            product_exp,
        )?)
    } else {
        None
    };
    if state.kind == F64OrderedKind::Infinity {
        if let Some(product) = rounded_product {
            if product.kind == F64OrderedKind::Nan {
                return Some(product);
            }
            if product.kind == F64OrderedKind::Infinity {
                return Some(if state.negative == product.negative {
                    state
                } else {
                    gpu_f64_ordered_nan()
                });
            }
        }
        return Some(state);
    }
    if let Some(product) = rounded_product {
        if product.kind == F64OrderedKind::Nan {
            return Some(product);
        }
        if product.kind == F64OrderedKind::Infinity {
            return Some(product);
        }
    }
    if state.magnitude == 0 {
        return rounded_product.or_else(|| {
            gpu_f64_ordered_round(
                F64SignedMagnitude {
                    magnitude: product,
                    negative: product_negative,
                },
                product_exp,
            )
        });
    }
    let (product, product_exp, product_negative) = rounded_product
        .map_or((product, product_exp, product_negative), |rounded| {
            (rounded.magnitude, rounded.exponent, rounded.negative)
        });
    // Pillow Resample.c rounds each ordered product/add to binary64. Keep
    // 127 significant alignment bits (one carry bit remains), jamming every
    // discarded nonzero tail into the low bit. The operands have at most 77
    // bits, so truncation only occurs when their leading bits are far apart:
    // cancellation cannot expose the jammed tail at the 53-bit rounding cut.
    // This preserves below/at/above-halfway distinctions for either sign even
    // when f32 subnormal and maximum-finite samples share a coefficient row.
    let high_bit = (state.exponent + (128 - state.magnitude.leading_zeros()) as i32)
        .max(product_exp + (128 - product.leading_zeros()) as i32);
    let minimum_exponent = state.exponent.min(product_exp).max(high_bit - 127);
    let align = |magnitude: u128, exponent: i32| -> Option<u128> {
        if exponent >= minimum_exponent {
            magnitude.checked_shl((exponent - minimum_exponent) as u32)
        } else {
            let shift = (minimum_exponent - exponent) as u32;
            if shift >= 128 {
                Some(u128::from(magnitude != 0))
            } else {
                let discarded = magnitude & ((1u128 << shift) - 1);
                Some((magnitude >> shift) | u128::from(discarded != 0))
            }
        }
    };
    let state_term = align(state.magnitude, state.exponent)?;
    let product_term = align(product, product_exp)?;
    let sum = gpu_f64_signed_u128_add(
        F64SignedMagnitude {
            magnitude: state_term,
            negative: state.negative,
        },
        product_term,
        product_negative,
    )?;
    gpu_f64_ordered_round(sum, minimum_exponent)
}

pub(in crate::compute::pool_gpu) fn gpu_f64_ordered_state_to_f32(
    state: F64OrderedState,
) -> Option<u32> {
    match state.kind {
        F64OrderedKind::Finite => {
            gpu_f64_u128_integer_to_f32(state.magnitude, state.negative, state.exponent)
        }
        F64OrderedKind::Infinity => Some(if state.negative {
            0xff80_0000
        } else {
            0x7f80_0000
        }),
        F64OrderedKind::Nan => Some(0x7fc0_0000),
    }
}

/// Evaluate a bounded f64 coefficient row with Pillow's ordered arm64
/// semantics. Marker 9 keeps the exact real sum and is necessarily
/// conservative when an intermediate f64 rounding changes the final f32 word;
/// marker 12 and its tiled transport emulate the scalar FMA path and the
/// >15-tap horizontal vector product/add path in integer arithmetic. Tiled
/// dispatches retain that state rather than storing an intermediate FLOAT32.
pub(in crate::compute::pool_gpu) fn gpu_f_resize_f64_ordered_sample_bits(
    bytes: &[u8],
    source_dimensions: (u32, u32),
    coeffs: &FilterCoeffsF64,
    output_index: usize,
    horizontal: bool,
    line: usize,
) -> Option<u32> {
    let source_w = usize::try_from(source_dimensions.0).ok()?;
    let source_h = usize::try_from(source_dimensions.1).ok()?;
    let source_start = usize::try_from(*coeffs.xmin.get(output_index)?).ok()?;
    let weights = coeffs.weights.get(output_index)?;
    if *coeffs.count.get(output_index)? != weights.len()
        || weights.len() > GPU_BUFFER_CAPACITY as usize
    {
        return None;
    }
    if line >= if horizontal { source_h } else { source_w } {
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
    let mut state = F64OrderedState {
        magnitude: 0,
        exponent: 0,
        negative: false,
        kind: F64OrderedKind::Finite,
    };
    let mut ordered_accumulator = 0.0f64;
    for (tap, &weight) in weights.iter().enumerate() {
        let coeff = gpu_f64_integer_parts(weight)?;
        let pixel = source_pixel(tap)?;
        let offset = pixel.checked_mul(4)?;
        let word = bytes.get(offset..offset.checked_add(4)?)?;
        let bits = u32::from_le_bytes([word[0], word[1], word[2], word[3]]);
        let Some(sample) = gpu_f32_f64_integer_parts(bits) else {
            // A preceding FLOAT32 pass may overflow even when every source
            // word was finite. Reuse marker 9's ordered IEEE special scan;
            // its coefficient-aware NaN/infinity result needs no finite sum.
            return gpu_f_resize_f64_sample_bits(
                bytes,
                source_dimensions,
                coeffs,
                output_index,
                horizontal,
                line,
            );
        };
        let separate_product_add =
            gpu_f_resize_uses_separate_horizontal_product_add(horizontal, weights.len(), tap);
        // f32 subnormal inputs are represented exactly at the fixed 2^-149
        // scale. They remain normal in the f64 accumulator for Pillow's
        // finite resampling coefficients, so marker 12 can model them with
        // the same ordered integer state as normal inputs.
        gpu_f_resize_accumulate_f64(
            &mut ordered_accumulator,
            weight,
            f32::from_bits(bits),
            separate_product_add,
        );
        if coeff.mantissa == 0 || sample.mantissa == 0 {
            continue;
        }
        let product = u128::from(sample.mantissa).checked_mul(u128::from(coeff.mantissa))?;
        let product_exp = sample
            .exponent
            .checked_sub(23)?
            .checked_add(coeff.exponent)?;
        state = gpu_f64_ordered_add_product(
            state,
            product,
            product_exp,
            sample.negative != coeff.negative,
            separate_product_add,
        )?;
    }
    let actual = gpu_f64_ordered_state_to_f32(state)?;
    let expected = (ordered_accumulator as f32).to_bits();
    (actual == expected).then_some(actual)
}

pub(in crate::compute::pool_gpu) fn gpu_f_resize_f64_ordered_pass_bits(
    bytes: &[u8],
    source_dimensions: (u32, u32),
    coeffs: &FilterCoeffsF64,
    horizontal: bool,
) -> Option<Vec<u32>> {
    let output_count = coeffs.xmin.len();
    let line_count = if horizontal {
        usize::try_from(source_dimensions.1).ok()?
    } else {
        usize::try_from(source_dimensions.0).ok()?
    };
    let word_count = output_count.checked_mul(line_count)?;
    if word_count > GPU_BUFFER_CAPACITY as usize {
        return None;
    }
    let mut result = Vec::new();
    result.try_reserve(word_count).ok()?;
    if horizontal {
        for line in 0..line_count {
            for output_index in 0..output_count {
                result.push(gpu_f_resize_f64_ordered_sample_bits(
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
                result.push(gpu_f_resize_f64_ordered_sample_bits(
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
