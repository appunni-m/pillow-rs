use super::{
    DynamicImage, FilterCoeffs, fixed_point_to_u8_i32, raw_to_dynamic_owned,
    resize_coefficients_fit_source, resize_u8_coefficients_fit_i32,
};
/// Resize the four stored C/M/Y/K bytes with narrow, exact fixed-point sums.
/// CMYK shares an `ImageRgba8` carrier, but none of its channels are alpha.
#[cfg(not(feature = "parallel"))]
pub(super) fn pil_resize_cmyk_i32(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    horizontal: &FilterCoeffs,
    vertical: &FilterCoeffs,
) -> Option<DynamicImage> {
    let DynamicImage::ImageRgba8(source) = img else {
        return None;
    };
    let source_width = usize::try_from(source.width()).ok()?;
    let source_height = usize::try_from(source.height()).ok()?;
    let output_width = usize::try_from(output_width).ok()?;
    let output_height = usize::try_from(output_height).ok()?;
    if source_width == 0
        || source_height == 0
        || output_width == 0
        || output_height == 0
        || horizontal.xmin.len() != output_width
        || vertical.xmin.len() != output_height
        || !resize_coefficients_fit_source(horizontal, source_width)
        || !resize_coefficients_fit_source(vertical, source_height)
        || !resize_u8_coefficients_fit_i32(horizontal)
        || !resize_u8_coefficients_fit_i32(vertical)
    {
        return None;
    }

    let source_stride = source_width.checked_mul(4)?;
    let intermediate_stride = output_width.checked_mul(4)?;
    let intermediate_len = source_height.checked_mul(intermediate_stride)?;
    let output_len = output_height.checked_mul(intermediate_stride)?;
    if source.as_raw().len() != source_height.checked_mul(source_stride)? {
        return None;
    }

    let mut intermediate = vec![0; intermediate_len];
    for source_y in 0..source_height {
        let source_start = source_y.checked_mul(source_stride)?;
        let source_row = source
            .as_raw()
            .get(source_start..source_start.checked_add(source_stride)?)?;
        let output_start = source_y.checked_mul(intermediate_stride)?;
        let output_row =
            intermediate.get_mut(output_start..output_start.checked_add(intermediate_stride)?)?;
        horizontal_pass_row_cmyk_i32(source_row, horizontal, output_width, output_row);
    }

    // CMYK vertical taps read four row-major streams. Keeping those streams
    // contiguous lets the output-x loop reuse cache lines and avoids a full
    // intermediate-frame transpose/copy.
    let vertical_source = &intermediate;
    let mut output = vec![0; output_len];
    for output_y in 0..output_height {
        let output_start = output_y.checked_mul(intermediate_stride)?;
        let output_row =
            output.get_mut(output_start..output_start.checked_add(intermediate_stride)?)?;
        let weights = vertical.weights_for(output_y);
        if weights.is_empty() {
            continue;
        }
        let first_source_y = usize::try_from(vertical.xmin[output_y]).ok()?;
        if weights.len() == 4 {
            // Bicubic uses four taps. Hoist row-invariant coefficients and
            // source offsets so the pixel loop only performs channel MACs.
            let weight0 = weights[0] as i32;
            let weight1 = weights[1] as i32;
            let weight2 = weights[2] as i32;
            let weight3 = weights[3] as i32;
            let first_source_row = first_source_y.checked_mul(intermediate_stride)?;
            for (output_x, output_pixel) in output_row.chunks_exact_mut(4).enumerate() {
                let source_start = output_x * 4 + first_source_row;
                let (mut cyan, mut magenta, mut yellow, mut black) = (
                    i32::from(vertical_source[source_start]) * weight0,
                    i32::from(vertical_source[source_start + 1]) * weight0,
                    i32::from(vertical_source[source_start + 2]) * weight0,
                    i32::from(vertical_source[source_start + 3]) * weight0,
                );
                let source_start = source_start + intermediate_stride;
                cyan += i32::from(vertical_source[source_start]) * weight1;
                magenta += i32::from(vertical_source[source_start + 1]) * weight1;
                yellow += i32::from(vertical_source[source_start + 2]) * weight1;
                black += i32::from(vertical_source[source_start + 3]) * weight1;
                let source_start = source_start + intermediate_stride;
                cyan += i32::from(vertical_source[source_start]) * weight2;
                magenta += i32::from(vertical_source[source_start + 1]) * weight2;
                yellow += i32::from(vertical_source[source_start + 2]) * weight2;
                black += i32::from(vertical_source[source_start + 3]) * weight2;
                let source_start = source_start + intermediate_stride;
                cyan += i32::from(vertical_source[source_start]) * weight3;
                magenta += i32::from(vertical_source[source_start + 1]) * weight3;
                yellow += i32::from(vertical_source[source_start + 2]) * weight3;
                black += i32::from(vertical_source[source_start + 3]) * weight3;
                output_pixel[0] = fixed_point_to_u8_i32(cyan);
                output_pixel[1] = fixed_point_to_u8_i32(magenta);
                output_pixel[2] = fixed_point_to_u8_i32(yellow);
                output_pixel[3] = fixed_point_to_u8_i32(black);
            }
        } else {
            for (output_x, output_pixel) in output_row.chunks_exact_mut(4).enumerate() {
                output_pixel.copy_from_slice(&vertical_pass_col_cmyk_i32(
                    vertical_source,
                    intermediate_stride,
                    first_source_y,
                    output_x,
                    weights,
                ));
            }
        }
    }

    Some(raw_to_dynamic_owned(
        output,
        output_width as u32,
        output_height as u32,
        4,
    ))
}

#[cfg(not(feature = "parallel"))]
fn horizontal_pass_row_cmyk_i32(
    source_row: &[u8],
    coeffs: &FilterCoeffs,
    output_width: usize,
    output_row: &mut [u8],
) {
    for output_x in 0..output_width {
        let weights = coeffs.weights_for(output_x);
        if weights.is_empty() {
            continue;
        }
        let mut cyan = 0i32;
        let mut magenta = 0i32;
        let mut yellow = 0i32;
        let mut black = 0i32;
        let mut source_start = coeffs.xmin[output_x] as usize * 4;
        if weights.len() == 4 {
            let weight = weights[0] as i32;
            cyan = i32::from(source_row[source_start]) * weight;
            magenta = i32::from(source_row[source_start + 1]) * weight;
            yellow = i32::from(source_row[source_start + 2]) * weight;
            black = i32::from(source_row[source_start + 3]) * weight;
            source_start += 4;

            let weight = weights[1] as i32;
            cyan += i32::from(source_row[source_start]) * weight;
            magenta += i32::from(source_row[source_start + 1]) * weight;
            yellow += i32::from(source_row[source_start + 2]) * weight;
            black += i32::from(source_row[source_start + 3]) * weight;
            source_start += 4;

            let weight = weights[2] as i32;
            cyan += i32::from(source_row[source_start]) * weight;
            magenta += i32::from(source_row[source_start + 1]) * weight;
            yellow += i32::from(source_row[source_start + 2]) * weight;
            black += i32::from(source_row[source_start + 3]) * weight;
            source_start += 4;

            let weight = weights[3] as i32;
            cyan += i32::from(source_row[source_start]) * weight;
            magenta += i32::from(source_row[source_start + 1]) * weight;
            yellow += i32::from(source_row[source_start + 2]) * weight;
            black += i32::from(source_row[source_start + 3]) * weight;
        } else {
            for (tap, &weight) in weights.iter().enumerate() {
                let sample = coeffs.xmin[output_x] as usize + tap;
                let source_start = sample * 4;
                let weight = weight as i32;
                cyan += i32::from(source_row[source_start]) * weight;
                magenta += i32::from(source_row[source_start + 1]) * weight;
                yellow += i32::from(source_row[source_start + 2]) * weight;
                black += i32::from(source_row[source_start + 3]) * weight;
            }
        }
        let output_start = output_x * 4;
        output_row[output_start] = fixed_point_to_u8_i32(cyan);
        output_row[output_start + 1] = fixed_point_to_u8_i32(magenta);
        output_row[output_start + 2] = fixed_point_to_u8_i32(yellow);
        output_row[output_start + 3] = fixed_point_to_u8_i32(black);
    }
}

#[cfg(not(feature = "parallel"))]
fn vertical_pass_col_cmyk_i32(
    intermediate: &[u8],
    source_stride: usize,
    first_source_y: usize,
    output_x: usize,
    weights: &[i64],
) -> [u8; 4] {
    let mut source_start = first_source_y * source_stride + output_x * 4;
    let (mut cyan, mut magenta, mut yellow, mut black) = (0i32, 0i32, 0i32, 0i32);
    for &weight in weights {
        let weight = weight as i32;
        cyan += i32::from(intermediate[source_start]) * weight;
        magenta += i32::from(intermediate[source_start + 1]) * weight;
        yellow += i32::from(intermediate[source_start + 2]) * weight;
        black += i32::from(intermediate[source_start + 3]) * weight;
        source_start += source_stride;
    }
    [
        fixed_point_to_u8_i32(cyan),
        fixed_point_to_u8_i32(magenta),
        fixed_point_to_u8_i32(yellow),
        fixed_point_to_u8_i32(black),
    ]
}
