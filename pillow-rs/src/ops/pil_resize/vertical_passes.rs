use super::alpha::unpremultiply_channel;
use super::coefficients::fixed_point_to_u8;
use super::*;

/// Resample one column into the output rows.
///
/// This is the vertical pass: it computes one column of the final image
/// from one column of the intermediate image. Uses PIL's fixed-point arithmetic.
/// Returns a single value per channel for this (x, y) position.
fn vertical_pass_col(
    intermediate: &[u8],
    _src_rows: u32,
    out_x: u32,
    out_w: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    out_y: usize,
) -> [u8; 4] {
    let y0 = coeffs.xmin[out_y];
    let cnt = coeffs.count[out_y];
    if cnt == 0 {
        return [0u8; 4];
    }
    let weights = coeffs.weights_for(out_y);
    let mut result = [0u8; 4];
    if channels == 3 {
        let x_index = out_x as usize * 3;
        let row_stride = out_w as usize * 3;
        let (red_acc, green_acc, blue_acc) = if cnt == 4 {
            let mut red_acc = 0i64;
            let mut green_acc = 0i64;
            let mut blue_acc = 0i64;
            let mut source_index = y0 as usize * row_stride + x_index;
            red_acc += i64::from(intermediate[source_index]) * weights[0];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[0];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[0];
            source_index += row_stride;
            red_acc += i64::from(intermediate[source_index]) * weights[1];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[1];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[1];
            source_index += row_stride;
            red_acc += i64::from(intermediate[source_index]) * weights[2];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[2];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[2];
            source_index += row_stride;
            red_acc += i64::from(intermediate[source_index]) * weights[3];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[3];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[3];
            (red_acc, green_acc, blue_acc)
        } else if cnt == 8 {
            let weights: &[i64; 8] = weights
                .try_into()
                .expect("an eight-tap coefficient span contains eight weights");
            let mut source_index = y0 as usize * row_stride + x_index;
            let mut red_acc = i64::from(intermediate[source_index]) * weights[0];
            let mut green_acc = i64::from(intermediate[source_index + 1]) * weights[0];
            let mut blue_acc = i64::from(intermediate[source_index + 2]) * weights[0];
            source_index += row_stride;
            red_acc += i64::from(intermediate[source_index]) * weights[1];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[1];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[1];
            source_index += row_stride;
            red_acc += i64::from(intermediate[source_index]) * weights[2];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[2];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[2];
            source_index += row_stride;
            red_acc += i64::from(intermediate[source_index]) * weights[3];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[3];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[3];
            source_index += row_stride;
            red_acc += i64::from(intermediate[source_index]) * weights[4];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[4];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[4];
            source_index += row_stride;
            red_acc += i64::from(intermediate[source_index]) * weights[5];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[5];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[5];
            source_index += row_stride;
            red_acc += i64::from(intermediate[source_index]) * weights[6];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[6];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[6];
            source_index += row_stride;
            red_acc += i64::from(intermediate[source_index]) * weights[7];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[7];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[7];
            (red_acc, green_acc, blue_acc)
        } else {
            let mut red_acc = 0i64;
            let mut green_acc = 0i64;
            let mut blue_acc = 0i64;
            for (cix, &weight) in weights.iter().enumerate() {
                let sy = (y0 + cix as i64) as usize;
                let source_index = sy * row_stride + x_index;
                red_acc += i64::from(intermediate[source_index]) * weight;
                green_acc += i64::from(intermediate[source_index + 1]) * weight;
                blue_acc += i64::from(intermediate[source_index + 2]) * weight;
            }
            (red_acc, green_acc, blue_acc)
        };
        result[0] = fixed_point_to_u8(red_acc);
        result[1] = fixed_point_to_u8(green_acc);
        result[2] = fixed_point_to_u8(blue_acc);
        return result;
    }

    if channels == 4 {
        let x_index = out_x as usize * 4;
        let row_stride = out_w as usize * 4;
        let mut red_acc = 0i64;
        let mut green_acc = 0i64;
        let mut blue_acc = 0i64;
        let mut alpha_acc = 0i64;
        for (tap, &weight) in weights.iter().enumerate() {
            let source_index = (y0 as usize + tap) * row_stride + x_index;
            red_acc += i64::from(intermediate[source_index]) * weight;
            green_acc += i64::from(intermediate[source_index + 1]) * weight;
            blue_acc += i64::from(intermediate[source_index + 2]) * weight;
            alpha_acc += i64::from(intermediate[source_index + 3]) * weight;
        }
        result[0] = fixed_point_to_u8(red_acc);
        result[1] = fixed_point_to_u8(green_acc);
        result[2] = fixed_point_to_u8(blue_acc);
        result[3] = fixed_point_to_u8(alpha_acc);
        return result;
    }

    for c in 0..channels {
        let mut acc: i64 = 0;
        for (cix, &w) in weights.iter().enumerate() {
            let sy = (y0 + cix as i64) as usize;
            let src_idx = (sy * out_w as usize + out_x as usize) * channels;
            acc += intermediate[src_idx + c] as i64 * w;
        }
        result[c] = fixed_point_to_u8(acc);
    }
    result
}

// A row-major intermediate makes the vertical pass revisit a different cache
// line for every source row and output column.  Above this size, one explicit
// transpose makes the samples for each output column contiguous.  Keep the
// small-image path unchanged: the extra allocation and copy are not amortized
// there, and preserving that path gives the benchmark a real crossover point.
const RESIZE_VERTICAL_TRANSPOSE_THRESHOLD: usize = 512 * 512;

#[inline]
pub(super) fn should_transpose_vertical(
    source_rows: u32,
    output_width: u32,
    channels: usize,
) -> bool {
    matches!(channels, 1..=4)
        && source_rows > 1
        && output_width > 1
        && (source_rows as usize).saturating_mul(output_width as usize)
            >= RESIZE_VERTICAL_TRANSPOSE_THRESHOLD
}

/// Reorder the horizontal-pass result from `[source_y][output_x][channel]` to
/// `[output_x][source_y][channel]` for the cache-local vertical pass.
pub(super) fn transpose_resize_intermediate(
    source: &[u8],
    source_rows: u32,
    output_width: u32,
    channels: usize,
) -> Vec<u8> {
    let source_row_stride = output_width as usize * channels;
    let destination_row_stride = source_rows as usize * channels;
    let mut destination = vec![0u8; source.len()];

    // RGB's three-byte pixels make the current column-at-a-time transpose
    // touch one source cache line per row and channel. Tile both dimensions so
    // each source tile is read in row-major order while the corresponding
    // transposed destination rows stay resident during their short write span.
    // Parallel CPU keeps its disjoint-column scheduling below.
    #[cfg(not(feature = "parallel"))]
    if channels == 3 {
        const TILE_WIDTH: usize = 32;
        const TILE_HEIGHT: usize = 32;
        let source_rows = source_rows as usize;
        let output_width = output_width as usize;
        for tile_y in (0..source_rows).step_by(TILE_HEIGHT) {
            let end_y = (tile_y + TILE_HEIGHT).min(source_rows);
            for tile_x in (0..output_width).step_by(TILE_WIDTH) {
                let end_x = (tile_x + TILE_WIDTH).min(output_width);
                for y in tile_y..end_y {
                    let source_row_start = y * source_row_stride;
                    for x in tile_x..end_x {
                        let source_start = source_row_start + x * 3;
                        let destination_start = x * destination_row_stride + y * 3;
                        destination[destination_start..destination_start + 3]
                            .copy_from_slice(&source[source_start..source_start + 3]);
                    }
                }
            }
        }
        return destination;
    }

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        &mut destination,
        destination_row_stride,
        output_width as usize,
        |_row_start, _row_end, x, row| {
            let x = x as usize;
            for y in 0..source_rows as usize {
                let source_start = y * source_row_stride + x * channels;
                let destination_start = y * channels;
                row[destination_start..destination_start + channels]
                    .copy_from_slice(&source[source_start..source_start + channels]);
            }
        }
    );

    #[cfg(not(feature = "parallel"))]
    for x in 0..output_width as usize {
        let row_start = x * destination_row_stride;
        let row = &mut destination[row_start..row_start + destination_row_stride];
        for y in 0..source_rows as usize {
            let source_start = y * source_row_stride + x * channels;
            let destination_start = y * channels;
            row[destination_start..destination_start + channels]
                .copy_from_slice(&source[source_start..source_start + channels]);
        }
    }

    destination
}

#[cfg(all(test, not(feature = "parallel")))]
mod resize_transpose_tests {
    use super::transpose_resize_intermediate;

    #[test]
    fn tiled_rgb_transpose_matches_reference_across_partial_tiles() {
        for (source_rows, output_width) in [(1u32, 1u32), (31, 33), (33, 31), (65, 67)] {
            let source = (0..source_rows as usize * output_width as usize * 3)
                .map(|index| (index.wrapping_mul(73).wrapping_add(index / 7 * 19)) as u8)
                .collect::<Vec<_>>();
            let mut expected = vec![0; source.len()];
            let source_stride = output_width as usize * 3;
            let destination_stride = source_rows as usize * 3;
            for x in 0..output_width as usize {
                for y in 0..source_rows as usize {
                    let source_start = y * source_stride + x * 3;
                    let destination_start = x * destination_stride + y * 3;
                    expected[destination_start..destination_start + 3]
                        .copy_from_slice(&source[source_start..source_start + 3]);
                }
            }

            assert_eq!(
                transpose_resize_intermediate(&source, source_rows, output_width, 3),
                expected,
                "source_rows={source_rows}, output_width={output_width}"
            );
        }
    }
}

#[inline]
fn vertical_pass_col_transposed(
    intermediate: &[u8],
    source_rows: u32,
    out_x: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    out_y: usize,
) -> [u8; 4] {
    let y0 = coeffs.xmin[out_y];
    let cnt = coeffs.count[out_y];
    if cnt == 0 {
        return [0u8; 4];
    }
    let weights = coeffs.weights_for(out_y);
    let column_start = out_x as usize * source_rows as usize * channels;
    let mut result = [0u8; 4];
    if channels == 3 {
        let column_start = out_x as usize * source_rows as usize * 3;
        let (red_acc, green_acc, blue_acc) = if cnt == 4 {
            let mut red_acc = 0i64;
            let mut green_acc = 0i64;
            let mut blue_acc = 0i64;
            let mut source_index = column_start + y0 as usize * 3;
            red_acc += i64::from(intermediate[source_index]) * weights[0];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[0];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[0];
            source_index += 3;
            red_acc += i64::from(intermediate[source_index]) * weights[1];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[1];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[1];
            source_index += 3;
            red_acc += i64::from(intermediate[source_index]) * weights[2];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[2];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[2];
            source_index += 3;
            red_acc += i64::from(intermediate[source_index]) * weights[3];
            green_acc += i64::from(intermediate[source_index + 1]) * weights[3];
            blue_acc += i64::from(intermediate[source_index + 2]) * weights[3];
            (red_acc, green_acc, blue_acc)
        } else {
            let mut red_acc = 0i64;
            let mut green_acc = 0i64;
            let mut blue_acc = 0i64;
            for (cix, &weight) in weights.iter().enumerate() {
                let sy = (y0 + cix as i64) as usize;
                let source_index = column_start + sy * 3;
                red_acc += i64::from(intermediate[source_index]) * weight;
                green_acc += i64::from(intermediate[source_index + 1]) * weight;
                blue_acc += i64::from(intermediate[source_index + 2]) * weight;
            }
            (red_acc, green_acc, blue_acc)
        };
        result[0] = fixed_point_to_u8(red_acc);
        result[1] = fixed_point_to_u8(green_acc);
        result[2] = fixed_point_to_u8(blue_acc);
        return result;
    }

    if channels == 4 {
        let mut red_acc = 0i64;
        let mut green_acc = 0i64;
        let mut blue_acc = 0i64;
        let mut alpha_acc = 0i64;
        let mut source_index = column_start + y0 as usize * 4;
        for &weight in weights {
            red_acc += i64::from(intermediate[source_index]) * weight;
            green_acc += i64::from(intermediate[source_index + 1]) * weight;
            blue_acc += i64::from(intermediate[source_index + 2]) * weight;
            alpha_acc += i64::from(intermediate[source_index + 3]) * weight;
            source_index += 4;
        }
        result[0] = fixed_point_to_u8(red_acc);
        result[1] = fixed_point_to_u8(green_acc);
        result[2] = fixed_point_to_u8(blue_acc);
        result[3] = fixed_point_to_u8(alpha_acc);
        return result;
    }

    for c in 0..channels {
        let mut acc: i64 = 0;
        for (cix, &w) in weights.iter().enumerate() {
            let sy = (y0 + cix as i64) as usize;
            let src_idx = column_start + sy * channels;
            acc += intermediate[src_idx + c] as i64 * w;
        }
        result[c] = fixed_point_to_u8(acc);
    }
    result
}

/// Resample one RGB output row while loading its vertical coefficients and
/// source-row bases only once. A boxed resize uses the same vertical filter
/// for every x coordinate in an output row, so repeating this setup in
/// `vertical_pass_col` needlessly multiplies it by the output width.
#[inline]
fn vertical_pass_rgb_row(
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

/// Return whether any source row referenced by a boxed vertical output is
/// marked nonzero. Invalid spans are treated as active so an incomplete hint
/// can only lose performance, never image contributions.
fn sparse_vertical_row_has_nonzero_source(
    coeffs: &FilterCoeffs,
    first_source_row: u32,
    nonzero_source_rows: &[bool],
    output_y: usize,
) -> bool {
    let Some(&first_relative_row) = coeffs.xmin.get(output_y) else {
        return true;
    };
    let Some(&tap_count) = coeffs.count.get(output_y) else {
        return true;
    };
    let Ok(first_relative_row) = usize::try_from(first_relative_row) else {
        return true;
    };
    let Some(first_absolute_row) = (first_source_row as usize).checked_add(first_relative_row)
    else {
        return true;
    };
    for tap in 0..tap_count {
        let Some(source_y) = first_absolute_row.checked_add(tap) else {
            return true;
        };
        let Some(&is_nonzero) = nonzero_source_rows.get(source_y) else {
            return true;
        };
        if is_nonzero {
            return true;
        }
    }
    false
}

/// Resample only RGB output rows whose boxed vertical tap span intersects a
/// nonzero source row. The output buffer is zero-initialized by the caller.
pub(super) fn vertical_pass_rows_sparse_rgb(
    intermediate: &[u8],
    output_width: u32,
    output_height: u32,
    coeffs: &FilterCoeffs,
    first_source_row: u32,
    nonzero_source_rows: &[bool],
    output: &mut [u8],
    parallel_pixel_threshold: usize,
) {
    let output_stride = output_width as usize * 3;
    if output_height == 0 || output_stride == 0 {
        return;
    }

    #[cfg(feature = "parallel")]
    if (output_width as usize).saturating_mul(output_height as usize) >= parallel_pixel_threshold {
        crate::par_rows_mut!(
            output,
            output_stride,
            output_height as usize,
            |_row_start, _row_end, y, row| {
                if sparse_vertical_row_has_nonzero_source(
                    coeffs,
                    first_source_row,
                    nonzero_source_rows,
                    y as usize,
                ) {
                    vertical_pass_rgb_row(intermediate, output_width, coeffs, y as usize, row);
                } else {
                    row[..output_stride].fill(0);
                }
            }
        );
        return;
    }

    #[cfg(not(feature = "parallel"))]
    let _ = parallel_pixel_threshold;

    for y in 0..output_height as usize {
        let output_start = y * output_stride;
        let row = &mut output[output_start..output_start + output_stride];
        if sparse_vertical_row_has_nonzero_source(coeffs, first_source_row, nonzero_source_rows, y)
        {
            vertical_pass_rgb_row(intermediate, output_width, coeffs, y, row);
        } else {
            row.fill(0);
        }
    }
}

pub(super) fn vertical_pass_rows(
    intermediate: &[u8],
    source_rows: u32,
    output_width: u32,
    output_height: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    output: &mut [u8],
    parallel_pixel_threshold: usize,
) {
    let _ = parallel_pixel_threshold;
    let output_stride = output_width as usize * channels;
    if output_height == 0 || output_stride == 0 {
        return;
    }

    #[cfg(feature = "parallel")]
    if (output_width as usize).saturating_mul(output_height as usize) < parallel_pixel_threshold {
        for y in 0..output_height as usize {
            let output_start = y * output_stride;
            let row = &mut output[output_start..output_start + output_stride];
            if channels == 3 {
                vertical_pass_rgb_row(intermediate, output_width, coeffs, y, row);
                continue;
            }
            for dx in 0..output_width {
                let value = vertical_pass_col(
                    intermediate,
                    source_rows,
                    dx,
                    output_width,
                    channels,
                    coeffs,
                    y,
                );
                let start = dx as usize * channels;
                row[start..start + channels].copy_from_slice(&value[..channels]);
            }
        }
        return;
    }

    if should_transpose_vertical(source_rows, output_width, channels) {
        let transposed =
            transpose_resize_intermediate(intermediate, source_rows, output_width, channels);
        vertical_pass_rows_transposed(
            &transposed,
            source_rows,
            output_width,
            output_height,
            channels,
            coeffs,
            output,
        );
        return;
    }

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        output,
        output_stride,
        output_height as usize,
        |_row_start, _row_end, y, row| {
            if channels == 3 {
                vertical_pass_rgb_row(
                    intermediate,
                    output_width,
                    coeffs,
                    y as usize,
                    &mut row[..output_stride],
                );
            } else {
                for dx in 0..output_width {
                    let value = vertical_pass_col(
                        intermediate,
                        source_rows,
                        dx,
                        output_width,
                        channels,
                        coeffs,
                        y as usize,
                    );
                    let start = dx as usize * channels;
                    row[start..start + channels].copy_from_slice(&value[..channels]);
                }
            }
        }
    );

    #[cfg(not(feature = "parallel"))]
    for y in 0..output_height as usize {
        let output_start = y * output_stride;
        let row = &mut output[output_start..output_start + output_stride];
        if channels == 3 {
            vertical_pass_rgb_row(intermediate, output_width, coeffs, y, row);
            continue;
        }
        for dx in 0..output_width {
            let value = vertical_pass_col(
                intermediate,
                source_rows,
                dx,
                output_width,
                channels,
                coeffs,
                y,
            );
            let start = dx as usize * channels;
            row[start..start + channels].copy_from_slice(&value[..channels]);
        }
    }
}

fn vertical_pass_rows_transposed(
    intermediate: &[u8],
    source_rows: u32,
    output_width: u32,
    output_height: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    output: &mut [u8],
) {
    let output_stride = output_width as usize * channels;

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        output,
        output_stride,
        output_height as usize,
        |_row_start, _row_end, y, row| {
            for dx in 0..output_width {
                let value = vertical_pass_col_transposed(
                    intermediate,
                    source_rows,
                    dx,
                    channels,
                    coeffs,
                    y as usize,
                );
                let start = dx as usize * channels;
                row[start..start + channels].copy_from_slice(&value[..channels]);
            }
        }
    );

    #[cfg(not(feature = "parallel"))]
    for y in 0..output_height as usize {
        let output_start = y * output_stride;
        let row = &mut output[output_start..output_start + output_stride];
        for dx in 0..output_width {
            let value =
                vertical_pass_col_transposed(intermediate, source_rows, dx, channels, coeffs, y);
            let start = dx as usize * channels;
            row[start..start + channels].copy_from_slice(&value[..channels]);
        }
    }
}

#[cfg(test)]
mod rgb_vertical_pass_tests {
    use super::{
        filter_from_resample, precompute_coeffs_boxed, vertical_pass_col, vertical_pass_rgb_row,
    };
    use crate::pipeline::ResampleFilter;

    #[test]
    fn rgb_row_kernel_matches_column_reference_for_boxed_filters_and_edges() {
        for filter in [
            ResampleFilter::Bilinear,
            ResampleFilter::Bicubic,
            ResampleFilter::Lanczos,
            ResampleFilter::Box,
            ResampleFilter::Hamming,
        ] {
            let (kernel, support) = filter_from_resample(filter);
            for (source_rows, output_height, output_width) in
                [(7u32, 3u32, 1u32), (19, 11, 7), (37, 23, 31)]
            {
                let coeffs = precompute_coeffs_boxed(
                    output_height,
                    source_rows,
                    0.0,
                    f64::from(source_rows),
                    kernel,
                    support,
                );
                let source = (0..source_rows as usize * output_width as usize * 3)
                    .map(|index| {
                        index
                            .wrapping_mul(71)
                            .wrapping_add(index / (output_width as usize * 3) * 43)
                            as u8
                    })
                    .collect::<Vec<_>>();
                for y in 0..output_height as usize {
                    let mut expected = vec![0u8; output_width as usize * 3];
                    for x in 0..output_width {
                        let pixel =
                            vertical_pass_col(&source, source_rows, x, output_width, 3, &coeffs, y);
                        let start = x as usize * 3;
                        expected[start..start + 3].copy_from_slice(&pixel[..3]);
                    }
                    let mut actual = vec![0u8; output_width as usize * 3];
                    vertical_pass_rgb_row(&source, output_width, &coeffs, y, &mut actual);
                    assert_eq!(
                        actual, expected,
                        "{filter:?} {source_rows}x{output_width} row {y}"
                    );
                }
            }
        }
    }
}

#[inline(always)]
fn write_vertical_pass_alpha_pixel(
    row: &mut [u8],
    start: usize,
    channels: usize,
    alpha_channel: usize,
    value: [u8; 4],
) {
    let alpha = value[alpha_channel];
    for channel in 0..channels {
        row[start + channel] = if channel == alpha_channel {
            alpha
        } else {
            unpremultiply_channel(value[channel], alpha)
        };
    }
}

#[inline]
pub(super) fn vertical_pass_rows_alpha(
    intermediate: &[u8],
    source_rows: u32,
    output_width: u32,
    output_height: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    output: &mut [u8],
    parallel_pixel_threshold: usize,
) {
    let _ = parallel_pixel_threshold;
    debug_assert!(matches!(channels, 2 | 4));
    let output_stride = output_width as usize * channels;
    let alpha_channel = channels - 1;
    if output_height == 0 || output_stride == 0 {
        return;
    }

    #[cfg(feature = "parallel")]
    if (output_width as usize).saturating_mul(output_height as usize) < parallel_pixel_threshold {
        for y in 0..output_height as usize {
            let output_start = y * output_stride;
            let row = &mut output[output_start..output_start + output_stride];
            for dx in 0..output_width {
                let value = vertical_pass_col(
                    intermediate,
                    source_rows,
                    dx,
                    output_width,
                    channels,
                    coeffs,
                    y,
                );
                let start = dx as usize * channels;
                write_vertical_pass_alpha_pixel(row, start, channels, alpha_channel, value);
            }
        }
        return;
    }

    if should_transpose_vertical(source_rows, output_width, channels) {
        let transposed =
            transpose_resize_intermediate(intermediate, source_rows, output_width, channels);
        vertical_pass_rows_alpha_transposed(
            &transposed,
            source_rows,
            output_width,
            output_height,
            channels,
            coeffs,
            output,
        );
        return;
    }

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        output,
        output_stride,
        output_height as usize,
        |_row_start, _row_end, y, row| {
            let y = y as usize;
            for dx in 0..output_width {
                let value = vertical_pass_col(
                    intermediate,
                    source_rows,
                    dx,
                    output_width,
                    channels,
                    coeffs,
                    y,
                );
                let start = dx as usize * channels;
                write_vertical_pass_alpha_pixel(row, start, channels, alpha_channel, value);
            }
        }
    );

    #[cfg(not(feature = "parallel"))]
    for y in 0..output_height as usize {
        let output_start = y * output_stride;
        let row = &mut output[output_start..output_start + output_stride];
        for dx in 0..output_width {
            let value = vertical_pass_col(
                intermediate,
                source_rows,
                dx,
                output_width,
                channels,
                coeffs,
                y,
            );
            let start = dx as usize * channels;
            write_vertical_pass_alpha_pixel(row, start, channels, alpha_channel, value);
        }
    }
}

fn vertical_pass_rows_alpha_transposed(
    intermediate: &[u8],
    source_rows: u32,
    output_width: u32,
    output_height: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    output: &mut [u8],
) {
    debug_assert!(matches!(channels, 2 | 4));
    let output_stride = output_width as usize * channels;
    let alpha_channel = channels - 1;

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        output,
        output_stride,
        output_height as usize,
        |_row_start, _row_end, y, row| {
            let y = y as usize;
            for dx in 0..output_width {
                let value = vertical_pass_col_transposed(
                    intermediate,
                    source_rows,
                    dx,
                    channels,
                    coeffs,
                    y,
                );
                let start = dx as usize * channels;
                let alpha = value[alpha_channel];
                for c in 0..channels {
                    row[start + c] = if c == alpha_channel {
                        alpha
                    } else {
                        unpremultiply_channel(value[c], alpha)
                    };
                }
            }
        }
    );

    #[cfg(not(feature = "parallel"))]
    for y in 0..output_height as usize {
        let output_start = y * output_stride;
        let row = &mut output[output_start..output_start + output_stride];
        for dx in 0..output_width {
            let value =
                vertical_pass_col_transposed(intermediate, source_rows, dx, channels, coeffs, y);
            let start = dx as usize * channels;
            let alpha = value[alpha_channel];
            for c in 0..channels {
                row[start + c] = if c == alpha_channel {
                    alpha
                } else {
                    unpremultiply_channel(value[c], alpha)
                };
            }
        }
    }
}
