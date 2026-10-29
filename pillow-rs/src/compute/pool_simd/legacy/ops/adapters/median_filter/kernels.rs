//! Lane-wise SIMD kernels for fixed-size median windows.

use super::super::{SIMD_BYTE_VECTOR_LANES, u8x16};

#[inline(always)]
fn compare_exchange(values: &mut [u8x16], left: usize, right: usize) {
    let a = values[left];
    let b = values[right];
    values[left] = a.min(b);
    values[right] = a.max(b);
}

/// Select the median of nine vector values with a 19-pair selection network.
///
/// Each compare-exchange applies lane-wise vector min/max, so all sixteen
/// independent RGB component positions are processed together.
#[inline]
pub(super) fn median_of_nine_vectors(values: &mut [u8x16]) -> u8x16 {
    debug_assert_eq!(values.len(), 9);
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

/// Apply the native RGB 3x3 median using only its nine gathered SIMD vectors.
///
/// This avoids zero-initializing the generic 225-vector order-statistic
/// scratch buffer. It also gathers left, center, and right columns together
/// for each source row, so clamped pixel coordinates are calculated once per
/// lane instead of once for each of the three samples.
#[allow(clippy::too_many_arguments)]
pub(super) fn median_filter_rgb_3x3_row(
    raw: &[u8],
    output_row: &mut [u8],
    row_stride: usize,
    width: usize,
    height: usize,
    y: usize,
) {
    for channel in 0..3 {
        let mut x = 0usize;
        while x < width {
            let active = super::rank_filter_vector_pixels(width - x);
            let top = y.saturating_sub(1) * row_stride;
            let middle = y * row_stride;
            let bottom = y.saturating_add(1).min(height - 1) * row_stride;
            let [top_left, top_center, top_right] =
                gather_rgb_row_columns(raw, top, width, channel, x, active);
            let [middle_left, middle_center, middle_right] =
                gather_rgb_row_columns(raw, middle, width, channel, x, active);
            let [bottom_left, bottom_center, bottom_right] =
                gather_rgb_row_columns(raw, bottom, width, channel, x, active);
            let mut values = [
                top_left,
                top_center,
                top_right,
                middle_left,
                middle_center,
                middle_right,
                bottom_left,
                bottom_center,
                bottom_right,
            ];
            let selected = median_of_nine_vectors(&mut values);
            super::store_rank_filter_channel_vector(output_row, 0, 3, channel, x, active, selected);
            x += active;
        }
    }
}

/// Apply a 3×3 two-channel order statistic with both channels in each vector.
///
/// Eight interleaved LA pixels fill one 16-byte vector. Interior blocks load
/// adjacent pixels contiguously; borders and the final partial block preserve
/// replicated-edge behavior with scalar gathers. Each lane remains on the
/// same channel across all nine window vectors.
pub(super) fn la_order_statistic_3x3_row<const RANK: usize>(
    raw: &[u8],
    output_row: &mut [u8],
    row_stride: usize,
    width: usize,
    height: usize,
    y: usize,
) {
    let top = y.saturating_sub(1) * row_stride;
    let middle = y * row_stride;
    let bottom = y.saturating_add(1).min(height - 1) * row_stride;
    let row_starts = [top, middle, bottom];
    let mut x = 0usize;

    while x < width {
        let active_pixels = (width - x).min(SIMD_BYTE_VECTOR_LANES / 2);
        let mut values = [u8x16::splat(u8::MAX); 9];
        for (row_index, row_start) in row_starts.into_iter().enumerate() {
            let source_row = &raw[row_start..row_start + row_stride];
            let columns = gather_la_row_columns(source_row, width, x, active_pixels);
            values[row_index * 3..row_index * 3 + 3].copy_from_slice(&columns);
        }

        let selected = if RANK == 1 {
            super::select_second_order_statistic_vector(&values)
        } else {
            median_of_nine_vectors(&mut values)
        };
        let selected = selected.to_array();
        let active_bytes = active_pixels * 2;
        let output_start = x * 2;
        output_row[output_start..output_start + active_bytes]
            .copy_from_slice(&selected[..active_bytes]);
        x += active_pixels;
    }
}

#[inline(always)]
fn gather_la_row_columns(
    source_row: &[u8],
    width: usize,
    x_start: usize,
    active_pixels: usize,
) -> [u8x16; 3] {
    let pixels_per_vector = SIMD_BYTE_VECTOR_LANES / 2;
    if active_pixels == pixels_per_vector && x_start > 0 && x_start + active_pixels < width {
        let left_start = (x_start - 1) * 2;
        return [
            super::super::simd_blur_load_u8x16(source_row, left_start),
            super::super::simd_blur_load_u8x16(source_row, left_start + 2),
            super::super::simd_blur_load_u8x16(source_row, left_start + 4),
        ];
    }

    let mut columns = [[u8::MAX; SIMD_BYTE_VECTOR_LANES]; 3];
    for lane in 0..active_pixels {
        let pixel_x = x_start + lane;
        let neighbors = [
            pixel_x.saturating_sub(1),
            pixel_x,
            pixel_x.saturating_add(1).min(width - 1),
        ];
        for (column, source_x) in neighbors.into_iter().enumerate() {
            let source = source_x * 2;
            columns[column][lane * 2] = source_row[source];
            columns[column][lane * 2 + 1] = source_row[source + 1];
        }
    }
    [
        u8x16::new(columns[0]),
        u8x16::new(columns[1]),
        u8x16::new(columns[2]),
    ]
}

/// Gather the three adjacent RGB columns in one lane walk. The interleaved
/// layout still requires scalar loads, but sharing each clamped x coordinate
/// avoids repeating the lane loop for left, center, and right vectors.
#[inline(always)]
fn gather_rgb_row_columns(
    raw: &[u8],
    row_start: usize,
    width: usize,
    channel: usize,
    x: usize,
    active: usize,
) -> [u8x16; 3] {
    let mut left = [0u8; 16];
    let mut center = [0u8; 16];
    let mut right = [0u8; 16];
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

#[cfg(test)]
mod tests {
    use super::super::super::u8x16;
    use super::median_of_nine_vectors;

    #[test]
    fn median_network_matches_full_sort_with_ties_and_replicated_edges() {
        let mut state = 0x6d2b_79f5u32;
        for case in 0..4096u32 {
            let mut samples = [u8x16::splat(0); 9];
            let mut expected = [[0u8; 16]; 9];
            for (index, sample) in samples.iter_mut().enumerate() {
                let mut lanes = [0u8; 16];
                for (lane, value) in lanes.iter_mut().enumerate() {
                    state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    // A small domain deliberately exercises repeated samples,
                    // as replicated image borders do in real windows.
                    *value = ((state >> 16) % 7) as u8;
                    expected[index][lane] = *value;
                }
                *sample = u8x16::new(lanes);
            }

            // Include all-equal, ascending, and descending lane vectors in
            // addition to the deterministic repeated-value sequence.
            if case == 0 {
                samples.fill(u8x16::splat(91));
                expected.fill([91; 16]);
            } else if case == 1 || case == 2 {
                for index in 0..9 {
                    let value = if case == 1 {
                        index as u8
                    } else {
                        (8 - index) as u8
                    };
                    samples[index] = u8x16::splat(value);
                    expected[index] = [value; 16];
                }
            }

            let actual = median_of_nine_vectors(&mut samples).to_array();
            for lane in 0..16 {
                let mut sorted = expected.map(|sample| sample[lane]);
                sorted.sort_unstable();
                assert_eq!(actual[lane], sorted[4], "case={case} lane={lane}");
            }
        }
    }
}
