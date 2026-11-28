use super::super::resize::FilterResolver;
use super::{
    SIMD_F64_LANES, SIMD_RESIZE_LANES, f32_samples_from_le_bytes, f32x8, f64x8,
    invalid_vector_request,
};
use crate::compute::pool_cpu::ops::resize::helpers::native_nearest;
use crate::error::PilError;
use crate::ops::pil_resize::precompute_coeffs_f64_boxed_with_kernel;
use crate::pipeline::ResampleFilter;
use crate::raster::RgbaImage;

/// Run boxed vector resize with an architecture-selected coefficient kernel.
// VERIFY: source=reviewed@2026-11-28; parity=pass@f32-vector-closure-20261128; benchmark=pending
pub(super) fn execute_with_filter<const PRODUCT_BLOCK_WIDTH: usize>(
    image: &RgbaImage,
    output_width: u32,
    output_height: u32,
    bounds: (f64, f64, f64, f64),
    filter: ResampleFilter,
    resolve_filter: FilterResolver,
) -> Result<RgbaImage, PilError> {
    let source_width = usize::try_from(image.width()).map_err(|_| invalid_vector_request())?;
    let source_height = usize::try_from(image.height()).map_err(|_| invalid_vector_request())?;
    let output_width = usize::try_from(output_width).map_err(|_| invalid_vector_request())?;
    let output_height = usize::try_from(output_height).map_err(|_| invalid_vector_request())?;
    // Pillow narrows the boxed geometry to float32 before Resample.c decides
    // whether a pass is needed; an unchanged axis must not expose discarded NaN/Inf taps.
    let bounds = (
        bounds.0 as f32 as f64,
        bounds.1 as f32 as f64,
        bounds.2 as f32 as f64,
        bounds.3 as f32 as f64,
    );
    let (left, top, right, bottom) = bounds;
    let need_horizontal =
        output_width != source_width || left != 0.0 || right != output_width as f64;
    let need_vertical =
        output_height != source_height || top != 0.0 || bottom != output_height as f64;
    let pixel_count = source_width
        .checked_mul(source_height)
        .ok_or_else(invalid_vector_request)?;
    let output_count = output_width
        .checked_mul(output_height)
        .ok_or_else(invalid_vector_request)?;
    let source_len = pixel_count
        .checked_mul(4)
        .ok_or_else(invalid_vector_request)?;
    if image.as_raw().len() != source_len {
        return Err(PilError::InternalError(
            "ImageF32 boxed vector Resize source buffer shape mismatch".into(),
        ));
    }
    let samples = f32_samples_from_le_bytes(image.as_raw(), pixel_count);
    let (output_floats, vector_blocks) = if matches!(filter, ResampleFilter::Nearest) {
        boxed_f_resize_nearest(
            &samples,
            source_width,
            source_height,
            output_width,
            output_height,
            output_count,
            bounds,
        )?
    } else {
        boxed_f_resize_filter::<PRODUCT_BLOCK_WIDTH>(
            &samples,
            source_width,
            source_height,
            output_width,
            output_height,
            bounds,
            filter,
            need_horizontal,
            need_vertical,
            resolve_filter,
        )?
    };
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    let output: Vec<u8> = output_floats
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect();
    RgbaImage::from_raw(output_width as u32, output_height as u32, output).ok_or_else(|| {
        PilError::ValueError("ImageF32 boxed vector Resize output shape mismatch".into())
    })
}

// VERIFY: source=reviewed@2026-11-28; parity=pass@f32-vector-closure-20261128; benchmark=pending
fn boxed_f_resize_nearest(
    source: &[f32],
    source_width: usize,
    source_height: usize,
    output_width: usize,
    output_height: usize,
    output_count: usize,
    bounds: (f64, f64, f64, f64),
) -> Result<(Vec<f32>, u64), PilError> {
    let (left, top, right, bottom) = bounds;
    let x_indices = native_nearest::boxed_nearest_indices(
        source_width as u32,
        output_width as u32,
        left,
        right,
    )
    .ok_or_else(|| invalid_vector_request())?;
    let y_indices = native_nearest::boxed_nearest_indices(
        source_height as u32,
        output_height as u32,
        top,
        bottom,
    )
    .ok_or_else(|| invalid_vector_request())?;
    let mut output = vec![0.0f32; output_count];
    let mut vector_blocks = 0u64;
    for (output_y, &source_y) in y_indices.iter().enumerate() {
        let output_row = output_y * output_width;
        for output_x in (0..output_width).step_by(SIMD_F64_LANES) {
            let count = (output_width - output_x).min(SIMD_F64_LANES);
            let values = std::array::from_fn(|lane| {
                if lane < count {
                    source[source_y * source_width + x_indices[output_x + lane]]
                } else {
                    0.0
                }
            });
            output[output_row + output_x..output_row + output_x + count]
                .copy_from_slice(&f32x8::new(values).to_array()[..count]);
            vector_blocks = vector_blocks.saturating_add(1);
        }
    }
    Ok((output, vector_blocks))
}

// VERIFY: source=reviewed@2026-11-28; parity=pass@f32-vector-closure-20261128; benchmark=pending
fn boxed_f_resize_filter<const PRODUCT_BLOCK_WIDTH: usize>(
    source: &[f32],
    source_width: usize,
    source_height: usize,
    output_width: usize,
    output_height: usize,
    bounds: (f64, f64, f64, f64),
    filter: ResampleFilter,
    need_horizontal: bool,
    need_vertical: bool,
    resolve_filter: FilterResolver,
) -> Result<(Vec<f32>, u64), PilError> {
    let (left, top, right, bottom) = bounds;
    let (kernel, support) = resolve_filter(filter);
    let (intermediate, horizontal_blocks) = if need_horizontal {
        let coeffs = precompute_coeffs_f64_boxed_with_kernel(
            output_width as u32,
            source_width as u32,
            left,
            right,
            kernel,
            support,
        );
        boxed_f_resize_horizontal::<PRODUCT_BLOCK_WIDTH>(
            source,
            source_width,
            source_height,
            output_width,
            &coeffs,
        )?
    } else {
        (source.to_vec(), 0)
    };
    if need_vertical {
        let coeffs = precompute_coeffs_f64_boxed_with_kernel(
            output_height as u32,
            source_height as u32,
            top,
            bottom,
            kernel,
            support,
        );
        let (output, vertical_blocks) =
            boxed_f_resize_vertical(&intermediate, output_width, output_height, &coeffs)?;
        Ok((output, horizontal_blocks.saturating_add(vertical_blocks)))
    } else {
        Ok((intermediate, horizontal_blocks))
    }
}

// VERIFY: source=reviewed@2026-11-28; parity=pass@f32-vector-closure-20261128; benchmark=pending
fn boxed_f_resize_horizontal<const PRODUCT_BLOCK_WIDTH: usize>(
    source: &[f32],
    source_width: usize,
    source_height: usize,
    output_width: usize,
    coeffs: &crate::ops::pil_resize::FilterCoeffsF64,
) -> Result<(Vec<f32>, u64), PilError> {
    let mut intermediate = vec![0.0f32; source_height * output_width];
    let mut vector_blocks = 0u64;
    for source_y in 0..source_height {
        let source_row = source_y * source_width;
        let intermediate_row = source_y * output_width;
        for output_x in (0..output_width).step_by(SIMD_F64_LANES) {
            let count = (output_width - output_x).min(SIMD_F64_LANES);
            let values = boxed_f_resize_horizontal_block::<PRODUCT_BLOCK_WIDTH>(
                source, source_row, output_x, count, coeffs,
            );
            intermediate[intermediate_row + output_x..intermediate_row + output_x + count]
                .copy_from_slice(&values[..count]);
            vector_blocks = vector_blocks.saturating_add(1);
        }
    }
    Ok((intermediate, vector_blocks))
}

// VERIFY: source=reviewed@2026-11-28; parity=pass@f32-vector-closure-20261128; benchmark=pending
fn boxed_f_resize_horizontal_block<const PRODUCT_BLOCK_WIDTH: usize>(
    source: &[f32],
    source_row: usize,
    output_x: usize,
    count: usize,
    coeffs: &crate::ops::pil_resize::FilterCoeffsF64,
) -> [f32; SIMD_F64_LANES] {
    let mut sums = f64x8::splat(0.0);
    let first_weights = &coeffs.weights[output_x];
    let full_eight_taps = count == SIMD_F64_LANES
        && (0..count).all(|lane| coeffs.weights[output_x + lane].len() == 8);
    let shared_weights = full_eight_taps
        && (1..count).all(|lane| coeffs.weights[output_x + lane].as_slice() == first_weights);
    if shared_weights {
        // Broadcast only after proving all eight coefficient lanes match.
        for tap in 0..8 {
            let values = std::array::from_fn(|lane| {
                let output_index = output_x + lane;
                let source_x = coeffs.xmin[output_index] as usize + tap;
                f64::from(source[source_row + source_x])
            });
            sums = f64x8::splat(first_weights[tap]).mul_add(f64x8::new(values), sums);
        }
    } else if full_eight_taps {
        for tap in 0..8 {
            let values = std::array::from_fn(|lane| {
                let output_index = output_x + lane;
                let source_x = coeffs.xmin[output_index] as usize + tap;
                f64::from(source[source_row + source_x])
            });
            let weights = std::array::from_fn(|lane| coeffs.weights[output_x + lane][tap]);
            sums = f64x8::new(weights).mul_add(f64x8::new(values), sums);
        }
    } else {
        let max_count = (0..count)
            .map(|lane| coeffs.weights[output_x + lane].len())
            .max()
            .unwrap_or(0);
        // Pillow's fixed eight-tap helper remains fused; other horizontal
        // rows separate full architecture-sized product groups from the tail.
        let product_counts: [usize; SIMD_F64_LANES] = std::array::from_fn(|lane| {
            if lane >= count || coeffs.weights[output_x + lane].len() == 8 {
                return 0;
            }
            let taps = coeffs.weights[output_x + lane].len();
            taps / PRODUCT_BLOCK_WIDTH * PRODUCT_BLOCK_WIDTH
        });
        let max_product_count = product_counts[..count].iter().copied().max().unwrap_or(0);
        let mut ordered_sums = [0.0; SIMD_F64_LANES];
        for tap in 0..max_count {
            let mut values = [0.0; SIMD_F64_LANES];
            let mut weights = [0.0; SIMD_F64_LANES];
            for lane in 0..count {
                let output_index = output_x + lane;
                if tap < coeffs.weights[output_index].len() {
                    let source_x = coeffs.xmin[output_index] as usize + tap;
                    values[lane] = f64::from(source[source_row + source_x]);
                    weights[lane] = coeffs.weights[output_index][tap];
                }
            }
            let weight_vector = f64x8::new(weights);
            let value_vector = f64x8::new(values);
            if max_product_count == 0 {
                sums = weight_vector.mul_add(value_vector, sums);
            } else {
                let fused = weight_vector
                    .mul_add(value_vector, f64x8::new(ordered_sums))
                    .to_array();
                let products = if tap < max_product_count {
                    std::hint::black_box(weight_vector * value_vector).to_array()
                } else {
                    [0.0; SIMD_F64_LANES]
                };
                for lane in 0..count {
                    ordered_sums[lane] = if tap < product_counts[lane] {
                        ordered_sums[lane] + products[lane]
                    } else {
                        fused[lane]
                    };
                }
            }
        }
        if max_product_count != 0 {
            return ordered_sums.map(|value| value as f32);
        }
    }
    sums.to_array().map(|value| value as f32)
}

// VERIFY: source=reviewed@2026-11-28; parity=pass@f32-vector-closure-20261128; benchmark=pending
fn boxed_f_resize_vertical(
    intermediate: &[f32],
    output_width: usize,
    output_height: usize,
    coeffs: &crate::ops::pil_resize::FilterCoeffsF64,
) -> Result<(Vec<f32>, u64), PilError> {
    let mut output = vec![0.0f32; output_width * output_height];
    let mut vector_blocks = 0u64;
    for output_y in 0..output_height {
        let first_source_row = coeffs.xmin[output_y] as usize;
        let weights = &coeffs.weights[output_y];
        let output_row = output_y * output_width;
        for output_x in (0..output_width).step_by(SIMD_F64_LANES) {
            let count = (output_width - output_x).min(SIMD_F64_LANES);
            let values = boxed_f_resize_vertical_block(
                intermediate,
                output_width,
                output_x,
                count,
                first_source_row,
                weights,
            );
            output[output_row + output_x..output_row + output_x + count]
                .copy_from_slice(&values[..count]);
            vector_blocks = vector_blocks.saturating_add(1);
        }
    }
    Ok((output, vector_blocks))
}

// VERIFY: source=reviewed@2026-11-28; parity=pass@f32-vector-closure-20261128; benchmark=pending
fn boxed_f_resize_vertical_block(
    intermediate: &[f32],
    output_width: usize,
    output_x: usize,
    count: usize,
    first_source_row: usize,
    weights: &[f64],
) -> [f32; SIMD_F64_LANES] {
    let mut sums = f64x8::splat(0.0);
    if weights.len() == 8 {
        for (tap, &weight) in weights.iter().enumerate() {
            let source_row = (first_source_row + tap) * output_width;
            let values = std::array::from_fn(|lane| {
                if lane < count {
                    f64::from(intermediate[source_row + output_x + lane])
                } else {
                    0.0
                }
            });
            sums = f64x8::splat(weight).mul_add(f64x8::new(values), sums);
        }
    } else {
        for (tap, &weight) in weights.iter().enumerate() {
            let source_row = (first_source_row + tap) * output_width;
            let values = if count == SIMD_RESIZE_LANES {
                std::array::from_fn(|lane| f64::from(intermediate[source_row + output_x + lane]))
            } else {
                std::array::from_fn(|lane| {
                    if lane < count {
                        f64::from(intermediate[source_row + output_x + lane])
                    } else {
                        0.0
                    }
                })
            };
            sums = f64x8::splat(weight).mul_add(f64x8::new(values), sums);
        }
    }
    sums.to_array().map(|value| value as f32)
}

#[inline]
// VERIFY: source=reviewed@2026-11-28; parity=pass@f32-vector-closure-20261128; benchmark=pending
pub(super) fn write_f32_samples_le(output: &mut [u8], sample_offset: usize, samples: &[f32]) {
    let byte_start = sample_offset * std::mem::size_of::<f32>();
    let byte_end = byte_start + std::mem::size_of_val(samples);
    #[cfg(target_endian = "little")]
    output[byte_start..byte_end].copy_from_slice(bytemuck::cast_slice(samples));
    #[cfg(target_endian = "big")]
    for (bytes, sample) in output[byte_start..byte_end]
        .chunks_exact_mut(std::mem::size_of::<f32>())
        .zip(samples)
    {
        bytes.copy_from_slice(&sample.to_le_bytes());
    }
}
