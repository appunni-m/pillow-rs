//! Sliding-window line recurrences for box blur.

use super::blur_line_steps;

#[inline(always)]
pub(super) fn blur_line_step<const FRACTIONAL: bool>(
    source: &[u8],
    destination: &mut [u8],
    accumulator: &mut [u32],
    line_start: usize,
    element_width: usize,
    output: usize,
    subtract: usize,
    add: usize,
    far_left: usize,
    far_right: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let output_base = line_start + output * element_width;
    let subtract_base = line_start + subtract * element_width;
    let add_base = line_start + add * element_width;
    let far_left_base = line_start + far_left * element_width;
    let far_right_base = line_start + far_right * element_width;

    match element_width {
        1 => blur_line_steps::l_step::<FRACTIONAL>(
            source,
            destination,
            accumulator,
            output_base,
            subtract_base,
            add_base,
            far_left_base,
            far_right_base,
            whole_weight,
            fractional_weight,
        ),
        2 => blur_line_steps::la_step::<FRACTIONAL>(
            source,
            destination,
            accumulator,
            output_base,
            subtract_base,
            add_base,
            far_left_base,
            far_right_base,
            whole_weight,
            fractional_weight,
        ),
        3 => blur_line_steps::rgb_step::<FRACTIONAL>(
            source,
            destination,
            accumulator,
            output_base,
            subtract_base,
            add_base,
            far_left_base,
            far_right_base,
            whole_weight,
            fractional_weight,
        ),
        _ => blur_line_steps::interleaved_step::<FRACTIONAL>(
            source,
            destination,
            accumulator,
            element_width,
            output_base,
            subtract_base,
            add_base,
            far_left_base,
            far_right_base,
            whole_weight,
            fractional_weight,
        ),
    }
}

/// Blur one contiguous interleaved row using Pillow's four edge regions.
///
/// `src/libImaging/BoxBlur.c::ImagingLineBoxBlur{8,32}` initializes the
/// accumulator for logical pixel `-1`, then advances it by exactly one
/// entering and one leaving sample for every output pixel. Keeping the same
/// regions avoids a radius-sized inner loop while preserving Pillow's edge
/// replication and unsigned 24-bit fixed-point arithmetic.
#[inline(always)]
pub(super) fn blur_line(
    source: &[u8],
    destination: &mut [u8],
    line_start: usize,
    line_length: usize,
    element_width: usize,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
    accumulator: &mut [u32],
) {
    debug_assert!(line_length > 0);
    debug_assert!(element_width > 0);
    debug_assert!(accumulator.len() >= element_width);

    let last = line_length - 1;
    let edge_a = (radius + 1).min(line_length);
    let edge_b = line_length.saturating_sub(radius + 1);
    let first_base = line_start;
    let last_base = line_start + last * element_width;
    accumulator[..element_width].fill(0);

    // Pillow starts with the clamped window centered at x=-1. This lets the
    // first MOVE_ACC produce the exact window centered at output x=0.
    for component in 0..element_width {
        accumulator[component] =
            (source[first_base + component] as u32).wrapping_mul((radius + 1) as u32);
    }
    for position in 0..edge_a.saturating_sub(1) {
        let base = line_start + position * element_width;
        for component in 0..element_width {
            accumulator[component] =
                accumulator[component].wrapping_add(source[base + component] as u32);
        }
    }
    let last_count = radius.saturating_add(1).saturating_sub(edge_a);
    for component in 0..element_width {
        accumulator[component] = accumulator[component]
            .wrapping_add((source[last_base + component] as u32).wrapping_mul(last_count as u32));
    }

    if fractional_weight == 0 {
        blur_line_steps::run::<false>(
            source,
            destination,
            line_start,
            element_width,
            radius,
            whole_weight,
            fractional_weight,
            accumulator,
            last,
            edge_a,
            edge_b,
        );
    } else {
        blur_line_steps::run::<true>(
            source,
            destination,
            line_start,
            element_width,
            radius,
            whole_weight,
            fractional_weight,
            accumulator,
            last,
            edge_a,
            edge_b,
        );
    }
}

#[inline]
pub(super) fn blur_one_row(
    source: &[u8],
    destination: &mut [u8],
    row_start: usize,
    width: usize,
    channels: usize,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let row_length = width * channels;
    let mut accumulator = [0u32; 4];
    blur_line(
        &source[row_start..row_start + row_length],
        destination,
        0,
        width,
        channels,
        radius,
        whole_weight,
        fractional_weight,
        &mut accumulator,
    );
}

pub(super) fn blur_columns(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
    accumulator: &mut [u32],
) {
    // Treat each complete image row as one wide line element. Every component
    // gets the recurrence it would receive after Pillow's transpose, while
    // both the input and output accesses remain contiguous in row-major order.
    let row_length = width * channels;
    blur_line(
        source,
        destination,
        0,
        height,
        row_length,
        radius,
        whole_weight,
        fractional_weight,
        accumulator,
    );
}

pub(super) fn blur_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let row_length = width * channels;
    for row in 0..height {
        blur_one_row(
            source,
            &mut destination[row * row_length..(row + 1) * row_length],
            row * row_length,
            width,
            channels,
            radius,
            whole_weight,
            fractional_weight,
        );
    }
}
