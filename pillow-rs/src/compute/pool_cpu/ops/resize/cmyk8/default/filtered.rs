//! Narrow fixed-point CMYK Resize kernels retained from the serial path.

use crate::checked_dims::CheckedDims;
use crate::ops::pil_resize::FilterCoeffs;

use crate::compute::pool_cpu::ops::resize::helpers::coefficients::{
    byte_coefficients_fit_i32, byte_coefficients_fit_source, fixed_point_to_u8_i32,
};

pub(crate) fn execute(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    horizontal: &FilterCoeffs,
    vertical: &FilterCoeffs,
) -> Option<Vec<u8>> {
    let (source_width, source_height) = (source_size.0 as usize, source_size.1 as usize);
    let (output_width, output_height) =
        (destination_size.0 as usize, destination_size.1 as usize);
    let source_dims = CheckedDims::new(source_size.0, source_size.1, 4).ok()?;
    let intermediate_dims =
        CheckedDims::new(destination_size.0, source_size.1, 4).ok()?;
    let output_dims = CheckedDims::new(destination_size.0, destination_size.1, 4).ok()?;
    if source_width == 0
        || source_height == 0
        || output_width == 0
        || output_height == 0
        || horizontal.xmin.len() != output_width
        || vertical.xmin.len() != output_height
        || !byte_coefficients_fit_source(horizontal, source_width)
        || !byte_coefficients_fit_source(vertical, source_height)
        || !byte_coefficients_fit_i32(horizontal)
        || !byte_coefficients_fit_i32(vertical)
        || source.len() != source_dims.total_bytes()
    {
        return None;
    }
    let source_stride = source_width.checked_mul(4)?;
    let intermediate_stride = output_width.checked_mul(4)?;
    let mut intermediate = intermediate_dims.alloc_buffer();
    for source_y in 0..source_height {
        let source_start = source_y.checked_mul(source_stride)?;
        let output_start = source_y.checked_mul(intermediate_stride)?;
        horizontal_row(
            &source[source_start..source_start + source_stride],
            horizontal,
            output_width,
            &mut intermediate[output_start..output_start + intermediate_stride],
        );
    }
    let mut output = output_dims.alloc_buffer();
    vertical_rows(
        &intermediate,
        intermediate_stride,
        output_height,
        vertical,
        &mut output,
    );
    Some(output)
}

fn horizontal_row(
    source: &[u8],
    coefficients: &FilterCoeffs,
    output_width: usize,
    output: &mut [u8],
) {
    for output_x in 0..output_width {
        let weights = weights(coefficients, output_x);
        if weights.is_empty() {
            continue;
        }
        let (mut cyan, mut magenta, mut yellow, mut black) = (0_i32, 0_i32, 0_i32, 0_i32);
        let mut source_start = coefficients.xmin[output_x] as usize * 4;
        if weights.len() == 4 {
            let weight = weights[0] as i32;
            cyan = i32::from(source[source_start]) * weight;
            magenta = i32::from(source[source_start + 1]) * weight;
            yellow = i32::from(source[source_start + 2]) * weight;
            black = i32::from(source[source_start + 3]) * weight;
            source_start += 4;

            let weight = weights[1] as i32;
            cyan += i32::from(source[source_start]) * weight;
            magenta += i32::from(source[source_start + 1]) * weight;
            yellow += i32::from(source[source_start + 2]) * weight;
            black += i32::from(source[source_start + 3]) * weight;
            source_start += 4;

            let weight = weights[2] as i32;
            cyan += i32::from(source[source_start]) * weight;
            magenta += i32::from(source[source_start + 1]) * weight;
            yellow += i32::from(source[source_start + 2]) * weight;
            black += i32::from(source[source_start + 3]) * weight;
            source_start += 4;

            let weight = weights[3] as i32;
            cyan += i32::from(source[source_start]) * weight;
            magenta += i32::from(source[source_start + 1]) * weight;
            yellow += i32::from(source[source_start + 2]) * weight;
            black += i32::from(source[source_start + 3]) * weight;
        } else {
            for (tap, &weight) in weights.iter().enumerate() {
                let source_start = (coefficients.xmin[output_x] as usize + tap) * 4;
                let weight = weight as i32;
                cyan += i32::from(source[source_start]) * weight;
                magenta += i32::from(source[source_start + 1]) * weight;
                yellow += i32::from(source[source_start + 2]) * weight;
                black += i32::from(source[source_start + 3]) * weight;
            }
        }
        let output_start = output_x * 4;
        output[output_start] = fixed_point_to_u8_i32(cyan);
        output[output_start + 1] = fixed_point_to_u8_i32(magenta);
        output[output_start + 2] = fixed_point_to_u8_i32(yellow);
        output[output_start + 3] = fixed_point_to_u8_i32(black);
    }
}

fn vertical_rows(
    intermediate: &[u8],
    row_stride: usize,
    output_height: usize,
    coefficients: &FilterCoeffs,
    output: &mut [u8],
) {
    for output_y in 0..output_height {
        let weights = weights(coefficients, output_y);
        if weights.is_empty() {
            continue;
        }
        let first_source = coefficients.xmin[output_y] as usize * row_stride;
        let output_start = output_y * row_stride;
        let output_row = &mut output[output_start..output_start + row_stride];
        if weights.len() == 4 {
            let weight0 = weights[0] as i32;
            let weight1 = weights[1] as i32;
            let weight2 = weights[2] as i32;
            let weight3 = weights[3] as i32;
            let row0 = first_source;
            let row1 = row0 + row_stride;
            let row2 = row1 + row_stride;
            let row3 = row2 + row_stride;
            for (output_x, output_pixel) in output_row.chunks_exact_mut(4).enumerate() {
                let source_start = output_x * 4;
                let mut cyan = i32::from(intermediate[row0 + source_start]) * weight0;
                let mut magenta = i32::from(intermediate[row0 + source_start + 1]) * weight0;
                let mut yellow = i32::from(intermediate[row0 + source_start + 2]) * weight0;
                let mut black = i32::from(intermediate[row0 + source_start + 3]) * weight0;
                cyan += i32::from(intermediate[row1 + source_start]) * weight1;
                magenta += i32::from(intermediate[row1 + source_start + 1]) * weight1;
                yellow += i32::from(intermediate[row1 + source_start + 2]) * weight1;
                black += i32::from(intermediate[row1 + source_start + 3]) * weight1;
                cyan += i32::from(intermediate[row2 + source_start]) * weight2;
                magenta += i32::from(intermediate[row2 + source_start + 1]) * weight2;
                yellow += i32::from(intermediate[row2 + source_start + 2]) * weight2;
                black += i32::from(intermediate[row2 + source_start + 3]) * weight2;
                cyan += i32::from(intermediate[row3 + source_start]) * weight3;
                magenta += i32::from(intermediate[row3 + source_start + 1]) * weight3;
                yellow += i32::from(intermediate[row3 + source_start + 2]) * weight3;
                black += i32::from(intermediate[row3 + source_start + 3]) * weight3;
                output_pixel[0] = fixed_point_to_u8_i32(cyan);
                output_pixel[1] = fixed_point_to_u8_i32(magenta);
                output_pixel[2] = fixed_point_to_u8_i32(yellow);
                output_pixel[3] = fixed_point_to_u8_i32(black);
            }
        } else {
            for (output_x, output_pixel) in output_row.chunks_exact_mut(4).enumerate() {
                let pixel = vertical_pixel(
                    intermediate,
                    row_stride,
                    first_source,
                    output_x,
                    weights,
                );
                output_pixel.copy_from_slice(&pixel);
            }
        }
    }
}

fn vertical_pixel(
    source: &[u8],
    row_stride: usize,
    first_source: usize,
    output_x: usize,
    weights: &[i64],
) -> [u8; 4] {
    let mut source_start = first_source + output_x * 4;
    let (mut cyan, mut magenta, mut yellow, mut black) = (0_i32, 0_i32, 0_i32, 0_i32);
    for &weight in weights {
        let weight = weight as i32;
        cyan += i32::from(source[source_start]) * weight;
        magenta += i32::from(source[source_start + 1]) * weight;
        yellow += i32::from(source[source_start + 2]) * weight;
        black += i32::from(source[source_start + 3]) * weight;
        source_start += row_stride;
    }
    [
        fixed_point_to_u8_i32(cyan),
        fixed_point_to_u8_i32(magenta),
        fixed_point_to_u8_i32(yellow),
        fixed_point_to_u8_i32(black),
    ]
}

fn weights(coefficients: &FilterCoeffs, index: usize) -> &[i64] {
    let start = coefficients.offsets[index];
    &coefficients.weights[start..start + coefficients.count[index]]
}
