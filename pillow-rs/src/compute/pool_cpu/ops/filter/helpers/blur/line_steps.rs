//! Fixed-region advancement for the native box-blur line kernel.

use super::{BOX_BLUR_BIAS, blur_line_step};

#[inline(always)]
pub(super) fn l_step<const FRACTIONAL: bool>(
    source: &[u8],
    destination: &mut [u8],
    accumulator: &mut [u32],
    output_base: usize,
    subtract_base: usize,
    add_base: usize,
    far_left_base: usize,
    far_right_base: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let next = accumulator[0]
        .wrapping_sub(u32::from(source[subtract_base]))
        .wrapping_add(u32::from(source[add_base]));
    accumulator[0] = next;
    let mut bulk = next.wrapping_mul(whole_weight);
    if FRACTIONAL {
        let far = (u32::from(source[far_left_base]) + u32::from(source[far_right_base]))
            .wrapping_mul(fractional_weight);
        bulk = bulk.wrapping_add(far);
    }
    destination[output_base] = (bulk.wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
}

#[inline(always)]
pub(super) fn la_step<const FRACTIONAL: bool>(
    source: &[u8],
    destination: &mut [u8],
    accumulator: &mut [u32],
    output_base: usize,
    subtract_base: usize,
    add_base: usize,
    far_left_base: usize,
    far_right_base: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let output = &mut destination[output_base..output_base + 2];
    let subtract = &source[subtract_base..subtract_base + 2];
    let add = &source[add_base..add_base + 2];
    let next0 = accumulator[0]
        .wrapping_sub(u32::from(subtract[0]))
        .wrapping_add(u32::from(add[0]));
    let next1 = accumulator[1]
        .wrapping_sub(u32::from(subtract[1]))
        .wrapping_add(u32::from(add[1]));
    accumulator[0] = next0;
    accumulator[1] = next1;
    if !FRACTIONAL {
        output[0] = (next0.wrapping_mul(whole_weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
        output[1] = (next1.wrapping_mul(whole_weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
    } else {
        let far_left = &source[far_left_base..far_left_base + 2];
        let far_right = &source[far_right_base..far_right_base + 2];
        let far0 =
            (u32::from(far_left[0]) + u32::from(far_right[0])).wrapping_mul(fractional_weight);
        let far1 =
            (u32::from(far_left[1]) + u32::from(far_right[1])).wrapping_mul(fractional_weight);
        output[0] = (next0
            .wrapping_mul(whole_weight)
            .wrapping_add(far0)
            .wrapping_add(BOX_BLUR_BIAS)
            >> 24) as u8;
        output[1] = (next1
            .wrapping_mul(whole_weight)
            .wrapping_add(far1)
            .wrapping_add(BOX_BLUR_BIAS)
            >> 24) as u8;
    }
}

#[inline(always)]
pub(super) fn rgb_step<const FRACTIONAL: bool>(
    source: &[u8],
    destination: &mut [u8],
    accumulator: &mut [u32],
    output_base: usize,
    subtract_base: usize,
    add_base: usize,
    far_left_base: usize,
    far_right_base: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let output = &mut destination[output_base..output_base + 3];
    let subtract = &source[subtract_base..subtract_base + 3];
    let add = &source[add_base..add_base + 3];
    let next0 = accumulator[0]
        .wrapping_sub(u32::from(subtract[0]))
        .wrapping_add(u32::from(add[0]));
    let next1 = accumulator[1]
        .wrapping_sub(u32::from(subtract[1]))
        .wrapping_add(u32::from(add[1]));
    let next2 = accumulator[2]
        .wrapping_sub(u32::from(subtract[2]))
        .wrapping_add(u32::from(add[2]));
    accumulator[0] = next0;
    accumulator[1] = next1;
    accumulator[2] = next2;
    if !FRACTIONAL {
        output[0] = (next0.wrapping_mul(whole_weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
        output[1] = (next1.wrapping_mul(whole_weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
        output[2] = (next2.wrapping_mul(whole_weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
    } else {
        let far_left = &source[far_left_base..far_left_base + 3];
        let far_right = &source[far_right_base..far_right_base + 3];
        let far0 =
            (u32::from(far_left[0]) + u32::from(far_right[0])).wrapping_mul(fractional_weight);
        let far1 =
            (u32::from(far_left[1]) + u32::from(far_right[1])).wrapping_mul(fractional_weight);
        let far2 =
            (u32::from(far_left[2]) + u32::from(far_right[2])).wrapping_mul(fractional_weight);
        output[0] = (next0
            .wrapping_mul(whole_weight)
            .wrapping_add(far0)
            .wrapping_add(BOX_BLUR_BIAS)
            >> 24) as u8;
        output[1] = (next1
            .wrapping_mul(whole_weight)
            .wrapping_add(far1)
            .wrapping_add(BOX_BLUR_BIAS)
            >> 24) as u8;
        output[2] = (next2
            .wrapping_mul(whole_weight)
            .wrapping_add(far2)
            .wrapping_add(BOX_BLUR_BIAS)
            >> 24) as u8;
    }
}

#[inline(always)]
pub(super) fn interleaved_step<const FRACTIONAL: bool>(
    source: &[u8],
    destination: &mut [u8],
    accumulator: &mut [u32],
    element_width: usize,
    output_base: usize,
    subtract_base: usize,
    add_base: usize,
    far_left_base: usize,
    far_right_base: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let output = &mut destination[output_base..output_base + element_width];
    let subtract = &source[subtract_base..subtract_base + element_width];
    let add = &source[add_base..add_base + element_width];
    if !FRACTIONAL {
        for (((accumulator, output), &subtract), &add) in accumulator[..element_width]
            .iter_mut()
            .zip(output)
            .zip(subtract)
            .zip(add)
        {
            *accumulator = accumulator
                .wrapping_sub(u32::from(subtract))
                .wrapping_add(u32::from(add));
            let bulk = accumulator.wrapping_mul(whole_weight);
            *output = (bulk.wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
        }
    } else {
        let far_left = &source[far_left_base..far_left_base + element_width];
        let far_right = &source[far_right_base..far_right_base + element_width];
        for (((((accumulator, output), &subtract), &add), &far_left), &far_right) in accumulator
            [..element_width]
            .iter_mut()
            .zip(output)
            .zip(subtract)
            .zip(add)
            .zip(far_left)
            .zip(far_right)
        {
            *accumulator = accumulator
                .wrapping_sub(u32::from(subtract))
                .wrapping_add(u32::from(add));
            let far = (u32::from(far_left) + u32::from(far_right)).wrapping_mul(fractional_weight);
            let bulk = accumulator.wrapping_mul(whole_weight).wrapping_add(far);
            *output = (bulk.wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
        }
    }
}

#[inline(always)]
pub(super) fn run<const FRACTIONAL: bool>(
    source: &[u8],
    destination: &mut [u8],
    line_start: usize,
    element_width: usize,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
    accumulator: &mut [u32],
    last: usize,
    edge_a: usize,
    edge_b: usize,
) {
    if edge_a <= edge_b {
        for output in 0..edge_a {
            blur_line_step::<FRACTIONAL>(
                source,
                destination,
                accumulator,
                line_start,
                element_width,
                output,
                0,
                output + radius,
                0,
                output + radius + 1,
                whole_weight,
                fractional_weight,
            );
        }
        for output in edge_a..edge_b {
            blur_line_step::<FRACTIONAL>(
                source,
                destination,
                accumulator,
                line_start,
                element_width,
                output,
                output - radius - 1,
                output + radius,
                output - radius - 1,
                output + radius + 1,
                whole_weight,
                fractional_weight,
            );
        }
        for output in edge_b..=last {
            blur_line_step::<FRACTIONAL>(
                source,
                destination,
                accumulator,
                line_start,
                element_width,
                output,
                output - radius - 1,
                last,
                output - radius - 1,
                last,
                whole_weight,
                fractional_weight,
            );
        }
    } else {
        // The radius overlaps both edges. Pillow separates the overlap so no
        // index ever leaves the row, even when radius exceeds its length.
        for output in 0..edge_b {
            blur_line_step::<FRACTIONAL>(
                source,
                destination,
                accumulator,
                line_start,
                element_width,
                output,
                0,
                output + radius,
                0,
                output + radius + 1,
                whole_weight,
                fractional_weight,
            );
        }
        for output in edge_b..edge_a {
            blur_line_step::<FRACTIONAL>(
                source,
                destination,
                accumulator,
                line_start,
                element_width,
                output,
                0,
                last,
                0,
                last,
                whole_weight,
                fractional_weight,
            );
        }
        for output in edge_a..=last {
            blur_line_step::<FRACTIONAL>(
                source,
                destination,
                accumulator,
                line_start,
                element_width,
                output,
                output - radius - 1,
                last,
                output - radius - 1,
                last,
                whole_weight,
                fractional_weight,
            );
        }
    }
}
