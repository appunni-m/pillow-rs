use super::{
    DynamicImage, PilError, PipelineOp, f32x8, native_float_rank_supported_for_image,
    native_uniform_f_mode_image, preserve_mode, simd_order_statistic_filter, simd_unsupported,
};
const SIMD_FLOAT_ORDER_STATISTIC_MAX_AREA: usize = 81;
const SIMD_FLOAT_ORDER_STATISTIC_LANES: usize = 8;
#[cfg(feature = "parallel")]
const SIMD_FLOAT_ORDER_STATISTIC_PARALLEL_PIXEL_THRESHOLD: usize = 32 * 32;
// The byte-domain selector switches away from the fixed network above 5x5,
// but the existing F-mode vector sorter is independently bounded by its
// 9x9/81-value storage. Keep that exact F-mode contract available.
pub(super) const SIMD_FLOAT_ORDER_STATISTIC_MAX_SIZE: u32 = 9;

#[inline]
fn sort_float_order_statistic_vectors(values: &mut [f32x8]) {
    for pass in 0..values.len() {
        let mut index = pass & 1;
        while index + 1 < values.len() {
            let left = values[index];
            let right = values[index + 1];
            values[index] = left.min(right);
            values[index + 1] = left.max(right);
            index += 2;
        }
    }
}

#[inline]
fn rank_filter_float_scalar(
    raw: &[u8],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    half: usize,
    rank: usize,
) -> f32 {
    let size = half.saturating_mul(2).saturating_add(1);
    let area = size.saturating_mul(size);
    debug_assert!(area <= SIMD_FLOAT_ORDER_STATISTIC_MAX_AREA);
    let mut values = [0.0f32; SIMD_FLOAT_ORDER_STATISTIC_MAX_AREA];
    let mut index = 0usize;
    for row_offset in 0..size {
        let source_y = y
            .saturating_add(row_offset)
            .saturating_sub(half)
            .min(height - 1);
        for column_offset in 0..size {
            let source_x = x
                .saturating_add(column_offset)
                .saturating_sub(half)
                .min(width - 1);
            let base = (source_y * width + source_x) * 4;
            values[index] =
                f32::from_le_bytes([raw[base], raw[base + 1], raw[base + 2], raw[base + 3]]);
            index += 1;
        }
    }
    values[..area].sort_unstable_by(|left, right| {
        left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
    });
    values[rank]
}

#[inline]
fn rank_filter_float_window_vector(
    raw: &[u8],
    width: usize,
    height: usize,
    x_start: usize,
    active: usize,
    y: usize,
    half: usize,
    row_offset: usize,
    column_offset: usize,
) -> f32x8 {
    let mut values = [0.0f32; SIMD_FLOAT_ORDER_STATISTIC_LANES];
    let source_y = y
        .saturating_add(row_offset)
        .saturating_sub(half)
        .min(height - 1);
    for (lane, value) in values.iter_mut().enumerate().take(active) {
        let source_x = x_start
            .saturating_add(lane)
            .saturating_add(column_offset)
            .saturating_sub(half)
            .min(width - 1);
        let base = (source_y * width + source_x) * 4;
        *value = f32::from_le_bytes([raw[base], raw[base + 1], raw[base + 2], raw[base + 3]]);
    }
    f32x8::from(values)
}

#[inline]
fn store_rank_filter_float_vector(
    output: &mut [u8],
    width: usize,
    y: usize,
    x_start: usize,
    active: usize,
    values: f32x8,
) {
    let values = values.to_array();
    for (lane, value) in values.into_iter().enumerate().take(active) {
        let base = (y * width + x_start + lane) * 4;
        output[base..base + 4].copy_from_slice(&value.to_le_bytes());
    }
}

/// Native SIMD rank-family implementation for F-mode images.
///
/// F-mode pixels are one little-endian f32 in the four-byte native buffer.
/// Coordinates are gathered with scalar control, while extrema and the
/// compare/exchange sorting network operate independently in eight lanes.
/// The bounded size keeps the fixed network deterministic and matches the
/// exact scalar ordering used by Pillow for the supported public contract.
fn rank_filter_float_order_statistic_row(
    raw: &[u8],
    output_row: &mut [u8],
    width: usize,
    height: usize,
    size: usize,
    half: usize,
    area: usize,
    rank: usize,
    y: usize,
) {
    let mut x = 0usize;
    while x < width {
        let active = (width - x).min(SIMD_FLOAT_ORDER_STATISTIC_LANES);
        if active == 0 {
            let value = rank_filter_float_scalar(raw, width, height, x, y, half, rank);
            output_row[x * 4..(x + 1) * 4].copy_from_slice(&value.to_le_bytes());
            x += 1;
            continue;
        }
        let mut values = [f32x8::splat(0.0); SIMD_FLOAT_ORDER_STATISTIC_MAX_AREA];
        let mut value_index = 0usize;
        for row_offset in 0..size {
            for column_offset in 0..size {
                values[value_index] = rank_filter_float_window_vector(
                    raw,
                    width,
                    height,
                    x,
                    active,
                    y,
                    half,
                    row_offset,
                    column_offset,
                );
                value_index += 1;
            }
        }
        let selected = if rank == 0 {
            values[..area]
                .iter()
                .copied()
                .fold(f32x8::splat(f32::INFINITY), |current, value| {
                    current.min(value)
                })
        } else if rank + 1 == area {
            values[..area]
                .iter()
                .copied()
                .fold(f32x8::splat(f32::NEG_INFINITY), |current, value| {
                    current.max(value)
                })
        } else {
            sort_float_order_statistic_vectors(&mut values[..area]);
            values[rank]
        };
        store_rank_filter_float_vector(output_row, 0, 0, x, active, selected);
        x += active;
    }
}

pub(in crate::compute::pool_simd::ops::adapters) fn simd_float_order_statistic_filter(
    img: &DynamicImage,
    size: u32,
    rank: u32,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if !native_float_rank_supported_for_image(img, mode, size, rank) {
        return Err(simd_unsupported("MedianFilter/RankFilter"));
    }
    let width = img.width() as usize;
    let height = img.height() as usize;
    let size = size as usize;
    let half = size / 2;
    let area = size * size;
    let rank = rank as usize;
    let raw = img.as_bytes();
    if area > 1 && native_uniform_f_mode_image(raw, width, height) {
        // The F-mode rank filter is the identity on bit-identical scalar
        // samples; report the data fast path separately from vector work.
        crate::compute::record_pipeline_operation_path("native-identity");
        return Ok(preserve_mode(img, img.clone()));
    }
    let mut output = raw.to_vec();
    #[cfg(feature = "parallel")]
    if width.saturating_mul(height) >= SIMD_FLOAT_ORDER_STATISTIC_PARALLEL_PIXEL_THRESHOLD {
        super::super::simd_rows_mut_serial!(
            &mut output,
            width * 4,
            height,
            |_row_start, _row_end, y, row| {
                rank_filter_float_order_statistic_row(
                    raw, row, width, height, size, half, area, rank, y as usize,
                );
            }
        );
    } else {
        for y in 0..height {
            let row_start = y * width * 4;
            let row_end = row_start + width * 4;
            rank_filter_float_order_statistic_row(
                raw,
                &mut output[row_start..row_end],
                width,
                height,
                size,
                half,
                area,
                rank,
                y,
            );
        }
    }
    #[cfg(not(feature = "parallel"))]
    for y in 0..height {
        let row_start = y * width * 4;
        let row_end = row_start + width * 4;
        rank_filter_float_order_statistic_row(
            raw,
            &mut output[row_start..row_end],
            width,
            height,
            size,
            half,
            area,
            rank,
            y,
        );
    }
    let vector_blocks = width.div_ceil(SIMD_FLOAT_ORDER_STATISTIC_LANES) as u64 * height as u64;
    let scalar_tail = 0u64;

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    let result = crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, 4)?;
    Ok(preserve_mode(img, result))
}

/// Native SIMD adapter for Pillow's median filter.
pub(crate) fn simd_median_filter(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::MedianFilter { size } = op else {
        return Err(PilError::ValueError("expected MedianFilter op".into()));
    };
    if mode == Some("F") {
        return simd_float_order_statistic_filter(img, *size, size.saturating_mul(*size) / 2, mode);
    }
    simd_order_statistic_filter(img, *size, size.saturating_mul(*size) / 2, mode)
}

/// Native SIMD adapter for Pillow's rank filter.
pub(crate) fn simd_rank_filter(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::RankFilter { size, rank } = op else {
        return Err(PilError::ValueError("expected RankFilter op".into()));
    };
    if mode == Some("F") {
        return simd_float_order_statistic_filter(img, *size, *rank, mode);
    }
    simd_order_statistic_filter(img, *size, *rank, mode)
}
