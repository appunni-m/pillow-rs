use super::BOX_BLUR_BIAS;
/// Apply one L vertical Gaussian box pass with five replicated-edge taps.
/// The direct row-major form keeps every source row contiguous and avoids the
/// generic transposed-line accumulator for this common radius.
#[cfg(not(feature = "parallel"))]
pub(in crate::compute::pool_cpu::ops::filter::blur) fn blur_luma_vertical_radius_one_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    debug_assert_eq!(source.len(), width * height);
    debug_assert_eq!(destination.len(), source.len());
    if width == 0 || height == 0 {
        return;
    }

    let last_y = height - 1;
    for y in 0..height {
        let top_two = y.saturating_sub(2) * width;
        let top_one = y.saturating_sub(1) * width;
        let center = y * width;
        let bottom_one = y.saturating_add(1).min(last_y) * width;
        let bottom_two = y.saturating_add(2).min(last_y) * width;
        let top_two_row = &source[top_two..top_two + width];
        let top_one_row = &source[top_one..top_one + width];
        let center_row = &source[center..center + width];
        let bottom_one_row = &source[bottom_one..bottom_one + width];
        let bottom_two_row = &source[bottom_two..bottom_two + width];
        let destination_row = &mut destination[center..center + width];

        if fractional_weight == 0 {
            for (((output, &top), &middle), &bottom) in destination_row
                .iter_mut()
                .zip(top_one_row)
                .zip(center_row)
                .zip(bottom_one_row)
            {
                let neighbors = u32::from(top) + u32::from(middle) + u32::from(bottom);
                let weighted = neighbors
                    .wrapping_mul(whole_weight)
                    .wrapping_add(BOX_BLUR_BIAS);
                *output = (weighted >> 24) as u8;
            }
        } else {
            for (((((output, &far_top), &top), &middle), &bottom), &far_bottom) in destination_row
                .iter_mut()
                .zip(top_two_row)
                .zip(top_one_row)
                .zip(center_row)
                .zip(bottom_one_row)
                .zip(bottom_two_row)
            {
                let neighbors = u32::from(top) + u32::from(middle) + u32::from(bottom);
                let far_edges = u32::from(far_top) + u32::from(far_bottom);
                let weighted = neighbors
                    .wrapping_mul(whole_weight)
                    .wrapping_add(far_edges.wrapping_mul(fractional_weight))
                    .wrapping_add(BOX_BLUR_BIAS);
                *output = (weighted >> 24) as u8;
            }
        }
    }
}

/// Apply one interleaved vertical Gaussian box pass with five replicated-edge taps.
#[cfg(not(feature = "parallel"))]
pub(in crate::compute::pool_cpu::ops::filter::blur) fn blur_interleaved_vertical_radius_one_fractional_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let row_stride = width * channels;
    debug_assert_eq!(source.len(), row_stride * height);
    debug_assert_eq!(destination.len(), source.len());
    debug_assert_ne!(fractional_weight, 0);
    if width == 0 || height == 0 {
        return;
    }

    let last_y = height - 1;
    for y in 0..height {
        let far_top = y.saturating_sub(2) * row_stride;
        let top = y.saturating_sub(1) * row_stride;
        let center = y * row_stride;
        let bottom = y.saturating_add(1).min(last_y) * row_stride;
        let far_bottom = y.saturating_add(2).min(last_y) * row_stride;
        let far_top_row = &source[far_top..far_top + row_stride];
        let top_row = &source[top..top + row_stride];
        let center_row = &source[center..center + row_stride];
        let bottom_row = &source[bottom..bottom + row_stride];
        let far_bottom_row = &source[far_bottom..far_bottom + row_stride];
        let destination_row = &mut destination[center..center + row_stride];

        for (((((output, &far_top), &top), &center), &bottom), &far_bottom) in destination_row
            .iter_mut()
            .zip(far_top_row)
            .zip(top_row)
            .zip(center_row)
            .zip(bottom_row)
            .zip(far_bottom_row)
        {
            let whole = u32::from(top) + u32::from(center) + u32::from(bottom);
            let fractional = u32::from(far_top) + u32::from(far_bottom);
            let weighted = whole
                .wrapping_mul(whole_weight)
                .wrapping_add(fractional.wrapping_mul(fractional_weight))
                .wrapping_add(BOX_BLUR_BIAS);
            *output = (weighted >> 24) as u8;
        }
    }
}

#[cfg(feature = "parallel")]
pub(in crate::compute::pool_cpu::ops::filter::blur) const VERTICAL_BLUR_TRANSPOSE_THRESHOLD: usize =
    512 * 512;

#[cfg(feature = "parallel")]
pub(in crate::compute::pool_cpu::ops::filter::blur) fn transpose_interleaved_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
) {
    let source_row_stride = width * channels;
    let destination_row_stride = height * channels;
    crate::par_rows_mut!(
        destination,
        destination_row_stride,
        width,
        |row_start, row_end, x, row| {
            let _ = (row_start, row_end);
            let x = x as usize;
            for y in 0..height {
                let source_start = y * source_row_stride + x * channels;
                let destination_start = y * channels;
                row[destination_start..destination_start + channels]
                    .copy_from_slice(&source[source_start..source_start + channels]);
            }
        }
    );
}

#[cfg(feature = "parallel")]
pub(in crate::compute::pool_cpu::ops::filter::blur) fn blur_columns_radius_one_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    whole_weight: u32,
) {
    let row_stride = width * channels;
    crate::par_rows_mut!(
        destination,
        row_stride,
        height,
        |_row_start, _row_end, y, row| {
            let y = y as usize;
            let above = y.saturating_sub(1) * row_stride;
            let center = y * row_stride;
            let below = y.saturating_add(1).min(height - 1) * row_stride;
            for (index, output) in row.iter_mut().enumerate() {
                let sum = u32::from(source[above + index])
                    + u32::from(source[center + index])
                    + u32::from(source[below + index]);
                *output = (sum.wrapping_mul(whole_weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
            }
        }
    );
}

#[cfg(all(test, feature = "parallel"))]
mod parallel_radius_one_vertical_tests {
    use super::super::super::{blur_parameters, line::blur_columns};
    use super::blur_columns_radius_one_rows;

    #[test]
    fn row_parallel_radius_one_matches_fixed_point_column_blur() {
        let (radius, whole_weight, fractional_weight) = blur_parameters(1.0);
        assert_eq!(radius, 1);
        assert_eq!(fractional_weight, 0);

        for (width, height) in [(1, 1), (1, 7), (2, 3), (17, 9), (65, 33)] {
            for channels in [1, 2, 3, 4] {
                let len = width * height * channels;
                let source = (0..len)
                    .map(|index| ((index * 73 + index / 11 * 29 + 37) % 256) as u8)
                    .collect::<Vec<_>>();
                let mut actual = vec![0; len];
                let mut expected = vec![0; len];

                blur_columns_radius_one_rows(
                    &source,
                    &mut actual,
                    width,
                    height,
                    channels,
                    whole_weight,
                );
                blur_columns(
                    &source,
                    &mut expected,
                    width,
                    height,
                    channels,
                    radius,
                    whole_weight,
                    fractional_weight,
                    &mut vec![0; width * channels],
                );

                assert_eq!(actual, expected, "{width}x{height}, channels={channels}");
            }
        }
    }
}
