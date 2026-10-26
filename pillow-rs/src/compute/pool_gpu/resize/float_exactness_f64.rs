//! f64 proof for exact F-mode GPU resize admission.

use super::super::{
    CheckedDims, DynamicImage, FilterCoeffsF64, GPU_BUFFER_CAPACITY, GenericImageView, PipelineOp,
    PixelMode, ResampleFilter, filter_from_resample, gpu_f_resize_f64_coefficients_fit_binding,
    gpu_f_resize_f64_pass_bits, gpu_f_resize_nearest_pass_words, gpu_f_resize_relocate_words,
    gpu_f64_integer_parts, gpu_resize_coefficients, precompute_coeffs_f64,
};

/// Admit one or more filtered F resizes to marker 9 when every f64 coefficient
/// product and final f32 store is reproduced by the exact integer reducer.
/// Each changed axis is materialized as rounded f32 words before the next
/// resize is checked, matching Pillow's observable boundary between chained
/// operations. The encoder places every F resize's horizontal and vertical
/// reducers in separate compute passes so the next stage reads completed
/// intermediates.
pub(in crate::compute::pool_gpu) fn gpu_f_resize_f64_is_exact(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if logical_mode != Some("F") || ops.is_empty() || !matches!(image, DynamicImage::ImageRgba8(_))
    {
        return false;
    }
    let DynamicImage::ImageRgba8(pixels) = image else {
        return false;
    };
    let mut source_dimensions = image.dimensions();
    let source_checked = CheckedDims::new(source_dimensions.0, source_dimensions.1, 4).ok();
    if source_dimensions.0 == 0
        || source_dimensions.1 == 0
        || source_checked
            .as_ref()
            .is_none_or(|dims| dims.total_pixels() > GPU_BUFFER_CAPACITY as usize)
    {
        return false;
    }
    let expected_bytes = source_checked.map(|dims| dims.total_bytes());
    let mut bytes = pixels.as_raw().to_vec();
    if expected_bytes != Some(bytes.len()) {
        return false;
    }
    let mut changed_any = false;

    for op in ops {
        if !gpu_f_resize_f64_materialize_op(
            op,
            &mut bytes,
            &mut source_dimensions,
            &mut changed_any,
        ) {
            return false;
        }
    }
    changed_any
}

fn gpu_f_resize_f64_materialize_op(
    op: &PipelineOp,
    bytes: &mut Vec<u8>,
    source_dimensions: &mut (u32, u32),
    changed_any: &mut bool,
) -> bool {
    if let PipelineOp::PutData {
        data,
        mode: PixelMode::F,
    } = op
    {
        // PutData(F) is a raw little-endian word replacement in the GPU
        // shader. Replace the proof source with those exact words before
        // checking the following resize, just as Pillow's deferred
        // pipeline does at its materialization boundary.
        let expected_bytes = CheckedDims::new(source_dimensions.0, source_dimensions.1, 4)
            .ok()
            .map(|dims| dims.total_bytes());
        if expected_bytes != Some(data.len()) || !data.chunks_exact(4).remainder().is_empty() {
            return false;
        }
        *bytes = data.to_vec();
        return true;
    }
    if let PipelineOp::Resize { w, h, filter } = op {
        return gpu_f_resize_f64_resize_is_exact(
            *w,
            *h,
            *filter,
            bytes,
            source_dimensions,
            changed_any,
        );
    }

    // Complete-word relocation operations preserve the scalar F
    // representation exactly. Materialize those words in the proof so a
    // following filtered resize sees the same intermediate as GPU dispatch.
    let Some((next_bytes, next_dimensions)) =
        gpu_f_resize_relocate_words(bytes, *source_dimensions, op)
    else {
        // Arithmetic, mode transitions, and geometry that changes a sample
        // value still need their own typed contract.
        return false;
    };
    *bytes = next_bytes;
    *source_dimensions = next_dimensions;
    true
}

fn gpu_f_resize_f64_resize_is_exact(
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bytes: &mut Vec<u8>,
    source_dimensions: &mut (u32, u32),
    changed_any: &mut bool,
) -> bool {
    if width == 0
        || height == 0
        || CheckedDims::new(width, height, 1)
            .ok()
            .is_none_or(|dims| dims.total_pixels() > GPU_BUFFER_CAPACITY as usize)
    {
        return false;
    }
    if matches!(filter, ResampleFilter::Nearest) {
        return gpu_f_resize_f64_nearest_is_exact(width, height, bytes, source_dimensions);
    }
    gpu_f_resize_f64_filtered_is_exact(width, height, filter, bytes, source_dimensions, changed_any)
}

fn gpu_f_resize_f64_nearest_is_exact(
    width: u32,
    height: u32,
    bytes: &mut Vec<u8>,
    source_dimensions: &mut (u32, u32),
) -> bool {
    let horizontal_changed = width != source_dimensions.0;
    let vertical_changed = height != source_dimensions.1;
    if horizontal_changed {
        let coefficients =
            gpu_resize_coefficients(width, source_dimensions.0, ResampleFilter::Nearest);
        let Some(words) =
            gpu_f_resize_nearest_pass_words(bytes, *source_dimensions, &coefficients, true)
        else {
            return false;
        };
        let Some(next_bytes) = gpu_f_resize_words_to_bytes(words) else {
            return false;
        };
        *bytes = next_bytes;
        source_dimensions.0 = width;
    }
    if vertical_changed {
        let coefficients =
            gpu_resize_coefficients(height, source_dimensions.1, ResampleFilter::Nearest);
        let Some(words) =
            gpu_f_resize_nearest_pass_words(bytes, *source_dimensions, &coefficients, false)
        else {
            return false;
        };
        let Some(next_bytes) = gpu_f_resize_words_to_bytes(words) else {
            return false;
        };
        *bytes = next_bytes;
        *source_dimensions = (width, height);
    }
    true
}

fn gpu_f_resize_f64_filtered_is_exact(
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bytes: &mut Vec<u8>,
    source_dimensions: &mut (u32, u32),
    changed_any: &mut bool,
) -> bool {
    let (kernel, support) = filter_from_resample(filter);
    let horizontal = precompute_coeffs_f64(width, source_dimensions.0, kernel, support);
    let vertical = precompute_coeffs_f64(height, source_dimensions.1, kernel, support);
    if !gpu_f_resize_f64_coefficients_are_supported(&horizontal)
        || !gpu_f_resize_f64_coefficients_are_supported(&vertical)
    {
        return false;
    }

    let horizontal_changed = width != source_dimensions.0;
    let vertical_changed = height != source_dimensions.1;
    *changed_any |= horizontal_changed || vertical_changed;
    if horizontal_changed
        && !gpu_f_resize_f64_materialize_pass(bytes, source_dimensions, &horizontal, true, width)
    {
        return false;
    }
    if vertical_changed
        && !gpu_f_resize_f64_materialize_pass(bytes, source_dimensions, &vertical, false, height)
    {
        return false;
    }
    true
}

fn gpu_f_resize_f64_coefficients_are_supported(coeffs: &FilterCoeffsF64) -> bool {
    coeffs.xmin.len() == coeffs.count.len()
        && coeffs.xmin.len() == coeffs.weights.len()
        && gpu_f_resize_f64_coefficients_fit_binding(coeffs)
        // Marker 9's exact-real reducer is not a proof of Pillow's arm64
        // wide-row contract. Finite rows above 32 taps use marker 12's
        // ordered reducer; special-value rows can use marker 9's prepass.
        && !coeffs.weights.iter().any(|row| {
            row.iter()
                .any(|&weight| gpu_f64_integer_parts(weight).is_none())
        })
}

fn gpu_f_resize_f64_materialize_pass(
    bytes: &mut Vec<u8>,
    source_dimensions: &mut (u32, u32),
    coeffs: &FilterCoeffsF64,
    horizontal: bool,
    output_axis: u32,
) -> bool {
    let Some(words) = gpu_f_resize_f64_pass_bits(bytes, *source_dimensions, coeffs, horizontal)
    else {
        return false;
    };
    let Some(next_bytes) = gpu_f_resize_words_to_bytes(words) else {
        return false;
    };
    *bytes = next_bytes;
    if horizontal {
        source_dimensions.0 = output_axis;
    } else {
        source_dimensions.1 = output_axis;
    }
    true
}

fn gpu_f_resize_words_to_bytes(words: Vec<u32>) -> Option<Vec<u8>> {
    let byte_count = words.len().checked_mul(4)?;
    let mut result = Vec::new();
    result.try_reserve(byte_count).ok()?;
    for word in words {
        result.extend_from_slice(&word.to_le_bytes());
    }
    Some(result)
}
