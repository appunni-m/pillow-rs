pub(crate) fn binomial5x5_parameters_match(kernel: &[f32; 25], scale: f32, offset: f32) -> bool {
    const WEIGHTS: [f32; 5] = [1.0, 4.0, 6.0, 4.0, 1.0];
    scale == 256.0
        && offset == 0.0
        && kernel
            .iter()
            .enumerate()
            .all(|(index, coefficient)| *coefficient == WEIGHTS[index / 5] * WEIGHTS[index % 5])
}

/// Apply the exact L-mode 5x5 binomial kernel with a rolling row window.
/// The copied source rows retain Pillow's unchanged two-pixel border.
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
