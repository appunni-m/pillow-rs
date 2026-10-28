//! Mode-specific radius-one box-blur row and column paths.

use super::BOX_BLUR_BIAS;

use super::line::blur_line;

pub(super) use vertical::{
    blur_interleaved_vertical_radius_one_fractional_rows, blur_luma_vertical_radius_one_rows,
};
#[path = "radius_one/vertical.rs"]
mod vertical;

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
