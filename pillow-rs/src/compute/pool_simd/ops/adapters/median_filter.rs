//! SIMD MedianFilter, RankFilter, and rank-family extrema adapters and kernels.

mod extrema;
mod kernels;
pub(crate) use self::extrema::{simd_max_filter, simd_min_filter};

use super::{
    SIMD_RANK_FILTER_LANES, SIMD_RANK_FILTER_MIN_VECTOR_PIXELS, native_copy_image_bytes,
    native_filter_byte_layout, native_float_rank_supported_for_image,
    native_small_uniform_byte_image, native_uniform_f_mode_image, rank_filter_vector_pixels,
    simd_blur_load_u8x16, simd_unsupported, store_rank_filter_channel_vector, u8x16,
};
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;
use wide::f32x8;

#[inline]
pub(super) fn median_of_nine_vectors(values: &mut [u8x16]) -> u8x16 {
    kernels::median_of_nine_vectors(values)
}

#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub(super) fn order_statistic_row(
    raw: &[u8],
    output_row: &mut [u8],
    row_stride: usize,
    width: usize,
    height: usize,
    channels: usize,
    use_luma_alpha_median_network: bool,
    size: usize,
    half: usize,
    area: usize,
    rank: usize,
    y: usize,
) {
    if channels == 2 && size == 3 && (rank == 1 || (rank == 4 && use_luma_alpha_median_network)) {
        if rank == 1 {
            kernels::la_order_statistic_3x3_row::<1>(raw, output_row, row_stride, width, height, y);
        } else {
            kernels::la_order_statistic_3x3_row::<4>(raw, output_row, row_stride, width, height, y);
        }
    } else if channels == 3 && size == 3 && rank == 4 {
        kernels::median_filter_rgb_3x3_row(raw, output_row, row_stride, width, height, y);
    } else {
        rank_filter_order_statistic_row(
            raw,
            output_row,
            row_stride,
            width,
            height,
            channels,
            use_luma_alpha_median_network,
            size,
            half,
            area,
            rank,
            y,
        );
    }
}

const SIMD_ORDER_STATISTIC_MAX_AREA: usize = 225;
// The fixed network's quadratic comparisons overtake eight byte-domain
// selection rounds at 7x7, so retain it only through 5x5 windows.
const SIMD_ORDER_STATISTIC_SORT_MAX_SIZE: u32 = 5;
const SIMD_ORDER_STATISTIC_HISTOGRAM_MAX_SIZE: u32 = 15;

/// Sort one order-statistic window independently in every vector lane.
///
/// Odd-even transposition is used here because it is a fixed compare/exchange
/// network: every comparison is a `u8x16` min/max, and no lane can branch on
/// the value it is sorting. It is intentionally limited to the smaller
/// windows; larger windows use the vector binary-select implementation below
/// rather than paying this quadratic setup.
#[inline]
fn sort_order_statistic_vectors(values: &mut [u8x16]) {
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
fn rank_filter_order_statistic_scalar(
    raw: &[u8],
    row_stride: usize,
    width: usize,
    height: usize,
    channels: usize,
    channel: usize,
    x: usize,
    y: usize,
    half: usize,
    rank: usize,
) -> u8 {
    let size = half.saturating_mul(2).saturating_add(1);
    let area = size.saturating_mul(size);
    debug_assert!(area <= SIMD_ORDER_STATISTIC_MAX_AREA);
    let mut values = [0u8; SIMD_ORDER_STATISTIC_MAX_AREA];
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
            values[index] = raw[source_y * row_stride + source_x * channels + channel];
            index += 1;
        }
    }
    values[..area].sort_unstable();
    values[rank]
}

/// Gather one sample from each output pixel in a vector block. The source
/// coordinates are clamped by scalar control; the resulting order statistic
/// is selected by vector compare/exchange operations.
#[inline]
fn rank_filter_order_statistic_window_vector(
    raw: &[u8],
    row_start: usize,
    width: usize,
    channels: usize,
    channel: usize,
    x_start: usize,
    active: usize,
    half: usize,
    column_offset: usize,
) -> u8x16 {
    // The upper lanes are never stored for an incomplete block. Zero is a
    // valid padding value because the sorting network is lane-independent.
    let mut values = [0u8; SIMD_RANK_FILTER_LANES];
    let last = width - 1;
    for (lane, value) in values.iter_mut().enumerate().take(active) {
        let source_x = x_start
            .saturating_add(lane)
            .saturating_add(column_offset)
            .saturating_sub(half)
            .min(last);
        *value = raw[row_start + source_x * channels + channel];
    }
    u8x16::new(values)
}

#[inline(always)]
fn rank_filter_luma_window_vector(
    source_row: &[u8],
    width: usize,
    x_start: usize,
    active: usize,
    column_offset: usize,
) -> u8x16 {
    // Interior lanes are contiguous in native L; require room for the full
    // right-neighbor load. Border and tail blocks keep explicit clamping.
    if active == SIMD_RANK_FILTER_LANES
        && x_start > 0
        && x_start.saturating_add(SIMD_RANK_FILTER_LANES) < width
    {
        return simd_blur_load_u8x16(source_row, x_start + column_offset - 1);
    }

    let mut values = [0u8; SIMD_RANK_FILTER_LANES];
    for (lane, value) in values.iter_mut().enumerate().take(active) {
        let source_x = x_start
            .saturating_add(lane)
            .saturating_add(column_offset)
            .saturating_sub(1)
            .min(width - 1);
        *value = source_row[source_x];
    }
    u8x16::new(values)
}

/// Select one order statistic from a set of vector lanes without sorting it.
///
/// Each iteration counts, in parallel, how many window samples are less than
/// or equal to the current per-lane midpoint. Eight binary-search rounds are
/// sufficient for the byte domain. The contextual preflight caps this path at
/// a 15×15 window (225 samples), so the per-lane `u8x16` counters cannot
/// overflow. The gather remains scalar control; the comparisons, counts, and
/// narrowing decisions are vector data-plane operations.
#[inline]
fn select_order_statistic_vectors(values: &[u8x16], rank: u8) -> u8x16 {
    let mut lower = u8x16::splat(0);
    let mut upper = u8x16::splat(u8::MAX);
    let rank = u8x16::splat(rank);
    let one = u8x16::splat(1);

    for _ in 0..u8::BITS {
        let midpoint = lower + ((upper - lower) >> 1u32);
        let mut count = u8x16::splat(0);
        for &value in values {
            let at_or_below = value.simd_le(midpoint);
            count += at_or_below.select(one, u8x16::splat(0));
        }
        let at_or_above_rank = count.simd_gt(rank);
        upper = at_or_above_rank.select(midpoint, upper);
        lower = at_or_above_rank.select(lower, midpoint + one);
    }
    lower
}

/// Select the second-smallest value independently in each byte lane.
///
/// The pairwise min/max reduction is exact even when the minimum occurs more
/// than once. It uses a fixed amount of vector work and avoids sorting all nine
/// samples for the common 3x3 RankFilter(rank=1) path.
#[inline]
fn select_second_order_statistic_vector(values: &[u8x16]) -> u8x16 {
    debug_assert!(values.len() >= 2);
    let mut smallest = values[0].min(values[1]);
    let mut second = values[0].max(values[1]);
    for &value in &values[2..] {
        let lower = smallest.min(value);
        let upper = smallest.max(value);
        second = second.min(upper);
        smallest = lower;
    }
    second
}

/// Apply a 3×3 native-L order statistic with nine live vectors.
#[inline]
fn rank_filter_luma_order_statistic_3x3_row<const RANK: usize>(
    raw: &[u8],
    output_row: &mut [u8],
    row_stride: usize,
    width: usize,
    height: usize,
    y: usize,
) {
    let mut x = 0usize;
    while x < width {
        let active = rank_filter_vector_pixels(width - x);
        // This 3×3 L window needs nine vectors; avoid zeroing the generic
        // 225-slot scratch buffer for every block.
        let mut values = [u8x16::splat(0); 9];
        let mut value_index = 0usize;
        for row_offset in 0..3 {
            let source_y = y
                .saturating_add(row_offset)
                .saturating_sub(1)
                .min(height - 1);
            let source_start = source_y * row_stride;
            let source_row = &raw[source_start..source_start + width];
            for column_offset in 0..3 {
                values[value_index] =
                    rank_filter_luma_window_vector(source_row, width, x, active, column_offset);
                value_index += 1;
            }
        }
        // `RANK` is const so each 3×3 operation keeps only its own selector.
        let selected = if RANK == 1 {
            select_second_order_statistic_vector(&values)
        } else {
            median_of_nine_vectors(&mut values)
        };
        store_rank_filter_channel_vector(output_row, 0, 1, 0, x, active, selected);
        x += active;
    }
}

/// Native SIMD MedianFilter/RankFilter for 8-bit interleaved layouts.
///
/// The window gather and border handling are scalar control. The actual
/// order-statistic work is lane-wise vector compare/exchange or vector binary
/// selection, and the output remains in the source's native L/LA/RGB/RGBA
/// byte layout.
fn rank_filter_order_statistic_row(
    raw: &[u8],
    output_row: &mut [u8],
    row_stride: usize,
    width: usize,
    height: usize,
    channels: usize,
    use_luma_alpha_median_network: bool,
    size: usize,
    half: usize,
    area: usize,
    rank: usize,
    y: usize,
) {
    if channels == 1 && size == 3 && matches!(rank, 1 | 4) {
        if rank == 1 {
            rank_filter_luma_order_statistic_3x3_row::<1>(
                raw, output_row, row_stride, width, height, y,
            );
        } else {
            rank_filter_luma_order_statistic_3x3_row::<4>(
                raw, output_row, row_stride, width, height, y,
            );
        }
        return;
    }

    for channel in 0..channels {
        let mut x = 0usize;
        while x < width {
            let active = rank_filter_vector_pixels(width - x);
            if active == 0 {
                for tail_x in x..width {
                    output_row[tail_x * channels + channel] = rank_filter_order_statistic_scalar(
                        raw, row_stride, width, height, channels, channel, tail_x, y, half, rank,
                    );
                }
                break;
            }

            let mut values = [u8x16::splat(0); SIMD_ORDER_STATISTIC_MAX_AREA];
            let mut value_index = 0usize;
            for row_offset in 0..size {
                let source_y = y
                    .saturating_add(row_offset)
                    .saturating_sub(half)
                    .min(height - 1);
                let source_row = source_y * row_stride;
                for column_offset in 0..size {
                    values[value_index] = rank_filter_order_statistic_window_vector(
                        raw,
                        source_row,
                        width,
                        channels,
                        channel,
                        x,
                        active,
                        half,
                        column_offset,
                    );
                    value_index += 1;
                }
            }
            let selected = if channels == 1 && size == 3 && rank == 4 {
                median_of_nine_vectors(&mut values[..area])
            } else if use_luma_alpha_median_network && size == 3 && rank == 4 {
                median_of_nine_vectors(&mut values[..area])
            } else if size <= SIMD_ORDER_STATISTIC_SORT_MAX_SIZE as usize {
                sort_order_statistic_vectors(&mut values[..area]);
                values[rank]
            } else {
                select_order_statistic_vectors(&values[..area], rank as u8)
            };
            store_rank_filter_channel_vector(output_row, 0, channels, channel, x, active, selected);
            x += active;
        }
    }
}

pub(super) fn simd_order_statistic_filter(
    img: &DynamicImage,
    size: u32,
    rank: u32,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let channels = native_filter_byte_layout(img, mode)
        .ok_or_else(|| simd_unsupported("MedianFilter/RankFilter"))?;
    let use_luma_alpha_median_network = channels == 2
        && matches!(img, DynamicImage::ImageLumaA8(_))
        && matches!(mode, None | Some("LA"));
    let area = u64::from(size).saturating_mul(u64::from(size));
    if size == 0
        || size % 2 == 0
        || size > SIMD_ORDER_STATISTIC_HISTOGRAM_MAX_SIZE
        || area > SIMD_ORDER_STATISTIC_MAX_AREA as u64
        || u64::from(rank) >= area
        || img.width() < SIMD_RANK_FILTER_MIN_VECTOR_PIXELS as u32
        || img.height() == 0
    {
        return Err(simd_unsupported("MedianFilter/RankFilter"));
    }
    let width = img.width() as usize;
    let height = img.height() as usize;
    let row_stride = width
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("SIMD rank-filter row overflow".into()))?;
    let expected_len = row_stride
        .checked_mul(height)
        .ok_or_else(|| PilError::ValueError("SIMD rank-filter dimensions overflow".into()))?;
    let raw = img.as_bytes();
    if raw.len() != expected_len {
        return Err(PilError::InternalError(
            "SIMD rank-filter source buffer shape mismatch".into(),
        ));
    }
    if native_small_uniform_byte_image(img, channels) {
        return native_copy_image_bytes(img, mode)?
            .ok_or_else(|| simd_unsupported("MedianFilter/RankFilter"));
    }
    // Current Apple ARM64 L 9x9 rank-40 controls measured SIMD at 4.289/4.200
    // ms vs CPU at 3.747/3.789 ms (see
    // docs/evidence/filter/thumbnail-fi-rank9-arm64-20261022/README.md).
    // Profiles place time in this SIMD selector but do not split gathering
    // from selection. Source inspection shows it regathers all 81 samples
    // for each 16-pixel block and scans those vectors in eight rank rounds;
    // the CPU L path reuses a 256-bin histogram as x advances. Contiguous
    // vector loads did not close the gap, so revisit window reuse or a
    // lane-wise histogram strategy with a new measured hypothesis.
    let half = (size / 2) as usize;
    let area = area as usize;
    let rank = rank as usize;
    let mut output = vec![0u8; expected_len];
    #[cfg(feature = "parallel")]
    super::simd_rows_mut_serial!(
        &mut output,
        row_stride,
        height,
        |_row_start, _row_end, y, row| {
            order_statistic_row(
                raw,
                row,
                row_stride,
                width,
                height,
                channels,
                use_luma_alpha_median_network,
                size as usize,
                half,
                area,
                rank,
                y as usize,
            );
        }
    );
    #[cfg(not(feature = "parallel"))]
    for y in 0..height {
        let row_start = y * row_stride;
        let row_end = row_start + row_stride;
        order_statistic_row(
            raw,
            &mut output[row_start..row_end],
            row_stride,
            width,
            height,
            channels,
            use_luma_alpha_median_network,
            size as usize,
            half,
            area,
            rank,
            y,
        );
    }
    let vector_blocks =
        width.div_ceil(SIMD_RANK_FILTER_LANES) as u64 * channels as u64 * height as u64;
    let scalar_tail = 0u64;

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    let result =
        crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)?;
    Ok(preserve_mode(img, result))
}

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

pub(super) fn simd_float_order_statistic_filter(
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
        super::simd_rows_mut_serial!(
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
