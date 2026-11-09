//! Narrow fixed-point Luma8 resampling retained from the serial CPU path.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::resize::helpers::coefficients::{
    byte_coefficients_fit_i32, byte_coefficients_fit_source, fixed_point_to_u8_i32,
};
use crate::ops::pil_resize::FilterCoeffs;

pub(crate) fn execute(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    horizontal: &FilterCoeffs,
    vertical: &FilterCoeffs,
) -> Option<Vec<u8>> {
    let output_dims = CheckedDims::new(destination_size.0, destination_size.1, 1).ok()?;
    let mut output = vec![0; output_dims.total_bytes()];
    execute_into(
        source,
        source_size,
        destination_size,
        horizontal,
        vertical,
        &mut output,
    )?;
    Some(output)
}

/// Resizes native L bytes into a caller-owned output window.
pub(crate) fn execute_into(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    horizontal: &FilterCoeffs,
    vertical: &FilterCoeffs,
    output: &mut [u8],
) -> Option<()> {
    let source_dims = CheckedDims::new(source_size.0, source_size.1, 1).ok()?;
    let intermediate_dims = CheckedDims::new(destination_size.0, source_size.1, 1).ok()?;
    let output_dims = CheckedDims::new(destination_size.0, destination_size.1, 1).ok()?;
    let (source_width, source_height) = source_size;
    let (output_width, output_height) = destination_size;
    let source_width = usize::try_from(source_width).ok()?;
    let source_height = usize::try_from(source_height).ok()?;
    let output_width = usize::try_from(output_width).ok()?;
    let output_height = usize::try_from(output_height).ok()?;
    if horizontal.xmin.len() != output_width
        || vertical.xmin.len() != output_height
        || !byte_coefficients_fit_source(horizontal, source_width)
        || !byte_coefficients_fit_source(vertical, source_height)
        || !byte_coefficients_fit_i32(horizontal)
        || !byte_coefficients_fit_i32(vertical)
    {
        return None;
    }
    let intermediate_len = intermediate_dims.total_bytes();
    if source.len() != source_dims.total_bytes() || output.len() != output_dims.total_bytes() {
        return None;
    }

    let mut intermediate = vec![0; intermediate_len];
    for source_y in 0..source_height {
        let source_start = source_y.checked_mul(source_width)?;
        let destination_start = source_y.checked_mul(output_width)?;
        horizontal_row(
            &source[source_start..source_start + source_width],
            horizontal,
            &mut intermediate[destination_start..destination_start + output_width],
        );
    }
    vertical_rows(&intermediate, output_width, vertical, output);
    Some(())
}

fn horizontal_row(source: &[u8], coefficients: &FilterCoeffs, output: &mut [u8]) {
    for (output_x, destination) in output.iter_mut().enumerate() {
        let weights = weights(coefficients, output_x);
        if weights.is_empty() {
            continue;
        }
        let first_source_x = coefficients.xmin[output_x] as usize;
        let sum = match weights.len() {
            4 => sum_four(source, first_source_x, weights),
            5 => sum_five(source, first_source_x, weights),
            6 => sum_six(source, first_source_x, weights),
            _ => sum_general(source, first_source_x, weights),
        };
        *destination = fixed_point_to_u8_i32(sum);
    }
}

fn sum_four(source: &[u8], first: usize, weights: &[i64]) -> i32 {
    let mut sum = i32::from(source[first]) * weights[0] as i32;
    sum += i32::from(source[first + 1]) * weights[1] as i32;
    sum += i32::from(source[first + 2]) * weights[2] as i32;
    sum + i32::from(source[first + 3]) * weights[3] as i32
}

fn sum_five(source: &[u8], first: usize, weights: &[i64]) -> i32 {
    let mut sum = sum_four(source, first, weights);
    sum += i32::from(source[first + 4]) * weights[4] as i32;
    sum
}

fn sum_six(source: &[u8], first: usize, weights: &[i64]) -> i32 {
    let mut sum = sum_five(source, first, weights);
    sum += i32::from(source[first + 5]) * weights[5] as i32;
    sum
}

fn sum_general(source: &[u8], first: usize, weights: &[i64]) -> i32 {
    let mut sum = 0_i32;
    for (tap, &weight) in weights.iter().enumerate() {
        sum += i32::from(source[first + tap]) * weight as i32;
    }
    sum
}

fn vertical_rows(
    intermediate: &[u8],
    output_width: usize,
    coefficients: &FilterCoeffs,
    output: &mut [u8],
) {
    for (output_y, output_row) in output.chunks_exact_mut(output_width).enumerate() {
        let weights = weights(coefficients, output_y);
        if weights.is_empty() {
            output_row.fill(0);
            continue;
        }
        let source_y = coefficients.xmin[output_y] as usize;
        match weights.len() {
            4 => vertical_four(intermediate, output_width, source_y, weights, output_row),
            5 => vertical_five(intermediate, output_width, source_y, weights, output_row),
            6 => vertical_six(intermediate, output_width, source_y, weights, output_row),
            _ => vertical_general(intermediate, output_width, source_y, weights, output_row),
        }
    }
}

fn vertical_four(
    intermediate: &[u8],
    width: usize,
    first_row: usize,
    weights: &[i64],
    output: &mut [u8],
) {
    let row0 = first_row * width;
    let row1 = row0 + width;
    let row2 = row1 + width;
    let row3 = row2 + width;
    let weights = [
        weights[0] as i32,
        weights[1] as i32,
        weights[2] as i32,
        weights[3] as i32,
    ];
    for (x, pixel) in output.iter_mut().enumerate() {
        let mut sum = i32::from(intermediate[row0 + x]) * weights[0];
        sum += i32::from(intermediate[row1 + x]) * weights[1];
        sum += i32::from(intermediate[row2 + x]) * weights[2];
        sum += i32::from(intermediate[row3 + x]) * weights[3];
        *pixel = fixed_point_to_u8_i32(sum);
    }
}

fn vertical_five(
    intermediate: &[u8],
    width: usize,
    first_row: usize,
    weights: &[i64],
    output: &mut [u8],
) {
    let row0 = first_row * width;
    let row1 = row0 + width;
    let row2 = row1 + width;
    let row3 = row2 + width;
    let row4 = row3 + width;
    let weights = [
        weights[0] as i32,
        weights[1] as i32,
        weights[2] as i32,
        weights[3] as i32,
        weights[4] as i32,
    ];
    for (x, pixel) in output.iter_mut().enumerate() {
        let mut sum = i32::from(intermediate[row0 + x]) * weights[0];
        sum += i32::from(intermediate[row1 + x]) * weights[1];
        sum += i32::from(intermediate[row2 + x]) * weights[2];
        sum += i32::from(intermediate[row3 + x]) * weights[3];
        sum += i32::from(intermediate[row4 + x]) * weights[4];
        *pixel = fixed_point_to_u8_i32(sum);
    }
}

fn vertical_six(
    intermediate: &[u8],
    width: usize,
    first_row: usize,
    weights: &[i64],
    output: &mut [u8],
) {
    let row0 = first_row * width;
    let row1 = row0 + width;
    let row2 = row1 + width;
    let row3 = row2 + width;
    let row4 = row3 + width;
    let row5 = row4 + width;
    let weights = [
        weights[0] as i32,
        weights[1] as i32,
        weights[2] as i32,
        weights[3] as i32,
        weights[4] as i32,
        weights[5] as i32,
    ];
    for (x, pixel) in output.iter_mut().enumerate() {
        let mut sum = i32::from(intermediate[row0 + x]) * weights[0];
        sum += i32::from(intermediate[row1 + x]) * weights[1];
        sum += i32::from(intermediate[row2 + x]) * weights[2];
        sum += i32::from(intermediate[row3 + x]) * weights[3];
        sum += i32::from(intermediate[row4 + x]) * weights[4];
        sum += i32::from(intermediate[row5 + x]) * weights[5];
        *pixel = fixed_point_to_u8_i32(sum);
    }
}

fn vertical_general(
    intermediate: &[u8],
    width: usize,
    first_row: usize,
    weights: &[i64],
    output: &mut [u8],
) {
    for (x, pixel) in output.iter_mut().enumerate() {
        let mut sum = 0_i32;
        for (tap, &weight) in weights.iter().enumerate() {
            sum += i32::from(intermediate[(first_row + tap) * width + x]) * weight as i32;
        }
        *pixel = fixed_point_to_u8_i32(sum);
    }
}

fn weights(coefficients: &FilterCoeffs, index: usize) -> &[i64] {
    let start = coefficients.offsets[index];
    &coefficients.weights[start..start + coefficients.count[index]]
}
