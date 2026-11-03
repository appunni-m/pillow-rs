//! Shared eight-pixel AlphaComposite vector kernel for exact LA and RGBA modes.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use wide::{i32x8, u16x8, u32x8};

pub(crate) fn apply<const CHANNELS: usize>(
    source: &[u8],
    destination: &[u8],
    width: u32,
    height: u32,
    mode_name: &'static str,
) -> Result<Vec<u8>, PilError> {
    if CHANNELS != 2 && CHANNELS != 4 {
        return Err(PilError::InternalError(
            "AlphaComposite vector channel count is invalid".into(),
        ));
    }
    let channels = u8::try_from(CHANNELS)
        .map_err(|_| PilError::InternalError("AlphaComposite channel count is invalid".into()))?;
    let dimensions = CheckedDims::new_allow_empty(width, height, channels)?;
    if source.len() != dimensions.total_bytes() || destination.len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(format!(
            "{mode_name} AlphaComposite source storage did not match its dimensions"
        )));
    }

    let mut output = destination.to_vec();
    alpha_composite_chunk::<CHANNELS>(source, &mut output);
    let pixels = dimensions.total_bytes() / CHANNELS;
    let vector_blocks = pixels / 8;
    let scalar_tail = pixels % 8;
    crate::compute::record_pipeline_operation_path(if vector_blocks == 0 {
        "scalar-control"
    } else {
        "vector"
    });
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks as u64);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);
    Ok(output)
}

#[inline]
fn alpha_composite_div255(value: u32x8) -> u32x8 {
    // Pillow's SHIFTFORDIV255(a) is ((a >> 8) + a) >> 8.  This is the
    // exact fixed-point division used by libImaging/AlphaComposite.c, not a
    // mathematically equivalent floating-point rounding operation.
    ((value >> 8u32) + value) >> 8u32
}

#[inline]
fn alpha_composite_coefficient(source_alpha: u32x8, denominator: u32x8) -> u32x8 {
    const NUMERATOR_SCALE: u32x8 = u32x8::new([255 * 255 * 128; 8]);
    const ONE: u32x8 = u32x8::new([1; 8]);
    let numerator = source_alpha * NUMERATOR_SCALE;
    // numerator = (sa * 65025) * 128: the integer in parentheses is < 2^24,
    // so both operands are exactly representable as f32. The quotient is at
    // most 32640. Rounded f32 division can cross its upper integer boundary
    // by less than one; an exact u32 product detects and corrects that case.
    // Products stay below 2^31. This retains Pillow's integer floor without
    // widening eight coefficient lanes to doubles or accepting float rounding.
    let n: i32x8 = bytemuck::cast(numerator);
    let d: i32x8 = bytemuck::cast(denominator.max(ONE));
    let quotient: u32x8 = bytemuck::cast((n.round_float() / d.round_float()).trunc_int());
    quotient + (quotient * denominator).simd_gt(numerator)
}

#[inline]
fn alpha_composite_store_vector_block<const CHANNELS: usize>(
    source: u32x8,
    destination: u32x8,
    source_alpha: u32x8,
    out_alpha_255: u32x8,
    coefficient_source: u32x8,
    coefficient_destination: u32x8,
    output: &mut [u8],
) {
    const BYTE: u32x8 = u32x8::new([255; 8]);
    const COLOR_BIAS: u32x8 = u32x8::new([0x80 << 7; 8]);
    const ALPHA_BIAS: u32x8 = u32x8::new([0x80; 8]);
    const ZERO: u32x8 = u32x8::new([0; 8]);
    let alpha_shift = ((CHANNELS - 1) * 8) as u32;
    let channel = |shift: u32| {
        let blended = ((source >> shift) & BYTE) * coefficient_source
            + ((destination >> shift) & BYTE) * coefficient_destination;
        alpha_composite_div255(blended + COLOR_BIAS) >> 7u32
    };
    let mut result =
        channel(0) | (alpha_composite_div255(out_alpha_255 + ALPHA_BIAS) << alpha_shift);
    if CHANNELS == 4 {
        result = result | (channel(8) << 8u32) | (channel(16) << 16u32);
    }
    // A transparent source preserves all destination color bytes exactly.
    result = source_alpha.simd_eq(ZERO).bitselect(destination, result);
    if CHANNELS == 4 {
        output.copy_from_slice(bytemuck::cast_slice(&result.to_array().map(u32::to_le)));
    } else {
        output.copy_from_slice(bytemuck::cast_slice(
            &result.to_array().map(|v| (v as u16).to_le()),
        ));
    }
}

#[inline]
fn alpha_composite_vector_block<const CHANNELS: usize>(source: &[u8], output: &mut [u8]) {
    const BYTE: u32x8 = u32x8::new([255; 8]);
    let load = |bytes: &[u8]| -> u32x8 {
        if CHANNELS == 4 {
            let words: [u32; 8] = bytemuck::pod_read_unaligned(bytes);
            u32x8::new(words.map(u32::from_le))
        } else {
            let words: [u16; 8] = bytemuck::pod_read_unaligned(bytes);
            u32x8::from(u16x8::new(words.map(u16::from_le)))
        }
    };
    let source = load(source);
    let destination = load(output);
    let alpha_shift = ((CHANNELS - 1) * 8) as u32;
    let source_alpha = source >> alpha_shift;
    let destination_alpha = destination >> alpha_shift;
    let out_alpha_255 = source_alpha * BYTE + destination_alpha * (BYTE - source_alpha);
    let coefficient_source = alpha_composite_coefficient(source_alpha, out_alpha_255);
    let coefficient_destination = u32x8::splat(255 << 7) - coefficient_source;
    alpha_composite_store_vector_block::<CHANNELS>(
        source,
        destination,
        source_alpha,
        out_alpha_255,
        coefficient_source,
        coefficient_destination,
        output,
    );
}

#[inline]
fn alpha_composite_uniform_vector_block<const CHANNELS: usize>(
    source: &[u8],
    output: &mut [u8],
    source_alpha: u8,
    out_alpha_255: u32,
    coefficient_source: u32,
    coefficient_destination: u32,
) {
    let load = |bytes: &[u8]| -> u32x8 {
        if CHANNELS == 4 {
            let words: [u32; 8] = bytemuck::pod_read_unaligned(bytes);
            u32x8::new(words.map(u32::from_le))
        } else {
            let words: [u16; 8] = bytemuck::pod_read_unaligned(bytes);
            u32x8::from(u16x8::new(words.map(u16::from_le)))
        }
    };
    let source = load(source);
    let destination = load(output);
    let source_alpha_value = u32::from(source_alpha);
    if source_alpha_value == 0 {
        return;
    }
    alpha_composite_store_vector_block::<CHANNELS>(
        source,
        destination,
        u32x8::splat(source_alpha_value),
        u32x8::splat(out_alpha_255),
        u32x8::splat(coefficient_source),
        u32x8::splat(coefficient_destination),
        output,
    );
}

#[inline]
fn alpha_composite_uniform_alpha_pair<const CHANNELS: usize>(
    source: &[u8],
    output: &[u8],
) -> Option<(u8, u8)> {
    let source_alpha = source[CHANNELS - 1];
    let destination_alpha = output[CHANNELS - 1];
    for pixel in 1..8 {
        let alpha_index = pixel * CHANNELS + CHANNELS - 1;
        if source[alpha_index] != source_alpha || output[alpha_index] != destination_alpha {
            return None;
        }
    }
    Some((source_alpha, destination_alpha))
}

#[inline]
pub(crate) fn alpha_composite_repeated_pixel_pair<const CHANNELS: usize>(
    source: &[u8],
    output: &[u8],
) -> bool {
    debug_assert!(CHANNELS == 2 || CHANNELS == 4);
    debug_assert_eq!(source.len(), CHANNELS * 8);
    debug_assert_eq!(output.len(), CHANNELS * 8);

    // Compare whole native pixels in packed lanes. The SIMD fallback must
    // check this condition for each eight-pixel block, so avoid seven scalar
    // slice comparisons for both source and destination on every block.
    if CHANNELS == 4 {
        let source_words: [u32; 8] = bytemuck::pod_read_unaligned(source);
        if !u32x8::new(source_words)
            .simd_eq(u32x8::splat(source_words[0]))
            .all()
        {
            return false;
        }
        let output_words: [u32; 8] = bytemuck::pod_read_unaligned(output);
        u32x8::new(output_words)
            .simd_eq(u32x8::splat(output_words[0]))
            .all()
    } else {
        let source_words: [u16; 8] = bytemuck::pod_read_unaligned(source);
        if !u16x8::new(source_words)
            .simd_eq(u16x8::splat(source_words[0]))
            .all()
        {
            return false;
        }
        let output_words: [u16; 8] = bytemuck::pod_read_unaligned(output);
        u16x8::new(output_words)
            .simd_eq(u16x8::splat(output_words[0]))
            .all()
    }
}

#[inline]
pub(crate) fn alpha_composite_broadcast_pixel<const CHANNELS: usize>(
    pixel: &[u8],
    output: &mut [u8],
) {
    if CHANNELS == 4 {
        let value = u32::from_le_bytes([pixel[0], pixel[1], pixel[2], pixel[3]]);
        let words = [value.to_le(); 8];
        output.copy_from_slice(bytemuck::cast_slice(&words));
    } else {
        let value = u16::from_le_bytes([pixel[0], pixel[1]]);
        let words = [value.to_le(); 8];
        output.copy_from_slice(bytemuck::cast_slice(&words));
    }
}

pub(crate) fn alpha_composite_chunk<const CHANNELS: usize>(source: &[u8], output: &mut [u8]) {
    let block_bytes = CHANNELS * 8;
    let vector_bytes = output.len() / block_bytes * block_bytes;
    let mut cached_parameters = None;
    let mut cached_repeated_pixel = None;
    for (source, output) in source[..vector_bytes]
        .chunks_exact(block_bytes)
        .zip(output[..vector_bytes].chunks_exact_mut(block_bytes))
    {
        if alpha_composite_repeated_pixel_pair::<CHANNELS>(source, output) {
            let mut source_pixel = [0; 4];
            source_pixel[..CHANNELS].copy_from_slice(&source[..CHANNELS]);
            let mut destination_pixel = [0; 4];
            destination_pixel[..CHANNELS].copy_from_slice(&output[..CHANNELS]);
            let result_pixel = match cached_repeated_pixel {
                Some((cached_source, cached_destination, result))
                    if cached_source == source_pixel && cached_destination == destination_pixel =>
                {
                    result
                }
                _ => {
                    let mut result = destination_pixel;
                    alpha_composite_scalar_pixel(
                        &source_pixel[..CHANNELS],
                        &mut result[..CHANNELS],
                        CHANNELS,
                    );
                    result
                }
            };
            cached_repeated_pixel = Some((source_pixel, destination_pixel, result_pixel));
            alpha_composite_broadcast_pixel::<CHANNELS>(&result_pixel[..CHANNELS], output);
            continue;
        }
        let Some((source_alpha, destination_alpha)) =
            alpha_composite_uniform_alpha_pair::<CHANNELS>(source, output)
        else {
            alpha_composite_vector_block::<CHANNELS>(source, output);
            continue;
        };
        if source_alpha == 0 {
            continue;
        }
        let parameters = match cached_parameters {
            Some((source, destination, parameters))
                if source == source_alpha && destination == destination_alpha =>
            {
                parameters
            }
            _ => {
                let source = u32::from(source_alpha);
                let destination = u32::from(destination_alpha);
                let out_alpha = source * 255 + destination * (255 - source);
                let coefficient_source = source * 255 * 255 * 128 / out_alpha;
                let coefficient_destination = (255 << 7) - coefficient_source;
                (out_alpha, coefficient_source, coefficient_destination)
            }
        };
        cached_parameters = Some((source_alpha, destination_alpha, parameters));
        // Reuse exact coefficients across adjacent blocks with the same alpha
        // pair; mixed-alpha blocks retain the ordinary vector calculation.
        alpha_composite_uniform_vector_block::<CHANNELS>(
            source,
            output,
            source_alpha,
            parameters.0,
            parameters.1,
            parameters.2,
        );
    }
    for (source, output) in source[vector_bytes..]
        .chunks_exact(CHANNELS)
        .zip(output[vector_bytes..].chunks_exact_mut(CHANNELS))
    {
        alpha_composite_scalar_pixel(source, output, CHANNELS);
    }
}

#[inline]
pub(crate) fn alpha_composite_scalar_pixel(source: &[u8], output: &mut [u8], channels: usize) {
    let source_alpha = u32::from(source[channels - 1]);
    let destination_alpha = u32::from(output[channels - 1]);
    if source_alpha == 0 {
        return;
    }
    let blend = destination_alpha * (255 - source_alpha);
    let out_alpha_255 = source_alpha * 255 + blend;
    let coefficient_source = source_alpha * 255 * 255 * (1 << 7) / out_alpha_255;
    let coefficient_destination = (255 << 7) - coefficient_source;
    let div255 = |value: u32| ((value >> 8) + value) >> 8;
    let channel = |source: u8, destination: u8| {
        (div255(
            u32::from(source) * coefficient_source
                + u32::from(destination) * coefficient_destination
                + (0x80 << 7),
        ) >> 7)
            .min(255) as u8
    };
    output[0] = channel(source[0], output[0]);
    if channels == 4 {
        output[1] = channel(source[1], output[1]);
        output[2] = channel(source[2], output[2]);
    }
    output[channels - 1] = div255(out_alpha_255 + 0x80).min(255) as u8;
}


#[cfg(test)]
mod tests {
    #[test]
    fn alpha_composite_coefficient_matches_integer_division_for_all_alpha_pairs() {
        for source in 0..=255u32 {
            for destination_start in (0..256u32).step_by(8) {
                let denominators = std::array::from_fn(|lane| {
                    source * 255 + (destination_start + lane as u32) * (255 - source)
                });
                let actual = super::alpha_composite_coefficient(
                    wide::u32x8::splat(source),
                    wide::u32x8::new(denominators),
                )
                .to_array();
                for (lane, denominator) in denominators.into_iter().enumerate() {
                    let expected = source * 255 * 255 * 128 / denominator.max(1);
                    assert_eq!(
                        actual[lane],
                        expected,
                        "sa={source} da={}",
                        destination_start + lane as u32
                    );
                }
            }
        }
    }

    #[test]
    fn alpha_composite_uniform_vector_blocks_match_scalar_for_all_alpha_pairs() {
        let mut source = [0u8; 32];
        let mut destination = [0u8; 32];

        for source_alpha in 0..=u8::MAX {
            for destination_alpha in 0..=u8::MAX {
                for pixel in 0..8 {
                    let offset = pixel * 4;
                    let seed = (pixel as u8).wrapping_mul(31);
                    source[offset] = seed.wrapping_add(17);
                    source[offset + 1] = seed.wrapping_add(83);
                    source[offset + 2] = seed.wrapping_add(151);
                    source[offset + 3] = source_alpha;
                    destination[offset] = seed.wrapping_add(211);
                    destination[offset + 1] = seed.wrapping_add(139);
                    destination[offset + 2] = seed.wrapping_add(67);
                    destination[offset + 3] = destination_alpha;
                }

                let mut expected = destination;
                for pixel in 0..8 {
                    let offset = pixel * 4;
                    super::alpha_composite_scalar_pixel(
                        &source[offset..offset + 4],
                        &mut expected[offset..offset + 4],
                        4,
                    );
                }
                let mut actual = destination;
                super::alpha_composite_chunk::<4>(&source, &mut actual);
                assert_eq!(actual, expected, "sa={source_alpha} da={destination_alpha}");
            }
        }
    }

    #[test]
    fn alpha_composite_uniform_la_vector_blocks_match_scalar_for_all_alpha_pairs() {
        let mut source = [0u8; 16];
        let mut destination = [0u8; 16];

        for source_alpha in 0..=u8::MAX {
            for destination_alpha in 0..=u8::MAX {
                for pixel in 0..8 {
                    let offset = pixel * 2;
                    source[offset] = (pixel as u8).wrapping_mul(31).wrapping_add(17);
                    source[offset + 1] = source_alpha;
                    destination[offset] = (pixel as u8).wrapping_mul(19).wrapping_add(211);
                    destination[offset + 1] = destination_alpha;
                }

                let mut expected = destination;
                for pixel in 0..8 {
                    let offset = pixel * 2;
                    super::alpha_composite_scalar_pixel(
                        &source[offset..offset + 2],
                        &mut expected[offset..offset + 2],
                        2,
                    );
                }
                let mut actual = destination;
                super::alpha_composite_chunk::<2>(&source, &mut actual);
                assert_eq!(
                    actual, expected,
                    "LA sa={source_alpha} da={destination_alpha}"
                );
            }
        }
    }

    #[test]
    fn alpha_composite_repeated_pixel_blocks_match_scalar_for_all_alpha_pairs() {
        let mut source = [0u8; 64];
        let mut destination = [0u8; 64];

        for source_alpha in 0..=u8::MAX {
            for destination_alpha in 0..=u8::MAX {
                for pixel in 0..16 {
                    let offset = pixel * 4;
                    source[offset..offset + 4].copy_from_slice(&[17, 83, 151, source_alpha]);
                    destination[offset..offset + 4].copy_from_slice(&[
                        211,
                        139,
                        67,
                        destination_alpha,
                    ]);
                }

                let mut expected = destination;
                for pixel in 0..16 {
                    let offset = pixel * 4;
                    super::alpha_composite_scalar_pixel(
                        &source[offset..offset + 4],
                        &mut expected[offset..offset + 4],
                        4,
                    );
                }
                let mut actual = destination;
                super::alpha_composite_chunk::<4>(&source, &mut actual);
                assert_eq!(actual, expected, "sa={source_alpha} da={destination_alpha}");
            }
        }
    }

    #[test]
    fn alpha_composite_repeated_la_pixel_blocks_match_scalar_for_all_alpha_pairs() {
        let mut source = [0u8; 32];
        let mut destination = [0u8; 32];

        for source_alpha in 0..=u8::MAX {
            for destination_alpha in 0..=u8::MAX {
                for pixel in 0..16 {
                    let offset = pixel * 2;
                    source[offset..offset + 2].copy_from_slice(&[17, source_alpha]);
                    destination[offset..offset + 2].copy_from_slice(&[211, destination_alpha]);
                }

                let mut expected = destination;
                for pixel in 0..16 {
                    let offset = pixel * 2;
                    super::alpha_composite_scalar_pixel(
                        &source[offset..offset + 2],
                        &mut expected[offset..offset + 2],
                        2,
                    );
                }
                let mut actual = destination;
                super::alpha_composite_chunk::<2>(&source, &mut actual);
                assert_eq!(
                    actual, expected,
                    "LA sa={source_alpha} da={destination_alpha}"
                );
            }
        }
    }
}
