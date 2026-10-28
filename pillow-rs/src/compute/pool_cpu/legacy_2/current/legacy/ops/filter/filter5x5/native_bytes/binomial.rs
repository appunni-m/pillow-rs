pub(crate) fn binomial5x5_parameters_match(kernel: &[f32; 25], scale: f32, offset: f32) -> bool {
    const WEIGHTS: [f32; 5] = [1.0, 4.0, 6.0, 4.0, 1.0];
    scale == 256.0
        && offset == 0.0
        && kernel
            .iter()
            .enumerate()
            .all(|(index, coefficient)| *coefficient == WEIGHTS[index / 5] * WEIGHTS[index % 5])
}

/// Apply the exact L-mode 5x5 binomial kernel in parallel row bands.
///
/// Each band owns its horizontal-sum ring and recomputes the four input rows
/// shared with neighboring bands. The indexed block collector preserves row
/// order; the copied source rows retain Pillow's unchanged two-pixel border.
#[cfg(feature = "parallel")]
pub(in crate::compute::pool_cpu::ops::filter) fn filter_5x5_binomial_luma_parallel_rows(
    raw: &[u8],
    width: usize,
    height: usize,
) -> Option<Vec<u8>> {
    let pixel_count = width.checked_mul(height)?;
    if raw.len() != pixel_count {
        return None;
    }
    if width < 5 || height < 5 {
        return Some(raw.to_vec());
    }

    const WEIGHTS: [u32; 5] = [1, 4, 6, 4, 1];
    const BLOCK_ROWS: usize = 64;
    let horizontal_len = width.checked_mul(5)?;
    let interior_rows = height - 4;
    let block_count = interior_rows.div_ceil(BLOCK_ROWS);
    let blocks = crate::par_row_blocks_collect!(
        block_count,
        1,
        || vec![0_u16; horizontal_len],
        |horizontal_rows, block_index| {
            let start_y = 2 + block_index * BLOCK_ROWS;
            let end_y = (start_y + BLOCK_ROWS).min(height - 2);
            let mut output = Vec::with_capacity((end_y - start_y) * width);
            let fill_horizontal_row = |source_y: usize, destination: &mut [u16]| {
                let row_start = source_y * width;
                for x in 2..width - 2 {
                    destination[x] = u16::from(raw[row_start + x - 2])
                        + u16::from(raw[row_start + x - 1]) * 4
                        + u16::from(raw[row_start + x]) * 6
                        + u16::from(raw[row_start + x + 1]) * 4
                        + u16::from(raw[row_start + x + 2]);
                }
            };

            for source_y in start_y - 2..start_y + 3 {
                let ring_start = (source_y % 5) * width;
                fill_horizontal_row(
                    source_y,
                    &mut horizontal_rows[ring_start..ring_start + width],
                );
            }
            for y in start_y..end_y {
                let source_row = y * width;
                output.extend_from_slice(&raw[source_row..source_row + width]);
                let output_row = output.len() - width;
                let row_starts = [
                    ((y - 2) % 5) * width,
                    ((y - 1) % 5) * width,
                    (y % 5) * width,
                    ((y + 1) % 5) * width,
                    ((y + 2) % 5) * width,
                ];
                for x in 2..width - 2 {
                    let sum = horizontal_rows[row_starts[0] + x] as u32 * WEIGHTS[0]
                        + horizontal_rows[row_starts[1] + x] as u32 * WEIGHTS[1]
                        + horizontal_rows[row_starts[2] + x] as u32 * WEIGHTS[2]
                        + horizontal_rows[row_starts[3] + x] as u32 * WEIGHTS[3]
                        + horizontal_rows[row_starts[4] + x] as u32 * WEIGHTS[4];
                    output[output_row + x] = ((sum + 128) / 256) as u8;
                }

                let next_source_y = y + 3;
                if next_source_y < height {
                    let ring_start = (next_source_y % 5) * width;
                    fill_horizontal_row(
                        next_source_y,
                        &mut horizontal_rows[ring_start..ring_start + width],
                    );
                }
            }
            output
        }
    );

    let mut output = Vec::with_capacity(pixel_count);
    output.extend_from_slice(&raw[..2 * width]);
    for block in blocks {
        output.extend_from_slice(&block);
    }
    output.extend_from_slice(&raw[(height - 2) * width..]);
    (output.len() == pixel_count).then_some(output)
}

/// Apply the exact LA-mode 5x5 binomial kernel in parallel row bands.
///
/// Each band owns a five-row horizontal-sum ring for both interleaved bands.
/// The block collector preserves row order, while copying source rows before
/// updating interior pixels retains Pillow's unchanged two-pixel border.
#[cfg(feature = "parallel")]
pub(in crate::compute::pool_cpu::ops::filter) fn filter_5x5_binomial_la_parallel_rows(
    raw: &[u8],
    width: usize,
    height: usize,
) -> Option<Vec<u8>> {
    let pixel_count = width.checked_mul(height)?;
    let row_samples = width.checked_mul(2)?;
    let sample_count = pixel_count.checked_mul(2)?;
    if raw.len() != sample_count {
        return None;
    }
    if width < 5 || height < 5 {
        return Some(raw.to_vec());
    }

    const WEIGHTS: [u32; 5] = [1, 4, 6, 4, 1];
    const BLOCK_ROWS: usize = 64;
    let ring_len = row_samples.checked_mul(5)?;
    let interior_rows = height - 4;
    let block_count = interior_rows.div_ceil(BLOCK_ROWS);
    let blocks = crate::par_row_blocks_collect!(
        block_count,
        1,
        || vec![0_u16; ring_len],
        |horizontal_rows, block_index| {
            let start_y = 2 + block_index * BLOCK_ROWS;
            let end_y = (start_y + BLOCK_ROWS).min(height - 2);
            let mut output = Vec::with_capacity((end_y - start_y) * row_samples);
            let fill_horizontal_row = |source_y: usize, destination: &mut [u16]| {
                let row_start = source_y * row_samples;
                for x in 2..width - 2 {
                    let pixel = row_start + x * 2;
                    for channel in 0..2 {
                        let sample = pixel + channel;
                        destination[x * 2 + channel] = u16::from(raw[sample - 4])
                            + u16::from(raw[sample - 2]) * 4
                            + u16::from(raw[sample]) * 6
                            + u16::from(raw[sample + 2]) * 4
                            + u16::from(raw[sample + 4]);
                    }
                }
            };

            for source_y in start_y - 2..start_y + 3 {
                let ring_start = (source_y % 5) * row_samples;
                fill_horizontal_row(
                    source_y,
                    &mut horizontal_rows[ring_start..ring_start + row_samples],
                );
            }
            for y in start_y..end_y {
                let source_row = y * row_samples;
                output.extend_from_slice(&raw[source_row..source_row + row_samples]);
                let output_row = output.len() - row_samples;
                let row_starts = [
                    ((y - 2) % 5) * row_samples,
                    ((y - 1) % 5) * row_samples,
                    (y % 5) * row_samples,
                    ((y + 1) % 5) * row_samples,
                    ((y + 2) % 5) * row_samples,
                ];
                for x in 2..width - 2 {
                    let pixel = x * 2;
                    for channel in 0..2 {
                        let sample = pixel + channel;
                        let sum = horizontal_rows[row_starts[0] + sample] as u32 * WEIGHTS[0]
                            + horizontal_rows[row_starts[1] + sample] as u32 * WEIGHTS[1]
                            + horizontal_rows[row_starts[2] + sample] as u32 * WEIGHTS[2]
                            + horizontal_rows[row_starts[3] + sample] as u32 * WEIGHTS[3]
                            + horizontal_rows[row_starts[4] + sample] as u32 * WEIGHTS[4];
                        output[output_row + sample] = ((sum + 128) / 256) as u8;
                    }
                }

                let next_source_y = y + 3;
                if next_source_y < height {
                    let ring_start = (next_source_y % 5) * row_samples;
                    fill_horizontal_row(
                        next_source_y,
                        &mut horizontal_rows[ring_start..ring_start + row_samples],
                    );
                }
            }
            output
        }
    );

    let mut output = Vec::with_capacity(sample_count);
    output.extend_from_slice(&raw[..2 * row_samples]);
    for block in blocks {
        output.extend_from_slice(&block);
    }
    output.extend_from_slice(&raw[(height - 2) * row_samples..]);
    (output.len() == sample_count).then_some(output)
}

/// Apply Pillow's 5x5 binomial blur to native L bytes using exact integer
/// arithmetic and five reusable horizontal rows. For this kernel, every
/// normalized coefficient is an exact multiple of 1/256 and the largest
/// weighted sum is 65,280, so the ordered f32 accumulation is exactly
/// representable and Pillow's final `+0.5` truncation equals `(sum + 128) / 256`.
#[cfg(not(feature = "parallel"))]
pub(in crate::compute::pool_cpu::ops::filter) fn filter_5x5_binomial_luma_rows(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
) -> bool {
    let Some(pixel_count) = width.checked_mul(height) else {
        return false;
    };
    if raw.len() != pixel_count || out.len() != pixel_count {
        return false;
    }
    if width < 5 || height < 5 {
        return true;
    }

    const WEIGHTS: [u32; 5] = [1, 4, 6, 4, 1];
    let mut horizontal_rows: [Vec<u16>; 5] = std::array::from_fn(|_| vec![0; width]);
    let fill_horizontal_row = |source_y: usize, destination: &mut [u16]| {
        let row_start = source_y * width;
        for x in 2..width - 2 {
            destination[x] = u16::from(raw[row_start + x - 2])
                + u16::from(raw[row_start + x - 1]) * 4
                + u16::from(raw[row_start + x]) * 6
                + u16::from(raw[row_start + x + 1]) * 4
                + u16::from(raw[row_start + x + 2]);
        }
    };

    for source_y in 0..5 {
        fill_horizontal_row(source_y, &mut horizontal_rows[source_y]);
    }
    for y in 2..height - 2 {
        let rows = [
            &horizontal_rows[(y - 2) % 5],
            &horizontal_rows[(y - 1) % 5],
            &horizontal_rows[y % 5],
            &horizontal_rows[(y + 1) % 5],
            &horizontal_rows[(y + 2) % 5],
        ];
        let output_row = y * width;
        for x in 2..width - 2 {
            let sum = rows[0][x] as u32 * WEIGHTS[0]
                + rows[1][x] as u32 * WEIGHTS[1]
                + rows[2][x] as u32 * WEIGHTS[2]
                + rows[3][x] as u32 * WEIGHTS[3]
                + rows[4][x] as u32 * WEIGHTS[4];
            out[output_row + x] = ((sum + 128) / 256) as u8;
        }

        let next_source_y = y + 3;
        if next_source_y < height {
            fill_horizontal_row(next_source_y, &mut horizontal_rows[next_source_y % 5]);
        }
    }
    true
}

/// Apply the exact binomial kernel to native LA samples. Luminance and alpha
/// are independent Pillow filter bands, so the ring stores both interleaved
/// samples and never widens or reinterprets the image as RGBA.
#[cfg(not(feature = "parallel"))]
pub(in crate::compute::pool_cpu::ops::filter) fn filter_5x5_binomial_la_rows(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
) -> bool {
    let Some(pixel_count) = width.checked_mul(height) else {
        return false;
    };
    let Some(sample_count) = pixel_count.checked_mul(2) else {
        return false;
    };
    if raw.len() != sample_count || out.len() != sample_count {
        return false;
    }
    if width < 5 || height < 5 {
        return true;
    }

    const WEIGHTS: [u32; 5] = [1, 4, 6, 4, 1];
    let row_samples = width * 2;
    let mut horizontal_rows: [Vec<u16>; 5] = std::array::from_fn(|_| vec![0; row_samples]);
    let fill_horizontal_row = |source_y: usize, destination: &mut [u16]| {
        let row_start = source_y * row_samples;
        for x in 2..width - 2 {
            let base = row_start + x * 2;
            for channel in 0..2 {
                let sample = base + channel;
                let outer = u16::from(raw[sample - 4]) + u16::from(raw[sample + 4]);
                let inner = u16::from(raw[sample - 2]) + u16::from(raw[sample + 2]);
                destination[x * 2 + channel] = outer + inner * 4 + u16::from(raw[sample]) * 6;
            }
        }
    };

    for source_y in 0..5 {
        fill_horizontal_row(source_y, &mut horizontal_rows[source_y]);
    }
    for y in 2..height - 2 {
        let rows = [
            &horizontal_rows[(y - 2) % 5],
            &horizontal_rows[(y - 1) % 5],
            &horizontal_rows[y % 5],
            &horizontal_rows[(y + 1) % 5],
            &horizontal_rows[(y + 2) % 5],
        ];
        let output_row = y * row_samples;
        for x in 2..width - 2 {
            let base = x * 2;
            for channel in 0..2 {
                let sample = base + channel;
                let outer = u32::from(rows[0][sample]) + u32::from(rows[4][sample]);
                let inner = u32::from(rows[1][sample]) + u32::from(rows[3][sample]);
                let center = u32::from(rows[2][sample]);
                let sum = outer + inner * WEIGHTS[1] + center * WEIGHTS[2];
                out[output_row + sample] = ((sum + 128) / 256) as u8;
            }
        }

        let next_source_y = y + 3;
        if next_source_y < height {
            fill_horizontal_row(next_source_y, &mut horizontal_rows[next_source_y % 5]);
        }
    }
    true
}
