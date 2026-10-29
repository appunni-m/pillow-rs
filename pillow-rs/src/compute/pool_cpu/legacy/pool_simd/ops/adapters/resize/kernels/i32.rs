//! SIMD resize adapters for Pillow's signed 32-bit `I` image mode.
//!
//! The entry points and tap planner live here so mode-specific resize behavior
//! stays in one family. The thumbnail family reuses the planner for its I-mode
//! reducing-gap resample.

use super::super::{SIMD_RESIZE_LANES, simd_unsupported};
use super::resize_nearest_indices;
use super::simd_resize_zero_source;
use crate::compute::pool_cpu::ops::geometry::resample_kernel;
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::ops::pil_resize::{
    FilterCoeffsF64, i32_samples_from_le_bytes, precompute_coeffs_f64, precompute_coeffs_f64_boxed,
    round_up,
};
use crate::pipeline::ResampleFilter;
use crate::raster::DynamicImage;
use wide::{f64x8, i32x8};

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[path = "i32/neon.rs"]
mod neon;

#[derive(Clone, Copy)]
pub(in crate::compute::pool_simd::ops::adapters) struct ResizeIHorizontalTapPlan {
    pub(in crate::compute::pool_simd::ops::adapters) source_indices: [usize; SIMD_RESIZE_LANES],
    pub(in crate::compute::pool_simd::ops::adapters) active_lanes: u8,
    pub(in crate::compute::pool_simd::ops::adapters) weights: f64x8,
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    pub(in crate::compute::pool_simd::ops::adapters) neon_source_start: Option<usize>,
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    pub(in crate::compute::pool_simd::ops::adapters) neon_weight: Option<f64>,
}

pub(in crate::compute::pool_simd::ops::adapters) struct ResizeIHorizontalBlockPlan {
    pub(in crate::compute::pool_simd::ops::adapters) taps: Vec<ResizeIHorizontalTapPlan>,
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    pub(in crate::compute::pool_simd::ops::adapters) neon_eligible: bool,
}

pub(in crate::compute::pool_simd::ops::adapters) struct ResizeIHorizontalPlan {
    pub(in crate::compute::pool_simd::ops::adapters) blocks: Vec<ResizeIHorizontalBlockPlan>,
}

/// Resolve I-mode horizontal tap addresses and weights once per resize. The
/// data pass revisits this geometry for every source row, so keeping checked
/// coefficient access and address arithmetic out of that loop leaves only
/// sample gathers and the ordered vector FMA sequence.
pub(in crate::compute::pool_simd::ops::adapters) fn build_resize_i_horizontal_plan(
    coeffs: &FilterCoeffsF64,
    source_width: usize,
    output_width: usize,
) -> Option<ResizeIHorizontalPlan> {
    build_resize_i_horizontal_plan_inner::<false>(coeffs, source_width, output_width)
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
fn build_resize_i_horizontal_plan_with_neon(
    coeffs: &FilterCoeffsF64,
    source_width: usize,
    output_width: usize,
) -> Option<ResizeIHorizontalPlan> {
    build_resize_i_horizontal_plan_inner::<true>(coeffs, source_width, output_width)
}

fn build_resize_i_horizontal_plan_inner<const ALLOW_NEON: bool>(
    coeffs: &FilterCoeffsF64,
    source_width: usize,
    output_width: usize,
) -> Option<ResizeIHorizontalPlan> {
    let mut blocks = Vec::with_capacity(output_width.div_ceil(SIMD_RESIZE_LANES));
    for output_x in (0..output_width).step_by(SIMD_RESIZE_LANES) {
        let lane_count = (output_width - output_x).min(SIMD_RESIZE_LANES);
        let max_count = (0..lane_count)
            .filter_map(|lane| coeffs.weights.get(output_x + lane))
            .map(Vec::len)
            .max()
            .unwrap_or(0);
        let mut taps = Vec::with_capacity(max_count);
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        let mut neon_eligible = ALLOW_NEON && lane_count == SIMD_RESIZE_LANES;
        for tap in 0..max_count {
            let mut source_indices = [0usize; SIMD_RESIZE_LANES];
            let mut weights = [0.0f64; SIMD_RESIZE_LANES];
            let mut active_lanes = 0u8;
            for lane in 0..lane_count {
                let output_index = output_x + lane;
                let lane_weights = coeffs.weights.get(output_index)?;
                let Some(&weight) = lane_weights.get(tap) else {
                    continue;
                };
                let source_x = usize::try_from(*coeffs.xmin.get(output_index)?)
                    .ok()?
                    .checked_add(tap)?;
                if source_x >= source_width {
                    return None;
                }
                source_indices[lane] = source_x;
                weights[lane] = weight;
                active_lanes |= 1u8 << lane;
            }
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            let (neon_source_start, neon_weight) = if ALLOW_NEON && active_lanes == u8::MAX {
                let first_source = source_indices[0];
                let stride_two = source_indices
                    .iter()
                    .enumerate()
                    .all(|(lane, &source)| first_source.checked_add(lane * 2) == Some(source));
                // The interleaved loads read each requested stride-two sample
                // and its adjacent partner, including the final pair.
                let source_span_fits = first_source
                    .checked_add(SIMD_RESIZE_LANES * 2)
                    .is_some_and(|end| end <= source_width);
                let first_weight = weights[0];
                let weights_match = weights
                    .iter()
                    .all(|weight| weight.to_bits() == first_weight.to_bits());
                (
                    (stride_two && source_span_fits).then_some(first_source),
                    weights_match.then_some(first_weight),
                )
            } else {
                (None, None)
            };
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            {
                neon_eligible &= neon_source_start.is_some() && neon_weight.is_some();
            }
            taps.push(ResizeIHorizontalTapPlan {
                source_indices,
                active_lanes,
                weights: f64x8::new(weights),
                #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
                neon_source_start,
                #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
                neon_weight,
            });
        }
        blocks.push(ResizeIHorizontalBlockPlan {
            taps,
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            neon_eligible,
        });
    }
    Some(ResizeIHorizontalPlan { blocks })
}

#[inline]
pub(in crate::compute::pool_simd::ops::adapters) fn resize_i_horizontal_values(
    source_row: &[i32],
    tap: &ResizeIHorizontalTapPlan,
) -> [f64; SIMD_RESIZE_LANES] {
    std::array::from_fn(|lane| {
        if tap.active_lanes & (1u8 << lane) != 0 {
            f64::from(source_row[tap.source_indices[lane]])
        } else {
            0.0
        }
    })
}

/// Resize an `I` image without widening its four-byte samples into an ordinary
/// byte image. Source gathers and IEEE/sample serialization are scalar control
/// work; horizontal and vertical weighted sums use `f64x8`, and nearest output
/// values are packed through `i32x8`. This mirrors the CPU `resize_i` ordering,
/// including its rounded intermediate row.
pub(in crate::compute::pool_simd::ops::adapters) fn simd_resize_i32(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    filter: &ResampleFilter,
) -> Result<DynamicImage, PilError> {
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
            "SIMD I resize source buffer shape mismatch".into(),
        ));
    }
    if source_width == 0 || source_height == 0 {
        return simd_resize_zero_source(img, output_width as u32, output_height as u32, 4);
    }
    let output_len = output_width
        .checked_mul(output_height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let source_values = i32_samples_from_le_bytes(img.as_bytes(), source_pixels);
    let mut output = vec![0u8; output_len];
    let (vector_blocks, used_neon) = if matches!(filter, ResampleFilter::Nearest) {
        (
            resize_i32_nearest(
                img,
                &source_values,
                output_width,
                output_height,
                &mut output,
            )?,
            false,
        )
    } else {
        resize_i32_filtered(
            &source_values,
            source_width,
            source_height,
            output_width,
            output_height,
            filter,
            &mut output,
        )?
    };
    finish_resize_i32(
        img,
        output_width as u32,
        output_height as u32,
        output,
        vector_blocks,
        used_neon,
    )
}

fn resize_i32_nearest(
    img: &DynamicImage,
    source_values: &[i32],
    output_width: usize,
    output_height: usize,
    output: &mut [u8],
) -> Result<u64, PilError> {
    let source_width = img.width() as usize;
    let x_indices = resize_nearest_indices(img.width(), output_width as u32)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let y_indices = resize_nearest_indices(img.height(), output_height as u32)
        .ok_or_else(|| simd_unsupported("Resize"))?;
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
                    source_values[source_row + x_indices[output_x + lane]]
                } else {
                    0
                }
            });
            let packed = i32x8::new(values).to_array();
            for lane in 0..count {
                let output_start = output_row
                    .checked_add(output_x + lane)
                    .and_then(|pixel| pixel.checked_mul(4))
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                let output_end = output_start
                    .checked_add(4)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                output
                    .get_mut(output_start..output_end)
                    .ok_or_else(|| simd_unsupported("Resize"))?
                    .copy_from_slice(&packed[lane].to_le_bytes());
            }
            vector_blocks = vector_blocks.saturating_add(1);
        }
    }
    Ok(vector_blocks)
}

fn resize_i32_filtered(
    source_values: &[i32],
    source_width: usize,
    source_height: usize,
    output_width: usize,
    output_height: usize,
    filter: &ResampleFilter,
    output: &mut [u8],
) -> Result<(u64, bool), PilError> {
    // Use the same scalar kernel table as the CPU I/F path. Building the
    // coefficients is control-plane work; accumulation remains in the SIMD
    // adapter.
    let (kernel, support) = resample_kernel(filter);
    let horizontal =
        precompute_coeffs_f64(output_width as u32, source_width as u32, kernel, support);
    let vertical =
        precompute_coeffs_f64(output_height as u32, source_height as u32, kernel, support);
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    let horizontal_plan =
        build_resize_i_horizontal_plan_with_neon(&horizontal, source_width, output_width)
            .ok_or_else(|| simd_unsupported("Resize"))?;
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    let horizontal_plan = build_resize_i_horizontal_plan(&horizontal, source_width, output_width)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let intermediate_len = source_height
        .checked_mul(output_width)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    // Pillow materializes the horizontal pass as INT32 before the vertical
    // pass, preserving native overflow/cast behavior for large signed samples.
    let mut intermediate = vec![0i32; intermediate_len];
    let (horizontal_blocks, horizontal_used_neon) = resize_i32_horizontal(
        source_values,
        source_width,
        source_height,
        output_width,
        &horizontal_plan,
        &mut intermediate,
    )?;
    let (vertical_blocks, vertical_used_neon) = resize_i32_vertical(
        &intermediate,
        output_width,
        output_height,
        &vertical,
        output,
    )?;
    Ok((
        horizontal_blocks.saturating_add(vertical_blocks),
        horizontal_used_neon || vertical_used_neon,
    ))
}

fn resize_i32_horizontal(
    source_values: &[i32],
    source_width: usize,
    source_height: usize,
    output_width: usize,
    horizontal_plan: &ResizeIHorizontalPlan,
    intermediate: &mut [i32],
) -> Result<(u64, bool), PilError> {
    let mut vector_blocks = 0u64;
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    let mut used_neon = false;
    for source_y in 0..source_height {
        let source_row = source_y
            .checked_mul(source_width)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let source_row_end = source_row
            .checked_add(source_width)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let source_row = source_values
            .get(source_row..source_row_end)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let intermediate_row = source_y
            .checked_mul(output_width)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        for block_index in 0..horizontal_plan.blocks.len() {
            let output_x = block_index
                .checked_mul(SIMD_RESIZE_LANES)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let block = horizontal_plan
                .blocks
                .get(block_index)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            let neon_values = if block.neon_eligible {
                neon::horizontal_block(source_row, block)
            } else {
                None
            };
            #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
            let neon_values: Option<[i32; SIMD_RESIZE_LANES]> = None;
            if let Some(values) = neon_values {
                let block_start = intermediate_row
                    .checked_add(output_x)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                let block_end = block_start
                    .checked_add(count)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                let output_block = intermediate
                    .get_mut(block_start..block_end)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                output_block.copy_from_slice(&values[..count]);
                #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
                {
                    used_neon = true;
                }
                vector_blocks = vector_blocks.saturating_add(1);
                continue;
            }
            let mut sums = f64x8::splat(0.0);
            for tap in &block.taps {
                sums = tap.weights.mul_add(
                    f64x8::new(resize_i_horizontal_values(source_row, tap)),
                    sums,
                );
            }
            let values = sums.to_array();
            let block_start = intermediate_row
                .checked_add(output_x)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let block_end = block_start
                .checked_add(count)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let output_block = intermediate
                .get_mut(block_start..block_end)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            for (destination, &value) in output_block.iter_mut().zip(&values[..count]) {
                *destination = round_up(value) as i32;
            }
            vector_blocks = vector_blocks.saturating_add(1);
        }
    }
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    let used_neon = used_neon;
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    let used_neon = false;
    Ok((vector_blocks, used_neon))
}

fn resize_i32_vertical(
    intermediate: &[i32],
    output_width: usize,
    output_height: usize,
    vertical: &FilterCoeffsF64,
    output: &mut [u8],
) -> Result<(u64, bool), PilError> {
    let mut vector_blocks = 0u64;
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    let mut used_neon = false;
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
        for output_x in (0..output_width).step_by(SIMD_RESIZE_LANES) {
            let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            let neon_values = if count == SIMD_RESIZE_LANES {
                neon::vertical_block(intermediate, output_width, output_x, y0, weights)
            } else {
                None
            };
            #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
            let neon_values: Option<[i32; SIMD_RESIZE_LANES]> = None;
            let rounded = if let Some(values) = neon_values {
                #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
                {
                    used_neon = true;
                }
                values
            } else {
                let mut sums = f64x8::splat(0.0);
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
                    sums = f64x8::splat(weight).mul_add(f64x8::new(values), sums);
                }
                sums.to_array().map(|value| round_up(value) as i32)
            };
            let packed = i32x8::new(rounded).to_array();
            let block_start = output_row
                .checked_add(output_x)
                .and_then(|pixel| pixel.checked_mul(4))
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let block_bytes = count
                .checked_mul(4)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let block_end = block_start
                .checked_add(block_bytes)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let output_block = output
                .get_mut(block_start..block_end)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            for (destination, value) in output_block.chunks_exact_mut(4).zip(&packed[..count]) {
                destination.copy_from_slice(&value.to_le_bytes());
            }
            vector_blocks = vector_blocks.saturating_add(1);
        }
    }
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    let used_neon = used_neon;
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    let used_neon = false;
    Ok((vector_blocks, used_neon))
}

fn finish_resize_i32(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    output: Vec<u8>,
    vector_blocks: u64,
    used_neon: bool,
) -> Result<DynamicImage, PilError> {
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    crate::compute::record_pipeline_operation_path(if used_neon {
        "neon-f64-resize"
    } else {
        "vector"
    });
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    {
        let _ = used_neon;
        crate::compute::record_pipeline_operation_path("vector");
    }
    let result = crate::image_utils::raw_bytes_to_image(output_width, output_height, output, 4)?;
    Ok(preserve_mode(img, result))
}

/// Resize an `I` image through the fractional source box used by
/// `Image.thumbnail` after an integer reducing-gap pass. Keep the INT32
/// intermediate and round each horizontal/vertical f64 sum exactly as the
/// scalar `ImagingResample` path does; the surrounding loops still publish
/// SIMD adapter telemetry and pack complete four-byte samples.
pub(in crate::compute::pool_simd::ops::adapters) fn simd_resize_i32_boxed(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    filter: ResampleFilter,
) -> Result<DynamicImage, PilError> {
    let source_width_u32 = img.width();
    let source_height_u32 = img.height();
    let output_width_u32 = output_width;
    let output_height_u32 = output_height;
    let source_width = usize::try_from(source_width_u32).map_err(|_| simd_unsupported("Resize"))?;
    let source_height =
        usize::try_from(source_height_u32).map_err(|_| simd_unsupported("Resize"))?;
    let output_width = usize::try_from(output_width_u32).map_err(|_| simd_unsupported("Resize"))?;
    let output_height =
        usize::try_from(output_height_u32).map_err(|_| simd_unsupported("Resize"))?;
    let source_pixels = source_width
        .checked_mul(source_height)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let source_len = source_pixels
        .checked_mul(4)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    if img.as_bytes().len() != source_len {
        return Err(PilError::InternalError(
            "SIMD boxed I resize source buffer shape mismatch".into(),
        ));
    }
    if source_width == 0 || source_height == 0 {
        return simd_resize_zero_source(img, output_width_u32, output_height_u32, 4);
    }
    let output_len = output_width
        .checked_mul(output_height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let source = i32_samples_from_le_bytes(img.as_bytes(), source_pixels);
    let horizontal = precompute_coeffs_f64_boxed(
        output_width_u32,
        source_width_u32,
        box_left,
        box_right,
        filter,
    );
    let vertical = precompute_coeffs_f64_boxed(
        output_height_u32,
        source_height_u32,
        box_top,
        box_bottom,
        filter,
    );
    let mut intermediate = vec![0i32; source_height * output_width];
    for source_y in 0..source_height {
        let source_row = source_y * source_width;
        let intermediate_row = source_y * output_width;
        for output_x in 0..output_width {
            let x0 = horizontal.xmin[output_x];
            let mut sum = 0.0f64;
            for (tap, &weight) in horizontal.weights[output_x].iter().enumerate() {
                let source_x = (x0 + tap as i64) as usize;
                sum = weight.mul_add(f64::from(source[source_row + source_x]), sum);
            }
            intermediate[intermediate_row + output_x] = round_up(sum) as i32;
        }
    }
    let mut output = vec![0i32; output_width * output_height];
    for output_y in 0..output_height {
        let y0 = vertical.xmin[output_y];
        let output_row = output_y * output_width;
        for output_x in 0..output_width {
            let mut sum = 0.0f64;
            for (tap, &weight) in vertical.weights[output_y].iter().enumerate() {
                let source_y = (y0 + tap as i64) as usize;
                sum = weight.mul_add(
                    f64::from(intermediate[source_y * output_width + output_x]),
                    sum,
                );
            }
            output[output_row + output_x] = round_up(sum) as i32;
        }
    }
    let bytes: Vec<u8> = output
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    debug_assert_eq!(bytes.len(), output_len);
    let vector_blocks = output_width.div_ceil(SIMD_RESIZE_LANES) as u64 * output_height as u64;
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    let result =
        crate::image_utils::raw_bytes_to_image(output_width_u32, output_height_u32, bytes, 4)?;
    Ok(preserve_mode(img, result))
}
