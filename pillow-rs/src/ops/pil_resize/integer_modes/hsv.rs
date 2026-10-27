use super::{
    DynamicImage, FilterCoeffs, fixed_point_to_u8_i32, raw_to_dynamic_owned,
    should_transpose_vertical, transpose_resize_intermediate,
};
#[cfg(not(feature = "parallel"))]
fn horizontal_pass_row_hsv_i32(
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
        let mut red_sum = 0i32;
        let mut green_sum = 0i32;
        let mut blue_sum = 0i32;
        let mut source_start = coeffs.xmin[output_x] as usize * 3;
        if weights.len() == 4 {
            let weight = weights[0] as i32;
            red_sum = i32::from(source_row[source_start]) * weight;
            green_sum = i32::from(source_row[source_start + 1]) * weight;
            blue_sum = i32::from(source_row[source_start + 2]) * weight;
            source_start += 3;
            let weight = weights[1] as i32;
            red_sum += i32::from(source_row[source_start]) * weight;
            green_sum += i32::from(source_row[source_start + 1]) * weight;
            blue_sum += i32::from(source_row[source_start + 2]) * weight;
            source_start += 3;
            let weight = weights[2] as i32;
            red_sum += i32::from(source_row[source_start]) * weight;
            green_sum += i32::from(source_row[source_start + 1]) * weight;
            blue_sum += i32::from(source_row[source_start + 2]) * weight;
            source_start += 3;
            let weight = weights[3] as i32;
            red_sum += i32::from(source_row[source_start]) * weight;
            green_sum += i32::from(source_row[source_start + 1]) * weight;
            blue_sum += i32::from(source_row[source_start + 2]) * weight;
        } else {
            for (tap, &weight) in weights.iter().enumerate() {
                let source_start = (coeffs.xmin[output_x] as usize + tap) * 3;
                let weight = weight as i32;
                red_sum += i32::from(source_row[source_start]) * weight;
                green_sum += i32::from(source_row[source_start + 1]) * weight;
                blue_sum += i32::from(source_row[source_start + 2]) * weight;
            }
        }
        let output_start = output_x * 3;
        output_row[output_start] = fixed_point_to_u8_i32(red_sum);
        output_row[output_start + 1] = fixed_point_to_u8_i32(green_sum);
        output_row[output_start + 2] = fixed_point_to_u8_i32(blue_sum);
    }
}

#[cfg(not(feature = "parallel"))]
fn vertical_pass_col_hsv_i32(
    intermediate: &[u8],
    source_rows: usize,
    output_width: usize,
    output_x: usize,
    coeffs: &FilterCoeffs,
    output_y: usize,
    transposed: bool,
) -> [u8; 3] {
    let weights = coeffs.weights_for(output_y);
    if weights.is_empty() {
        return [0; 3];
    }
    let first_source_y = coeffs.xmin[output_y] as usize;
    let (mut source_start, source_step) = if transposed {
        (output_x * source_rows * 3 + first_source_y * 3, 3)
    } else {
        (
            first_source_y * output_width * 3 + output_x * 3,
            output_width * 3,
        )
    };
    let mut red_sum = 0i32;
    let mut green_sum = 0i32;
    let mut blue_sum = 0i32;
    if weights.len() == 4 {
        let weight = weights[0] as i32;
        red_sum = i32::from(intermediate[source_start]) * weight;
        green_sum = i32::from(intermediate[source_start + 1]) * weight;
        blue_sum = i32::from(intermediate[source_start + 2]) * weight;
        source_start += source_step;
        let weight = weights[1] as i32;
        red_sum += i32::from(intermediate[source_start]) * weight;
        green_sum += i32::from(intermediate[source_start + 1]) * weight;
        blue_sum += i32::from(intermediate[source_start + 2]) * weight;
        source_start += source_step;
        let weight = weights[2] as i32;
        red_sum += i32::from(intermediate[source_start]) * weight;
        green_sum += i32::from(intermediate[source_start + 1]) * weight;
        blue_sum += i32::from(intermediate[source_start + 2]) * weight;
        source_start += source_step;
        let weight = weights[3] as i32;
        red_sum += i32::from(intermediate[source_start]) * weight;
        green_sum += i32::from(intermediate[source_start + 1]) * weight;
        blue_sum += i32::from(intermediate[source_start + 2]) * weight;
    } else {
        for &weight in weights {
            let weight = weight as i32;
            red_sum += i32::from(intermediate[source_start]) * weight;
            green_sum += i32::from(intermediate[source_start + 1]) * weight;
            blue_sum += i32::from(intermediate[source_start + 2]) * weight;
            source_start += source_step;
        }
    }
    [
        fixed_point_to_u8_i32(red_sum),
        fixed_point_to_u8_i32(green_sum),
        fixed_point_to_u8_i32(blue_sum),
    ]
}

#[cfg(not(feature = "parallel"))]
pub(in crate::ops::pil_resize) fn pil_resize_hsv_i32(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    horizontal: &FilterCoeffs,
    vertical: &FilterCoeffs,
) -> Option<DynamicImage> {
    let source_width = usize::try_from(img.width()).ok()?;
    let source_height = usize::try_from(img.height()).ok()?;
    let output_width_usize = usize::try_from(output_width).ok()?;
    let output_height_usize = usize::try_from(output_height).ok()?;
    let source_stride = source_width.checked_mul(3)?;
    let intermediate_stride = output_width_usize.checked_mul(3)?;
    let intermediate_len = source_height.checked_mul(intermediate_stride)?;
    let output_len = output_height_usize.checked_mul(intermediate_stride)?;
    let mut intermediate = vec![0u8; intermediate_len];
    for source_y in 0..source_height {
        let source_start = source_y.checked_mul(source_stride)?;
        let source_row = img
            .as_bytes()
            .get(source_start..source_start.checked_add(source_stride)?)?;
        let output_start = source_y.checked_mul(intermediate_stride)?;
        let output_row =
            intermediate.get_mut(output_start..output_start.checked_add(intermediate_stride)?)?;
        horizontal_pass_row_hsv_i32(source_row, horizontal, output_width_usize, output_row);
    }

    let transposed = should_transpose_vertical(img.height(), output_width, 3);
    let transposed_intermediate = transposed
        .then(|| transpose_resize_intermediate(&intermediate, img.height(), output_width, 3));
    let vertical_source = transposed_intermediate.as_deref().unwrap_or(&intermediate);
    let mut output = vec![0u8; output_len];
    for output_y in 0..output_height_usize {
        let output_row_start = output_y.checked_mul(intermediate_stride)?;
        let output_row =
            output.get_mut(output_row_start..output_row_start.checked_add(intermediate_stride)?)?;
        for output_x in 0..output_width_usize {
            let pixel = vertical_pass_col_hsv_i32(
                vertical_source,
                source_height,
                output_width_usize,
                output_x,
                vertical,
                output_y,
                transposed,
            );
            let output_start = output_x * 3;
            output_row[output_start..output_start + 3].copy_from_slice(&pixel);
        }
    }
    Some(raw_to_dynamic_owned(output, output_width, output_height, 3))
}
