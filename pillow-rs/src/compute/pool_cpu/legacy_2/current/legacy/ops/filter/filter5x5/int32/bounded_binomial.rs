#[cfg(feature = "parallel")]
use super::read_i32_pixel_le;
// A native I-mode convolution can amortize a uniformity proof over a larger
// image than the byte filters: each interior output otherwise reads and
// converts 25 four-byte samples. Keep the proof bounded so a nearly-uniform
// large image cannot add an unbounded extra full-frame scan.
pub(super) const I32_UNIFORM_FILTER5X5_MAX_PIXELS: usize = 1024 * 1024;
pub(super) const I32_BINOMIAL_FILTER5X5_WEIGHTS: [i32; 5] = [1, 4, 6, 4, 1];
pub(super) const I32_BINOMIAL_FILTER5X5_MAX_ABS_SAMPLE: u32 = 65_535;

pub(super) fn i32_filter5x5_binomial_kernel_is_applicable(kernel: &[f32; 25]) -> bool {
    for (y, vertical) in I32_BINOMIAL_FILTER5X5_WEIGHTS.iter().enumerate() {
        for (x, horizontal) in I32_BINOMIAL_FILTER5X5_WEIGHTS.iter().enumerate() {
            let expected = (*vertical * *horizontal) as f32 / 256.0;
            if kernel[y * 5 + x] != expected {
                return false;
            }
        }
    }
    true
}

/// Whether an I-mode input can use the exact integer form of the normalized
/// binomial 5x5 kernel. The 16-bit magnitude bound keeps every weighted
/// partial sum below 2^24, so Pillow's f32 path is exact before its +0.5 cast.
#[cfg(feature = "parallel")]
pub(crate) fn i32_filter5x5_bounded_binomial_is_applicable(
    raw: &[u8],
    width: usize,
    height: usize,
    kernel: &[f32; 25],
    offset: f32,
) -> bool {
    if width < 5 || height < 5 || offset != 0.0 {
        return false;
    }
    let Some(pixel_count) = width.checked_mul(height) else {
        return false;
    };
    let Some(byte_count) = pixel_count.checked_mul(4) else {
        return false;
    };
    if raw.len() != byte_count {
        return false;
    }
    if !i32_filter5x5_binomial_kernel_is_applicable(kernel) {
        return false;
    }
    raw.chunks_exact(4).all(|pixel| {
        let value = i32::from_le_bytes([pixel[0], pixel[1], pixel[2], pixel[3]]);
        value.unsigned_abs() <= I32_BINOMIAL_FILTER5X5_MAX_ABS_SAMPLE
    })
}

#[cfg(not(feature = "parallel"))]
pub(super) fn i32_filter5x5_bounded_binomial_kernel_is_applicable(
    sample_count: usize,
    width: usize,
    height: usize,
    kernel: &[f32; 25],
    offset: f32,
) -> bool {
    if width < 5 || height < 5 || offset != 0.0 {
        return false;
    }
    let Some(pixel_count) = width.checked_mul(height) else {
        return false;
    };
    if sample_count != pixel_count {
        return false;
    }
    i32_filter5x5_binomial_kernel_is_applicable(kernel)
}

/// Evaluate the exact integer factorization of the binomial I-mode kernel.
/// Five horizontal rows form a ring buffer; each source row is filtered once
/// and reused by the five vertical contributions. The returned bytes preserve
/// Pillow's copied two-pixel border and positive-only I-mode clipping.
#[cfg(feature = "parallel")]
pub(super) fn filter_5x5_i32_bounded_binomial(
    raw: &[u8],
    width: usize,
    height: usize,
) -> Option<Vec<u8>> {
    let ring_len = width.checked_mul(5)?;
    let mut horizontal_rows = vec![0_i32; ring_len];
    let mut output = raw.to_vec();

    let horizontal_row = |source_y: usize, row: &mut [i32]| {
        let row_start = source_y * width;
        let mut window = [
            read_i32_pixel_le(raw, row_start),
            read_i32_pixel_le(raw, row_start + 1),
            read_i32_pixel_le(raw, row_start + 2),
            read_i32_pixel_le(raw, row_start + 3),
            read_i32_pixel_le(raw, row_start + 4),
        ];
        for x in 2..width - 2 {
            let sum = window[0] + window[1] * 4 + window[2] * 6 + window[3] * 4 + window[4];
            row[x] = sum;
            if x + 1 < width - 2 {
                window[0] = window[1];
                window[1] = window[2];
                window[2] = window[3];
                window[3] = window[4];
                window[4] = read_i32_pixel_le(raw, row_start + x + 3);
            }
        }
    };

    for source_y in 0..5 {
        horizontal_row(
            source_y,
            &mut horizontal_rows[source_y % 5 * width..(source_y % 5 + 1) * width],
        );
    }
    for y in 2..height - 2 {
        let bottom = (y + 2) % 5 * width;
        let lower = (y + 1) % 5 * width;
        let center = y % 5 * width;
        let upper = (y - 1) % 5 * width;
        let top = (y - 2) % 5 * width;
        for x in 2..width - 2 {
            let sum = horizontal_rows[bottom + x]
                + horizontal_rows[lower + x] * 4
                + horizontal_rows[center + x] * 6
                + horizontal_rows[upper + x] * 4
                + horizontal_rows[top + x];
            let filtered = if sum >= 0 { (sum + 128) / 256 } else { 0 };
            let pixel = (y * width + x) * 4;
            output[pixel..pixel + 4].copy_from_slice(&filtered.to_le_bytes());
        }

        let next_source_y = y + 3;
        if next_source_y < height {
            let slot = next_source_y % 5;
            horizontal_row(
                next_source_y,
                &mut horizontal_rows[slot * width..(slot + 1) * width],
            );
        }
    }
    Some(output)
}
