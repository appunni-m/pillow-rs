//! Native float-mode SIMD Resize kernels for direct and boxed geometry.

use super::super::{SIMD_F64_LANES, SIMD_RESIZE_LANES, boxed_nearest_indices, simd_unsupported};
use super::resize_nearest_indices;
use crate::compute::pool_cpu::ops::geometry::resample_kernel;
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::ops::pil_resize::{
    f32_samples_from_le_bytes, precompute_coeffs_f64, precompute_coeffs_f64_boxed,
};
use crate::pipeline::ResampleFilter;
use crate::raster::DynamicImage;
use wide::{f32x8, f64x8};

/// Execute boxed resampling for an F-mode image. The source and intermediate
/// values are f32 samples, not four independent bytes. Coordinate/index
/// construction and sample accumulation use SIMD lanes; scalar work is
/// limited to portable gathers and IEEE-754 byte serialization.
pub(in crate::compute::pool_simd::ops::adapters) fn simd_resize_f_boxed(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    filter: ResampleFilter,
) -> Result<DynamicImage, PilError> {
    let source_width = usize::try_from(img.width()).map_err(|_| simd_unsupported("Fit"))?;
    let source_height = usize::try_from(img.height()).map_err(|_| simd_unsupported("Fit"))?;
    let output_width = usize::try_from(output_width).map_err(|_| simd_unsupported("Fit"))?;
    let output_height = usize::try_from(output_height).map_err(|_| simd_unsupported("Fit"))?;
    // Pillow narrows the boxed geometry to float32 before Resample.c decides
    // whether either pass is required. Keep the same boundary and pass
    // elision as the scalar F implementation; filtering an unchanged axis
    // can make discarded NaN/Inf taps observable.
    let box_left = box_left as f32 as f64;
    let box_top = box_top as f32 as f64;
    let box_right = box_right as f32 as f64;
    let box_bottom = box_bottom as f32 as f64;
    let need_horizontal =
        output_width != source_width || box_left != 0.0 || box_right != output_width as f64;
    let need_vertical =
        output_height != source_height || box_top != 0.0 || box_bottom != output_height as f64;
    let pixel_count = source_width
        .checked_mul(source_height)
        .ok_or_else(|| simd_unsupported("Fit"))?;
    let output_count = output_width
        .checked_mul(output_height)
        .ok_or_else(|| simd_unsupported("Fit"))?;
    let source_len = pixel_count
        .checked_mul(4)
        .ok_or_else(|| simd_unsupported("Fit"))?;
    if img.as_bytes().len() != source_len {
        return Err(PilError::InternalError(
            "SIMD Fit F source buffer shape mismatch".into(),
        ));
    }
    let mut vector_blocks = 0u64;
    let source_samples = f32_samples_from_le_bytes(img.as_bytes(), pixel_count);
    let source: &[f32] = source_samples.as_ref();

    let output_floats = if source_width == 0 || source_height == 0 {
        let mut output_floats = vec![0.0f32; output_count];
        let zero = f32x8::splat(0.0).to_array();
        for block in output_floats.chunks_exact_mut(SIMD_F64_LANES) {
            block.copy_from_slice(&zero);
            vector_blocks = vector_blocks.saturating_add(1);
        }
        let remainder = output_floats.len() % SIMD_F64_LANES;
        if remainder != 0 {
            let start = output_floats.len() - remainder;
            output_floats[start..].copy_from_slice(&zero[..remainder]);
        }
        output_floats
    } else if matches!(filter, ResampleFilter::Nearest) {
        let mut output_floats = vec![0.0f32; output_count];
        let x_indices = boxed_nearest_indices(
            source_width as u32,
            output_width as u32,
            box_left,
            box_right,
        )
        .ok_or_else(|| simd_unsupported("Fit"))?;
        let y_indices = boxed_nearest_indices(
            source_height as u32,
            output_height as u32,
            box_top,
            box_bottom,
        )
        .ok_or_else(|| simd_unsupported("Fit"))?;
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
                output_floats[output_row + output_x..output_row + output_x + count]
                    .copy_from_slice(&f32x8::new(values).to_array()[..count]);
                vector_blocks = vector_blocks.saturating_add(1);
            }
        }
        output_floats
    } else {
        let intermediate = if need_horizontal {
            let horizontal = precompute_coeffs_f64_boxed(
                output_width as u32,
                source_width as u32,
                box_left,
                box_right,
                filter,
            );
            let mut intermediate = vec![0.0f32; source_height * output_width];
            for source_y in 0..source_height {
                let source_row = source_y * source_width;
                let intermediate_row = source_y * output_width;
                for output_x in (0..output_width).step_by(SIMD_F64_LANES) {
                    let count = (output_width - output_x).min(SIMD_F64_LANES);
                    let mut sums = f64x8::splat(0.0);
                    let first_weights = &horizontal.weights[output_x];
                    let full_eight_taps = count == SIMD_F64_LANES
                        && (0..count).all(|lane| horizontal.weights[output_x + lane].len() == 8);
                    let shared_weights = full_eight_taps
                        && (1..count).all(|lane| {
                            horizontal.weights[output_x + lane].as_slice() == first_weights
                        });
                    if shared_weights {
                        // Scale-two boxed bicubic rows often repeat the same
                        // eight coefficients across a complete output block.
                        // Once equality is proven, broadcast each exact
                        // coefficient instead of rebuilding a lane vector.
                        for tap in 0..8 {
                            let values = std::array::from_fn(|lane| {
                                let output_index = output_x + lane;
                                let source_x = horizontal.xmin[output_index] as usize + tap;
                                f64::from(source[source_row + source_x])
                            });
                            sums =
                                f64x8::splat(first_weights[tap]).mul_add(f64x8::new(values), sums);
                        }
                    } else if full_eight_taps {
                        for tap in 0..8 {
                            let values = std::array::from_fn(|lane| {
                                let output_index = output_x + lane;
                                let source_x = horizontal.xmin[output_index] as usize + tap;
                                f64::from(source[source_row + source_x])
                            });
                            let weights = std::array::from_fn(|lane| {
                                horizontal.weights[output_x + lane][tap]
                            });
                            sums = f64x8::new(weights).mul_add(f64x8::new(values), sums);
                        }
                    } else {
                        let max_count = (0..count)
                            .map(|lane| horizontal.weights[output_x + lane].len())
                            .max()
                            .unwrap_or(0);
                        for tap in 0..max_count {
                            let mut values = [0.0; SIMD_F64_LANES];
                            let mut weights = [0.0; SIMD_F64_LANES];
                            for lane in 0..count {
                                let output_index = output_x + lane;
                                if tap < horizontal.weights[output_index].len() {
                                    let source_x = horizontal.xmin[output_index] as usize + tap;
                                    values[lane] = f64::from(source[source_row + source_x]);
                                    weights[lane] = horizontal.weights[output_index][tap];
                                }
                            }
                            sums = f64x8::new(weights).mul_add(f64x8::new(values), sums);
                        }
                    }
                    // Pillow's 32bpc path preserves the sign of zero. Do not
                    // canonicalize a negative cancellation result to +0.0.
                    let values = sums.to_array().map(|value| value as f32);
                    intermediate[intermediate_row + output_x..intermediate_row + output_x + count]
                        .copy_from_slice(&values[..count]);
                    vector_blocks = vector_blocks.saturating_add(1);
                }
            }
            intermediate
        } else {
            source.to_vec()
        };
        if need_vertical {
            let vertical = precompute_coeffs_f64_boxed(
                output_height as u32,
                source_height as u32,
                box_top,
                box_bottom,
                filter,
            );
            let mut output_floats = vec![0.0f32; output_count];
            for output_y in 0..output_height {
                let y0 = vertical.xmin[output_y] as usize;
                let weights = &vertical.weights[output_y];
                let output_row = output_y * output_width;
                for output_x in (0..output_width).step_by(SIMD_F64_LANES) {
                    let count = (output_width - output_x).min(SIMD_F64_LANES);
                    let mut sums = f64x8::splat(0.0);
                    if weights.len() == 8 {
                        for tap in 0..8 {
                            let source_row = (y0 + tap) * output_width;
                            let values = std::array::from_fn(|lane| {
                                if lane < count {
                                    f64::from(intermediate[source_row + output_x + lane])
                                } else {
                                    0.0
                                }
                            });
                            sums = f64x8::splat(weights[tap]).mul_add(f64x8::new(values), sums);
                        }
                    } else {
                        for (tap, &weight) in weights.iter().enumerate() {
                            let source_row = (y0 + tap) * output_width;
                            let values = if count == SIMD_RESIZE_LANES {
                                std::array::from_fn(|lane| {
                                    f64::from(intermediate[source_row + output_x + lane])
                                })
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
                    // Pillow's 32bpc path preserves the sign of zero. Do not
                    // canonicalize a negative cancellation result to +0.0.
                    let values = sums.to_array().map(|value| value as f32);
                    output_floats[output_row + output_x..output_row + output_x + count]
                        .copy_from_slice(&values[..count]);
                    vector_blocks = vector_blocks.saturating_add(1);
                }
            }
            output_floats
        } else {
            intermediate
        }
    };

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    let output: Vec<u8> = output_floats
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect();
    Ok(crate::image_utils::raw_bytes_to_image(
        output_width as u32,
        output_height as u32,
        output,
        4,
    )?)
}

#[inline]
fn write_f32_samples_le(output: &mut [u8], sample_offset: usize, samples: &[f32]) {
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

/// Resize an `F` image with the ordinary integer geometry coefficients.
///
/// The boxed F kernel is used by `ImageOps.fit`; direct `Image.resize` needs
/// the unboxed coefficient builder used by Pillow's native F path. Keeping
/// the two builders distinct avoids f32 box-boundary rounding from creating
/// signed-zero/near-zero differences in otherwise zero pixels.
pub(in crate::compute::pool_simd::ops::adapters) fn simd_resize_f(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    filter: &ResampleFilter,
) -> Result<DynamicImage, PilError> {
    // Pillow's wide FLOAT32 horizontal reducer uses complete 16-tap product
    // blocks before returning to scalar FMA for the tail.
    const F_RESIZE_VECTOR_WIDTH: usize = 16;
    // Pillow Image.resize reduces images taller than 100 times their width
    // vertically first. The intermediate F storage rounds to f32, so reversing
    // these passes changes bytes even when both reductions use ordered f64 FMA.
    if !matches!(filter, ResampleFilter::Nearest)
        && output_width != 0
        && output_height != 0
        && output_width != img.width()
        && u64::from(img.height()) > u64::from(img.width()) * 100
        && output_height < img.height()
    {
        let intermediate = simd_resize_f(img, img.width(), output_height, filter)?;
        return simd_resize_f(&intermediate, output_width, output_height, filter);
    }
    let source_width = usize::try_from(img.width()).map_err(|_| simd_unsupported("Resize"))?;
    let source_height = usize::try_from(img.height()).map_err(|_| simd_unsupported("Resize"))?;
    let output_width = usize::try_from(output_width).map_err(|_| simd_unsupported("Resize"))?;
    let output_height = usize::try_from(output_height).map_err(|_| simd_unsupported("Resize"))?;
    let source_pixels = source_width
        .checked_mul(source_height)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let source_len = source_pixels
        .checked_mul(4)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    if img.as_bytes().len() != source_len {
        return Err(PilError::InternalError(
            "SIMD F resize source buffer shape mismatch".into(),
        ));
    }
    let output_count = output_width
        .checked_mul(output_height)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let source = f32_samples_from_le_bytes(img.as_bytes(), source_width * source_height);
    let output_byte_count = output_count
        .checked_mul(std::mem::size_of::<f32>())
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let mut output = vec![0u8; output_byte_count];
    let mut vector_blocks = 0u64;

    if source_width == 0 || source_height == 0 {
        let zero = f32x8::splat(0.0).to_array();
        for block in output.chunks_exact_mut(SIMD_RESIZE_LANES * 4) {
            block.copy_from_slice(bytemuck::cast_slice(&zero));
            vector_blocks = vector_blocks.saturating_add(1);
        }
        let remainder = output_count % SIMD_RESIZE_LANES;
        if remainder != 0 {
            let start = output_count - remainder;
            write_f32_samples_le(&mut output, start, &zero[..remainder]);
            vector_blocks = vector_blocks.saturating_add(1);
        }
    } else if matches!(filter, ResampleFilter::Nearest) {
        let x_indices = resize_nearest_indices(img.width(), output_width as u32)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let y_indices = resize_nearest_indices(img.height(), output_height as u32)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        for (output_y, &source_y) in y_indices.iter().enumerate() {
            let source_row = source_y
                .checked_mul(source_width)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let output_row = output_y
                .checked_mul(output_width)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            for output_x in (0..output_width).step_by(SIMD_RESIZE_LANES) {
                let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
                let values = std::array::from_fn(|lane| {
                    if lane < count {
                        source[source_row + x_indices[output_x + lane]]
                    } else {
                        0.0
                    }
                });
                write_f32_samples_le(
                    &mut output,
                    output_row + output_x,
                    &f32x8::new(values).to_array()[..count],
                );
                vector_blocks = vector_blocks.saturating_add(1);
            }
        }
    } else {
        let (kernel, support) = resample_kernel(filter);
        let needs_horizontal = output_width != source_width;
        let needs_vertical = output_height != source_height;
        let intermediate_len = source_height
            .checked_mul(output_width)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let mut intermediate = vec![0.0f32; intermediate_len];

        if needs_horizontal {
            let horizontal =
                precompute_coeffs_f64(output_width as u32, source_width as u32, kernel, support);
            // For the common scale-two bicubic pass, source coordinates and
            // weights are identical for every source row. Transpose those
            // coefficients into SIMD lane groups once instead of rebuilding
            // them inside the row-by-row pixel loop. Keep wider rows on the
            // generic path because Pillow switches their first 16 products
            // to separate multiply/add rounding.
            let horizontal_lane_plan = if matches!(filter, ResampleFilter::Bicubic)
                && output_width.checked_mul(2) == Some(source_width)
            {
                let mut block_ranges = Vec::with_capacity(output_width.div_ceil(SIMD_RESIZE_LANES));
                let mut lane_taps = Vec::new();
                let mut supported = true;
                for output_x in (0..output_width).step_by(SIMD_RESIZE_LANES) {
                    let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
                    let max_count = (0..count)
                        .map(|lane| horizontal.weights[output_x + lane].len())
                        .max()
                        .unwrap_or(0);
                    if max_count >= F_RESIZE_VECTOR_WIDTH {
                        supported = false;
                        break;
                    }
                    let tap_start = lane_taps.len();
                    for tap in 0..max_count {
                        let mut source_x = [0usize; SIMD_RESIZE_LANES];
                        let mut weights = [0.0; SIMD_RESIZE_LANES];
                        let mut active_lanes = 0u8;
                        for lane in 0..count {
                            let output_index = output_x + lane;
                            let lane_weights = horizontal
                                .weights
                                .get(output_index)
                                .ok_or_else(|| simd_unsupported("Resize"))?;
                            let Some(&weight) = lane_weights.get(tap) else {
                                continue;
                            };
                            let xmin = *horizontal
                                .xmin
                                .get(output_index)
                                .ok_or_else(|| simd_unsupported("Resize"))?;
                            let source_index = usize::try_from(xmin)
                                .map_err(|_| simd_unsupported("Resize"))?
                                .checked_add(tap)
                                .filter(|&index| index < source_width)
                                .ok_or_else(|| simd_unsupported("Resize"))?;
                            source_x[lane] = source_index;
                            weights[lane] = weight;
                            active_lanes |= 1u8 << lane;
                        }
                        let stride_two = active_lanes == u8::MAX
                            && (1..SIMD_RESIZE_LANES).all(|lane| {
                                source_x[0].checked_add(lane * 2) == Some(source_x[lane])
                            });
                        lane_taps.push((source_x, f64x8::new(weights), active_lanes, stride_two));
                    }
                    block_ranges.push((tap_start, max_count));
                }
                supported.then_some((block_ranges, lane_taps))
            } else {
                None
            };
            let horizontal_step = if horizontal_lane_plan.is_some() {
                SIMD_RESIZE_LANES
            } else {
                SIMD_RESIZE_LANES
            };
            for source_y in 0..source_height {
                let source_row = source_y
                    .checked_mul(source_width)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                let intermediate_row = source_y
                    .checked_mul(output_width)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                for output_x in (0..output_width).step_by(horizontal_step) {
                    let count = (output_width - output_x).min(horizontal_step);
                    if let Some((block_ranges, lane_taps)) = &horizontal_lane_plan {
                        let (tap_start, tap_count) = block_ranges[output_x / SIMD_RESIZE_LANES];
                        let mut sums = f64x8::splat(0.0);
                        for (source_x, weights, active_lanes, stride_two) in
                            &lane_taps[tap_start..tap_start + tap_count]
                        {
                            // The source row multiplication was checked above,
                            // and each planned x coordinate was clipped to
                            // `source_width` before this row loop.
                            let values = if *stride_two {
                                let source_start = source_row
                                    .checked_add(source_x[0])
                                    .ok_or_else(|| simd_unsupported("Resize"))?;
                                let source_end = source_start
                                    .checked_add(15)
                                    .ok_or_else(|| simd_unsupported("Resize"))?;
                                let source_values = source
                                    .get(source_start..source_end)
                                    .ok_or_else(|| simd_unsupported("Resize"))?;
                                [
                                    f64::from(source_values[0]),
                                    f64::from(source_values[2]),
                                    f64::from(source_values[4]),
                                    f64::from(source_values[6]),
                                    f64::from(source_values[8]),
                                    f64::from(source_values[10]),
                                    f64::from(source_values[12]),
                                    f64::from(source_values[14]),
                                ]
                            } else if *active_lanes == u8::MAX {
                                std::array::from_fn(|lane| {
                                    f64::from(source[source_row + source_x[lane]])
                                })
                            } else {
                                std::array::from_fn(|lane| {
                                    if active_lanes & (1u8 << lane) != 0 {
                                        f64::from(source[source_row + source_x[lane]])
                                    } else {
                                        0.0
                                    }
                                })
                            };
                            sums = weights.mul_add(f64x8::new(values), sums);
                        }
                        let values = sums.to_array().map(|value| value as f32);
                        intermediate
                            [intermediate_row + output_x..intermediate_row + output_x + count]
                            .copy_from_slice(&values[..count]);
                        vector_blocks = vector_blocks.saturating_add(1);
                        continue;
                    }
                    let max_count = (0..count)
                        .map(|lane| horizontal.weights[output_x + lane].len())
                        .max()
                        .unwrap_or(0);
                    // Pillow's FLOAT32 horizontal kernel switches from scalar
                    // FMA to separate product/add operations for complete
                    // 16-tap blocks.  The scalar CPU path keeps that split
                    // because contraction changes the final f32 ULP for
                    // cancellation-heavy rows.  Preserve it per lane here:
                    // edge coefficients in the same SIMD block can have a
                    // shorter tap span and must retain scalar FMA semantics.
                    let vector_product_counts: [usize; SIMD_RESIZE_LANES] =
                        std::array::from_fn(|lane| {
                            if lane < count {
                                (horizontal.weights[output_x + lane].len() / F_RESIZE_VECTOR_WIDTH)
                                    * F_RESIZE_VECTOR_WIDTH
                            } else {
                                0
                            }
                        });
                    // Most resize kernels (including bicubic downsampling by
                    // two) have fewer than Pillow's 16-tap split point. Do
                    // not materialize a second vector product for those
                    // blocks: every active lane consumes the fused result.
                    let max_vector_product_count = vector_product_counts[..count]
                        .iter()
                        .copied()
                        .max()
                        .unwrap_or(0);
                    // Keep Pillow's left-to-right f64 reduction order for each
                    // lane. The eight products are still calculated together;
                    // reducing the vector with reassociated adds would create
                    // tiny side lobes in cancellation-heavy Lanczos samples.
                    let mut vector_sums = f64x8::splat(0.0);
                    let mut ordered_sums = [0.0; SIMD_RESIZE_LANES];
                    for tap in 0..max_count {
                        let mut values = [0.0; SIMD_RESIZE_LANES];
                        let mut weights = [0.0; SIMD_RESIZE_LANES];
                        for lane in 0..count {
                            let output_index = output_x + lane;
                            let lane_weights = horizontal
                                .weights
                                .get(output_index)
                                .ok_or_else(|| simd_unsupported("Resize"))?;
                            let Some(&weight) = lane_weights.get(tap) else {
                                continue;
                            };
                            let source_x = usize::try_from(
                                *horizontal
                                    .xmin
                                    .get(output_index)
                                    .ok_or_else(|| simd_unsupported("Resize"))?,
                            )
                            .map_err(|_| simd_unsupported("Resize"))?
                            .checked_add(tap)
                            .ok_or_else(|| simd_unsupported("Resize"))?;
                            let source_index = source_row
                                .checked_add(source_x)
                                .ok_or_else(|| simd_unsupported("Resize"))?;
                            values[lane] = f64::from(
                                *source
                                    .get(source_index)
                                    .ok_or_else(|| simd_unsupported("Resize"))?,
                            );
                            weights[lane] = weight;
                        }
                        let weight_vector = f64x8::new(weights);
                        let value_vector = f64x8::new(values);
                        if max_vector_product_count == 0 {
                            // Keep the common short-kernel path in vector
                            // registers. Round-trip through `[f64; 8]` only
                            // for the rare wide rows with Pillow's split
                            // product/add rule.
                            vector_sums = weight_vector.mul_add(value_vector, vector_sums);
                        } else {
                            let fused = weight_vector
                                .mul_add(value_vector, f64x8::new(ordered_sums))
                                .to_array();
                            // Keep the product out of the add for wide-row
                            // lanes. `black_box` prevents LLVM from
                            // contracting this multiply/add back into an FMA.
                            let products = if tap < max_vector_product_count {
                                std::hint::black_box(weight_vector * value_vector).to_array()
                            } else {
                                [0.0; SIMD_RESIZE_LANES]
                            };
                            for lane in 0..count {
                                ordered_sums[lane] = if tap < vector_product_counts[lane] {
                                    ordered_sums[lane] + products[lane]
                                } else {
                                    fused[lane]
                                };
                            }
                        }
                    }
                    // Pillow's 32bpc path preserves the sign of zero. Do not
                    // canonicalize a negative cancellation result to +0.0.
                    let values = if max_vector_product_count == 0 {
                        vector_sums.to_array().map(|value| value as f32)
                    } else {
                        ordered_sums.map(|value| value as f32)
                    };
                    intermediate[intermediate_row + output_x..intermediate_row + output_x + count]
                        .copy_from_slice(&values[..count]);
                    vector_blocks = vector_blocks.saturating_add(1);
                }
            }
        } else {
            // Resample.c skips an axis whose destination size already equals
            // the source size. Copy that axis in vector-sized blocks instead
            // of applying an unnecessary convolution that changes the bytes.
            for source_y in 0..source_height {
                let source_row = source_y
                    .checked_mul(source_width)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                let intermediate_row = source_y
                    .checked_mul(output_width)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                for output_x in (0..output_width).step_by(SIMD_RESIZE_LANES) {
                    let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
                    let values = std::array::from_fn(|lane| {
                        if lane < count {
                            source[source_row + output_x + lane]
                        } else {
                            0.0
                        }
                    });
                    intermediate[intermediate_row + output_x..intermediate_row + output_x + count]
                        .copy_from_slice(&f32x8::new(values).to_array()[..count]);
                    vector_blocks = vector_blocks.saturating_add(1);
                }
            }
        }

        if needs_vertical {
            let vertical =
                precompute_coeffs_f64(output_height as u32, source_height as u32, kernel, support);
            let vertical_lane_plan = matches!(filter, ResampleFilter::Bicubic)
                && output_height.checked_mul(2) == Some(source_height);
            let mut vertical_row_taps = Vec::new();
            if vertical_lane_plan {
                vertical_row_taps.reserve(SIMD_RESIZE_LANES);
            }
            for output_y in 0..output_height {
                let y0 = usize::try_from(
                    *vertical
                        .xmin
                        .get(output_y)
                        .ok_or_else(|| simd_unsupported("Resize"))?,
                )
                .map_err(|_| simd_unsupported("Resize"))?;
                let weights = vertical
                    .weights
                    .get(output_y)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                let output_row = output_y
                    .checked_mul(output_width)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                if vertical_lane_plan {
                    vertical_row_taps.clear();
                    for (tap, &weight) in weights.iter().enumerate() {
                        let source_y = y0
                            .checked_add(tap)
                            .filter(|&row| row < source_height)
                            .ok_or_else(|| simd_unsupported("Resize"))?;
                        let source_row = source_y
                            .checked_mul(output_width)
                            .ok_or_else(|| simd_unsupported("Resize"))?;
                        vertical_row_taps.push((source_row, weight));
                    }
                }
                let vertical_step = if vertical_lane_plan {
                    SIMD_RESIZE_LANES
                } else {
                    SIMD_RESIZE_LANES
                };
                for output_x in (0..output_width).step_by(vertical_step) {
                    let count = (output_width - output_x).min(vertical_step);
                    if vertical_lane_plan {
                        let mut sums = f64x8::splat(0.0);
                        for &(source_row, weight) in &vertical_row_taps {
                            // Each planned row start is checked once per
                            // output row and the current x-lane range is
                            // bounded by the validated intermediate image.
                            let values = std::array::from_fn(|lane| {
                                if lane < count {
                                    f64::from(intermediate[source_row + output_x + lane])
                                } else {
                                    0.0
                                }
                            });
                            sums = f64x8::splat(weight).mul_add(f64x8::new(values), sums);
                        }
                        // Keep Pillow's ordered f64 FMA accumulation and
                        // f32 pass boundary while reusing each tap vector over
                        // all horizontal blocks in this output row.
                        let values = sums.to_array().map(|value| value as f32);
                        write_f32_samples_le(&mut output, output_row + output_x, &values[..count]);
                        vector_blocks = vector_blocks.saturating_add(1);
                        continue;
                    }
                    // As in the horizontal pass, preserve the scalar tap order
                    // after vector multiplication so exact Pillow bytes remain
                    // stable for symmetric kernels.
                    let mut sums = [0.0; SIMD_RESIZE_LANES];
                    for (tap, &weight) in weights.iter().enumerate() {
                        let source_y = y0
                            .checked_add(tap)
                            .ok_or_else(|| simd_unsupported("Resize"))?;
                        let source_row = source_y
                            .checked_mul(output_width)
                            .ok_or_else(|| simd_unsupported("Resize"))?;
                        let values = std::array::from_fn(|lane| {
                            if lane < count {
                                f64::from(intermediate[source_row + output_x + lane])
                            } else {
                                0.0
                            }
                        });
                        sums = f64x8::splat(weight)
                            .mul_add(f64x8::new(values), f64x8::new(sums))
                            .to_array();
                    }
                    // Pillow's 32bpc path preserves the sign of zero. Do not
                    // canonicalize a negative cancellation result to +0.0.
                    let values = sums.map(|value| value as f32);
                    write_f32_samples_le(&mut output, output_row + output_x, &values[..count]);
                    vector_blocks = vector_blocks.saturating_add(1);
                }
            }
        } else {
            write_f32_samples_le(&mut output, 0, &intermediate);
        }
    }

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    let result = crate::image_utils::raw_bytes_to_image(
        output_width as u32,
        output_height as u32,
        output,
        4,
    )?;
    Ok(preserve_mode(img, result))
}
