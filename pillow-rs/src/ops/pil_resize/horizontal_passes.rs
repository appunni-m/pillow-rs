#[cfg(any(not(feature = "parallel"), test))]
use super::alpha::premultiply_alpha_row;
use super::alpha::premultiply_channel;
use super::coefficients::fixed_point_to_u8;
use super::*;

/// Resample one row of pixels into the output columns.
///
/// This is the horizontal pass: it computes one row of the intermediate image
/// from one row of the source image. Uses PIL's fixed-point arithmetic:
///   `result = (sum + (1 << (PRECISION_BITS - 1))) >> PRECISION_BITS`
pub(super) fn horizontal_pass_row(
    src_row: &[u8],
    _src_w: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    out_w: u32,
    intermediate_row: &mut [u8],
) {
    // Native RGB has no alpha conversion or mode-dependent channel mapping.
    // Accumulate its interleaved channels together so each coefficient and
    // source pixel is visited once instead of repeating the tap walk 3 times.
    if channels == 3 {
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
        return;
    }

    if channels == 4 {
        for ox in 0..out_w as usize {
            let x0 = coeffs.xmin[ox];
            let cnt = coeffs.count[ox];
            if cnt == 0 {
                continue;
            }
            let weights = coeffs.weights_for(ox);
            let mut red_acc = 0i64;
            let mut green_acc = 0i64;
            let mut blue_acc = 0i64;
            let mut alpha_acc = 0i64;
            for (tap, &weight) in weights.iter().enumerate() {
                let source_start = (x0 + tap as i64) as usize * 4;
                red_acc += i64::from(src_row[source_start]) * weight;
                green_acc += i64::from(src_row[source_start + 1]) * weight;
                blue_acc += i64::from(src_row[source_start + 2]) * weight;
                alpha_acc += i64::from(src_row[source_start + 3]) * weight;
            }
            let destination_start = ox * 4;
            intermediate_row[destination_start] = fixed_point_to_u8(red_acc);
            intermediate_row[destination_start + 1] = fixed_point_to_u8(green_acc);
            intermediate_row[destination_start + 2] = fixed_point_to_u8(blue_acc);
            intermediate_row[destination_start + 3] = fixed_point_to_u8(alpha_acc);
        }
        return;
    }

    for ox in 0..out_w as usize {
        let x0 = coeffs.xmin[ox];
        let cnt = coeffs.count[ox];
        if cnt == 0 {
            continue;
        }
        let weights = coeffs.weights_for(ox);
        for c in 0..channels {
            let mut acc: i64 = 0;
            for (cix, &w) in weights.iter().enumerate() {
                let sx = (x0 + cix as i64) as usize;
                acc += src_row[sx * channels + c] as i64 * w;
            }
            intermediate_row[ox * channels + c] = fixed_point_to_u8(acc);
        }
    }
}

#[inline]
pub(super) fn horizontal_pass_row_alpha(
    src_row: &[u8],
    channels: usize,
    coeffs: &FilterCoeffs,
    out_w: u32,
    intermediate_row: &mut [u8],
) {
    debug_assert!(matches!(channels, 2 | 4));
    let alpha_channel = channels - 1;
    for ox in 0..out_w as usize {
        let x0 = coeffs.xmin[ox];
        let cnt = coeffs.count[ox];
        if cnt == 0 {
            continue;
        }
        let weights = coeffs.weights_for(ox);
        for c in 0..channels {
            let mut acc: i64 = 0;
            for (cix, &w) in weights.iter().enumerate() {
                let sx = (x0 + cix as i64) as usize;
                let pixel_start = sx * channels;
                let source = src_row[pixel_start + c];
                let sample = if c == alpha_channel {
                    source
                } else {
                    premultiply_channel(source, src_row[pixel_start + alpha_channel])
                };
                acc += sample as i64 * w;
            }
            intermediate_row[ox * channels + c] = fixed_point_to_u8(acc);
        }
    }
}

/// Resample one column into the output rows.
///
/// This is the vertical pass: it computes one column of the final image
/// from one column of the intermediate image. Uses PIL's fixed-point arithmetic.
/// Returns a single value per channel for this (x, y) position.
pub(super) fn horizontal_pass_rows(
    work_bytes: &[u8],
    source_width: u32,
    source_height: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    output_width: u32,
    intermediate: &mut [u8],
) {
    let source_stride = source_width as usize * channels;
    let output_stride = output_width as usize * channels;
    if source_height == 0 || output_stride == 0 {
        return;
    }

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        intermediate,
        output_stride,
        source_height as usize,
        |row_start, _row_end, y, row| {
            let source_start = y as usize * source_stride;
            horizontal_pass_row(
                &work_bytes[source_start..source_start + source_stride],
                source_width,
                channels,
                coeffs,
                output_width,
                &mut row[..output_stride],
            );
            debug_assert_eq!(row_start, y as usize * output_stride);
        }
    );

    #[cfg(not(feature = "parallel"))]
    for y in 0..source_height as usize {
        let source_start = y * source_stride;
        let output_start = y * output_stride;
        horizontal_pass_row(
            &work_bytes[source_start..source_start + source_stride],
            source_width,
            channels,
            coeffs,
            output_width,
            &mut intermediate[output_start..output_start + output_stride],
        );
    }
}

pub(super) fn horizontal_pass_rows_alpha(
    source: &[u8],
    source_width: u32,
    source_height: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    output_width: u32,
    intermediate: &mut [u8],
) {
    let source_stride = source_width as usize * channels;
    let output_stride = output_width as usize * channels;
    if source_height == 0 || output_stride == 0 {
        return;
    }

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        intermediate,
        output_stride,
        source_height as usize,
        |_row_start, _row_end, y, row| {
            let source_start = y as usize * source_stride;
            horizontal_pass_row_alpha(
                &source[source_start..source_start + source_stride],
                channels,
                coeffs,
                output_width,
                &mut row[..output_stride],
            );
        }
    );

    #[cfg(not(feature = "parallel"))]
    let mut premultiplied_row = vec![0u8; source_stride];

    #[cfg(not(feature = "parallel"))]
    for y in 0..source_height as usize {
        let source_start = y * source_stride;
        let output_start = y * output_stride;
        premultiply_alpha_row(
            &source[source_start..source_start + source_stride],
            channels,
            &mut premultiplied_row,
        );
        horizontal_pass_row(
            &premultiplied_row,
            source_width,
            channels,
            coeffs,
            output_width,
            &mut intermediate[output_start..output_start + output_stride],
        );
    }
}

/// Boxed-resize variant that runs the horizontal pass only for source rows
/// referenced by the rebased vertical coefficient table.
pub(super) fn horizontal_pass_boxed_rows(
    work_bytes: &[u8],
    source_width: u32,
    first_source_row: u32,
    source_row_count: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    output_width: u32,
    intermediate: &mut [u8],
    premultiplied_alpha: bool,
    parallel_pixel_threshold: usize,
    sparse_rgb_nonzero_rows: Option<&[bool]>,
) {
    let source_stride = source_width as usize * channels;
    let output_stride = output_width as usize * channels;
    if source_row_count == 0 || output_stride == 0 {
        return;
    }

    if let Some(nonzero_rows) = sparse_rgb_nonzero_rows {
        #[cfg(feature = "parallel")]
        if (source_row_count as usize).saturating_mul(output_width as usize)
            >= parallel_pixel_threshold
        {
            crate::par_rows_mut!(
                intermediate,
                output_stride,
                source_row_count as usize,
                |_row_start, _row_end, row_index, row| {
                    let source_y = first_source_row as usize + row_index as usize;
                    let source_start = source_y * source_stride;
                    let source_row = &work_bytes[source_start..source_start + source_stride];
                    if !nonzero_rows.get(source_y).copied().unwrap_or(true) {
                        row[..output_stride].fill(0);
                    } else {
                        horizontal_pass_row(
                            source_row,
                            source_width,
                            channels,
                            coeffs,
                            output_width,
                            &mut row[..output_stride],
                        );
                    }
                }
            );
            return;
        }

        #[cfg(not(feature = "parallel"))]
        let _ = parallel_pixel_threshold;

        for row_index in 0..source_row_count as usize {
            let source_y = first_source_row as usize + row_index;
            let source_start = source_y * source_stride;
            let output_start = row_index * output_stride;
            let source_row = &work_bytes[source_start..source_start + source_stride];
            let output_row = &mut intermediate[output_start..output_start + output_stride];
            if !nonzero_rows.get(source_y).copied().unwrap_or(true) {
                output_row.fill(0);
            } else {
                horizontal_pass_row(
                    source_row,
                    source_width,
                    channels,
                    coeffs,
                    output_width,
                    output_row,
                );
            }
        }
        return;
    }

    let _ = parallel_pixel_threshold;

    #[cfg(feature = "parallel")]
    if (source_row_count as usize).saturating_mul(output_width as usize) < parallel_pixel_threshold
    {
        for row_index in 0..source_row_count as usize {
            let source_y = first_source_row as usize + row_index;
            let source_start = source_y * source_stride;
            let output_start = row_index * output_stride;
            let source_row = &work_bytes[source_start..source_start + source_stride];
            let output_row = &mut intermediate[output_start..output_start + output_stride];
            if premultiplied_alpha {
                horizontal_pass_row_alpha(source_row, channels, coeffs, output_width, output_row);
            } else {
                horizontal_pass_row(
                    source_row,
                    source_width,
                    channels,
                    coeffs,
                    output_width,
                    output_row,
                );
            }
        }
        return;
    }

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        intermediate,
        output_stride,
        source_row_count as usize,
        |_row_start, _row_end, row_index, row| {
            let source_y = first_source_row as usize + row_index as usize;
            let source_start = source_y * source_stride;
            let source_row = &work_bytes[source_start..source_start + source_stride];
            if premultiplied_alpha {
                horizontal_pass_row_alpha(
                    source_row,
                    channels,
                    coeffs,
                    output_width,
                    &mut row[..output_stride],
                );
            } else {
                horizontal_pass_row(
                    source_row,
                    source_width,
                    channels,
                    coeffs,
                    output_width,
                    &mut row[..output_stride],
                );
            }
        }
    );

    #[cfg(not(feature = "parallel"))]
    for row_index in 0..source_row_count as usize {
        let source_y = first_source_row as usize + row_index;
        let source_start = source_y * source_stride;
        let output_start = row_index * output_stride;
        let source_row = &work_bytes[source_start..source_start + source_stride];
        let output_row = &mut intermediate[output_start..output_start + output_stride];
        if premultiplied_alpha {
            horizontal_pass_row_alpha(source_row, channels, coeffs, output_width, output_row);
        } else {
            horizontal_pass_row(
                source_row,
                source_width,
                channels,
                coeffs,
                output_width,
                output_row,
            );
        }
    }
}

#[cfg(test)]
mod alpha_row_precompute_tests {
    use super::{
        horizontal_pass_row, horizontal_pass_row_alpha, precompute_coeffs, premultiply_alpha_row,
        unpremultiply_channel,
    };
    use crate::pipeline::ResampleFilter;

    #[test]
    fn premultiplying_each_source_pixel_once_matches_each_alpha_tap() {
        for channels in [2usize, 4] {
            for source_width in [1u32, 7, 16, 33] {
                let source = (0..source_width as usize * channels)
                    .map(|index| {
                        let pixel = index / channels;
                        let channel = index % channels;
                        if channel == channels - 1 {
                            match pixel % 4 {
                                0 => 0,
                                1 => 255,
                                _ => (pixel * 47 + 13) as u8,
                            }
                        } else {
                            (pixel * 83 + channel * 61 + 19) as u8
                        }
                    })
                    .collect::<Vec<_>>();

                for output_width in [1u32, 3, 9, 17, 45] {
                    for filter in [
                        ResampleFilter::Bilinear,
                        ResampleFilter::Bicubic,
                        ResampleFilter::Lanczos,
                    ] {
                        let coeffs = precompute_coeffs(output_width, source_width, filter);
                        let output_len = output_width as usize * channels;
                        let mut expected = vec![0; output_len];
                        horizontal_pass_row_alpha(
                            &source,
                            channels,
                            &coeffs,
                            output_width,
                            &mut expected,
                        );

                        let mut premultiplied = vec![0; source.len()];
                        premultiply_alpha_row(&source, channels, &mut premultiplied);
                        let mut actual = vec![0; output_len];
                        horizontal_pass_row(
                            &premultiplied,
                            source_width,
                            channels,
                            &coeffs,
                            output_width,
                            &mut actual,
                        );
                        assert_eq!(
                            actual, expected,
                            "channels={channels}, {source_width}x1 -> {output_width}x1, {filter:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn integer_unpremultiplication_matches_pillow_float_for_every_byte_pair() {
        for alpha in 0..=u8::MAX {
            for value in 0..=u8::MAX {
                let expected = if alpha == 0 {
                    value
                } else {
                    (f64::from(value) * 255.0 / f64::from(alpha)) as u8
                };
                assert_eq!(
                    unpremultiply_channel(value, alpha),
                    expected,
                    "value={value}, alpha={alpha}"
                );
            }
        }
    }
}
