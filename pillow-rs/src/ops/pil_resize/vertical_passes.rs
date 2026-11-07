use super::coefficients::fixed_point_to_u8;
use super::*;

pub(crate) fn vertical_pass_rgb_row(
    intermediate: &[u8],
    output_width: u32,
    coeffs: &FilterCoeffs,
    out_y: usize,
    output_row: &mut [u8],
) {
    let y0 = coeffs.xmin[out_y] as usize;
    let weights = coeffs.weights_for(out_y);
    if weights.is_empty() {
        output_row.fill(0);
        return;
    }

    let source_stride = output_width as usize * 3;
    if let [weight0, weight1, weight2, weight3] = weights {
        let source_row0 = &intermediate[y0 * source_stride..];
        let source_row1 = &intermediate[(y0 + 1) * source_stride..];
        let source_row2 = &intermediate[(y0 + 2) * source_stride..];
        let source_row3 = &intermediate[(y0 + 3) * source_stride..];
        for (x, pixel) in output_row.chunks_exact_mut(3).enumerate() {
            let source_index = x * 3;
            let mut red = i64::from(source_row0[source_index]) * weight0;
            let mut green = i64::from(source_row0[source_index + 1]) * weight0;
            let mut blue = i64::from(source_row0[source_index + 2]) * weight0;
            red += i64::from(source_row1[source_index]) * weight1;
            green += i64::from(source_row1[source_index + 1]) * weight1;
            blue += i64::from(source_row1[source_index + 2]) * weight1;
            red += i64::from(source_row2[source_index]) * weight2;
            green += i64::from(source_row2[source_index + 1]) * weight2;
            blue += i64::from(source_row2[source_index + 2]) * weight2;
            red += i64::from(source_row3[source_index]) * weight3;
            green += i64::from(source_row3[source_index + 1]) * weight3;
            blue += i64::from(source_row3[source_index + 2]) * weight3;
            pixel[0] = fixed_point_to_u8(red);
            pixel[1] = fixed_point_to_u8(green);
            pixel[2] = fixed_point_to_u8(blue);
        }
        return;
    }

    for (x, pixel) in output_row.chunks_exact_mut(3).enumerate() {
        let source_index = x * 3;
        let mut red = 0i64;
        let mut green = 0i64;
        let mut blue = 0i64;
        for (tap, &weight) in weights.iter().enumerate() {
            let source_index = (y0 + tap) * source_stride + source_index;
            red += i64::from(intermediate[source_index]) * weight;
            green += i64::from(intermediate[source_index + 1]) * weight;
            blue += i64::from(intermediate[source_index + 2]) * weight;
        }
        pixel[0] = fixed_point_to_u8(red);
        pixel[1] = fixed_point_to_u8(green);
        pixel[2] = fixed_point_to_u8(blue);
    }
}
