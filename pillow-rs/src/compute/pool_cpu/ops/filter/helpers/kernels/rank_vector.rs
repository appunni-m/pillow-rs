//! Portable-wide kernels for exact native-byte rank windows.

use crate::raster::Mode;
use wide::u8x16;

const LANES: usize = 16;
const MAX_AREA: usize = 225;
const SORT_MAX_SIZE: usize = 5;

#[inline]
fn rank_filter_vector_pixels(remaining: usize) -> usize {
    remaining.min(LANES)
}

#[inline]
fn store_channel_vector(
    output: &mut [u8],
    channels: usize,
    channel: usize,
    x_start: usize,
    active: usize,
    values: u8x16,
) {
    for (lane, value) in values.to_array().into_iter().enumerate().take(active) {
        output[(x_start + lane) * channels + channel] = value;
    }
}

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
fn select_order_statistic_vectors(values: &[u8x16], rank: u8) -> u8x16 {
    let mut lower = u8x16::splat(0);
    let mut upper = u8x16::splat(u8::MAX);
    let rank = u8x16::splat(rank);
    let one = u8x16::splat(1);
    for _ in 0..u8::BITS {
        let midpoint = lower + ((upper - lower) >> 1u32);
        let mut count = u8x16::splat(0);
        for &value in values {
            count += value
                .simd_le(midpoint)
                .select(one, u8x16::splat(0));
        }
        let above_rank = count.simd_gt(rank);
        upper = above_rank.select(midpoint, upper);
        lower = above_rank.select(lower, midpoint + one);
    }
    lower
}

#[inline]
fn select_second_order_statistic_vector(values: &[u8x16]) -> u8x16 {
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

#[inline]
fn median_of_nine_vectors(values: &mut [u8x16]) -> u8x16 {
    compare_exchange(values, 1, 2);
    compare_exchange(values, 4, 5);
    compare_exchange(values, 7, 8);
    compare_exchange(values, 0, 1);
    compare_exchange(values, 3, 4);
    compare_exchange(values, 6, 7);
    compare_exchange(values, 0, 3);
    compare_exchange(values, 1, 2);
    compare_exchange(values, 4, 5);
    compare_exchange(values, 7, 8);
    compare_exchange(values, 3, 6);
    compare_exchange(values, 4, 7);
    compare_exchange(values, 5, 8);
    compare_exchange(values, 1, 4);
    compare_exchange(values, 2, 5);
    compare_exchange(values, 4, 7);
    compare_exchange(values, 4, 2);
    compare_exchange(values, 6, 4);
    compare_exchange(values, 4, 2);
    values[4]
}

#[inline(always)]
fn compare_exchange(values: &mut [u8x16], left: usize, right: usize) {
    let a = values[left];
    let b = values[right];
    values[left] = a.min(b);
    values[right] = a.max(b);
}

#[inline]
fn rank_filter_window_vector(
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
    let mut values = [0u8; LANES];
    for (lane, value) in values.iter_mut().enumerate().take(active) {
        let source_x = x_start
            .saturating_add(lane)
            .saturating_add(column_offset)
            .saturating_sub(half)
            .min(width - 1);
        *value = raw[row_start + source_x * channels + channel];
    }
    u8x16::new(values)
}

fn rank_filter_order_statistic_row(
    raw: &[u8],
    output: &mut [u8],
    row_stride: usize,
    width: usize,
    height: usize,
    channels: usize,
    mode: Mode,
    size: usize,
    half: usize,
    area: usize,
    rank: usize,
    y: usize,
) {
    if channels == 1 && size == 3 && matches!(rank, 1 | 4) {
        rank_filter_one_3x3_row(raw, output, row_stride, width, height, y, rank);
        return;
    }
    if mode == Mode::ImageLumaA8 && size == 3 && matches!(rank, 1 | 4) {
        rank_filter_la_3x3_row(raw, output, row_stride, width, height, y, rank);
        return;
    }
    if channels == 3 && size == 3 && rank == 4 {
        rank_filter_rgb_median_3x3_row(raw, output, row_stride, width, height, y);
        return;
    }
    rank_filter_generic_row(
        raw, output, row_stride, width, height, channels, size, half, area, rank, y,
    );
}

fn rank_filter_generic_row(
    raw: &[u8],
    output: &mut [u8],
    row_stride: usize,
    width: usize,
    height: usize,
    channels: usize,
    size: usize,
    half: usize,
    area: usize,
    rank: usize,
    y: usize,
) {
    for channel in 0..channels {
        let mut x = 0usize;
        while x < width {
            let active = rank_filter_vector_pixels(width - x);
            let mut values = [u8x16::splat(0); MAX_AREA];
            gather_order_statistic_window(
                raw,
                &mut values[..area],
                row_stride,
                width,
                height,
                channels,
                channel,
                x,
                active,
                size,
                half,
                y,
            );
            let selected = if size <= SORT_MAX_SIZE {
                sort_order_statistic_vectors(&mut values[..area]);
                values[rank]
            } else {
                select_order_statistic_vectors(&values[..area], rank as u8)
            };
            store_channel_vector(output, channels, channel, x, active, selected);
            x += active;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn gather_order_statistic_window(
    raw: &[u8],
    output: &mut [u8x16],
    row_stride: usize,
    width: usize,
    height: usize,
    channels: usize,
    channel: usize,
    x: usize,
    active: usize,
    size: usize,
    half: usize,
    y: usize,
) {
    let mut value_index = 0usize;
    for row_offset in 0..size {
        let source_y = y
            .saturating_add(row_offset)
            .saturating_sub(half)
            .min(height - 1);
        let row_start = source_y * row_stride;
        for column_offset in 0..size {
            output[value_index] = rank_filter_window_vector(
                raw,
                row_start,
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
}

fn rank_filter_one_3x3_row(
    raw: &[u8],
    output: &mut [u8],
    row_stride: usize,
    width: usize,
    height: usize,
    y: usize,
    rank: usize,
) {
    let mut x = 0usize;
    while x < width {
        let active = rank_filter_vector_pixels(width - x);
        let mut values = [u8x16::splat(0); 9];
        gather_one_3x3(raw, &mut values, row_stride, width, height, x, active, y);
        let selected = if rank == 1 {
            select_second_order_statistic_vector(&values)
        } else {
            median_of_nine_vectors(&mut values)
        };
        store_channel_vector(output, 1, 0, x, active, selected);
        x += active;
    }
}

fn gather_one_3x3(
    raw: &[u8],
    values: &mut [u8x16; 9],
    row_stride: usize,
    width: usize,
    height: usize,
    x: usize,
    active: usize,
    y: usize,
) {
    let mut index = 0usize;
    for row_offset in 0..3 {
        let source_y = y.saturating_add(row_offset).saturating_sub(1).min(height - 1);
        let row_start = source_y * row_stride;
        for column_offset in 0..3 {
            values[index] = rank_filter_window_vector(
                raw, row_start, width, 1, 0, x, active, 1, column_offset,
            );
            index += 1;
        }
    }
}

fn rank_filter_la_3x3_row(
    raw: &[u8],
    output: &mut [u8],
    row_stride: usize,
    width: usize,
    height: usize,
    y: usize,
    rank: usize,
) {
    let row_starts = [
        y.saturating_sub(1) * row_stride,
        y * row_stride,
        y.saturating_add(1).min(height - 1) * row_stride,
    ];
    let mut x = 0usize;
    while x < width {
        let active_pixels = (width - x).min(LANES / 2);
        let mut values = [u8x16::splat(u8::MAX); 9];
        gather_la_3x3(raw, row_stride, width, x, active_pixels, row_starts, &mut values);
        let selected = if rank == 1 {
            select_second_order_statistic_vector(&values)
        } else {
            median_of_nine_vectors(&mut values)
        }
        .to_array();
        let active_bytes = active_pixels * 2;
        let output_start = x * 2;
        output[output_start..output_start + active_bytes]
            .copy_from_slice(&selected[..active_bytes]);
        x += active_pixels;
    }
}

fn gather_la_3x3(
    raw: &[u8],
    row_stride: usize,
    width: usize,
    x: usize,
    active_pixels: usize,
    row_starts: [usize; 3],
    values: &mut [u8x16; 9],
) {
    for (row_index, row_start) in row_starts.into_iter().enumerate() {
        let source_row = &raw[row_start..row_start + row_stride];
        let columns = gather_la_row_columns(source_row, width, x, active_pixels);
        values[row_index * 3..row_index * 3 + 3].copy_from_slice(&columns);
    }
}

#[inline(always)]
fn gather_la_row_columns(
    source_row: &[u8],
    width: usize,
    x: usize,
    active_pixels: usize,
) -> [u8x16; 3] {
    let pixels_per_vector = LANES / 2;
    if active_pixels == pixels_per_vector && x > 0 && x + active_pixels < width {
        let left_start = (x - 1) * 2;
        return [
            load_u8x16(source_row, left_start),
            load_u8x16(source_row, left_start + 2),
            load_u8x16(source_row, left_start + 4),
        ];
    }
    let mut columns = [[u8::MAX; LANES]; 3];
    for lane in 0..active_pixels {
        let pixel_x = x + lane;
        let neighbors = [pixel_x.saturating_sub(1), pixel_x, pixel_x.saturating_add(1).min(width - 1)];
        for (column, source_x) in neighbors.into_iter().enumerate() {
            let source = source_x * 2;
            columns[column][lane * 2] = source_row[source];
            columns[column][lane * 2 + 1] = source_row[source + 1];
        }
    }
    columns.map(u8x16::new)
}

fn rank_filter_rgb_median_3x3_row(
    raw: &[u8],
    output: &mut [u8],
    row_stride: usize,
    width: usize,
    height: usize,
    y: usize,
) {
    for channel in 0..3 {
        let mut x = 0usize;
        while x < width {
            let active = rank_filter_vector_pixels(width - x);
            let top = y.saturating_sub(1) * row_stride;
            let middle = y * row_stride;
            let bottom = y.saturating_add(1).min(height - 1) * row_stride;
            let rows = [top, middle, bottom];
            let mut values = [u8x16::splat(0); 9];
            gather_rgb_3x3(raw, width, channel, x, active, rows, &mut values);
            let selected = median_of_nine_vectors(&mut values);
            store_channel_vector(output, 3, channel, x, active, selected);
            x += active;
        }
    }
}

fn gather_rgb_3x3(
    raw: &[u8],
    width: usize,
    channel: usize,
    x: usize,
    active: usize,
    rows: [usize; 3],
    values: &mut [u8x16; 9],
) {
    for (row_index, row_start) in rows.into_iter().enumerate() {
        values[row_index * 3..row_index * 3 + 3].copy_from_slice(&gather_rgb_row_columns(
            raw, row_start, width, channel, x, active,
        ));
    }
}

/// Gather all three RGB columns in one lane walk, as in the archived SIMD
/// kernel, so the clamped source coordinate is calculated once per pixel.
#[inline(always)]
fn gather_rgb_row_columns(
    raw: &[u8],
    row_start: usize,
    width: usize,
    channel: usize,
    x: usize,
    active: usize,
) -> [u8x16; 3] {
    let mut left = [0u8; LANES];
    let mut center = [0u8; LANES];
    let mut right = [0u8; LANES];
    for (lane, ((left_value, center_value), right_value)) in left
        .iter_mut()
        .zip(center.iter_mut())
        .zip(right.iter_mut())
        .enumerate()
        .take(active)
    {
        let source_x = x + lane;
        let center_offset = source_x * 3 + channel;
        let left_offset = source_x.saturating_sub(1) * 3 + channel;
        let right_offset = source_x.saturating_add(1).min(width - 1) * 3 + channel;
        *left_value = raw[row_start + left_offset];
        *center_value = raw[row_start + center_offset];
        *right_value = raw[row_start + right_offset];
    }
    [u8x16::new(left), u8x16::new(center), u8x16::new(right)]
}

#[inline(always)]
fn load_u8x16(bytes: &[u8], start: usize) -> u8x16 {
    let mut lanes = [0u8; LANES];
    lanes.copy_from_slice(&bytes[start..start + LANES]);
    u8x16::new(lanes)
}

/// Execute one row after the exact-mode adapter validates dimensions and rank.
pub(super) fn execute_row(
    raw: &[u8],
    output: &mut [u8],
    row_stride: usize,
    width: usize,
    height: usize,
    channels: usize,
    mode: Mode,
    size: usize,
    half: usize,
    area: usize,
    rank: usize,
    y: usize,
) {
    rank_filter_order_statistic_row(
        raw, output, row_stride, width, height, channels, mode, size, half, area, rank, y,
    );
}
