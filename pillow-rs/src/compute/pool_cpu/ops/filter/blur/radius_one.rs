//! Mode-specific radius-one box-blur row and column paths.

use super::BOX_BLUR_BIAS;
#[cfg(not(feature = "parallel"))]
use super::line::blur_line;

#[cfg(not(feature = "parallel"))]
#[inline(always)]
fn store_radius_one_fractional_sample(
    destination: &mut u8,
    whole_sum: u32,
    far_sum: u32,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let bulk = whole_sum
        .wrapping_mul(whole_weight)
        .wrapping_add(far_sum.wrapping_mul(fractional_weight))
        .wrapping_add(BOX_BLUR_BIAS);
    *destination = (bulk >> 24) as u8;
}

/// Blur one L row with Pillow's radius-one fractional taps.
#[cfg(not(feature = "parallel"))]
pub(super) fn blur_luma_radius_one_fractional_line(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    debug_assert!(width > 0);
    debug_assert!(source.len() >= width);
    debug_assert!(destination.len() >= width);
    debug_assert_ne!(fractional_weight, 0);

    if width < 5 {
        let mut accumulator = [0u32; 4];
        blur_line(
            source,
            destination,
            0,
            width,
            1,
            1,
            whole_weight,
            fractional_weight,
            &mut accumulator,
        );
        return;
    }

    let first = u32::from(source[0]);
    store_radius_one_fractional_sample(
        &mut destination[0],
        first * 2 + u32::from(source[1]),
        first + u32::from(source[2]),
        whole_weight,
        fractional_weight,
    );
    store_radius_one_fractional_sample(
        &mut destination[1],
        first + u32::from(source[1]) + u32::from(source[2]),
        first + u32::from(source[3]),
        whole_weight,
        fractional_weight,
    );

    let interior_destination = &mut destination[2..width - 2];
    let far_left_samples = &source[..width - 4];
    let left_samples = &source[1..width - 3];
    let center_samples = &source[2..width - 2];
    let right_samples = &source[3..width - 1];
    let far_right_samples = &source[4..];
    for (((((output, &far_left), &left), &center), &right), &far_right) in interior_destination
        .iter_mut()
        .zip(far_left_samples)
        .zip(left_samples)
        .zip(center_samples)
        .zip(right_samples)
        .zip(far_right_samples)
    {
        let whole_sum = u32::from(left) + u32::from(center) + u32::from(right);
        let far_sum = u32::from(far_left) + u32::from(far_right);
        store_radius_one_fractional_sample(
            output,
            whole_sum,
            far_sum,
            whole_weight,
            fractional_weight,
        );
    }

    let last = width - 1;
    let penultimate = last - 1;
    store_radius_one_fractional_sample(
        &mut destination[penultimate],
        u32::from(source[penultimate - 1])
            + u32::from(source[penultimate])
            + u32::from(source[last]),
        u32::from(source[penultimate - 2]) + u32::from(source[last]),
        whole_weight,
        fractional_weight,
    );
    store_radius_one_fractional_sample(
        &mut destination[last],
        u32::from(source[last - 1]) + u32::from(source[last]) * 2,
        u32::from(source[last - 2]) + u32::from(source[last]),
        whole_weight,
        fractional_weight,
    );
}

/// Blur one RGB row for a radius-one fractional Gaussian box pass.
#[cfg(not(feature = "parallel"))]
pub(super) fn blur_rgb_radius_one_fractional_line(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    debug_assert!(width > 0);
    debug_assert!(source.len() >= width * 3);
    debug_assert!(destination.len() >= width * 3);
    debug_assert_ne!(fractional_weight, 0);

    if width < 5 {
        let mut accumulator = [0u32; 3];
        blur_line(
            source,
            destination,
            0,
            width,
            3,
            1,
            whole_weight,
            fractional_weight,
            &mut accumulator,
        );
        return;
    }

    for channel in 0..3 {
        let first = u32::from(source[channel]);
        store_radius_one_fractional_sample(
            &mut destination[channel],
            first * 2 + u32::from(source[3 + channel]),
            first + u32::from(source[6 + channel]),
            whole_weight,
            fractional_weight,
        );
        store_radius_one_fractional_sample(
            &mut destination[3 + channel],
            first + u32::from(source[3 + channel]) + u32::from(source[6 + channel]),
            first + u32::from(source[9 + channel]),
            whole_weight,
            fractional_weight,
        );
    }

    let interior_destination = &mut destination[6..(width - 2) * 3];
    let far_left_samples = &source[..(width - 4) * 3];
    let left_samples = &source[3..(width - 3) * 3];
    let center_samples = &source[6..(width - 2) * 3];
    let right_samples = &source[9..(width - 1) * 3];
    let far_right_samples = &source[12..];
    for (((((output, &far_left), &left), &center), &right), &far_right) in interior_destination
        .iter_mut()
        .zip(far_left_samples)
        .zip(left_samples)
        .zip(center_samples)
        .zip(right_samples)
        .zip(far_right_samples)
    {
        let whole_sum = u32::from(left) + u32::from(center) + u32::from(right);
        let far_sum = u32::from(far_left) + u32::from(far_right);
        store_radius_one_fractional_sample(
            output,
            whole_sum,
            far_sum,
            whole_weight,
            fractional_weight,
        );
    }

    let penultimate = (width - 2) * 3;
    let last = (width - 1) * 3;
    for channel in 0..3 {
        store_radius_one_fractional_sample(
            &mut destination[penultimate + channel],
            u32::from(source[penultimate - 3 + channel])
                + u32::from(source[penultimate + channel])
                + u32::from(source[last + channel]),
            u32::from(source[penultimate - 6 + channel]) + u32::from(source[last + channel]),
            whole_weight,
            fractional_weight,
        );
        store_radius_one_fractional_sample(
            &mut destination[last + channel],
            u32::from(source[penultimate + channel]) + u32::from(source[last + channel]) * 2,
            u32::from(source[penultimate - 3 + channel]) + u32::from(source[last + channel]),
            whole_weight,
            fractional_weight,
        );
    }
}

/// Blur one RGBA row for a radius-one fractional Gaussian box pass.
#[cfg(not(feature = "parallel"))]
pub(super) fn blur_rgba_radius_one_fractional_line(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    debug_assert!(width > 0);
    debug_assert!(source.len() >= width * 4);
    debug_assert!(destination.len() >= width * 4);
    debug_assert_ne!(fractional_weight, 0);

    if width < 5 {
        let mut accumulator = [0u32; 4];
        blur_line(
            source,
            destination,
            0,
            width,
            4,
            1,
            whole_weight,
            fractional_weight,
            &mut accumulator,
        );
        return;
    }

    for channel in 0..4 {
        let first = u32::from(source[channel]);
        store_radius_one_fractional_sample(
            &mut destination[channel],
            first * 2 + u32::from(source[4 + channel]),
            first + u32::from(source[8 + channel]),
            whole_weight,
            fractional_weight,
        );
        store_radius_one_fractional_sample(
            &mut destination[4 + channel],
            first + u32::from(source[4 + channel]) + u32::from(source[8 + channel]),
            first + u32::from(source[12 + channel]),
            whole_weight,
            fractional_weight,
        );
    }

    let interior_destination = &mut destination[8..(width - 2) * 4];
    let far_left_samples = &source[..(width - 4) * 4];
    let left_samples = &source[4..(width - 3) * 4];
    let center_samples = &source[8..(width - 2) * 4];
    let right_samples = &source[12..(width - 1) * 4];
    let far_right_samples = &source[16..];
    for (((((output, far_left), left), center), right), far_right) in interior_destination
        .chunks_exact_mut(4)
        .zip(far_left_samples.chunks_exact(4))
        .zip(left_samples.chunks_exact(4))
        .zip(center_samples.chunks_exact(4))
        .zip(right_samples.chunks_exact(4))
        .zip(far_right_samples.chunks_exact(4))
    {
        for channel in 0..4 {
            let whole_sum =
                u32::from(left[channel]) + u32::from(center[channel]) + u32::from(right[channel]);
            let far_sum = u32::from(far_left[channel]) + u32::from(far_right[channel]);
            store_radius_one_fractional_sample(
                &mut output[channel],
                whole_sum,
                far_sum,
                whole_weight,
                fractional_weight,
            );
        }
    }

    let penultimate = (width - 2) * 4;
    let last = (width - 1) * 4;
    for channel in 0..4 {
        store_radius_one_fractional_sample(
            &mut destination[penultimate + channel],
            u32::from(source[penultimate - 4 + channel])
                + u32::from(source[penultimate + channel])
                + u32::from(source[last + channel]),
            u32::from(source[penultimate - 8 + channel]) + u32::from(source[last + channel]),
            whole_weight,
            fractional_weight,
        );
        store_radius_one_fractional_sample(
            &mut destination[last + channel],
            u32::from(source[penultimate + channel]) + u32::from(source[last + channel]) * 2,
            u32::from(source[penultimate - 4 + channel]) + u32::from(source[last + channel]),
            whole_weight,
            fractional_weight,
        );
    }
}

/// Run the three horizontal Gaussian box passes one interleaved row at a time.
///
/// Horizontal filtering never reads another image row, so the intermediate
/// byte rounding can stay in two reusable row buffers instead of two extra
/// full-frame buffers. The same per-pass `blur_line` operation preserves the
/// order and byte result of three image-wide horizontal passes.
#[cfg(not(feature = "parallel"))]
pub(super) fn blur_interleaved_rows_three_passes<const CHANNELS: usize>(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let row_length = width * CHANNELS;
    let mut first_intermediate = vec![0u8; row_length];
    let mut second_intermediate = vec![0u8; row_length];
    let mut accumulator = [0u32; 4];
    let rgb_radius_one_fractional =
        CHANNELS == 3 && radius == 1 && fractional_weight != 0 && width >= 5;
    let rgba_radius_one_fractional =
        CHANNELS == 4 && radius == 1 && fractional_weight != 0 && width >= 5;

    for row in 0..height {
        let start = row * row_length;
        let source_row = &source[start..start + row_length];
        let destination_row = &mut destination[start..start + row_length];
        if rgb_radius_one_fractional {
            blur_rgb_radius_one_fractional_line(
                source_row,
                &mut first_intermediate,
                width,
                whole_weight,
                fractional_weight,
            );
            blur_rgb_radius_one_fractional_line(
                &first_intermediate,
                &mut second_intermediate,
                width,
                whole_weight,
                fractional_weight,
            );
            blur_rgb_radius_one_fractional_line(
                &second_intermediate,
                destination_row,
                width,
                whole_weight,
                fractional_weight,
            );
        } else if rgba_radius_one_fractional {
            blur_rgba_radius_one_fractional_line(
                source_row,
                &mut first_intermediate,
                width,
                whole_weight,
                fractional_weight,
            );
            blur_rgba_radius_one_fractional_line(
                &first_intermediate,
                &mut second_intermediate,
                width,
                whole_weight,
                fractional_weight,
            );
            blur_rgba_radius_one_fractional_line(
                &second_intermediate,
                destination_row,
                width,
                whole_weight,
                fractional_weight,
            );
        } else {
            blur_line(
                source_row,
                &mut first_intermediate,
                0,
                width,
                CHANNELS,
                radius,
                whole_weight,
                fractional_weight,
                &mut accumulator,
            );
            blur_line(
                &first_intermediate,
                &mut second_intermediate,
                0,
                width,
                CHANNELS,
                radius,
                whole_weight,
                fractional_weight,
                &mut accumulator,
            );
            blur_line(
                &second_intermediate,
                destination_row,
                0,
                width,
                CHANNELS,
                radius,
                whole_weight,
                fractional_weight,
                &mut accumulator,
            );
        }
    }
}

/// Apply one fractional radius-one Gaussian box pass to an LA row. The three
/// regions keep the replicated edge indices out of the interior loop.
#[cfg(not(feature = "parallel"))]
pub(super) fn blur_la_radius_one_fractional_line(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    debug_assert!(width >= 5);
    debug_assert!(source.len() >= width * 2);
    debug_assert!(destination.len() >= width * 2);
    debug_assert_ne!(fractional_weight, 0);

    for channel in 0..2 {
        let first = u32::from(source[channel]);
        store_radius_one_fractional_sample(
            &mut destination[channel],
            first * 2 + u32::from(source[2 + channel]),
            first + u32::from(source[4 + channel]),
            whole_weight,
            fractional_weight,
        );
        store_radius_one_fractional_sample(
            &mut destination[2 + channel],
            first + u32::from(source[2 + channel]) + u32::from(source[4 + channel]),
            first + u32::from(source[6 + channel]),
            whole_weight,
            fractional_weight,
        );
    }

    let interior_end = (width - 2) * 2;
    let interior_destination = &mut destination[4..interior_end];
    let far_left_samples = &source[..(width - 4) * 2];
    let left_samples = &source[2..(width - 3) * 2];
    let center_samples = &source[4..interior_end];
    let right_samples = &source[6..(width - 1) * 2];
    let far_right_samples = &source[8..];
    for (((((output, &far_left), &left), &center), &right), &far_right) in interior_destination
        .iter_mut()
        .zip(far_left_samples)
        .zip(left_samples)
        .zip(center_samples)
        .zip(right_samples)
        .zip(far_right_samples)
    {
        let whole_sum = u32::from(left) + u32::from(center) + u32::from(right);
        let far_sum = u32::from(far_left) + u32::from(far_right);
        store_radius_one_fractional_sample(
            output,
            whole_sum,
            far_sum,
            whole_weight,
            fractional_weight,
        );
    }

    let last = width - 1;
    let penultimate = last - 1;
    for channel in 0..2 {
        let penultimate_base = penultimate * 2 + channel;
        let last_base = last * 2 + channel;
        store_radius_one_fractional_sample(
            &mut destination[penultimate_base],
            u32::from(source[penultimate_base - 2])
                + u32::from(source[penultimate_base])
                + u32::from(source[penultimate_base + 2]),
            u32::from(source[penultimate_base - 4]) + u32::from(source[penultimate_base + 2]),
            whole_weight,
            fractional_weight,
        );
        store_radius_one_fractional_sample(
            &mut destination[last_base],
            u32::from(source[last_base - 2]) + u32::from(source[last_base]) * 2,
            u32::from(source[last_base - 4]) + u32::from(source[last_base]),
            whole_weight,
            fractional_weight,
        );
    }
}

/// Keep the exact byte rounding between the three horizontal passes while
/// specializing the common LA GaussianBlur(radius=2) box radius.
#[cfg(not(feature = "parallel"))]
pub(super) fn blur_la_rows_three_passes_radius_one(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let row_length = width * 2;
    let mut first_intermediate = vec![0u8; row_length];
    let mut second_intermediate = vec![0u8; row_length];

    for row in 0..height {
        let start = row * row_length;
        let source_row = &source[start..start + row_length];
        let destination_row = &mut destination[start..start + row_length];
        blur_la_radius_one_fractional_line(
            source_row,
            &mut first_intermediate,
            width,
            whole_weight,
            fractional_weight,
        );
        blur_la_radius_one_fractional_line(
            &first_intermediate,
            &mut second_intermediate,
            width,
            whole_weight,
            fractional_weight,
        );
        blur_la_radius_one_fractional_line(
            &second_intermediate,
            destination_row,
            width,
            whole_weight,
            fractional_weight,
        );
    }
}

/// Run the three horizontal Gaussian box passes one L row at a time.
///
/// Each row is independent for a horizontal pass, so the exact intermediate
/// byte rounds can stay in two row buffers. This avoids writing and rereading
/// two full-frame horizontal intermediates while retaining Pillow's pass
/// order.
#[cfg(not(feature = "parallel"))]
pub(super) fn blur_luma_rows_three_passes(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let mut first_intermediate = vec![0u8; width];
    let mut second_intermediate = vec![0u8; width];
    let mut accumulator = [0u32; 4];
    let radius_one_fractional = radius == 1 && fractional_weight != 0 && width >= 5;

    for row in 0..height {
        let start = row * width;
        let source_row = &source[start..start + width];
        let destination_row = &mut destination[start..start + width];
        if radius_one_fractional {
            blur_luma_radius_one_fractional_line(
                source_row,
                &mut first_intermediate,
                width,
                whole_weight,
                fractional_weight,
            );
            blur_luma_radius_one_fractional_line(
                &first_intermediate,
                &mut second_intermediate,
                width,
                whole_weight,
                fractional_weight,
            );
            blur_luma_radius_one_fractional_line(
                &second_intermediate,
                destination_row,
                width,
                whole_weight,
                fractional_weight,
            );
        } else {
            blur_line(
                source_row,
                &mut first_intermediate,
                0,
                width,
                1,
                radius,
                whole_weight,
                fractional_weight,
                &mut accumulator,
            );
            blur_line(
                &first_intermediate,
                &mut second_intermediate,
                0,
                width,
                1,
                radius,
                whole_weight,
                fractional_weight,
                &mut accumulator,
            );
            blur_line(
                &second_intermediate,
                destination_row,
                0,
                width,
                1,
                radius,
                whole_weight,
                fractional_weight,
                &mut accumulator,
            );
        }
    }
}

/// Apply one L vertical Gaussian box pass with five replicated-edge taps.
/// The direct row-major form keeps every source row contiguous and avoids the
/// generic transposed-line accumulator for this common radius.
#[cfg(not(feature = "parallel"))]
pub(super) fn blur_luma_vertical_radius_one_rows(
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
pub(super) fn blur_interleaved_vertical_radius_one_fractional_rows(
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
pub(super) const VERTICAL_BLUR_TRANSPOSE_THRESHOLD: usize = 512 * 512;

#[cfg(feature = "parallel")]
pub(super) fn transpose_interleaved_rows(
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
pub(super) fn blur_columns_radius_one_rows(
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
    use super::super::{blur_parameters, line::blur_columns};
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
