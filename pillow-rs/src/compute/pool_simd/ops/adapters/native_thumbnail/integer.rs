//! Native scalar-I SIMD thumbnail reduction and streaming resample kernels.

use super::super::resize::{build_resize_i_horizontal_plan, resize_i_horizontal_values};
use super::super::{SIMD_RESIZE_LANES, simd_unsupported};
use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::geometry::resample_kernel;
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::ops::pil_resize::i32_samples_from_le_bytes;
use crate::ops::pil_resize::{precompute_coeffs_f64, round_up};
use crate::pipeline::ResampleFilter;
use crate::raster::DynamicImage;
use wide::{f64x8, i32x8};

pub(super) fn simd_thumbnail_reduce_i(
    img: &DynamicImage,
    factor_x: u32,
    factor_y: u32,
) -> Result<(DynamicImage, u64, u64), PilError> {
    let source_width = usize::try_from(img.width()).map_err(|_| simd_unsupported("Thumbnail"))?;
    let source_height = usize::try_from(img.height()).map_err(|_| simd_unsupported("Thumbnail"))?;
    let factor_x = usize::try_from(factor_x).map_err(|_| simd_unsupported("Thumbnail"))?;
    let factor_y = usize::try_from(factor_y).map_err(|_| simd_unsupported("Thumbnail"))?;
    let source_pixels = source_width
        .checked_mul(source_height)
        .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    let source_len = source_pixels
        .checked_mul(4)
        .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    if !matches!(img, DynamicImage::ImageRgba8(_)) || img.as_bytes().len() != source_len {
        return Err(simd_unsupported("Thumbnail"));
    }
    let output_width = source_width.div_ceil(factor_x.max(1));
    let output_height = source_height.div_ceil(factor_y.max(1));
    let output_pixels = output_width
        .checked_mul(output_height)
        .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    let source = i32_samples_from_le_bytes(img.as_bytes(), source_pixels);
    let mut output = Vec::with_capacity(
        output_pixels
            .checked_mul(4)
            .ok_or_else(|| simd_unsupported("Thumbnail"))?,
    );
    let mut vector_blocks = 0u64;
    for start in (0..output_pixels).step_by(SIMD_RESIZE_LANES) {
        let count = (output_pixels - start).min(SIMD_RESIZE_LANES);
        let mut values = [0i32; SIMD_RESIZE_LANES];
        for lane in 0..count {
            let index = start + lane;
            let output_y = index / output_width;
            let output_x = index % output_width;
            let source_x = output_x * factor_x;
            let source_y = output_y * factor_y;
            let block_width = factor_x.min(source_width - source_x);
            let block_height = factor_y.min(source_height - source_y);
            let main_width = source_width / factor_x;
            let main_height = source_height / factor_y;
            let mut sum = 0.0f64;
            let value = |offset_x: usize, offset_y: usize| {
                source[(source_y + offset_y) * source_width + source_x + offset_x]
            };

            // ImagingReduceNxN_32bpc keeps interior pair/quartet additions
            // in INT32 before promoting each group to the double accumulator.
            // Partial right/bottom blocks use the scalar corner helper, which
            // promotes each source sample directly.  A flat i64 sum changes
            // the result when the source words overflow signed 32-bit bounds.
            if output_x < main_width && output_y < main_height {
                let mut dy = 0;
                while dy + 1 < block_height {
                    let mut dx = 0;
                    while dx + 1 < block_width {
                        let quartet = value(dx, dy)
                            .wrapping_add(value(dx + 1, dy))
                            .wrapping_add(value(dx, dy + 1))
                            .wrapping_add(value(dx + 1, dy + 1));
                        sum += f64::from(quartet);
                        dx += 2;
                    }
                    if dx < block_width {
                        let pair = value(dx, dy).wrapping_add(value(dx, dy + 1));
                        sum += f64::from(pair);
                    }
                    dy += 2;
                }
                if dy < block_height {
                    let mut dx = 0;
                    while dx + 1 < block_width {
                        let pair = value(dx, dy).wrapping_add(value(dx + 1, dy));
                        sum += f64::from(pair);
                        dx += 2;
                    }
                    if dx < block_width {
                        sum += f64::from(value(dx, dy));
                    }
                }
            } else {
                for dy in 0..block_height {
                    for dx in 0..block_width {
                        sum += f64::from(value(dx, dy));
                    }
                }
            }
            values[lane] = round_up(sum / (block_width * block_height) as f64) as i32;
        }
        let packed = i32x8::new(values).to_array();
        for value in packed.into_iter().take(count) {
            output.extend_from_slice(&value.to_le_bytes());
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let result = crate::image_utils::raw_bytes_to_image(
        output_width as u32,
        output_height as u32,
        output,
        4,
    )?;
    Ok((result, vector_blocks, 0))
}

/// Stream an aligned native-I 2x2 thumbnail reduction into the SIMD resize
/// ring. The reduced samples and horizontal intermediate retain Pillow's
/// INT32 rounding boundaries; only the full-frame intermediates are omitted.
pub(super) fn simd_thumbnail_i_reduce2x2_resample(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    filter: ResampleFilter,
) -> Result<DynamicImage, PilError> {
    let source_width = usize::try_from(img.width()).map_err(|_| simd_unsupported("Thumbnail"))?;
    let source_height = usize::try_from(img.height()).map_err(|_| simd_unsupported("Thumbnail"))?;
    let output_width = usize::try_from(output_width).map_err(|_| simd_unsupported("Thumbnail"))?;
    let output_height =
        usize::try_from(output_height).map_err(|_| simd_unsupported("Thumbnail"))?;
    if source_width % 2 != 0 || source_height % 2 != 0 || output_width == 0 || output_height == 0 {
        return Err(simd_unsupported("Thumbnail"));
    }
    let source_pixels = source_width
        .checked_mul(source_height)
        .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    let source_len = source_pixels
        .checked_mul(4)
        .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    if !matches!(img, DynamicImage::ImageRgba8(_)) || img.as_bytes().len() != source_len {
        return Err(simd_unsupported("Thumbnail"));
    }

    let reduced_width = source_width / 2;
    let reduced_height = source_height / 2;
    let source = i32_samples_from_le_bytes(img.as_bytes(), source_pixels);
    let (kernel, support) = resample_kernel(&filter);
    let horizontal =
        precompute_coeffs_f64(output_width as u32, reduced_width as u32, kernel, support);
    let vertical =
        precompute_coeffs_f64(output_height as u32, reduced_height as u32, kernel, support);
    let horizontal_plan = build_resize_i_horizontal_plan(&horizontal, reduced_width, output_width)
        .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    let ring_rows = vertical
        .weights
        .iter()
        .map(Vec::len)
        .max()
        .unwrap_or(0)
        .checked_next_power_of_two()
        .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    let ring_len = ring_rows
        .checked_mul(output_width)
        .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    let output_dims = CheckedDims::new(output_width as u32, output_height as u32, 4)?;
    let output_stride = output_dims.row_stride();
    let mut output_bytes = output_dims.alloc_buffer();
    let mut ring = vec![0_i32; ring_len];
    let mut reduced_row = vec![0_i32; reduced_width];
    let ring_mask = ring_rows - 1;
    let mut next_reduced_row = 0usize;
    let mut vector_blocks = 0u64;

    for (output_y, output_row) in output_bytes.chunks_mut(output_stride).enumerate() {
        let first_vertical_row =
            usize::try_from(vertical.xmin[output_y]).map_err(|_| simd_unsupported("Thumbnail"))?;
        let vertical_weights = &vertical.weights[output_y];
        let source_end = first_vertical_row
            .checked_add(vertical_weights.len())
            .ok_or_else(|| simd_unsupported("Thumbnail"))?;
        if source_end > reduced_height {
            return Err(simd_unsupported("Thumbnail"));
        }

        while next_reduced_row < source_end {
            let top_start = next_reduced_row
                .checked_mul(2)
                .and_then(|row| row.checked_mul(source_width))
                .ok_or_else(|| simd_unsupported("Thumbnail"))?;
            let bottom_start = top_start
                .checked_add(source_width)
                .ok_or_else(|| simd_unsupported("Thumbnail"))?;
            let top = &source[top_start..top_start + source_width];
            let bottom = &source[bottom_start..bottom_start + source_width];
            for (output_x, reduced) in reduced_row.iter_mut().enumerate() {
                let source_x = output_x * 2;
                let quartet = top[source_x]
                    .wrapping_add(top[source_x + 1])
                    .wrapping_add(bottom[source_x])
                    .wrapping_add(bottom[source_x + 1]);
                *reduced = round_up(f64::from(quartet) / 4.0) as i32;
            }

            let ring_start = (next_reduced_row & ring_mask) * output_width;
            let horizontal_row = &mut ring[ring_start..ring_start + output_width];
            for (block_index, block) in horizontal_plan.blocks.iter().enumerate() {
                let output_x = block_index * SIMD_RESIZE_LANES;
                let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
                let mut sums = f64x8::splat(0.0);
                for tap in &block.taps {
                    sums = tap.weights.mul_add(
                        f64x8::new(resize_i_horizontal_values(&reduced_row, tap)),
                        sums,
                    );
                }
                for (lane, value) in sums.to_array().into_iter().take(count).enumerate() {
                    horizontal_row[output_x + lane] = round_up(value) as i32;
                }
                vector_blocks = vector_blocks.saturating_add(1);
            }
            next_reduced_row += 1;
        }

        for output_x in (0..output_width).step_by(SIMD_RESIZE_LANES) {
            let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
            let mut sums = f64x8::splat(0.0);
            for (tap, &weight) in vertical_weights.iter().enumerate() {
                let source_y = first_vertical_row + tap;
                let source_start = (source_y & ring_mask) * output_width + output_x;
                let values = std::array::from_fn(|lane| {
                    if lane < count {
                        f64::from(ring[source_start + lane])
                    } else {
                        0.0
                    }
                });
                sums = f64x8::splat(weight).mul_add(f64x8::new(values), sums);
            }
            for (lane, value) in sums
                .to_array()
                .map(|value| round_up(value) as i32)
                .into_iter()
                .take(count)
                .enumerate()
            {
                let byte_start = (output_x + lane) * 4;
                output_row[byte_start..byte_start + 4].copy_from_slice(&value.to_le_bytes());
            }
            vector_blocks = vector_blocks.saturating_add(1);
        }
    }

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    let result = crate::image_utils::raw_bytes_to_image(
        output_width as u32,
        output_height as u32,
        output_bytes,
        4,
    )?;
    Ok(preserve_mode(img, result))
}
