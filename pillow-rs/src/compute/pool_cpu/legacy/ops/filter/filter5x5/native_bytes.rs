//! Native-byte implementation for Pillow's 5x5 convolution filter.

use super::super::convolution::{clip8_filter, pillow_kernel_row_5};
#[cfg(not(feature = "parallel"))]
use super::SMALL_UNIFORM_BYTE_MAX_PIXELS;
#[cfg(not(feature = "parallel"))]
use super::{rgb_blur_ring, uniform_box};

/// Apply one row of the byte 5x5 convolution. The arithmetic and tap order
/// intentionally match the original scalar kernel exactly.
#[inline]
pub(crate) fn filter_5x5_byte_row(
    raw: &[u8],
    row: &mut [u8],
    y: i32,
    width: i32,
    height: i32,
    channels: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) {
    if y < 2 || y >= height - 2 || width < 5 {
        return;
    }
    for x in 2..width - 2 {
        let base =
            |dx: i32, dy: i32| -> usize { ((y + dy) * width + (x + dx)) as usize * channels };
        for c in 0..channels {
            let row0 = pillow_kernel_row_5(
                [
                    raw[base(-2, 2) + c] as f32,
                    raw[base(-1, 2) + c] as f32,
                    raw[base(0, 2) + c] as f32,
                    raw[base(1, 2) + c] as f32,
                    raw[base(2, 2) + c] as f32,
                ],
                &kernel[0..5],
            );
            let row1 = pillow_kernel_row_5(
                [
                    raw[base(-2, 1) + c] as f32,
                    raw[base(-1, 1) + c] as f32,
                    raw[base(0, 1) + c] as f32,
                    raw[base(1, 1) + c] as f32,
                    raw[base(2, 1) + c] as f32,
                ],
                &kernel[5..10],
            );
            let row2 = pillow_kernel_row_5(
                [
                    raw[base(-2, 0) + c] as f32,
                    raw[base(-1, 0) + c] as f32,
                    raw[base(0, 0) + c] as f32,
                    raw[base(1, 0) + c] as f32,
                    raw[base(2, 0) + c] as f32,
                ],
                &kernel[10..15],
            );
            let row3 = pillow_kernel_row_5(
                [
                    raw[base(-2, -1) + c] as f32,
                    raw[base(-1, -1) + c] as f32,
                    raw[base(0, -1) + c] as f32,
                    raw[base(1, -1) + c] as f32,
                    raw[base(2, -1) + c] as f32,
                ],
                &kernel[15..20],
            );
            let row4 = pillow_kernel_row_5(
                [
                    raw[base(-2, -2) + c] as f32,
                    raw[base(-1, -2) + c] as f32,
                    raw[base(0, -2) + c] as f32,
                    raw[base(1, -2) + c] as f32,
                    raw[base(2, -2) + c] as f32,
                ],
                &kernel[20..25],
            );
            let mut ss = rounding_bias;
            ss += row0;
            ss += row1;
            ss += row2;
            ss += row3;
            ss += row4;
            row[x as usize * channels + c] = clip8_filter(ss);
        }
    }
}

#[cfg_attr(not(feature = "parallel"), inline(never))]
pub(super) fn filter_5x5_byte_rows(
    raw: &[u8],
    out: &mut [u8],
    width: i32,
    height: i32,
    channels: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) {
    #[cfg(not(feature = "parallel"))]
    if uniform_box::apply_if_uniform(
        raw,
        out,
        width.max(0) as usize,
        height.max(0) as usize,
        channels,
        kernel,
        rounding_bias,
    ) {
        return;
    }
    let row_stride = width.max(0) as usize * channels;
    if row_stride == 0 || height <= 0 {
        return;
    }
    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        out,
        row_stride,
        height.max(0) as usize,
        |_row_start, _row_end, y, row| {
            filter_5x5_byte_row(
                raw,
                row,
                y as i32,
                width,
                height,
                channels,
                kernel,
                rounding_bias,
            );
        }
    );
    #[cfg(not(feature = "parallel"))]
    for y in 0..height {
        let row_start = y.max(0) as usize * row_stride;
        filter_5x5_byte_row(
            raw,
            &mut out[row_start..row_start + row_stride],
            y,
            width,
            height,
            channels,
            kernel,
            rounding_bias,
        );
    }
}

/// Evaluate one constant native-byte 5x5 convolution tuple in the same tap and
/// row-add order as [`filter_5x5_byte_row`].
#[inline]
fn filter_5x5_constant_pixel(
    samples: &[u8],
    channels: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) -> [u8; 4] {
    let mut filtered = [0u8; 4];
    for channel in 0..channels {
        let sample = samples[channel] as f32;
        let row0 = pillow_kernel_row_5([sample; 5], &kernel[0..5]);
        let row1 = pillow_kernel_row_5([sample; 5], &kernel[5..10]);
        let row2 = pillow_kernel_row_5([sample; 5], &kernel[10..15]);
        let row3 = pillow_kernel_row_5([sample; 5], &kernel[15..20]);
        let row4 = pillow_kernel_row_5([sample; 5], &kernel[20..25]);
        let mut value = rounding_bias;
        value += row0;
        value += row1;
        value += row2;
        value += row3;
        value += row4;
        filtered[channel] = clip8_filter(value);
    }
    filtered
}

/// Evaluate a constant native-byte 5x5 convolution with one computed interior
/// tuple and the same untouched border contract as the regular scalar path.
pub(super) fn filter_5x5_constant_bytes(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) {
    let filtered = filter_5x5_constant_pixel(raw, channels, kernel, rounding_bias);
    let row_stride = width * channels;
    for y in 2..height.saturating_sub(2) {
        for x in 2..width.saturating_sub(2) {
            let base = y * row_stride + x * channels;
            out[base..base + channels].copy_from_slice(&filtered[..channels]);
        }
    }
}

/// Return whether a large RGBA raster is uniform and its exact 5x5 output
/// tuple is byte-identical to the source tuple. The untouched two-pixel border
/// is already preserved by the caller's output copy.
#[cfg(not(feature = "parallel"))]
pub(super) fn filter_5x5_uniform_rgba_identity(
    raw: &[u8],
    width: usize,
    height: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) -> bool {
    let Some(pixel_count) = width.checked_mul(height) else {
        return false;
    };
    if pixel_count <= SMALL_UNIFORM_BYTE_MAX_PIXELS || width < 5 || height < 5 {
        return false;
    }
    let Some(byte_count) = pixel_count.checked_mul(4) else {
        return false;
    };
    if raw.len() != byte_count {
        return false;
    }
    let Some(first_bytes) = raw.get(..4) else {
        return false;
    };
    if !raw[4..].chunks_exact(4).all(|pixel| pixel == first_bytes) {
        return false;
    }
    let filtered = filter_5x5_constant_pixel(first_bytes, 4, kernel, rounding_bias);
    filtered.as_slice() == first_bytes
}

pub(crate) fn uniform5x5_parameters_match(kernel: &[f32; 25], scale: f32, offset: f32) -> bool {
    scale == 25.0 && offset == 0.0 && kernel.iter().all(|&coefficient| coefficient == 1.0)
}

#[cfg(not(feature = "parallel"))]
pub(crate) fn rgb_blur_ring5x5_parameters_match(
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
) -> bool {
    rgb_blur_ring::parameters_match(kernel, scale, offset)
}

pub(crate) fn la_single_outlier5x5_parameters_match(
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
) -> bool {
    scale == 25.0
        && offset == 0.0
        && kernel[..24].iter().all(|&coefficient| coefficient == 1.0)
        && kernel[24] == 2.0
}

/// Apply the native-LA 5x5 kernel whose final tap is 2 and whose other taps
/// are 1, using rolling integer sums.
///
/// The exact weighted numerator is the 5x5 byte sum plus the final tap's
/// sample. The normalized f32 coefficients and ordered FMA error stay below
/// 0.001 at the maximum result (265.2), while a denominator-25 integer sum is
/// at least 0.02 from a half-byte rounding boundary. Therefore Pillow's
/// rounded byte is exactly `(sum + 12) / 25`, clipped to a byte. The copied
/// two-pixel border and native interleaved L/A bytes are preserved.
#[cfg(not(feature = "parallel"))]
pub(super) fn filter_5x5_la_single_outlier_rows(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
) -> bool {
    let Some(row_stride) = width.checked_mul(2) else {
        return false;
    };
    let Some(sample_count) = row_stride.checked_mul(height) else {
        return false;
    };
    if raw.len() != sample_count || out.len() != sample_count {
        return false;
    }
    if width < 5 || height < 5 {
        return true;
    }

    let mut horizontal_rows: [Vec<u16>; 5] = std::array::from_fn(|_| vec![0; row_stride]);
    let fill_horizontal_row = |source_y: usize, destination: &mut [u16]| {
        let row_start = source_y * row_stride;
        let row = &raw[row_start..row_start + row_stride];
        let mut sums = [0u16; 2];
        for channel in 0..2 {
            sums[channel] = u16::from(row[channel])
                + u16::from(row[2 + channel])
                + u16::from(row[4 + channel])
                + u16::from(row[6 + channel])
                + u16::from(row[8 + channel]);
            destination[4 + channel] = sums[channel];
        }
        for x in 3..width - 2 {
            let leaving = (x - 3) * 2;
            let entering = (x + 2) * 2;
            for channel in 0..2 {
                sums[channel] = sums[channel] - u16::from(row[leaving + channel])
                    + u16::from(row[entering + channel]);
                destination[x * 2 + channel] = sums[channel];
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
        let output_row = y * row_stride;
        let extra_row = (y - 2) * row_stride;
        for x in 2..width - 2 {
            let pixel = x * 2;
            let extra_sample = extra_row + (x + 2) * 2;
            for channel in 0..2 {
                let sum = u32::from(rows[0][pixel + channel])
                    + u32::from(rows[1][pixel + channel])
                    + u32::from(rows[2][pixel + channel])
                    + u32::from(rows[3][pixel + channel])
                    + u32::from(rows[4][pixel + channel])
                    + u32::from(raw[extra_sample + channel]);
                out[output_row + pixel + channel] = ((sum + 12) / 25).min(255) as u8;
            }
        }

        let next_source_y = y + 3;
        if next_source_y < height {
            fill_horizontal_row(next_source_y, &mut horizontal_rows[next_source_y % 5]);
        }
    }
    true
}

/// Apply the uniform 5x5 kernel to native L bytes without intermediate rounding.
///
/// The integer sum is at most 6,375. Its exact mean plus 0.5 is at least
/// 1/50 away from an integer because the denominator 25 is odd. The normalized
/// f32 coefficient's error and all 30 ordered multiply/add roundings together
/// contribute less than 0.001, so Pillow's byte is exactly `(sum + 12) / 25`.
/// Five horizontal rows retain exact sums; the two-pixel border stays copied
/// from the source. Each successive horizontal sum needs only two byte loads.
pub(super) fn filter_5x5_uniform_luma_rows(
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

    let mut horizontal_rows: [Vec<u16>; 5] = std::array::from_fn(|_| vec![0; width]);
    let fill_row = |source_y: usize, destination: &mut [u16]| {
        let row = &raw[source_y * width..(source_y + 1) * width];
        let mut sum: u16 = row[..5].iter().map(|&sample| u16::from(sample)).sum();
        destination[2] = sum;
        for x in 3..width - 2 {
            sum = sum - u16::from(row[x - 3]) + u16::from(row[x + 2]);
            destination[x] = sum;
        }
    };
    for source_y in 0..5 {
        fill_row(source_y, &mut horizontal_rows[source_y]);
    }

    let mut vertical_sums = vec![0u16; width];
    for x in 2..width - 2 {
        vertical_sums[x] = horizontal_rows[0][x]
            + horizontal_rows[1][x]
            + horizontal_rows[2][x]
            + horizontal_rows[3][x]
            + horizontal_rows[4][x];
    }

    for y in 2..height - 2 {
        let output_row = y * width;
        for x in 2..width - 2 {
            out[output_row + x] = ((vertical_sums[x] + 12) / 25) as u8;
        }

        let next_y = y + 3;
        if next_y < height {
            // The ring slot for the outgoing row is also the incoming row's
            // slot. Update each vertical total before replacing that sample.
            let input_row = &raw[next_y * width..(next_y + 1) * width];
            let outgoing_and_incoming = &mut horizontal_rows[(y - 2) % 5];
            let mut horizontal_sum = u16::from(input_row[0])
                + u16::from(input_row[1])
                + u16::from(input_row[2])
                + u16::from(input_row[3])
                + u16::from(input_row[4]);
            for x in 2..width - 2 {
                if x > 2 {
                    horizontal_sum =
                        horizontal_sum - u16::from(input_row[x - 3]) + u16::from(input_row[x + 2]);
                }
                vertical_sums[x] = vertical_sums[x] - outgoing_and_incoming[x] + horizontal_sum;
                outgoing_and_incoming[x] = horizontal_sum;
            }
        }
    }
    true
}

/// Apply the all-ones 5x5 kernel to a large native RGBA raster using exact
/// rolling sums. The normalized f32 kernel's ordered accumulation can differ
/// from the exact mean by less than 0.001; with a denominator of 25, an integer
/// sum is at least 0.02 from a half-byte rounding boundary. Therefore Pillow's
/// rounded byte is exactly `(sum + 12) / 25`. The untouched two-pixel border
/// remains in the caller's output copy.
#[cfg(not(feature = "parallel"))]
pub(super) fn filter_5x5_uniform_rgba_rows(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
) -> bool {
    let Some(pixel_count) = width.checked_mul(height) else {
        return false;
    };
    if pixel_count <= SMALL_UNIFORM_BYTE_MAX_PIXELS {
        return false;
    }
    let Some(row_stride) = width.checked_mul(4) else {
        return false;
    };
    let Some(total_bytes) = row_stride.checked_mul(height) else {
        return false;
    };
    if raw.len() != total_bytes || out.len() != total_bytes {
        return false;
    }
    if width < 5 || height < 5 {
        return true;
    }

    let mut horizontal_rows: [Vec<u16>; 5] = std::array::from_fn(|_| vec![0; row_stride]);
    let fill_horizontal_row = |source_y: usize, destination: &mut [u16]| {
        let row_start = source_y * row_stride;
        let row = &raw[row_start..row_start + row_stride];
        let mut sums = [0u16; 4];
        for channel in 0..4 {
            sums[channel] = u16::from(row[channel])
                + u16::from(row[4 + channel])
                + u16::from(row[8 + channel])
                + u16::from(row[12 + channel])
                + u16::from(row[16 + channel]);
            destination[8 + channel] = sums[channel];
        }
        for x in 3..width - 2 {
            let leaving = (x - 3) * 4;
            let entering = (x + 2) * 4;
            for channel in 0..4 {
                sums[channel] = sums[channel] - u16::from(row[leaving + channel])
                    + u16::from(row[entering + channel]);
                destination[x * 4 + channel] = sums[channel];
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
        let output_row = y * row_stride;
        for x in 2..width - 2 {
            let pixel = x * 4;
            for channel in 0..4 {
                let sum = rows[0][pixel + channel]
                    + rows[1][pixel + channel]
                    + rows[2][pixel + channel]
                    + rows[3][pixel + channel]
                    + rows[4][pixel + channel];
                out[output_row + pixel + channel] = ((u32::from(sum) + 12) / 25) as u8;
            }
        }

        let next_source_y = y + 3;
        if next_source_y < height {
            fill_horizontal_row(next_source_y, &mut horizontal_rows[next_source_y % 5]);
        }
    }
    true
}

#[path = "native_bytes/binomial.rs"]
mod binomial;
pub(crate) use binomial::binomial5x5_parameters_match;
#[cfg(feature = "parallel")]
pub(super) use binomial::{
    filter_5x5_binomial_la_parallel_rows, filter_5x5_binomial_luma_parallel_rows,
};
#[cfg(not(feature = "parallel"))]
pub(super) use binomial::{filter_5x5_binomial_la_rows, filter_5x5_binomial_luma_rows};

#[cfg(test)]
#[path = "native_bytes/tests.rs"]
mod tests;
