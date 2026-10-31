//! Narrow fixed-point HSV8 Resize kernels retained from the serial path.

use crate::checked_dims::CheckedDims;
use crate::ops::pil_resize::FilterCoeffs;

use crate::compute::pool_cpu::ops::resize::helpers::coefficients::{
    byte_coefficients_fit_i32, byte_coefficients_fit_source, fixed_point_to_u8_i32,
};
use crate::compute::pool_cpu::ops::resize::helpers::byte_samples;

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
    let source_dims = CheckedDims::new(source_size.0, source_size.1, 3).ok()?;
    let intermediate_dims = CheckedDims::new(destination_size.0, source_size.1, 3).ok()?;
    let output_dims = CheckedDims::new(destination_size.0, destination_size.1, 3).ok()?;
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
    let source_stride = source_width.checked_mul(3)?;
    let intermediate_stride = output_width.checked_mul(3)?;
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
    if byte_samples::should_transpose_vertical(source_height as u32, output_width as u32) {
        let transposed = byte_samples::transpose_samples::<3>(
            &intermediate,
            source_height as u32,
            output_width as u32,
            intermediate_dims,
        );
        vertical_transposed(
            &transposed,
            source_height,
            output_width,
            output_height,
            vertical,
            &mut output,
        );
    } else {
        vertical_row_major(
            &intermediate,
            intermediate_stride,
            output_width,
            output_height,
            vertical,
            &mut output,
        );
    }
    Some(output)
}

fn horizontal_row(source: &[u8], coeffs: &FilterCoeffs, output_width: usize, output: &mut [u8]) {
    for output_x in 0..output_width {
        let weights = weights(coeffs, output_x);
        if weights.is_empty() {
            continue;
        }
        let (mut red, mut green, mut blue) = (0_i32, 0_i32, 0_i32);
        let mut source_start = coeffs.xmin[output_x] as usize * 3;
        if weights.len() == 4 {
            let weight = weights[0] as i32;
            red = i32::from(source[source_start]) * weight;
            green = i32::from(source[source_start + 1]) * weight;
            blue = i32::from(source[source_start + 2]) * weight;
            source_start += 3;
            let weight = weights[1] as i32;
            red += i32::from(source[source_start]) * weight;
            green += i32::from(source[source_start + 1]) * weight;
            blue += i32::from(source[source_start + 2]) * weight;
            source_start += 3;
            let weight = weights[2] as i32;
            red += i32::from(source[source_start]) * weight;
            green += i32::from(source[source_start + 1]) * weight;
            blue += i32::from(source[source_start + 2]) * weight;
            source_start += 3;
            let weight = weights[3] as i32;
            red += i32::from(source[source_start]) * weight;
            green += i32::from(source[source_start + 1]) * weight;
            blue += i32::from(source[source_start + 2]) * weight;
        } else {
            for (tap, &weight) in weights.iter().enumerate() {
                let source_start = (coeffs.xmin[output_x] as usize + tap) * 3;
                let weight = weight as i32;
                red += i32::from(source[source_start]) * weight;
                green += i32::from(source[source_start + 1]) * weight;
                blue += i32::from(source[source_start + 2]) * weight;
            }
        }
        let output_start = output_x * 3;
        output[output_start] = fixed_point_to_u8_i32(red);
        output[output_start + 1] = fixed_point_to_u8_i32(green);
        output[output_start + 2] = fixed_point_to_u8_i32(blue);
    }
}

fn vertical_row_major(
    intermediate: &[u8],
    row_stride: usize,
    output_width: usize,
    output_height: usize,
    coeffs: &FilterCoeffs,
    output: &mut [u8],
) {
    for output_y in 0..output_height {
        let weights = weights(coeffs, output_y);
        let output_start = output_y * row_stride;
        let output_row = &mut output[output_start..output_start + row_stride];
        if weights.is_empty() {
            output_row.fill(0);
            continue;
        }
        let first_source = coeffs.xmin[output_y] as usize * row_stride;
        for output_x in 0..output_width {
            let pixel = vertical_pixel(intermediate, row_stride, first_source, output_x, weights);
            let output_start = output_x * 3;
            output_row[output_start..output_start + 3].copy_from_slice(&pixel);
        }
    }
}

fn vertical_transposed(
    intermediate: &[u8],
    source_height: usize,
    output_width: usize,
    output_height: usize,
    coeffs: &FilterCoeffs,
    output: &mut [u8],
) {
    let output_stride = output_width * 3;
    for output_y in 0..output_height {
        let weights = weights(coeffs, output_y);
        let output_start = output_y * output_stride;
        let output_row = &mut output[output_start..output_start + output_stride];
        if weights.is_empty() {
            output_row.fill(0);
            continue;
        }
        let first_source_y = coeffs.xmin[output_y] as usize;
        for output_x in 0..output_width {
            let first_source = (output_x * source_height + first_source_y) * 3;
            let pixel = vertical_pixel(intermediate, 3, first_source, 0, weights);
            let output_start = output_x * 3;
            output_row[output_start..output_start + 3].copy_from_slice(&pixel);
        }
    }
}

#[inline(always)]
fn vertical_pixel(
    intermediate: &[u8],
    source_step: usize,
    first_source: usize,
    output_x: usize,
    weights: &[i64],
) -> [u8; 3] {
    let mut source_start = first_source + output_x * 3;
    let (mut red, mut green, mut blue) = (0_i32, 0_i32, 0_i32);
    if weights.len() == 4 {
        let weight = weights[0] as i32;
        red = i32::from(intermediate[source_start]) * weight;
        green = i32::from(intermediate[source_start + 1]) * weight;
        blue = i32::from(intermediate[source_start + 2]) * weight;
        source_start += source_step;
        let weight = weights[1] as i32;
        red += i32::from(intermediate[source_start]) * weight;
        green += i32::from(intermediate[source_start + 1]) * weight;
        blue += i32::from(intermediate[source_start + 2]) * weight;
        source_start += source_step;
        let weight = weights[2] as i32;
        red += i32::from(intermediate[source_start]) * weight;
        green += i32::from(intermediate[source_start + 1]) * weight;
        blue += i32::from(intermediate[source_start + 2]) * weight;
        source_start += source_step;
        let weight = weights[3] as i32;
        red += i32::from(intermediate[source_start]) * weight;
        green += i32::from(intermediate[source_start + 1]) * weight;
        blue += i32::from(intermediate[source_start + 2]) * weight;
    } else {
        for &weight in weights {
            let weight = weight as i32;
            red += i32::from(intermediate[source_start]) * weight;
            green += i32::from(intermediate[source_start + 1]) * weight;
            blue += i32::from(intermediate[source_start + 2]) * weight;
            source_start += source_step;
        }
    }
    [
        fixed_point_to_u8_i32(red),
        fixed_point_to_u8_i32(green),
        fixed_point_to_u8_i32(blue),
    ]
}

fn weights(coeffs: &FilterCoeffs, index: usize) -> &[i64] {
    let start = coeffs.offsets[index];
    &coeffs.weights[start..start + coeffs.count[index]]
}
