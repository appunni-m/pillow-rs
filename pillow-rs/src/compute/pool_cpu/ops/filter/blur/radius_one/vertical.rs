use super::BOX_BLUR_BIAS;
/// Apply one L vertical Gaussian box pass with five replicated-edge taps.
/// The direct row-major form keeps every source row contiguous and avoids the
/// generic transposed-line accumulator for this common radius.
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
