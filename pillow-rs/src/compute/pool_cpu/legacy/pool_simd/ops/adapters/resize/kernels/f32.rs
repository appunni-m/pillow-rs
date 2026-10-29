//! Native float-mode SIMD Resize kernels for direct and boxed geometry.

use super::super::{SIMD_F64_LANES, SIMD_RESIZE_LANES, simd_unsupported};
use super::{boxed_nearest_indices, resize_nearest_indices};
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
#[path = "f32/boxed.rs"]
mod boxed;
pub(in crate::compute::pool_simd::ops::adapters) use boxed::simd_resize_f_boxed;
pub(super) use boxed::write_f32_samples_le;

/// Resize an `F` image with the ordinary integer geometry coefficients.
///
/// The boxed F kernel is used by `ImageOps.fit`; direct `Image.resize` needs
/// the unboxed coefficient builder used by Pillow's native F path. Keeping
/// the two builders distinct avoids f32 box-boundary rounding from creating
/// signed-zero/near-zero differences in otherwise zero pixels.
const F_RESIZE_VECTOR_WIDTH: usize = 16;

type FResizeLaneTap = ([usize; SIMD_RESIZE_LANES], f64x8, u8, bool);
type FResizeHorizontalLanePlan = (Vec<(usize, usize)>, Vec<FResizeLaneTap>);

pub(in crate::compute::pool_simd::ops::adapters) fn simd_resize_f(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    filter: &ResampleFilter,
) -> Result<DynamicImage, PilError> {
    // Pillow reduces very tall images vertically first; the f32 pass boundary
    // makes the order observable even when both stages accumulate in f64.
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
    let output_bytes = output_count
        .checked_mul(std::mem::size_of::<f32>())
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let source = f32_samples_from_le_bytes(img.as_bytes(), source_pixels);
    let (output, vector_blocks) = if source_width == 0 || source_height == 0 {
        f_resize_zero_output(output_count, output_bytes)
    } else if matches!(filter, ResampleFilter::Nearest) {
        f_resize_nearest_output(
            &source,
            img,
            source_width,
            output_width,
            output_height,
            output_bytes,
        )?
    } else {
        f_resize_filtered_output(
            &source,
            source_width,
            source_height,
            output_width,
            output_height,
            filter,
            output_bytes,
        )?
    };
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

fn f_resize_zero_output(output_count: usize, output_byte_count: usize) -> (Vec<u8>, u64) {
    let mut output = vec![0u8; output_byte_count];
    let zero = f32x8::splat(0.0).to_array();
    let mut vector_blocks = 0u64;
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
    (output, vector_blocks)
}

fn f_resize_nearest_output(
    source: &[f32],
    img: &DynamicImage,
    source_width: usize,
    output_width: usize,
    output_height: usize,
    output_byte_count: usize,
) -> Result<(Vec<u8>, u64), PilError> {
    let x_indices = resize_nearest_indices(img.width(), output_width as u32)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let y_indices = resize_nearest_indices(img.height(), output_height as u32)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let mut output = vec![0u8; output_byte_count];
    let mut vector_blocks = 0u64;
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
    Ok((output, vector_blocks))
}

fn f_resize_filtered_output(
    source: &[f32],
    source_width: usize,
    source_height: usize,
    output_width: usize,
    output_height: usize,
    filter: &ResampleFilter,
    output_byte_count: usize,
) -> Result<(Vec<u8>, u64), PilError> {
    let (kernel, support) = resample_kernel(filter);
    let needs_horizontal = output_width != source_width;
    let needs_vertical = output_height != source_height;
    let intermediate_len = source_height
        .checked_mul(output_width)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let (intermediate, horizontal_blocks) = if needs_horizontal {
        let coeffs =
            precompute_coeffs_f64(output_width as u32, source_width as u32, kernel, support);
        f_resize_horizontal_pass(
            source,
            source_width,
            source_height,
            output_width,
            filter,
            &coeffs,
        )?
    } else {
        f_resize_identity_pass(source, source_width, source_height, output_width)?
    };
    if intermediate.len() != intermediate_len {
        return Err(simd_unsupported("Resize"));
    }
    let mut output = vec![0u8; output_byte_count];
    if needs_vertical {
        let coeffs =
            precompute_coeffs_f64(output_height as u32, source_height as u32, kernel, support);
        let vertical_blocks = f_resize_vertical_pass(
            &intermediate,
            source_height,
            output_width,
            output_height,
            filter,
            &coeffs,
            &mut output,
        )?;
        Ok((output, horizontal_blocks.saturating_add(vertical_blocks)))
    } else {
        write_f32_samples_le(&mut output, 0, &intermediate);
        Ok((output, horizontal_blocks))
    }
}

fn f_resize_identity_pass(
    source: &[f32],
    source_width: usize,
    source_height: usize,
    output_width: usize,
) -> Result<(Vec<f32>, u64), PilError> {
    let mut intermediate = vec![0.0f32; source_height * output_width];
    let mut vector_blocks = 0u64;
    for source_y in 0..source_height {
        let source_row = source_y
            .checked_mul(source_width)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let output_row = source_y
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
            intermediate[output_row + output_x..output_row + output_x + count]
                .copy_from_slice(&f32x8::new(values).to_array()[..count]);
            vector_blocks = vector_blocks.saturating_add(1);
        }
    }
    Ok((intermediate, vector_blocks))
}

fn f_resize_horizontal_pass(
    source: &[f32],
    source_width: usize,
    source_height: usize,
    output_width: usize,
    filter: &ResampleFilter,
    coeffs: &crate::ops::pil_resize::FilterCoeffsF64,
) -> Result<(Vec<f32>, u64), PilError> {
    // Cache the common scale-two bicubic lane coordinates/weights once; the
    // generic path retains Pillow's separate multiply/add rule for wide rows.
    let lane_plan = if matches!(filter, ResampleFilter::Bicubic)
        && output_width.checked_mul(2) == Some(source_width)
    {
        build_f_resize_horizontal_lane_plan(coeffs, source_width, output_width)?
    } else {
        None
    };
    let mut intermediate = vec![0.0f32; source_height * output_width];
    let mut vector_blocks = 0u64;
    for source_y in 0..source_height {
        let source_row = source_y
            .checked_mul(source_width)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let intermediate_row = source_y
            .checked_mul(output_width)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        for output_x in (0..output_width).step_by(SIMD_RESIZE_LANES) {
            let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
            let values = if let Some((ranges, taps)) = &lane_plan {
                let (start, count) = ranges[output_x / SIMD_RESIZE_LANES];
                f_resize_horizontal_planned_block(source, source_row, &taps[start..start + count])?
            } else {
                f_resize_horizontal_general_block(source, source_row, output_x, count, coeffs)?
            };
            intermediate[intermediate_row + output_x..intermediate_row + output_x + count]
                .copy_from_slice(&values[..count]);
            vector_blocks = vector_blocks.saturating_add(1);
        }
    }
    Ok((intermediate, vector_blocks))
}

fn build_f_resize_horizontal_lane_plan(
    coeffs: &crate::ops::pil_resize::FilterCoeffsF64,
    source_width: usize,
    output_width: usize,
) -> Result<Option<FResizeHorizontalLanePlan>, PilError> {
    let mut block_ranges = Vec::with_capacity(output_width.div_ceil(SIMD_RESIZE_LANES));
    let mut lane_taps = Vec::new();
    for output_x in (0..output_width).step_by(SIMD_RESIZE_LANES) {
        let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
        let max_count = (0..count)
            .map(|lane| coeffs.weights[output_x + lane].len())
            .max()
            .unwrap_or(0);
        if max_count >= F_RESIZE_VECTOR_WIDTH {
            return Ok(None);
        }
        let tap_start = lane_taps.len();
        for tap in 0..max_count {
            let mut source_x = [0usize; SIMD_RESIZE_LANES];
            let mut weights = [0.0; SIMD_RESIZE_LANES];
            let mut active_lanes = 0u8;
            for lane in 0..count {
                let output_index = output_x + lane;
                let lane_weights = coeffs
                    .weights
                    .get(output_index)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                let Some(&weight) = lane_weights.get(tap) else {
                    continue;
                };
                let xmin = *coeffs
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
                && (1..SIMD_RESIZE_LANES)
                    .all(|lane| source_x[0].checked_add(lane * 2) == Some(source_x[lane]));
            lane_taps.push((source_x, f64x8::new(weights), active_lanes, stride_two));
        }
        block_ranges.push((tap_start, max_count));
    }
    Ok(Some((block_ranges, lane_taps)))
}

fn f_resize_horizontal_planned_block(
    source: &[f32],
    source_row: usize,
    taps: &[FResizeLaneTap],
) -> Result<[f32; SIMD_RESIZE_LANES], PilError> {
    let mut sums = f64x8::splat(0.0);
    for (source_x, weights, active_lanes, stride_two) in taps {
        let values = if *stride_two {
            let start = source_row
                .checked_add(source_x[0])
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let end = start
                .checked_add(15)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let row = source
                .get(start..end)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            [
                f64::from(row[0]),
                f64::from(row[2]),
                f64::from(row[4]),
                f64::from(row[6]),
                f64::from(row[8]),
                f64::from(row[10]),
                f64::from(row[12]),
                f64::from(row[14]),
            ]
        } else if *active_lanes == u8::MAX {
            std::array::from_fn(|lane| f64::from(source[source_row + source_x[lane]]))
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
    Ok(sums.to_array().map(|value| value as f32))
}

fn f_resize_horizontal_general_block(
    source: &[f32],
    source_row: usize,
    output_x: usize,
    count: usize,
    coeffs: &crate::ops::pil_resize::FilterCoeffsF64,
) -> Result<[f32; SIMD_RESIZE_LANES], PilError> {
    let max_count = (0..count)
        .map(|lane| coeffs.weights[output_x + lane].len())
        .max()
        .unwrap_or(0);
    // Pillow's F-mode horizontal resampler separates complete 16-tap
    // product/add blocks from the fused tail. Neighboring output lanes can
    // have different tap counts, so preserve that boundary independently.
    let product_counts: [usize; SIMD_RESIZE_LANES] = std::array::from_fn(|lane| {
        if lane < count {
            (coeffs.weights[output_x + lane].len() / F_RESIZE_VECTOR_WIDTH) * F_RESIZE_VECTOR_WIDTH
        } else {
            0
        }
    });
    let max_product_count = product_counts[..count].iter().copied().max().unwrap_or(0);
    let mut vector_sums = f64x8::splat(0.0);
    let mut ordered_sums = [0.0; SIMD_RESIZE_LANES];
    for tap in 0..max_count {
        let mut values = [0.0; SIMD_RESIZE_LANES];
        let mut weights = [0.0; SIMD_RESIZE_LANES];
        for lane in 0..count {
            let output_index = output_x + lane;
            let lane_weights = coeffs
                .weights
                .get(output_index)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let Some(&weight) = lane_weights.get(tap) else {
                continue;
            };
            let source_x = usize::try_from(
                *coeffs
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
        if max_product_count == 0 {
            vector_sums = weight_vector.mul_add(value_vector, vector_sums);
        } else {
            let fused = weight_vector
                .mul_add(value_vector, f64x8::new(ordered_sums))
                .to_array();
            let products = if tap < max_product_count {
                std::hint::black_box(weight_vector * value_vector).to_array()
            } else {
                [0.0; SIMD_RESIZE_LANES]
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
    // Preserve Pillow's signed zero and per-lane 16-tap multiply/add split.
    let values = if max_product_count == 0 {
        vector_sums.to_array().map(|value| value as f32)
    } else {
        ordered_sums.map(|value| value as f32)
    };
    Ok(values)
}

fn f_resize_vertical_pass(
    intermediate: &[f32],
    source_height: usize,
    output_width: usize,
    output_height: usize,
    filter: &ResampleFilter,
    coeffs: &crate::ops::pil_resize::FilterCoeffsF64,
    output: &mut [u8],
) -> Result<u64, PilError> {
    let lane_plan = matches!(filter, ResampleFilter::Bicubic)
        && output_height.checked_mul(2) == Some(source_height);
    let mut row_taps = Vec::new();
    if lane_plan {
        row_taps.reserve(SIMD_RESIZE_LANES);
    }
    let mut vector_blocks = 0u64;
    for output_y in 0..output_height {
        let y0 = usize::try_from(
            *coeffs
                .xmin
                .get(output_y)
                .ok_or_else(|| simd_unsupported("Resize"))?,
        )
        .map_err(|_| simd_unsupported("Resize"))?;
        let weights = coeffs
            .weights
            .get(output_y)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let output_row = output_y
            .checked_mul(output_width)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        if lane_plan {
            row_taps.clear();
            for (tap, &weight) in weights.iter().enumerate() {
                let source_y = y0
                    .checked_add(tap)
                    .filter(|&row| row < source_height)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                let source_row = source_y
                    .checked_mul(output_width)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                row_taps.push((source_row, weight));
            }
        }
        for output_x in (0..output_width).step_by(SIMD_RESIZE_LANES) {
            let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
            let values = f_resize_vertical_block(
                intermediate,
                output_width,
                output_x,
                count,
                y0,
                weights,
                &row_taps,
                lane_plan,
            )?;
            write_f32_samples_le(output, output_row + output_x, &values[..count]);
            vector_blocks = vector_blocks.saturating_add(1);
        }
    }
    Ok(vector_blocks)
}

fn f_resize_vertical_block(
    intermediate: &[f32],
    output_width: usize,
    output_x: usize,
    count: usize,
    y0: usize,
    weights: &[f64],
    row_taps: &[(usize, f64)],
    lane_plan: bool,
) -> Result<[f32; SIMD_RESIZE_LANES], PilError> {
    if lane_plan {
        let mut sums = f64x8::splat(0.0);
        for &(source_row, weight) in row_taps {
            let values = std::array::from_fn(|lane| {
                if lane < count {
                    f64::from(intermediate[source_row + output_x + lane])
                } else {
                    0.0
                }
            });
            sums = f64x8::splat(weight).mul_add(f64x8::new(values), sums);
        }
        return Ok(sums.to_array().map(|value| value as f32));
    }
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
    // Keep the left-to-right tap order: it preserves signed zero and exact bytes.
    Ok(sums.map(|value| value as f32))
}
