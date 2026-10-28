use super::super::{RgbReduceRounding, RgbReduceRowContext, SparseRgbReduction};
use crate::error::PilError;
use crate::image_utils::raw_bytes_to_image;

#[inline(always)]
fn write_reduce_rgb_block(
    context: &RgbReduceRowContext<'_>,
    row: &mut [u8],
    output_x: u32,
    source_x: u32,
    source_y: usize,
    block_width: u32,
    block_height: u32,
    rounding: RgbReduceRounding,
) {
    let mut red_sum = 0u32;
    let mut green_sum = 0u32;
    let mut blue_sum = 0u32;
    let mut source_row_start = source_y * context.source_stride + source_x as usize * 3;
    for _ in 0..block_height {
        let mut red_row_sum = 0u32;
        let mut green_row_sum = 0u32;
        let mut blue_row_sum = 0u32;
        let mut source_index = source_row_start;
        for _ in 0..block_width {
            red_row_sum += u32::from(context.source[source_index]);
            green_row_sum += u32::from(context.source[source_index + 1]);
            blue_row_sum += u32::from(context.source[source_index + 2]);
            source_index += 3;
        }
        red_sum += red_row_sum;
        green_sum += green_row_sum;
        blue_sum += blue_row_sum;
        source_row_start += context.source_stride;
    }

    let destination_index = output_x as usize * 3;
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "the factor-area guard proves sums plus amend and the 24-bit reciprocal multiply fit u32"
    )]
    let red = ((red_sum + rounding.amend) * rounding.multiplier) >> 24;
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "the factor-area guard proves sums plus amend and the 24-bit reciprocal multiply fit u32"
    )]
    let green = ((green_sum + rounding.amend) * rounding.multiplier) >> 24;
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "the factor-area guard proves sums plus amend and the 24-bit reciprocal multiply fit u32"
    )]
    let blue = ((blue_sum + rounding.amend) * rounding.multiplier) >> 24;
    let red = red as u8;
    let green = green as u8;
    let blue = blue as u8;
    row[destination_index] = red;
    row[destination_index + 1] = green;
    row[destination_index + 2] = blue;
}

#[inline(always)]
fn process_reduce_rgb_row(context: &RgbReduceRowContext<'_>, y: u32, row: &mut [u8]) {
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
    let (interior_rounding, edge_rounding) = if full_y {
        (context.full, context.right)
    } else {
        (context.bottom, context.corner)
    };
    for x in 0..context.main_width {
        write_reduce_rgb_block(
            context,
            row,
            x,
            x * context.x_factor,
            source_y,
            context.x_factor,
            block_height,
            interior_rounding,
        );
    }
    if context.right_width != 0 {
        write_reduce_rgb_block(
            context,
            row,
            context.main_width,
            context.main_width * context.x_factor,
            source_y,
            context.right_width,
            block_height,
            edge_rounding,
        );
    }
}

pub(in crate::compute::pool_cpu::ops::reduce) fn execute_reduce_rgb_general(
    context: &RgbReduceRowContext<'_>,
    _width: u32,
    _height: u32,
    new_height: u32,
    mut output: Vec<u8>,
) -> Result<Option<SparseRgbReduction>, PilError> {
    let output_stride = context.output_width as usize * 3;
    #[cfg(feature = "parallel")]
    {
        const REDUCE_PARALLEL_PIXEL_THRESHOLD: usize = 512 * 512;
        let input_pixels = (_width as usize).saturating_mul(_height as usize);
        if input_pixels >= REDUCE_PARALLEL_PIXEL_THRESHOLD {
            crate::par_rows_mut!(
                &mut output,
                output_stride,
                new_height as usize,
                |_row_start, _row_end, y, row| {
                    process_reduce_rgb_row(context, y, row);
                }
            );
        } else {
            for y in 0..new_height {
                let start = y as usize * output_stride;
                process_reduce_rgb_row(context, y, &mut output[start..start + output_stride]);
            }
        }
    }

    #[cfg(not(feature = "parallel"))]
    for y in 0..new_height {
        let start = y as usize * output_stride;
        process_reduce_rgb_row(context, y, &mut output[start..start + output_stride]);
    }

    raw_bytes_to_image(context.output_width, new_height, output, 3).map(|image| Some((image, None)))
}

#[inline(always)]
pub(in crate::compute::pool_cpu::ops::reduce) fn reduce_rgb_dense_even_2x2(
    source: &[u8],
    source_stride: usize,
    _width: u32,
    _height: u32,
    new_width: u32,
    new_height: u32,
    output: &mut [u8],
) {
    let output_stride = new_width as usize * 3;
    let process_row = |output_y: usize, output_row: &mut [u8]| {
        let source_top = output_y * 2 * source_stride;
        let source_bottom = source_top + source_stride;
        let top_row = &source[source_top..source_top + source_stride];
        let bottom_row = &source[source_bottom..source_bottom + source_stride];

        for (block_x, output_pixel) in output_row.chunks_exact_mut(3).enumerate() {
            let source_x = block_x * 6;
            let top_right = source_x + 3;
            for channel in 0..3 {
                let sum = u32::from(top_row[source_x + channel])
                    + u32::from(top_row[top_right + channel])
                    + u32::from(bottom_row[source_x + channel])
                    + u32::from(bottom_row[top_right + channel]);
                // For a 2×2 block Pillow's reciprocal is exactly `(sum + 2) / 4`.
                #[expect(
                    clippy::arithmetic_side_effects,
                    reason = "four byte samples plus the Pillow rounding bias fit u32"
                )]
                let average = (sum + 2) >> 2;
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "the Pillow 2x2 average is bounded to one byte"
                )]
                {
                    output_pixel[channel] = average as u8;
                }
            }
        }
    };

    #[cfg(feature = "parallel")]
    {
        const REDUCE_PARALLEL_PIXEL_THRESHOLD: usize = 512 * 512;
        let input_pixels = (_width as usize).saturating_mul(_height as usize);
        if input_pixels >= REDUCE_PARALLEL_PIXEL_THRESHOLD {
            crate::par_rows_mut!(
                output,
                output_stride,
                new_height as usize,
                |_row_start, _row_end, y, row| { process_row(y as usize, row) }
            );
        } else {
            for y in 0..new_height as usize {
                let start = y * output_stride;
                process_row(y, &mut output[start..start + output_stride]);
            }
        }
    }

    #[cfg(not(feature = "parallel"))]
    for y in 0..new_height as usize {
        let start = y * output_stride;
        process_row(y, &mut output[start..start + output_stride]);
    }
}
