//! Specialized kernels for exact native-byte reduction.

use super::rgb::{RgbReduceRowContext, RgbReduceRounding};

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(crate) mod avx2;

pub(crate) mod wide;

#[inline(always)]
fn write_rgb_block(
    context: &RgbReduceRowContext<'_>,
    row: &mut [u8],
    output_x: u32,
    source_x: u32,
    source_y: usize,
    block_width: u32,
    block_height: u32,
    rounding: RgbReduceRounding,
) {
    let mut sums = [0u32; 3];
    let mut source_row = source_y * context.source_stride + source_x as usize * 3;
    for _ in 0..block_height {
        let mut index = source_row;
        for _ in 0..block_width {
            sums[0] += u32::from(context.source[index]);
            sums[1] += u32::from(context.source[index + 1]);
            sums[2] += u32::from(context.source[index + 2]);
            index += 3;
        }
        source_row += context.source_stride;
    }
    let destination = output_x as usize * 3;
    for channel in 0..3 {
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "factor-area admission bounds each channel sum and reciprocal product"
        )]
        let value = ((sums[channel] + rounding.amend) * rounding.multiplier) >> 24;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "Pillow's fixed-point RGB average is bounded to one byte"
        )]
        {
            row[destination + channel] = value as u8;
        }
    }
}

#[inline(always)]
fn process_rgb_row_range(
    context: &RgbReduceRowContext<'_>,
    y: u32,
    row: &mut [u8],
    start_x: u32,
    end_x: u32,
) {
    let full_y = y < context.main_height;
    let source_y = if full_y {
        (y * context.y_factor) as usize
    } else {
        (context.main_height * context.y_factor) as usize
    };
    let block_height = if full_y {
        context.y_factor
    } else {
        context.bottom_height
    };
    let (main_rounding, edge_rounding) = if full_y {
        (context.full, context.right)
    } else {
        (context.bottom, context.corner)
    };
    for x in start_x..end_x {
        let is_right = x >= context.main_width;
        let (source_x, block_width, rounding) = if is_right {
            (
                context.main_width * context.x_factor,
                context.right_width,
                edge_rounding,
            )
        } else {
            (x * context.x_factor, context.x_factor, main_rounding)
        };
        write_rgb_block(
            context,
            row,
            x,
            source_x,
            source_y,
            block_width,
            block_height,
            rounding,
        );
    }
}

pub(crate) fn reduce_rgb_row_range(
    context: &RgbReduceRowContext<'_>,
    y: u32,
    row: &mut [u8],
    start_x: u32,
    end_x: u32,
) {
    process_rgb_row_range(context, y, row, start_x, end_x);
}

#[inline(always)]
fn process_rgb_row(context: &RgbReduceRowContext<'_>, y: u32, row: &mut [u8]) {
    process_rgb_row_range(context, y, row, 0, context.output_width);
}

pub(super) fn reduce_rgb_general(
    context: &RgbReduceRowContext<'_>,
    output_height: u32,
    output: &mut [u8],
) {
    let row_stride = context.output_width as usize * 3;
    for y in 0..output_height {
        let start = y as usize * row_stride;
        process_rgb_row(context, y, &mut output[start..start + row_stride]);
    }
}

#[inline(always)]
pub(super) fn reduce_rgb_dense_even_2x2(
    source: &[u8],
    source_stride: usize,
    output_width: u32,
    output_height: u32,
    output: &mut [u8],
) {
    let output_stride = output_width as usize * 3;
    for y in 0..output_height as usize {
        let top_start = y * 2 * source_stride;
        let bottom_start = top_start + source_stride;
        let row_start = y * output_stride;
        for x in 0..output_width as usize {
            let source_x = x * 6;
            let top_right = source_x + 3;
            let destination = row_start + x * 3;
            for channel in 0..3 {
                let sum = u32::from(source[top_start + source_x + channel])
                    + u32::from(source[top_start + top_right + channel])
                    + u32::from(source[bottom_start + source_x + channel])
                    + u32::from(source[bottom_start + top_right + channel]);
                #[expect(
                    clippy::arithmetic_side_effects,
                    reason = "four byte samples plus Pillow's rounding bias fit u32"
                )]
                let value = (sum + 2) >> 2;
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "the 2x2 average is bounded to one byte"
                )]
                {
                    output[destination + channel] = value as u8;
                }
            }
        }
    }
}
