//! SIMD MaxFilter and MinFilter family adapter and native-byte kernels.

use super::super::{
    SIMD_RANK_FILTER_LANES, native_copy_image_bytes, native_filter_byte_layout,
    native_small_uniform_byte_image, native_zero_byte_image, rank_filter_vector_pixels,
    simd_unsupported, store_rank_filter_channel_vector, u8x16,
};
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

#[inline]
fn rank_filter_identity(select_max: bool) -> u8 {
    if select_max { 0 } else { u8::MAX }
}

/// Load one clamped horizontal window into the low lanes of a byte vector.
/// Coordinate arithmetic and interleaved-channel addressing are scalar
/// control; the window comparison itself is performed by the vector kernel.
#[inline]
fn rank_filter_horizontal_window_vector(
    raw: &[u8],
    row_start: usize,
    width: usize,
    channels: usize,
    channel: usize,
    x_start: usize,
    active: usize,
    half: usize,
    window_offset: usize,
    select_max: bool,
) -> u8x16 {
    let mut values = [rank_filter_identity(select_max); SIMD_RANK_FILTER_LANES];
    let last = width - 1;
    for (lane, value) in values.iter_mut().enumerate().take(active) {
        let source_x = x_start
            .saturating_add(lane)
            .saturating_add(window_offset)
            .saturating_sub(half)
            .min(last);
        *value = raw[row_start + source_x * channels + channel];
    }
    u8x16::new(values)
}

/// Load one clamped vertical window from the horizontal intermediate.
#[inline]
fn rank_filter_vertical_window_vector(
    horizontal: &[u8],
    row_stride: usize,
    width: usize,
    height: usize,
    channels: usize,
    channel: usize,
    x_start: usize,
    active: usize,
    y: usize,
    half: usize,
    window_offset: usize,
    select_max: bool,
) -> u8x16 {
    let mut values = [rank_filter_identity(select_max); SIMD_RANK_FILTER_LANES];
    let last = height - 1;
    let source_y = y
        .saturating_add(window_offset)
        .saturating_sub(half)
        .min(last);
    let row_start = source_y * row_stride;
    for (lane, value) in values.iter_mut().enumerate().take(active) {
        let source_x = x_start + lane;
        debug_assert!(source_x < width);
        *value = horizontal[row_start + source_x * channels + channel];
    }
    u8x16::new(values)
}

#[inline]
fn rank_filter_horizontal_scalar(
    raw: &[u8],
    row_start: usize,
    width: usize,
    channels: usize,
    channel: usize,
    x: usize,
    half: usize,
    select_max: bool,
) -> u8 {
    let last = width - 1;
    let mut selected = rank_filter_identity(select_max);
    let window_len = half.saturating_mul(2).saturating_add(1);
    for window_offset in 0..window_len {
        let source_x = x
            .saturating_add(window_offset)
            .saturating_sub(half)
            .min(last);
        let value = raw[row_start + source_x * channels + channel];
        selected = if select_max {
            selected.max(value)
        } else {
            selected.min(value)
        };
    }
    selected
}

#[inline]
fn rank_filter_vertical_scalar(
    horizontal: &[u8],
    row_stride: usize,
    height: usize,
    channels: usize,
    channel: usize,
    x: usize,
    y: usize,
    half: usize,
    select_max: bool,
) -> u8 {
    let last = height - 1;
    let mut selected = rank_filter_identity(select_max);
    let window_len = half.saturating_mul(2).saturating_add(1);
    for window_offset in 0..window_len {
        let source_y = y
            .saturating_add(window_offset)
            .saturating_sub(half)
            .min(last);
        let value = horizontal[source_y * row_stride + x * channels + channel];
        selected = if select_max {
            selected.max(value)
        } else {
            selected.min(value)
        };
    }
    selected
}

/// Process one row of the horizontal native-byte extrema pass.  The source
/// row is immutable while the destination row is exclusively owned by the
/// caller, which lets the outer pass use row-exclusive output without changing the
/// per-pixel clamp or vector reduction order.
fn rank_filter_horizontal_vectorized_row(
    raw: &[u8],
    horizontal_row: &mut [u8],
    row_start: usize,
    width: usize,
    channels: usize,
    half: usize,
    select_max: bool,
) {
    let effective_half = half.min(width - 1);
    let window_len = effective_half.saturating_mul(2).saturating_add(1);
    for channel in 0..channels {
        let mut x = 0usize;
        while x < width {
            let active = rank_filter_vector_pixels(width - x);
            if active == 0 {
                for tail_x in x..width {
                    horizontal_row[tail_x * channels + channel] = rank_filter_horizontal_scalar(
                        raw,
                        row_start,
                        width,
                        channels,
                        channel,
                        tail_x,
                        effective_half,
                        select_max,
                    );
                }
                break;
            }
            let mut selected = u8x16::splat(rank_filter_identity(select_max));
            for window_offset in 0..window_len {
                let values = rank_filter_horizontal_window_vector(
                    raw,
                    row_start,
                    width,
                    channels,
                    channel,
                    x,
                    active,
                    effective_half,
                    window_offset,
                    select_max,
                );
                selected = if select_max {
                    selected.max(values)
                } else {
                    selected.min(values)
                };
            }
            store_rank_filter_channel_vector(
                horizontal_row,
                0,
                channels,
                channel,
                x,
                active,
                selected,
            );
            x += active;
        }
    }
}

/// Horizontal native-byte extrema pass. It intentionally keeps all source
/// samples in their Pillow layout and vectorizes comparisons across output
/// pixels, while clamped edge coordinates remain scalar control. Rows are
/// independent and retain separate row writes. The SIMD adapter traverses
/// them serially even when the Parallel CPU feature is enabled.
fn rank_filter_horizontal_vectorized(
    raw: &[u8],
    horizontal: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    half: usize,
    select_max: bool,
) -> (u64, u64) {
    let row_stride = width * channels;
    #[cfg(feature = "parallel")]
    super::super::simd_rows_mut_serial!(
        horizontal,
        row_stride,
        height,
        |_row_start, _row_end, y, row| {
            rank_filter_horizontal_vectorized_row(
                raw,
                row,
                (y as usize) * row_stride,
                width,
                channels,
                half,
                select_max,
            );
        }
    );
    #[cfg(not(feature = "parallel"))]
    for y in 0..height {
        let row_start = y * row_stride;
        let row_end = row_start + row_stride;
        rank_filter_horizontal_vectorized_row(
            raw,
            &mut horizontal[row_start..row_end],
            row_start,
            width,
            channels,
            half,
            select_max,
        );
    }
    let vector_blocks =
        width.div_ceil(SIMD_RANK_FILTER_LANES) as u64 * channels as u64 * height as u64;
    (vector_blocks, 0)
}

/// Process one row of the vertical native-byte extrema pass over the
/// horizontal intermediate.
fn rank_filter_vertical_vectorized_row(
    horizontal: &[u8],
    output_row: &mut [u8],
    row_stride: usize,
    width: usize,
    height: usize,
    channels: usize,
    half: usize,
    select_max: bool,
    y: usize,
) {
    let effective_half = half.min(height - 1);
    let window_len = effective_half.saturating_mul(2).saturating_add(1);
    for channel in 0..channels {
        let mut x = 0usize;
        while x < width {
            let active = rank_filter_vector_pixels(width - x);
            if active == 0 {
                for tail_x in x..width {
                    output_row[tail_x * channels + channel] = rank_filter_vertical_scalar(
                        horizontal,
                        row_stride,
                        height,
                        channels,
                        channel,
                        tail_x,
                        y,
                        effective_half,
                        select_max,
                    );
                }
                break;
            }
            let mut selected = u8x16::splat(rank_filter_identity(select_max));
            for window_offset in 0..window_len {
                let values = rank_filter_vertical_window_vector(
                    horizontal,
                    row_stride,
                    width,
                    height,
                    channels,
                    channel,
                    x,
                    active,
                    y,
                    effective_half,
                    window_offset,
                    select_max,
                );
                selected = if select_max {
                    selected.max(values)
                } else {
                    selected.min(values)
                };
            }
            store_rank_filter_channel_vector(output_row, 0, channels, channel, x, active, selected);
            x += active;
        }
    }
}

/// Vertical native-byte extrema pass over the horizontal intermediate.
fn rank_filter_vertical_vectorized(
    horizontal: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    half: usize,
    select_max: bool,
) -> (u64, u64) {
    let row_stride = width * channels;
    #[cfg(feature = "parallel")]
    super::super::simd_rows_mut_serial!(
        output,
        row_stride,
        height,
        |_row_start, _row_end, y, row| {
            rank_filter_vertical_vectorized_row(
                horizontal, row, row_stride, width, height, channels, half, select_max, y as usize,
            );
        }
    );
    #[cfg(not(feature = "parallel"))]
    for y in 0..height {
        let row_start = y * row_stride;
        let row_end = row_start + row_stride;
        rank_filter_vertical_vectorized_row(
            horizontal,
            &mut output[row_start..row_end],
            row_stride,
            width,
            height,
            channels,
            half,
            select_max,
            y,
        );
    }
    let vector_blocks =
        width.div_ceil(SIMD_RANK_FILTER_LANES) as u64 * channels as u64 * height as u64;
    (vector_blocks, 0)
}

/// Execute a native-byte MaxFilter or MinFilter.
///
/// The extrema are separable, so the scalar border/index work is performed in
/// two passes and every complete group of eight or sixteen output pixels uses
/// `u8x16` min/max instructions. This is a data-plane SIMD implementation,
/// not a call into the packed CPU rank-filter implementation.
fn simd_extreme_filter(
    img: &DynamicImage,
    size: u32,
    mode: Option<&str>,
    select_max: bool,
) -> Result<DynamicImage, PilError> {
    let channels = native_filter_byte_layout(img, mode)
        .ok_or_else(|| simd_unsupported(if select_max { "MaxFilter" } else { "MinFilter" }))?;
    if size == 0 || size % 2 == 0 || img.width() == 0 || img.height() == 0 {
        return Err(simd_unsupported(if select_max {
            "MaxFilter"
        } else {
            "MinFilter"
        }));
    }
    let width = img.width() as usize;
    let height = img.height() as usize;
    let expected_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("SIMD rank-filter dimensions overflow".into()))?;
    let raw = img.as_bytes();
    if raw.len() != expected_len {
        return Err(PilError::InternalError(
            "SIMD rank-filter source buffer shape mismatch".into(),
        ));
    }
    if native_small_uniform_byte_image(img, channels) {
        return native_copy_image_bytes(img, mode)?
            .ok_or_else(|| simd_unsupported(if select_max { "MaxFilter" } else { "MinFilter" }));
    }
    if native_zero_byte_image(img, channels) {
        return native_copy_image_bytes(img, mode)?
            .ok_or_else(|| simd_unsupported(if select_max { "MaxFilter" } else { "MinFilter" }));
    }
    let half = (size / 2) as usize;
    let mut horizontal = vec![0u8; expected_len];
    let mut output = vec![0u8; expected_len];
    let (horizontal_blocks, horizontal_tail) = rank_filter_horizontal_vectorized(
        raw,
        &mut horizontal,
        width,
        height,
        channels,
        half,
        select_max,
    );
    let (vertical_blocks, vertical_tail) = rank_filter_vertical_vectorized(
        &horizontal,
        &mut output,
        width,
        height,
        channels,
        half,
        select_max,
    );
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(
        horizontal_blocks.saturating_add(vertical_blocks),
    );
    crate::compute::record_pipeline_operation_scalar_tail(
        horizontal_tail.saturating_add(vertical_tail),
    );
    let result =
        crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)?;
    Ok(preserve_mode(img, result))
}

/// Native SIMD adapter for Pillow's maximum filter.
pub fn simd_max_filter(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::MaxFilter { size } = op else {
        return Err(PilError::ValueError("expected MaxFilter op".into()));
    };
    if mode == Some("F") {
        return super::simd_float_order_statistic_filter(
            img,
            *size,
            size.saturating_mul(*size).saturating_sub(1),
            mode,
        );
    }
    simd_extreme_filter(img, *size, mode, true)
}

/// Native SIMD adapter for Pillow's minimum filter.
pub fn simd_min_filter(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::MinFilter { size } = op else {
        return Err(PilError::ValueError("expected MinFilter op".into()));
    };
    if mode == Some("F") {
        return super::simd_float_order_statistic_filter(img, *size, 0, mode);
    }
    simd_extreme_filter(img, *size, mode, false)
}
