use super::coefficients::fixed_point_to_u8;
use super::*;

#[inline(always)]
pub(crate) fn horizontal_pass_rgb_row(
    src_row: &[u8],
    coeffs: &FilterCoeffs,
    out_w: u32,
    intermediate_row: &mut [u8],
) {
    // Native RGB has no alpha conversion or mode-dependent channel mapping.
    // Accumulate its interleaved channels together so each coefficient and
    // source pixel is visited once instead of repeating the tap walk 3 times.
    for ox in 0..out_w as usize {
        let x0 = coeffs.xmin[ox];
        let cnt = coeffs.count[ox];
        if cnt == 0 {
            continue;
        }
        let weights = coeffs.weights_for(ox);
        let (red_acc, green_acc, blue_acc) = if cnt == 4 {
            let source_index = x0 as usize * 3;
            let mut red_acc = i64::from(src_row[source_index]) * weights[0];
            let mut green_acc = i64::from(src_row[source_index + 1]) * weights[0];
            let mut blue_acc = i64::from(src_row[source_index + 2]) * weights[0];
            let source_index = source_index + 3;
            red_acc += i64::from(src_row[source_index]) * weights[1];
            green_acc += i64::from(src_row[source_index + 1]) * weights[1];
            blue_acc += i64::from(src_row[source_index + 2]) * weights[1];
            let source_index = source_index + 3;
            red_acc += i64::from(src_row[source_index]) * weights[2];
            green_acc += i64::from(src_row[source_index + 1]) * weights[2];
            blue_acc += i64::from(src_row[source_index + 2]) * weights[2];
            let source_index = source_index + 3;
            red_acc += i64::from(src_row[source_index]) * weights[3];
            green_acc += i64::from(src_row[source_index + 1]) * weights[3];
            blue_acc += i64::from(src_row[source_index + 2]) * weights[3];
            (red_acc, green_acc, blue_acc)
        } else if cnt == 8 {
            let weights: &[i64; 8] = weights
                .try_into()
                .expect("an eight-tap coefficient span contains eight weights");
            let mut source_index = x0 as usize * 3;
            let mut red_acc = i64::from(src_row[source_index]) * weights[0];
            let mut green_acc = i64::from(src_row[source_index + 1]) * weights[0];
            let mut blue_acc = i64::from(src_row[source_index + 2]) * weights[0];
            source_index += 3;
            red_acc += i64::from(src_row[source_index]) * weights[1];
            green_acc += i64::from(src_row[source_index + 1]) * weights[1];
            blue_acc += i64::from(src_row[source_index + 2]) * weights[1];
            source_index += 3;
            red_acc += i64::from(src_row[source_index]) * weights[2];
            green_acc += i64::from(src_row[source_index + 1]) * weights[2];
            blue_acc += i64::from(src_row[source_index + 2]) * weights[2];
            source_index += 3;
            red_acc += i64::from(src_row[source_index]) * weights[3];
            green_acc += i64::from(src_row[source_index + 1]) * weights[3];
            blue_acc += i64::from(src_row[source_index + 2]) * weights[3];
            source_index += 3;
            red_acc += i64::from(src_row[source_index]) * weights[4];
            green_acc += i64::from(src_row[source_index + 1]) * weights[4];
            blue_acc += i64::from(src_row[source_index + 2]) * weights[4];
            source_index += 3;
            red_acc += i64::from(src_row[source_index]) * weights[5];
            green_acc += i64::from(src_row[source_index + 1]) * weights[5];
            blue_acc += i64::from(src_row[source_index + 2]) * weights[5];
            source_index += 3;
            red_acc += i64::from(src_row[source_index]) * weights[6];
            green_acc += i64::from(src_row[source_index + 1]) * weights[6];
            blue_acc += i64::from(src_row[source_index + 2]) * weights[6];
            source_index += 3;
            red_acc += i64::from(src_row[source_index]) * weights[7];
            green_acc += i64::from(src_row[source_index + 1]) * weights[7];
            blue_acc += i64::from(src_row[source_index + 2]) * weights[7];
            (red_acc, green_acc, blue_acc)
        } else {
            let mut red_acc = 0i64;
            let mut green_acc = 0i64;
            let mut blue_acc = 0i64;
            for (cix, &weight) in weights.iter().enumerate() {
                let source_index = (x0 + cix as i64) as usize * 3;
                red_acc += i64::from(src_row[source_index]) * weight;
                green_acc += i64::from(src_row[source_index + 1]) * weight;
                blue_acc += i64::from(src_row[source_index + 2]) * weight;
            }
            (red_acc, green_acc, blue_acc)
        };
        let destination_index = ox * 3;
        intermediate_row[destination_index] = fixed_point_to_u8(red_acc);
        intermediate_row[destination_index + 1] = fixed_point_to_u8(green_acc);
        intermediate_row[destination_index + 2] = fixed_point_to_u8(blue_acc);
    }
}
