//! Shared native-byte algorithms for Pillow's 3x3 convolution filter.

use super::convolution::{clip8_filter, pillow_kernel_row_3};
use super::uniform_image::{SMALL_UNIFORM_BYTE_MAX_PIXELS, native_small_uniform_bytes};
use crate::compute::helpers::filters::binomial3x3_parameters_match;

// ── Native-byte 3x3 filter ──

/// Apply one row of the byte 3x3 convolution while preserving Pillow's
/// per-pixel contraction order.
#[inline]
fn filter_3x3_byte_row(
    raw: &[u8],
    row: &mut [u8],
    y: i32,
    width: i32,
    height: i32,
    channels: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) {
    if y < 1 || y >= height - 1 || width < 3 {
        return;
    }
    for x in 1..width - 1 {
        let base =
            |dx: i32, dy: i32| -> usize { ((y + dy) * width + (x + dx)) as usize * channels };
        for c in 0..channels {
            let row_b = pillow_kernel_row_3(
                [
                    raw[base(-1, 1) + c] as f32,
                    raw[base(0, 1) + c] as f32,
                    raw[base(1, 1) + c] as f32,
                ],
                &kernel[0..3],
            );
            let row_c = pillow_kernel_row_3(
                [
                    raw[base(-1, 0) + c] as f32,
                    raw[base(0, 0) + c] as f32,
                    raw[base(1, 0) + c] as f32,
                ],
                &kernel[3..6],
            );
            let row_t = pillow_kernel_row_3(
                [
                    raw[base(-1, -1) + c] as f32,
                    raw[base(0, -1) + c] as f32,
                    raw[base(1, -1) + c] as f32,
                ],
                &kernel[6..9],
            );
            let mut ss = rounding_bias;
            ss += row_b;
            ss += row_c;
            ss += row_t;
            row[x as usize * channels + c] = clip8_filter(ss);
        }
    }
}

/// Apply the exact native-LA binomial 3x3 kernel through three reusable
/// horizontally filtered rows.  The [1, 2, 1] weights and scale 16 keep every
/// intermediate integral, so `(sum + 8) >> 4` matches Pillow's `f32` result
/// without changing the copied one-pixel border.
fn filter_3x3_binomial_la_rows(raw: &[u8], out: &mut [u8], width: usize, height: usize) -> bool {
    let Some(pixel_count) = width.checked_mul(height) else {
        return false;
    };
    let Some(sample_count) = pixel_count.checked_mul(2) else {
        return false;
    };
    if raw.len() != sample_count || out.len() != sample_count {
        return false;
    }
    if width < 3 || height < 3 {
        return true;
    }

    let row_samples = width * 2;
    let mut horizontal_rows: [Vec<u16>; 3] = std::array::from_fn(|_| vec![0; row_samples]);
    let fill_horizontal_row = |source_y: usize, destination: &mut [u16]| {
        let row_start = source_y * row_samples;
        for x in 1..width - 1 {
            let base = row_start + x * 2;
            for channel in 0..2 {
                let sample = base + channel;
                destination[x * 2 + channel] = u16::from(raw[sample - 2])
                    + u16::from(raw[sample]) * 2
                    + u16::from(raw[sample + 2]);
            }
        }
    };

    for source_y in 0..3 {
        fill_horizontal_row(source_y, &mut horizontal_rows[source_y]);
    }
    for y in 1..height - 1 {
        let above = &horizontal_rows[(y - 1) % 3];
        let center = &horizontal_rows[y % 3];
        let below = &horizontal_rows[(y + 1) % 3];
        let output_row = y * row_samples;
        for x in 1..width - 1 {
            let base = x * 2;
            for channel in 0..2 {
                let sample = base + channel;
                let sum = u32::from(above[sample])
                    + u32::from(center[sample]) * 2
                    + u32::from(below[sample]);
                out[output_row + sample] = ((sum + 8) >> 4) as u8;
            }
        }

        let next_source_y = y + 2;
        if next_source_y < height {
            fill_horizontal_row(next_source_y, &mut horizontal_rows[next_source_y % 3]);
        }
    }
    true
}

fn filter_3x3_byte_rows(
    raw: &[u8],
    out: &mut [u8],
    width: i32,
    height: i32,
    channels: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) {
    let row_stride = width.max(0) as usize * channels;
    if row_stride == 0 || height <= 0 {
        return;
    }

    for y in 0..height {
        let row_start = y.max(0) as usize * row_stride;
        filter_3x3_byte_row(
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

/// Evaluate a constant native-byte 3x3 convolution without visiting every
/// neighborhood.  The border remains the cloned input, matching the scalar
/// kernel's untouched edge pixels; every interior pixel has the same result
/// because all source tuples are equal.
fn filter_3x3_constant_bytes(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) {
    let mut filtered = [0u8; 4];
    for channel in 0..channels {
        let sample = raw[channel] as f32;
        let row_b = pillow_kernel_row_3([sample; 3], &kernel[0..3]);
        let row_c = pillow_kernel_row_3([sample; 3], &kernel[3..6]);
        let row_t = pillow_kernel_row_3([sample; 3], &kernel[6..9]);
        let mut value = rounding_bias;
        value += row_b;
        value += row_c;
        value += row_t;
        filtered[channel] = clip8_filter(value);
    }
    let row_stride = width * channels;
    for y in 1..height.saturating_sub(1) {
        for x in 1..width.saturating_sub(1) {
            let base = y * row_stride + x * channels;
            out[base..base + channels].copy_from_slice(&filtered[..channels]);
        }
    }
}

/// Evaluate the retained small-uniform shortcut before a vector row pass.
pub(crate) fn execute_filter3x3_small_uniform_bytes(
    raw: &[u8],
    width: u32,
    height: u32,
    channels: usize,
    kernel: &[f32; 9],
    scale: f32,
    offset: f32,
) -> Option<Vec<u8>> {
    if width < 3
        || height < 3
        || !native_small_uniform_bytes(raw, width, height, channels, SMALL_UNIFORM_BYTE_MAX_PIXELS)
    {
        return None;
    }
    let normalized_kernel = std::array::from_fn(|index| kernel[index] / scale);
    let mut output = raw.to_vec();
    filter_3x3_constant_bytes(
        raw,
        &mut output,
        width as usize,
        height as usize,
        channels,
        &normalized_kernel,
        offset + 0.5,
    );
    Some(output)
}

/// Retains the packed-vector LA binomial specialization for architecture
/// strategies that do not use the dedicated NEON kernel.
pub(crate) fn execute_filter3x3_la_binomial_vector_bytes(
    raw: &[u8],
    width: u32,
    height: u32,
    kernel: &[f32; 9],
    scale: f32,
    offset: f32,
) -> Option<(Vec<u8>, u64, u64)> {
    if !binomial3x3_parameters_match(kernel, scale, offset) {
        return None;
    }
    let mut output = raw.to_vec();
    let (vector_blocks, scalar_tail) = super::filter3x3_vector::apply_la_binomial(
        raw,
        &mut output,
        width as usize,
        height as usize,
    )?;
    Some((output, vector_blocks, scalar_tail))
}

/// Executes the shared native-byte 3x3 kernel for a mode-owned Default leaf.
///
/// The caller supplies the exact channel count and whether the archived small
/// uniform-image shortcut applies to its mode. This helper does not infer mode
/// from the raster carrier or choose an output identity.
pub(crate) fn execute_filter3x3_native_bytes(
    raw: &[u8],
    width: u32,
    height: u32,
    channels: usize,
    kernel: &[f32; 9],
    scale: f32,
    offset: f32,
    uniform_shortcut: bool,
) -> Vec<u8> {
    let (width_i32, height_i32) = (width as i32, height as i32);
    let normalized_kernel: [f32; 9] = std::array::from_fn(|index| kernel[index] / scale);
    let rounding_bias = offset + 0.5;
    let mut output = raw.to_vec();
    if uniform_shortcut
        && native_small_uniform_bytes(raw, width, height, channels, SMALL_UNIFORM_BYTE_MAX_PIXELS)
    {
        filter_3x3_constant_bytes(
            raw,
            &mut output,
            width as usize,
            height as usize,
            channels,
            &normalized_kernel,
            rounding_bias,
        );
    } else {
        filter_3x3_byte_rows(
            raw,
            &mut output,
            width_i32,
            height_i32,
            channels,
            &normalized_kernel,
            rounding_bias,
        );
    }
    output
}

/// Retains the archived two-channel LA specialization for its exact request.
pub(crate) fn execute_filter3x3_la_binomial_bytes(
    raw: &[u8],
    width: u32,
    height: u32,
    kernel: &[f32; 9],
    scale: f32,
    offset: f32,
) -> Option<Vec<u8>> {
    if !binomial3x3_parameters_match(kernel, scale, offset) {
        return None;
    }
    let mut output = raw.to_vec();
    filter_3x3_binomial_la_rows(raw, &mut output, width as usize, height as usize).then_some(output)
}
