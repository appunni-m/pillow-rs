//! Native SIMD alpha-composite adapters and fixed-point kernels.

use super::{SimdImageShape, SimdLayout, reverse_pixel_block, simd_unsupported};
use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::{Image, preserve_mode};
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, GenericImageView};
use wide::{i32x8, u16x8, u32x8};

fn alpha_composite_layout_mode(shape: SimdLayout) -> Option<&'static str> {
    match shape {
        SimdLayout::LumaA8 => Some("LA"),
        SimdLayout::Rgba8 => Some("RGBA"),
        _ => None,
    }
}

pub(super) fn simd_alpha_composite_operands_supported(
    img: &DynamicImage,
    source: &Image,
    mode: Option<&str>,
) -> bool {
    let expected_mode = match img {
        DynamicImage::ImageLumaA8(_) => "LA",
        DynamicImage::ImageRgba8(_) => "RGBA",
        _ => return false,
    };
    if mode.is_some_and(|value| value != expected_mode) {
        return false;
    }
    let pixels_supported = img
        .width()
        .checked_mul(img.height())
        .is_some_and(|pixels| pixels == 0 || pixels >= 8);
    source.mode().ok().as_deref() == Some(expected_mode)
        && source.size().ok() == Some(img.dimensions())
        && pixels_supported
}

fn simd_alpha_composite_pixels_supported(width: u32, height: u32) -> bool {
    width
        .checked_mul(height)
        .is_some_and(|pixels| pixels == 0 || pixels >= 8)
}

pub(super) fn simd_alpha_composite_shape_supported(
    shape: SimdImageShape,
    source: &Image,
    mode: Option<&str>,
) -> bool {
    let Some(expected_mode) = alpha_composite_layout_mode(shape.layout) else {
        return false;
    };
    mode.is_none_or(|value| value == expected_mode)
        && source.mode().ok().as_deref() == Some(expected_mode)
        && source.size().ok() == Some((shape.width, shape.height))
        && simd_alpha_composite_pixels_supported(shape.width, shape.height)
}

pub(super) fn supports_for_image(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    let PipelineOp::AlphaComposite { source, dest, src } = op else {
        return false;
    };
    *dest == (0, 0) && *src == (0, 0) && simd_alpha_composite_operands_supported(img, source, mode)
}

pub(super) fn supports_for_shape(
    shape: SimdImageShape,
    op: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    let PipelineOp::AlphaComposite { source, dest, src } = op else {
        return false;
    };
    *dest == (0, 0) && *src == (0, 0) && simd_alpha_composite_shape_supported(shape, source, mode)
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
fn alpha_composite_repeated_pixel_pair<const CHANNELS: usize>(
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
fn alpha_composite_broadcast_pixel<const CHANNELS: usize>(pixel: &[u8], output: &mut [u8]) {
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

fn alpha_composite_chunk<const CHANNELS: usize>(source: &[u8], output: &mut [u8]) {
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
fn alpha_composite_scalar_pixel(source: &[u8], output: &mut [u8], channels: usize) {
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

fn simd_alpha_composite_native(
    img: &DynamicImage,
    source: &Image,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    if !simd_alpha_composite_operands_supported(img, source, mode) {
        return Ok(None);
    }
    let source_image = source.materialized_shared()?;
    let channels = match (img, source_image.as_ref()) {
        (DynamicImage::ImageLumaA8(_), DynamicImage::ImageLumaA8(_)) => 2,
        (DynamicImage::ImageRgba8(_), DynamicImage::ImageRgba8(_)) => 4,
        _ => return Ok(None),
    };
    let source_bytes = source_image.as_bytes();
    let destination_bytes = img.as_bytes();
    if source_bytes.len() != destination_bytes.len() {
        return Ok(None);
    }
    let row_stride = (img.width() as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("SIMD AlphaComposite row stride overflow".into()))?;
    if row_stride.checked_mul(img.height() as usize) != Some(destination_bytes.len()) {
        return Ok(None);
    }

    let mut output = destination_bytes.to_vec();
    // There is no row dependency: complete pixels stream across row boundaries.
    // Coarse disjoint tiles amortize task scheduling for larger buffers.
    let pixels = output.len() / channels;
    let vector_blocks = pixels / 8;
    let scalar_tail = pixels % 8;
    let process_chunk = |source: &[u8], output: &mut [u8]| {
        if channels == 4 {
            alpha_composite_chunk::<4>(source, output);
        } else {
            alpha_composite_chunk::<2>(source, output);
        }
    };
    #[cfg(feature = "parallel")]
    if output.len() >= 1024 * 1024 {
        let tile_bytes = 64 * 1024;
        let tiles = output.len().div_ceil(tile_bytes);
        super::simd_rows_mut_serial!(
            &mut output,
            tile_bytes,
            tiles,
            |start, _end, _tile, bytes| {
                process_chunk(&source_bytes[start..start + bytes.len()], bytes);
            }
        );
    } else {
        process_chunk(source_bytes, &mut output);
    }
    #[cfg(not(feature = "parallel"))]
    process_chunk(source_bytes, &mut output);
    if vector_blocks == 0 {
        // Empty images have no pixel data to vectorize. They are still a
        // valid SIMD-capable operation after scalar validation and produce an
        // empty native buffer without entering a CPU implementation.
        crate::compute::record_pipeline_operation_path("scalar-control");
    } else {
        crate::compute::record_pipeline_operation_path("vector");
    }
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks as u64);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);
    crate::image_utils::raw_bytes_to_image_allow_empty(img.width(), img.height(), output, channels)
        .map(|result| preserve_mode(img, result))
        .map(Some)
}

/// Composite and horizontally mirror a full-frame RGBA image in one vector
/// pass. Eight source pixels are composited together, then their two native
/// four-pixel byte vectors are reversed into the final output row. Width tails
/// use the same exact scalar pixel helper as the single-operation adapter.
pub(crate) fn simd_alpha_composite_mirror(
    img: &DynamicImage,
    source: &Image,
    mode: Option<&str>,
) -> Result<(DynamicImage, u64, u64, u64, u64), PilError> {
    if !matches!(img, DynamicImage::ImageRgba8(_))
        || !matches!(mode, None | Some("RGBA"))
        || img.width() < 8
        || img.height() == 0
        || !simd_alpha_composite_operands_supported(img, source, mode)
    {
        return Err(simd_unsupported("AlphaComposite+Mirror"));
    }
    let source_owner = source.materialized_shared()?;
    let DynamicImage::ImageRgba8(source_image) = source_owner.as_ref() else {
        return Err(simd_unsupported("AlphaComposite+Mirror"));
    };
    if source_image.dimensions() != img.dimensions() {
        return Err(PilError::ValueError("images do not match".into()));
    }

    let dimensions = CheckedDims::new(img.width(), img.height(), 4)?;
    let row_stride = dimensions.row_stride();
    let destination_bytes = img.as_bytes();
    let source_bytes = source_image.as_raw();
    if destination_bytes.len() != dimensions.total_bytes()
        || source_bytes.len() != dimensions.total_bytes()
    {
        return Err(PilError::InternalError(
            "SIMD RGBA alpha composite mirror storage does not match its dimensions".into(),
        ));
    }

    let width = img.width() as usize;
    let vector_pixels = width / 8 * 8;
    let vector_groups_per_row = vector_pixels / 8;
    let mut output = dimensions.alloc_buffer();
    let mut mirror_vector_blocks = 0u64;
    for row in 0..img.height() as usize {
        let row_start = row * row_stride;
        for output_x in (0..vector_pixels).step_by(8) {
            let source_x = width - output_x - 8;
            let source_start = row_start + source_x * 4;
            let source_block = &source_bytes[source_start..source_start + 32];
            let destination_block = &destination_bytes[source_start..source_start + 32];
            let output_start = row_start + output_x * 4;

            if alpha_composite_repeated_pixel_pair::<4>(source_block, destination_block) {
                let mut result_pixel = [0u8; 4];
                result_pixel.copy_from_slice(&destination_block[..4]);
                alpha_composite_scalar_pixel(&source_block[..4], &mut result_pixel, 4);
                // The composite block is constant, so its mirror is identical.
                alpha_composite_broadcast_pixel::<4>(
                    &result_pixel,
                    &mut output[output_start..output_start + 32],
                );
                continue;
            }

            let mut composite = [0u8; 32];
            composite.copy_from_slice(destination_block);
            alpha_composite_chunk::<4>(source_block, &mut composite);

            let reversed_right = reverse_pixel_block(&composite[16..], 4).ok_or_else(|| {
                PilError::InternalError("SIMD RGBA mirror vector block is incomplete".into())
            })?;
            let reversed_left = reverse_pixel_block(&composite[..16], 4).ok_or_else(|| {
                PilError::InternalError("SIMD RGBA mirror vector block is incomplete".into())
            })?;
            output[output_start..output_start + 16].copy_from_slice(&reversed_right);
            output[output_start + 16..output_start + 32].copy_from_slice(&reversed_left);
            mirror_vector_blocks = mirror_vector_blocks.saturating_add(2);
        }

        // The ungrouped source pixels occupy the left edge of the source row
        // and therefore the right edge of the mirrored output row.
        for output_x in vector_pixels..width {
            let source_x = width - 1 - output_x;
            let source_offset = row_start + source_x * 4;
            let output_offset = row_start + output_x * 4;
            output[output_offset..output_offset + 4]
                .copy_from_slice(&destination_bytes[source_offset..source_offset + 4]);
            alpha_composite_scalar_pixel(
                &source_bytes[source_offset..source_offset + 4],
                &mut output[output_offset..output_offset + 4],
                4,
            );
        }
    }

    let vector_blocks = (vector_groups_per_row * img.height() as usize) as u64;
    let scalar_tail = ((width - vector_pixels) * img.height() as usize) as u64;
    let result =
        crate::image_utils::raw_bytes_to_image_allow_empty(img.width(), img.height(), output, 4)?;
    Ok((
        preserve_mode(img, result),
        vector_blocks,
        scalar_tail,
        mirror_vector_blocks,
        scalar_tail,
    ))
}

pub fn simd_alpha_composite(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::AlphaComposite { source, dest, src } = op else {
        return Err(PilError::ValueError(
            "expected AlphaComposite op".to_string(),
        ));
    };
    if *dest != (0, 0) || *src != (0, 0) {
        return Err(simd_unsupported("AlphaComposite"));
    }
    simd_alpha_composite_native(img, source, mode)?
        .ok_or_else(|| simd_unsupported("AlphaComposite"))
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
