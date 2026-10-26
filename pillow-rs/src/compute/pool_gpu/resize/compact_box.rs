//! compact box for GPU resize exactness.

use super::super::*;

/// Evaluate over-limit Box rows whose normalized coefficient is constant.
///
/// Pillow's `Resample.c::precompute_coeffs` gives an integer-ratio Box
/// downscale exactly `1 / tap_count` for every selected source word.  Such a
/// row does not need one four-word f64 coefficient record per tap: the compact
/// device path transports one record and repeats it in the ordered reducer.
/// Finite rows use the integer state below; IEEE special rows use the same
/// ordered NaN/infinity state machine as marker 9 before being admitted.
pub(in crate::compute::pool_gpu) fn gpu_f_resize_compact_box_sample_bits(
    bytes: &[u8],
    source_dimensions: (u32, u32),
    source_axis: u32,
    output_axis: u32,
    horizontal: bool,
    output_index: usize,
    line: usize,
) -> Option<u32> {
    let (source_w, tap_count, source_start) = gpu_f_resize_compact_box_sample_plan(
        source_dimensions,
        source_axis,
        output_axis,
        horizontal,
        output_index,
        line,
    )?;
    let coefficient = 1.0 / f64::from(u32::try_from(tap_count).ok()?);
    let coefficient_parts = gpu_f64_integer_parts(coefficient)?;
    let mut state = F64OrderedState {
        magnitude: 0,
        exponent: 0,
        negative: false,
        kind: F64OrderedKind::Finite,
    };
    let mut ordered_accumulator = 0.0f64;
    let mut first_nan = None;
    let mut positive_infinity = false;
    let mut negative_infinity = false;
    let mut has_special = false;
    for tap in 0..tap_count {
        let coordinate = source_start.checked_add(tap)?;
        let pixel = if horizontal {
            line.checked_mul(source_w)?.checked_add(coordinate)?
        } else {
            coordinate.checked_mul(source_w)?.checked_add(line)?
        };
        let offset = pixel.checked_mul(4)?;
        let word = bytes.get(offset..offset.checked_add(4)?)?;
        let bits = u32::from_le_bytes([word[0], word[1], word[2], word[3]]);
        let exponent_bits = (bits >> 23) & 0xff;
        let separate_product_add =
            gpu_f_resize_uses_separate_horizontal_product_add(horizontal, tap_count, tap);
        gpu_f_resize_accumulate_f64(
            &mut ordered_accumulator,
            coefficient,
            f32::from_bits(bits),
            separate_product_add,
        );
        if exponent_bits == 0xff {
            has_special = true;
            let fraction = bits & 0x7f_ff_ff;
            if fraction != 0 {
                first_nan.get_or_insert(bits | 0x0040_0000);
            } else if (bits & 0x8000_0000) != 0 {
                negative_infinity = true;
            } else {
                positive_infinity = true;
            }
            continue;
        }
        let sample = gpu_f32_f64_integer_parts(bits)?;
        if sample.mantissa == 0 {
            continue;
        }
        let product =
            u128::from(sample.mantissa).checked_mul(u128::from(coefficient_parts.mantissa))?;
        let product_exp = sample
            .exponent
            .checked_sub(23)?
            .checked_add(coefficient_parts.exponent)?;
        state = gpu_f64_ordered_add_product(
            state,
            product,
            product_exp,
            sample.negative != coefficient_parts.negative,
            separate_product_add,
        )?;
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
            return None;
        };
        let expected = (ordered_accumulator as f32).to_bits();
        return (actual == expected).then_some(actual);
    }
    let actual = gpu_f64_ordered_state_to_f32(state)?;
    let expected = (ordered_accumulator as f32).to_bits();
    (actual == expected).then_some(actual)
}

#[inline(always)]
fn gpu_f_resize_compact_box_sample_plan(
    source_dimensions: (u32, u32),
    source_axis: u32,
    output_axis: u32,
    horizontal: bool,
    output_index: usize,
    line: usize,
) -> Option<(usize, usize, usize)> {
    if source_axis == 0
        || output_axis == 0
        || !source_axis.is_multiple_of(output_axis)
        || output_index >= usize::try_from(output_axis).ok()?
        || line
            >= usize::try_from(if horizontal {
                source_dimensions.1
            } else {
                source_dimensions.0
            })
            .ok()?
    {
        return None;
    }
    let source_w = usize::try_from(source_dimensions.0).ok()?;
    let source_h = usize::try_from(source_dimensions.1).ok()?;
    let source_axis_usize = usize::try_from(source_axis).ok()?;
    let output_axis_usize = usize::try_from(output_axis).ok()?;
    if source_axis_usize != if horizontal { source_w } else { source_h } || output_axis_usize == 0 {
        return None;
    }
    let tap_count = source_axis_usize.checked_div(output_axis_usize)?;
    let source_start = output_index.checked_mul(tap_count)?;
    Some((source_w, tap_count, source_start))
}

pub(in crate::compute::pool_gpu) fn gpu_f_resize_compact_box_pass_bits(
    bytes: &[u8],
    source_dimensions: (u32, u32),
    source_axis: u32,
    output_axis: u32,
    horizontal: bool,
) -> Option<Vec<u32>> {
    let line_count = usize::try_from(if horizontal {
        source_dimensions.1
    } else {
        source_dimensions.0
    })
    .ok()?;
    let output_count = usize::try_from(output_axis).ok()?;
    let word_count = line_count.checked_mul(output_count)?;
    let mut result = Vec::new();
    result.try_reserve(word_count).ok()?;
    if horizontal {
        for line in 0..line_count {
            for output_index in 0..output_count {
                result.push(gpu_f_resize_compact_box_sample_bits(
                    bytes,
                    source_dimensions,
                    source_axis,
                    output_axis,
                    true,
                    output_index,
                    line,
                )?);
            }
        }
    } else {
        for output_index in 0..output_count {
            for line in 0..line_count {
                result.push(gpu_f_resize_compact_box_sample_bits(
                    bytes,
                    source_dimensions,
                    source_axis,
                    output_axis,
                    false,
                    output_index,
                    line,
                )?);
            }
        }
    }
    Some(result)
}

pub(in crate::compute::pool_gpu) fn gpu_f_resize_compact_box_axis(
    source_size: u32,
    output_size: u32,
) -> bool {
    output_size != 0
        && source_size.is_multiple_of(output_size)
        && source_size / output_size > GPU_F_RESIZE_ORDERED_MAX_TAPS as u32
}

#[inline]
pub(in crate::compute::pool_gpu) fn gpu_f_resize_compact_box_any_axis(
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
) -> bool {
    gpu_f_resize_compact_box_axis(source_dimensions.0, output_dimensions.0)
        || gpu_f_resize_compact_box_axis(source_dimensions.1, output_dimensions.1)
}

/// Return whether a direct F Box resize needs only the vertical compact pass.
/// The unchanged horizontal axis has one unit coefficient and must remain an
/// identity copy; skipping that pass is what keeps an over-limit source height
/// below the adapter's workgroup-per-dimension bound.
pub(in crate::compute::pool_gpu) fn gpu_f_resize_compact_box_vertical_only_geometry(
    op: &PipelineOp,
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    logical_mode: Option<&str>,
) -> bool {
    if logical_mode != Some("F") {
        return false;
    }
    let PipelineOp::Resize { w, h, filter } = op else {
        return false;
    };
    matches!(filter, ResampleFilter::Box)
        && (*w, *h) == output_dimensions
        && output_dimensions.0 == source_dimensions.0
        && !gpu_f_resize_compact_box_axis(source_dimensions.0, output_dimensions.0)
        && gpu_f_resize_compact_box_axis(source_dimensions.1, output_dimensions.1)
}

/// Prove the compact over-limit Box domain without materializing a full f64
/// coefficient table.  An integer-ratio Box downscale has one normalized
/// weight per source tap, so a single repeated coefficient is enough on the
/// device for every output row.  At most one axis can exceed the full-table
/// bound for an image that fits the device buffer budget. The other axis keeps
/// its ordinary coefficient table; when that table is also proven exact, the
/// second axis may change and the host proof below validates the materialized
/// intermediate between passes.
pub(in crate::compute::pool_gpu) fn gpu_f_resize_compact_box_is_exact(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if logical_mode != Some("F") || ops.len() != 1 || !matches!(image, DynamicImage::ImageRgba8(_))
    {
        return false;
    }
    let DynamicImage::ImageRgba8(pixels) = image else {
        return false;
    };
    let PipelineOp::Resize { w, h, filter } = &ops[0] else {
        return false;
    };
    if !matches!(filter, ResampleFilter::Box) || *w == 0 || *h == 0 {
        return false;
    }
    let source_dimensions = image.dimensions();
    let source_checked = CheckedDims::new(source_dimensions.0, source_dimensions.1, 4).ok();
    if source_dimensions.0 == 0
        || source_dimensions.1 == 0
        || source_checked
            .as_ref()
            .is_none_or(|dims| dims.total_pixels() > GPU_BUFFER_CAPACITY as usize)
        || source_checked.map(|dims| dims.total_bytes()) != Some(pixels.as_raw().len())
    {
        return false;
    }
    let horizontal_compact = gpu_f_resize_compact_box_axis(source_dimensions.0, *w);
    let vertical_compact = gpu_f_resize_compact_box_axis(source_dimensions.1, *h);
    if (horizontal_compact && vertical_compact) || (!horizontal_compact && !vertical_compact) {
        return false;
    }

    // Pillow's `src/libImaging/Resample.c` keeps a wide, non-tall Box resize
    // horizontal-first, so a compact horizontal row may be followed by an
    // ordinary small-axis Box pass. Keep this extension finite-only: the
    // compact shader's special-state machine is exact for a terminal row, but
    // a second pass would require proving NaN payload propagation through the
    // materialized intermediate.
    let second_axis_changes = (!horizontal_compact && *w != source_dimensions.0)
        || (!vertical_compact && *h != source_dimensions.1);
    if vertical_compact && *w != source_dimensions.0 {
        // This orientation is always a Pillow tall-image resize for an
        // image that fits the pixel-capacity budget; its native order is
        // vertical-first, while the GPU plan is horizontal-first.
        return false;
    }
    if second_axis_changes
        && pixels.as_raw().chunks_exact(4).any(|word| {
            let bits = u32::from_le_bytes([word[0], word[1], word[2], word[3]]);
            ((bits >> 23) & 0xff) == 0xff
        })
    {
        return false;
    }

    gpu_f_resize_compact_box_materialization_matches(
        pixels.as_raw(),
        source_dimensions,
        (*w, *h),
        horizontal_compact,
        vertical_compact,
        *filter,
    )
}

fn gpu_f_resize_compact_box_materialization_matches(
    source_bytes: &[u8],
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    horizontal_compact: bool,
    vertical_compact: bool,
    filter: ResampleFilter,
) -> bool {
    let (kernel, support) = filter_from_resample(filter);
    let ordered_pass =
        |bytes: &[u8], dimensions: (u32, u32), coeffs: &FilterCoeffsF64, horizontal| {
            let safe = coeffs.xmin.len() == coeffs.count.len()
                && coeffs.xmin.len() == coeffs.weights.len()
                && gpu_f_resize_f64_coefficients_fit_binding(coeffs)
                && coeffs
                    .count
                    .iter()
                    .all(|&count| count <= GPU_F_RESIZE_ORDERED_MAX_TAPS);
            if !safe {
                return None;
            }
            gpu_f_resize_f64_ordered_pass_bits(bytes, dimensions, coeffs, horizontal)
        };
    let words_to_bytes = |words: Vec<u32>| -> Option<Vec<u8>> {
        let byte_count = words.len().checked_mul(4)?;
        let mut result = Vec::new();
        result.try_reserve(byte_count).ok()?;
        for word in words {
            result.extend_from_slice(&word.to_le_bytes());
        }
        Some(result)
    };
    let mut bytes = source_bytes.to_vec();
    let mut dimensions = source_dimensions;

    if horizontal_compact {
        let Some(words) = gpu_f_resize_compact_box_pass_bits(
            &bytes,
            dimensions,
            source_dimensions.0,
            output_dimensions.0,
            true,
        ) else {
            return false;
        };
        let Some(next_bytes) = words_to_bytes(words) else {
            return false;
        };
        bytes = next_bytes;
        dimensions.0 = output_dimensions.0;
    } else if output_dimensions.0 != source_dimensions.0 {
        let coeffs =
            precompute_coeffs_f64(output_dimensions.0, source_dimensions.0, kernel, support);
        let Some(words) = ordered_pass(&bytes, dimensions, &coeffs, true) else {
            return false;
        };
        let Some(next_bytes) = words_to_bytes(words) else {
            return false;
        };
        bytes = next_bytes;
        dimensions.0 = output_dimensions.0;
    }

    if vertical_compact {
        let Some(words) = gpu_f_resize_compact_box_pass_bits(
            &bytes,
            dimensions,
            source_dimensions.1,
            output_dimensions.1,
            false,
        ) else {
            return false;
        };
        let Some(next_bytes) = words_to_bytes(words) else {
            return false;
        };
        bytes = next_bytes;
        dimensions.1 = output_dimensions.1;
    } else if output_dimensions.1 != source_dimensions.1 {
        let coeffs =
            precompute_coeffs_f64(output_dimensions.1, source_dimensions.1, kernel, support);
        let Some(words) = ordered_pass(&bytes, dimensions, &coeffs, false) else {
            return false;
        };
        let Some(next_bytes) = words_to_bytes(words) else {
            return false;
        };
        bytes = next_bytes;
        dimensions.1 = output_dimensions.1;
    }

    dimensions == output_dimensions && !bytes.is_empty()
}

pub(in crate::compute::pool_gpu) fn gpu_f_resize_compact_box_vertical_only_is_exact(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if ops.len() != 1 {
        return false;
    }
    let Some(output_dimensions) = op_output_dims(&ops[0], image.width(), image.height()) else {
        return false;
    };
    gpu_f_resize_compact_box_vertical_only_geometry(
        &ops[0],
        image.dimensions(),
        output_dimensions,
        logical_mode,
    ) && gpu_f_resize_compact_box_is_exact(ops, image, logical_mode)
}

/// Pillow's high-level Image.resize uses a vertical-first pair of resample
/// passes for very tall inputs before calling Resample.c. Pure F resize
/// chains lower this geometry into separately proven vertical/horizontal
/// segments; mixed pipelines retain the host-order guard.
pub(in crate::compute::pool_gpu) fn gpu_f_resize_uses_pillow_tall_order(
    source_dimensions: (u32, u32),
    destination_dimensions: (u32, u32),
) -> bool {
    source_dimensions.1 > source_dimensions.0.saturating_mul(100)
        && destination_dimensions.1 < source_dimensions.1
        && destination_dimensions.0 != source_dimensions.0
}

/// Check the static bounds required by the exact ordered-f64 GPU reducer.
/// Its integer implementation mirrors Pillow's per-tap binary64 rounding;
/// input words are handled in the shader, including subnormals and IEEE
/// specials. Keep the unrolled horizontal FMA domain bounded to 15 taps and
/// avoid tiled coefficient transport here; wider cases use the existing
/// input-driven proof until they have a matching device contract.
pub(in crate::compute::pool_gpu) fn gpu_f_resize_f64_ordered_shader_is_supported(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if logical_mode != Some("F") || ops.len() != 1 || !matches!(image, DynamicImage::ImageRgba8(_))
    {
        return false;
    }
    let DynamicImage::ImageRgba8(pixels) = image else {
        return false;
    };
    let PipelineOp::Resize { w, h, filter } = &ops[0] else {
        return false;
    };
    let source_dimensions = image.dimensions();
    let source_checked = CheckedDims::new(source_dimensions.0, source_dimensions.1, 4).ok();
    if source_checked
        .as_ref()
        .is_none_or(|dims| dims.total_pixels() > GPU_BUFFER_CAPACITY as usize)
        || source_checked.map(|dims| dims.total_bytes()) != Some(pixels.as_raw().len())
    {
        return false;
    }

    gpu_f_resize_f64_ordered_geometry_is_supported(source_dimensions, (*w, *h), *filter)
}

pub(in crate::compute::pool_gpu) fn gpu_f_resize_f64_ordered_geometry_is_supported(
    source_dimensions: (u32, u32),
    destination_dimensions: (u32, u32),
    filter: ResampleFilter,
) -> bool {
    gpu_f_resize_f64_ordered_coefficients(source_dimensions, destination_dimensions, filter)
        .is_some()
}

pub(in crate::compute::pool_gpu) fn gpu_f_resize_f64_ordered_coefficients(
    source_dimensions: (u32, u32),
    destination_dimensions: (u32, u32),
    filter: ResampleFilter,
) -> Option<(Arc<FilterCoeffsF64>, Arc<FilterCoeffsF64>)> {
    if matches!(filter, ResampleFilter::Nearest)
        || source_dimensions.0 == 0
        || source_dimensions.1 == 0
        || destination_dimensions.0 == 0
        || destination_dimensions.1 == 0
        || destination_dimensions == source_dimensions
        || CheckedDims::new(destination_dimensions.0, destination_dimensions.1, 1)
            .ok()
            .is_none_or(|dims| dims.total_pixels() > GPU_BUFFER_CAPACITY as usize)
    {
        return None;
    }

    let (kernel, support) = filter_from_resample(filter);
    let horizontal = precompute_coeffs_f64(
        destination_dimensions.0,
        source_dimensions.0,
        kernel,
        support,
    );
    let vertical = precompute_coeffs_f64(
        destination_dimensions.1,
        source_dimensions.1,
        kernel,
        support,
    );
    for (coeffs, horizontal_axis) in [(&horizontal, true), (&vertical, false)] {
        if coeffs.xmin.len() != coeffs.count.len()
            || coeffs.xmin.len() != coeffs.weights.len()
            || coeffs.count.iter().any(|&count| {
                count == 0
                    || count > GPU_F_RESIZE_TILE_TAPS
                    || (horizontal_axis && count > GPU_F_RESIZE_HORIZONTAL_FMA_MAX_TAPS)
            })
            || !gpu_f_resize_f64_coefficients_fit_binding(coeffs.as_ref())
        {
            return None;
        }
    }
    Some((horizontal, vertical))
}

/// Size the per-stage arenas for an explicit streamed F Resize before input
/// upload. The batch planner reserves this exact parameter and ordered-f64
/// coefficient storage, while eager dispatch keeps its independent limits.
pub(in crate::compute::pool_gpu) fn gpu_f_resize_ordered_stage_resource_bytes(
    source_dimensions: (u32, u32),
    destination_dimensions: (u32, u32),
    filter: ResampleFilter,
    uniform_alignment: usize,
    storage_alignment: usize,
) -> Option<usize> {
    let (horizontal, vertical) =
        gpu_f_resize_f64_ordered_coefficients(source_dimensions, destination_dimensions, filter)?;
    let coefficient_bytes = |coeffs: &FilterCoeffsF64| {
        resize_coeff_word_count_f64(coeffs)
            .ok()?
            .checked_mul(std::mem::size_of::<u32>())
            .map(|bytes| aligned_bytes(bytes, storage_alignment))
    };
    let horizontal_bytes = coefficient_bytes(horizontal.as_ref())?;
    let vertical_bytes = coefficient_bytes(vertical.as_ref())?;
    let resize = PipelineOp::Resize {
        w: destination_dimensions.0,
        h: destination_dimensions.1,
        filter,
    };
    let parameter_words = 4usize
        .checked_add(registry::extract_params(&resize).len())?
        .checked_add(2)?;
    let parameter_bytes = parameter_words.checked_mul(std::mem::size_of::<u32>())?;
    aligned_bytes(parameter_bytes, uniform_alignment)
        .checked_add(horizontal_bytes)?
        .checked_add(vertical_bytes)
}

/// Prove one direct, changed-axis F resize in the bounded ordered-f64 domain.
/// Intermediate horizontal words are materialized before the vertical pass,
/// exactly as Pillow's separable resampler does. Chained/relocation inputs
/// remain on exact host semantic control until they receive a separate proof.
pub(in crate::compute::pool_gpu) fn gpu_f_resize_f64_ordered_is_exact(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if gpu_f_resize_compact_box_is_exact(ops, image, logical_mode) {
        return true;
    }
    if logical_mode != Some("F") || ops.len() != 1 || !matches!(image, DynamicImage::ImageRgba8(_))
    {
        return false;
    }
    let DynamicImage::ImageRgba8(pixels) = image else {
        return false;
    };
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
    if CheckedDims::new(*w, *h, 1)
        .ok()
        .is_none_or(|dims| dims.total_pixels() > GPU_BUFFER_CAPACITY as usize)
    {
        return false;
    }
    let source_checked = CheckedDims::new(source_dimensions.0, source_dimensions.1, 4).ok();
    if source_checked
        .as_ref()
        .is_none_or(|dims| dims.total_pixels() > GPU_BUFFER_CAPACITY as usize)
        || source_checked.map(|dims| dims.total_bytes()) != Some(pixels.as_raw().len())
    {
        return false;
    }
    let (kernel, support) = filter_from_resample(*filter);
    let horizontal = precompute_coeffs_f64(*w, source_dimensions.0, kernel, support);
    let vertical = precompute_coeffs_f64(*h, source_dimensions.1, kernel, support);
    if [&horizontal, &vertical].iter().any(|coeffs| {
        coeffs.xmin.len() != coeffs.count.len()
            || coeffs.xmin.len() != coeffs.weights.len()
            || coeffs
                .count
                .iter()
                .any(|&count| count > GPU_BUFFER_CAPACITY as usize)
    }) {
        return false;
    }
    let horizontal_changed = *w != source_dimensions.0;
    let vertical_changed = *h != source_dimensions.1;
    if !horizontal_changed && !vertical_changed {
        return false;
    }
    let words_to_bytes = |words: Vec<u32>| -> Option<Vec<u8>> {
        let byte_count = words.len().checked_mul(4)?;
        let mut result = Vec::new();
        result.try_reserve(byte_count).ok()?;
        for word in words {
            result.extend_from_slice(&word.to_le_bytes());
        }
        Some(result)
    };
    let mut bytes = pixels.as_raw().to_vec();
    let mut dimensions = source_dimensions;
    if horizontal_changed {
        let words = gpu_f_resize_f64_ordered_pass_bits(&bytes, dimensions, &horizontal, true);
        let Some(next_bytes) = words.and_then(words_to_bytes) else {
            return false;
        };
        bytes = next_bytes;
        dimensions.0 = *w;
    }
    if vertical_changed {
        let words = gpu_f_resize_f64_ordered_pass_bits(&bytes, dimensions, &vertical, false);
        let Some(_next_bytes) = words.and_then(words_to_bytes) else {
            return false;
        };
    }
    true
}
