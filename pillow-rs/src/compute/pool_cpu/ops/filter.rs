//! Filter operations extracted from image.rs execute_op().
//!
//! These functions are standalone implementations of PIL-compatible filter
//! operations (Filter3x3, Filter5x5, GaussianBlur, BoxBlur, MedianFilter,
//! MaxFilter, MinFilter, RankFilter) that operate on DynamicImage and return
//! new DynamicImage instances.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::image_utils::raw_bytes_to_image;
use crate::raster::DynamicImage;

/// Push one source index into a monotonic queue used by a byte min/max line.
/// The queue stores indices rather than samples so stale values can be
/// removed when the replicated-edge window advances.
#[inline]
fn push_extreme_index(
    queue: &mut Vec<usize>,
    head: &mut usize,
    input: &[u8],
    channels: usize,
    channel: usize,
    index: usize,
    select_max: bool,
) {
    let value = input[index * channels + channel];
    while queue.len() > *head {
        let Some(&back) = queue.last() else {
            break;
        };
        let back_value = input[back * channels + channel];
        let remove_back = if select_max {
            back_value <= value
        } else {
            back_value >= value
        };
        if !remove_back {
            break;
        }
        queue.pop();
    }
    queue.push(index);
}

/// Apply a replicated-edge min/max window to one contiguous byte line.
///
/// A queue is reused for every channel, so the pass has no allocation in the
/// output-pixel loop and its work is linear in the line length, independent of
/// the square filter area.
fn rank_filter_bytes_extreme_line(
    input: &[u8],
    output: &mut [u8],
    length: usize,
    channels: usize,
    half: usize,
    select_max: bool,
    queues: &mut [Vec<usize>; 4],
) {
    if length == 0 || channels == 0 || channels > queues.len() {
        return;
    }
    let last_index = length - 1;
    let initial_end = half.min(last_index);
    for channel in 0..channels {
        let queue = &mut queues[channel];
        queue.clear();
        let mut head = 0usize;
        for index in 0..=initial_end {
            push_extreme_index(
                queue, &mut head, input, channels, channel, index, select_max,
            );
        }
        output[channel] = input[queue[head] * channels + channel];
        let mut last_added = initial_end;
        for x in 1..length {
            let left = x.saturating_sub(half);
            while head < queue.len() && queue[head] < left {
                head += 1;
            }
            let entering = x.saturating_add(half).min(last_index);
            if entering > last_added {
                push_extreme_index(
                    queue, &mut head, input, channels, channel, entering, select_max,
                );
                last_added = entering;
            }
            output[x * channels + channel] = input[queue[head] * channels + channel];
        }
    }
}

/// Evaluate one output row of the vertical extrema pass. Rebuilding a
/// monotonic queue for the bounded vertical window keeps this pass parallel
/// over disjoint destination rows while reducing the square scan to one
/// horizontal and one vertical window traversal per pixel.
fn rank_filter_bytes_extreme_vertical_row(
    input: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    half: usize,
    select_max: bool,
    y: usize,
    queues: &mut [Vec<usize>; 4],
) {
    let row_stride = width * channels;
    let top = y.saturating_sub(half);
    let bottom = y.saturating_add(half).min(height - 1);
    for x in 0..width {
        for channel in 0..channels {
            let queue = &mut queues[channel];
            queue.clear();
            let head = 0usize;
            for source_y in top..=bottom {
                let value = input[source_y * row_stride + x * channels + channel];
                while queue.len() > head {
                    let Some(&back) = queue.last() else {
                        break;
                    };
                    let back_value = input[back * row_stride + x * channels + channel];
                    let remove_back = if select_max {
                        back_value <= value
                    } else {
                        back_value >= value
                    };
                    if !remove_back {
                        break;
                    }
                    queue.pop();
                }
                queue.push(source_y);
            }
            output[x * channels + channel] =
                input[queue[head] * row_stride + x * channels + channel];
        }
    }
}

/// Exact native-byte min/max implementation using horizontal and vertical
/// monotonic windows. Pillow's square window is separable for extrema, and
/// duplicating the edge samples does not change a min or max. The intermediate
/// buffer is one full frame, while the per-line queues are bounded by one
/// image dimension and reused across channels.
fn rank_filter_bytes_extreme_separable(
    raw: &[u8],
    out: &mut [u8],
    w: i32,
    h: i32,
    channels: usize,
    half: i32,
    select_max: bool,
) -> bool {
    let Ok(width) = usize::try_from(w) else {
        return false;
    };
    let Ok(height) = usize::try_from(h) else {
        return false;
    };
    let Ok(half) = usize::try_from(half) else {
        return false;
    };
    let Some(row_stride) = width.checked_mul(channels) else {
        return false;
    };
    let Some(total_bytes) = row_stride.checked_mul(height) else {
        return false;
    };
    if channels == 0
        || channels > 4
        || raw.len() < total_bytes
        || out.len() < total_bytes
        || width == 0
        || height == 0
    {
        return false;
    }

    let mut horizontal = vec![0u8; total_bytes];
    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        &mut horizontal,
        row_stride,
        height,
        |row_start, _row_end, _y, row| {
            let mut queues: [Vec<usize>; 4] = std::array::from_fn(|_| Vec::with_capacity(width));
            rank_filter_bytes_extreme_line(
                &raw[row_start..row_start + row_stride],
                row,
                width,
                channels,
                half,
                select_max,
                &mut queues,
            );
        }
    );
    #[cfg(not(feature = "parallel"))]
    for y in 0..height {
        let row_start = y * row_stride;
        let mut queues: [Vec<usize>; 4] = std::array::from_fn(|_| Vec::with_capacity(width));
        rank_filter_bytes_extreme_line(
            &raw[row_start..row_start + row_stride],
            &mut horizontal[row_start..row_start + row_stride],
            width,
            channels,
            half,
            select_max,
            &mut queues,
        );
    }

    let queue_capacity = half.saturating_mul(2).saturating_add(1).min(height);
    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(out, row_stride, height, |_row_start, _row_end, y, row| {
        let mut queues: [Vec<usize>; 4] =
            std::array::from_fn(|_| Vec::with_capacity(queue_capacity));
        rank_filter_bytes_extreme_vertical_row(
            &horizontal,
            row,
            width,
            height,
            channels,
            half,
            select_max,
            y as usize,
            &mut queues,
        );
    });
    #[cfg(not(feature = "parallel"))]
    for y in 0..height {
        let row_start = y * row_stride;
        let mut queues: [Vec<usize>; 4] =
            std::array::from_fn(|_| Vec::with_capacity(queue_capacity));
        rank_filter_bytes_extreme_vertical_row(
            &horizontal,
            &mut out[row_start..row_start + row_stride],
            width,
            height,
            channels,
            half,
            select_max,
            y,
            &mut queues,
        );
    }
    true
}

// ─── Clip helper ──

/// PIL's clip8: truncating cast to u8, clamping at 0 and 255.
/// Matches PIL ImagingFilter's clip8(): `return ss <= 0.0 ? 0 : ss >= 255.0 ? 255 : (UINT8)ss`
fn clip8_filter(v: f32) -> u8 {
    if v <= 0.0 {
        0
    } else if v >= 255.0 {
        255
    } else {
        v as u8
    }
}

/// Evaluate Pillow's three-tap row expression with the contraction order used
/// by the pinned arm64 Pillow 12.2.0 oracle.
///
/// `src/libImaging/Filter.c` spells `KERNEL1x3` as a left-associated sum. The
/// oracle binary starts with the middle product, then emits fused
/// multiply-adds for the left and right products. Keeping that order explicit
/// avoids one-byte differences when a result lies just below an integer.
#[inline]
fn pillow_kernel_row_3(pixels: [f32; 3], kernel: &[f32]) -> f32 {
    let sum = pixels[1] * kernel[1];
    let sum = pixels[0].mul_add(kernel[0], sum);
    pixels[2].mul_add(kernel[2], sum)
}

/// Five-tap counterpart of [`pillow_kernel_row_3`].
///
/// The pinned oracle starts with tap 1, fuses tap 0, and then fuses taps 2
/// through 4 in source order.
#[inline]
fn pillow_kernel_row_5(pixels: [f32; 5], kernel: &[f32]) -> f32 {
    let sum = pixels[1] * kernel[1];
    let sum = pixels[0].mul_add(kernel[0], sum);
    let sum = pixels[2].mul_add(kernel[2], sum);
    let sum = pixels[3].mul_add(kernel[3], sum);
    pixels[4].mul_add(kernel[4], sum)
}

// ── 3x3 filter (I-mode) ──

/// Apply a 3x3 kernel filter on I-mode (32-bit signed integer) data.
/// I-mode pixel values are stored as 4 RGBA bytes (little-endian i32).
/// PIL applies the full kernel convolution with float (f32) arithmetic,
/// then truncates to 32-bit integer — NO clipping to [0,255].
/// Key PIL I-mode behaviors verified against PIL 12.2.0:
///   - Reversed Y-axis kernel: k[0] on bottom row, k[8] on top row
///     (matches UINT8 filter's row_b / row_c / row_t layout)
///   - Uses f32 for accumulation (same as UINT8 mode)
///   - With +0.5 rounding bias (same as UINT8 mode)
///   - Clips negative results to 0, allows values > 255
#[inline]
fn filter_3x3_i32_row(
    raw: &[u8],
    row: &mut [u8],
    y: i32,
    width: i32,
    height: i32,
    kernel: &[f32; 9],
    offset: i32,
) {
    if y < 1 || y >= height - 1 || width < 3 {
        return;
    }
    for x in 1..width - 1 {
        let base = |dx: i32, dy: i32| -> usize { ((y + dy) * width + (x + dx)) as usize * 4 };
        let read_pixel = |dx: i32, dy: i32| -> i32 {
            let index = base(dx, dy);
            i32::from_le_bytes([raw[index], raw[index + 1], raw[index + 2], raw[index + 3]])
        };
        let bottom = pillow_kernel_row_3(
            [
                read_pixel(-1, 1) as f32,
                read_pixel(0, 1) as f32,
                read_pixel(1, 1) as f32,
            ],
            &kernel[0..3],
        );
        let middle = pillow_kernel_row_3(
            [
                read_pixel(-1, 0) as f32,
                read_pixel(0, 0) as f32,
                read_pixel(1, 0) as f32,
            ],
            &kernel[3..6],
        );
        let top = pillow_kernel_row_3(
            [
                read_pixel(-1, -1) as f32,
                read_pixel(0, -1) as f32,
                read_pixel(1, -1) as f32,
            ],
            &kernel[6..9],
        );
        let mut value = offset as f32 + 0.5;
        value += bottom;
        value += middle;
        value += top;
        let result = if value >= 0.0 { value as i32 } else { 0 };
        let output = x as usize * 4;
        row[output..output + 4].copy_from_slice(&result.to_le_bytes());
    }
}

fn filter_3x3_i32(
    img: &DynamicImage,
    kernel: &[f32; 9],
    scale: f32,
    offset: i32,
) -> Result<DynamicImage, PilError> {
    let rgba = img.to_rgba8();
    let (w_u32, h_u32) = rgba.dimensions();
    let (w, h) = (w_u32 as i32, h_u32 as i32);
    let raw = rgba.into_raw();

    // `Image::kernel_filter` clamps custom scales to a positive value and all
    // built-in kernels have positive scales, so this execution boundary never
    // receives zero.
    let s = scale;
    let kd = [
        kernel[0] / s,
        kernel[1] / s,
        kernel[2] / s,
        kernel[3] / s,
        kernel[4] / s,
        kernel[5] / s,
        kernel[6] / s,
        kernel[7] / s,
        kernel[8] / s,
    ];

    let mut out = raw.clone();

    let row_stride = w_u32 as usize * 4;
    #[cfg(feature = "parallel")]
    if row_stride != 0 {
        crate::par_rows_mut!(
            &mut out,
            row_stride,
            h_u32 as usize,
            |_row_start, _row_end, y, row| {
                filter_3x3_i32_row(&raw, row, y as i32, w, h, &kd, offset);
            }
        );
    } else {
        for y in 0..h {
            let row_start = y as usize * row_stride;
            filter_3x3_i32_row(
                &raw,
                &mut out[row_start..row_start + row_stride],
                y,
                w,
                h,
                &kd,
                offset,
            );
        }
    }
    #[cfg(not(feature = "parallel"))]
    for y in 0..h {
        let row_start = y as usize * row_stride;
        filter_3x3_i32_row(
            &raw,
            &mut out[row_start..row_start + row_stride],
            y,
            w,
            h,
            &kd,
            offset,
        );
    }

    Ok(DynamicImage::ImageRgba8(
        crate::raster::RgbaImage::from_raw(w_u32, h_u32, out)
            .ok_or_else(|| PilError::ValueError("filter_3x3_i32: buffer error".into()))?,
    ))
}

// ── 5x5 filter (I-mode) ──

/// Apply a 5x5 kernel filter on I-mode (32-bit signed integer) data.
/// Same approach as filter_3x3_i32 — f32, reversed Y-axis, +0.5 rounding.
#[inline]
fn filter_5x5_i32_row(
    raw: &[u8],
    row: &mut [u8],
    y: i32,
    width: i32,
    height: i32,
    kernel: &[f32; 25],
    offset: i32,
) {
    if y < 2 || y >= height - 2 || width < 5 {
        return;
    }
    for x in 2..width - 2 {
        let base = |dx: i32, dy: i32| -> usize { ((y + dy) * width + (x + dx)) as usize * 4 };
        let read_pixel = |dx: i32, dy: i32| -> i32 {
            let index = base(dx, dy);
            i32::from_le_bytes([raw[index], raw[index + 1], raw[index + 2], raw[index + 3]])
        };
        let bottom0 = pillow_kernel_row_5(
            [
                read_pixel(-2, 2) as f32,
                read_pixel(-1, 2) as f32,
                read_pixel(0, 2) as f32,
                read_pixel(1, 2) as f32,
                read_pixel(2, 2) as f32,
            ],
            &kernel[0..5],
        );
        let bottom1 = pillow_kernel_row_5(
            [
                read_pixel(-2, 1) as f32,
                read_pixel(-1, 1) as f32,
                read_pixel(0, 1) as f32,
                read_pixel(1, 1) as f32,
                read_pixel(2, 1) as f32,
            ],
            &kernel[5..10],
        );
        let middle = pillow_kernel_row_5(
            [
                read_pixel(-2, 0) as f32,
                read_pixel(-1, 0) as f32,
                read_pixel(0, 0) as f32,
                read_pixel(1, 0) as f32,
                read_pixel(2, 0) as f32,
            ],
            &kernel[10..15],
        );
        let top1 = pillow_kernel_row_5(
            [
                read_pixel(-2, -1) as f32,
                read_pixel(-1, -1) as f32,
                read_pixel(0, -1) as f32,
                read_pixel(1, -1) as f32,
                read_pixel(2, -1) as f32,
            ],
            &kernel[15..20],
        );
        let top0 = pillow_kernel_row_5(
            [
                read_pixel(-2, -2) as f32,
                read_pixel(-1, -2) as f32,
                read_pixel(0, -2) as f32,
                read_pixel(1, -2) as f32,
                read_pixel(2, -2) as f32,
            ],
            &kernel[20..25],
        );
        let mut value = offset as f32 + 0.5;
        value += bottom0;
        value += bottom1;
        value += middle;
        value += top1;
        value += top0;
        let result = if value >= 0.0 { value as i32 } else { 0 };
        let output = x as usize * 4;
        row[output..output + 4].copy_from_slice(&result.to_le_bytes());
    }
}

fn filter_5x5_i32(
    img: &DynamicImage,
    kernel: &[f32; 25],
    scale: f32,
    offset: i32,
) -> Result<DynamicImage, PilError> {
    let rgba = img.to_rgba8();
    let (w_u32, h_u32) = rgba.dimensions();
    let (w, h) = (w_u32 as i32, h_u32 as i32);
    let raw = rgba.into_raw();

    // See the 3x3 path: public kernel construction guarantees a positive
    // scale, and built-in 5x5 filters use positive scales as well.
    let s = scale;
    // Pre-compute normalized kernel coefficients using f32 (matching PIL C construction)
    let kd: [f32; 25] = std::array::from_fn(|i| kernel[i] / s);

    let mut out = raw.clone();

    let row_stride = w_u32 as usize * 4;
    #[cfg(feature = "parallel")]
    if row_stride != 0 {
        crate::par_rows_mut!(
            &mut out,
            row_stride,
            h_u32 as usize,
            |_row_start, _row_end, y, row| {
                filter_5x5_i32_row(&raw, row, y as i32, w, h, &kd, offset);
            }
        );
    } else {
        for y in 0..h {
            let row_start = y as usize * row_stride;
            filter_5x5_i32_row(
                &raw,
                &mut out[row_start..row_start + row_stride],
                y,
                w,
                h,
                &kd,
                offset,
            );
        }
    }
    #[cfg(not(feature = "parallel"))]
    for y in 0..h {
        let row_start = y as usize * row_stride;
        filter_5x5_i32_row(
            &raw,
            &mut out[row_start..row_start + row_stride],
            y,
            w,
            h,
            &kd,
            offset,
        );
    }

    Ok(DynamicImage::ImageRgba8(
        crate::raster::RgbaImage::from_raw(w_u32, h_u32, out)
            .ok_or_else(|| PilError::ValueError("filter_5x5_i32: buffer error".into()))?,
    ))
}

/// Apply one row of the byte 3x3 convolution. The source is immutable and the
/// destination row is exclusive, so complete rows can be evaluated in
/// parallel without changing Pillow's per-pixel contraction order.
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
    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        out,
        row_stride,
        height.max(0) as usize,
        |_row_start, _row_end, y, row| {
            filter_3x3_byte_row(
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

/// Apply one row of the byte 5x5 convolution. The arithmetic and tap order
/// intentionally match the original scalar kernel exactly.
#[inline]
fn filter_5x5_byte_row(
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

fn filter_5x5_byte_rows(
    raw: &[u8],
    out: &mut [u8],
    width: i32,
    height: i32,
    channels: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) {
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

// ── PIL-style box blur ──

const BOX_BLUR_SCALE: u32 = 1 << 24;
const BOX_BLUR_BIAS: u32 = 1 << 23;
const UNIFORM_BOX_BLUR_MAX_PIXELS: usize = 64 * 64;

#[inline(always)]
fn blur_line_step(
    source: &[u8],
    destination: &mut [u8],
    accumulator: &mut [u32],
    line_start: usize,
    element_width: usize,
    output: usize,
    subtract: usize,
    add: usize,
    far_left: usize,
    far_right: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let output_base = line_start + output * element_width;
    let subtract_base = line_start + subtract * element_width;
    let add_base = line_start + add * element_width;
    let far_left_base = line_start + far_left * element_width;
    let far_right_base = line_start + far_right * element_width;

    for component in 0..element_width {
        accumulator[component] = accumulator[component]
            .wrapping_sub(source[subtract_base + component] as u32)
            .wrapping_add(source[add_base + component] as u32);
        let far = (source[far_left_base + component] as u32
            + source[far_right_base + component] as u32)
            .wrapping_mul(fractional_weight);
        let bulk = accumulator[component]
            .wrapping_mul(whole_weight)
            .wrapping_add(far);
        destination[output_base + component] = (bulk.wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
    }
}

/// Blur one contiguous interleaved row using Pillow's four edge regions.
///
/// `src/libImaging/BoxBlur.c::ImagingLineBoxBlur{8,32}` initializes the
/// accumulator for logical pixel `-1`, then advances it by exactly one
/// entering and one leaving sample for every output pixel. Keeping the same
/// regions avoids a radius-sized inner loop while preserving Pillow's edge
/// replication and unsigned 24-bit fixed-point arithmetic.
fn blur_line(
    source: &[u8],
    destination: &mut [u8],
    line_start: usize,
    line_length: usize,
    element_width: usize,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
    accumulator: &mut [u32],
) {
    debug_assert!(line_length > 0);
    debug_assert!(element_width > 0);
    debug_assert!(accumulator.len() >= element_width);

    let last = line_length - 1;
    let edge_a = (radius + 1).min(line_length);
    let edge_b = line_length.saturating_sub(radius + 1);
    let first_base = line_start;
    let last_base = line_start + last * element_width;
    accumulator[..element_width].fill(0);

    // Pillow starts with the clamped window centered at x=-1. This lets the
    // first MOVE_ACC produce the exact window centered at output x=0.
    for component in 0..element_width {
        accumulator[component] =
            (source[first_base + component] as u32).wrapping_mul((radius + 1) as u32);
    }
    for position in 0..edge_a.saturating_sub(1) {
        let base = line_start + position * element_width;
        for component in 0..element_width {
            accumulator[component] =
                accumulator[component].wrapping_add(source[base + component] as u32);
        }
    }
    let last_count = radius.saturating_add(1).saturating_sub(edge_a);
    for component in 0..element_width {
        accumulator[component] = accumulator[component]
            .wrapping_add((source[last_base + component] as u32).wrapping_mul(last_count as u32));
    }

    if edge_a <= edge_b {
        for output in 0..edge_a {
            blur_line_step(
                source,
                destination,
                accumulator,
                line_start,
                element_width,
                output,
                0,
                output + radius,
                0,
                output + radius + 1,
                whole_weight,
                fractional_weight,
            );
        }
        for output in edge_a..edge_b {
            blur_line_step(
                source,
                destination,
                accumulator,
                line_start,
                element_width,
                output,
                output - radius - 1,
                output + radius,
                output - radius - 1,
                output + radius + 1,
                whole_weight,
                fractional_weight,
            );
        }
        for output in edge_b..=last {
            blur_line_step(
                source,
                destination,
                accumulator,
                line_start,
                element_width,
                output,
                output - radius - 1,
                last,
                output - radius - 1,
                last,
                whole_weight,
                fractional_weight,
            );
        }
    } else {
        // The radius overlaps both edges. Pillow separates the overlap so no
        // index ever leaves the row, even when radius exceeds its length.
        for output in 0..edge_b {
            blur_line_step(
                source,
                destination,
                accumulator,
                line_start,
                element_width,
                output,
                0,
                output + radius,
                0,
                output + radius + 1,
                whole_weight,
                fractional_weight,
            );
        }
        for output in edge_b..edge_a {
            blur_line_step(
                source,
                destination,
                accumulator,
                line_start,
                element_width,
                output,
                0,
                last,
                0,
                last,
                whole_weight,
                fractional_weight,
            );
        }
        for output in edge_a..=last {
            blur_line_step(
                source,
                destination,
                accumulator,
                line_start,
                element_width,
                output,
                output - radius - 1,
                last,
                output - radius - 1,
                last,
                whole_weight,
                fractional_weight,
            );
        }
    }
}

#[inline]
fn blur_one_row(
    source: &[u8],
    destination: &mut [u8],
    row_start: usize,
    width: usize,
    channels: usize,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let row_length = width * channels;
    let mut accumulator = [0u32; 4];
    blur_line(
        &source[row_start..row_start + row_length],
        destination,
        0,
        width,
        channels,
        radius,
        whole_weight,
        fractional_weight,
        &mut accumulator,
    );
}

#[cfg(feature = "parallel")]
fn blur_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let row_length = width * channels;
    crate::par_rows_mut!(
        destination,
        row_length,
        height,
        |row_start, _row_end, _y, row| {
            blur_one_row(
                source,
                row,
                row_start,
                width,
                channels,
                radius,
                whole_weight,
                fractional_weight,
            );
        }
    );
}

#[cfg(not(feature = "parallel"))]
fn blur_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let row_length = width * channels;
    for row in 0..height {
        blur_one_row(
            source,
            &mut destination[row * row_length..(row + 1) * row_length],
            row * row_length,
            width,
            channels,
            radius,
            whole_weight,
            fractional_weight,
        );
    }
}

#[cfg(feature = "parallel")]
const VERTICAL_BLUR_TRANSPOSE_THRESHOLD: usize = 512 * 512;

#[cfg(feature = "parallel")]
fn transpose_interleaved_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
) {
    let source_row_stride = width * channels;
    let destination_row_stride = height * channels;
    crate::par_rows_mut!(
        destination,
        destination_row_stride,
        width,
        |row_start, row_end, x, row| {
            let _ = (row_start, row_end);
            let x = x as usize;
            for y in 0..height {
                let source_start = y * source_row_stride + x * channels;
                let destination_start = y * channels;
                row[destination_start..destination_start + channels]
                    .copy_from_slice(&source[source_start..source_start + channels]);
            }
        }
    );
}

fn blur_columns(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
    accumulator: &mut [u32],
) {
    // Treat each complete image row as one wide line element. Every component
    // gets the recurrence it would receive after Pillow's transpose, while
    // both the input and output accesses remain contiguous in row-major order.
    let row_length = width * channels;
    blur_line(
        source,
        destination,
        0,
        height,
        row_length,
        radius,
        whole_weight,
        fractional_weight,
        accumulator,
    );
}

/// PIL-style box blur with fractional radius support.
/// Uses sliding-window accumulator with fixed-point (24-bit) arithmetic.
/// Matches PIL order: ALL horizontal passes first, then ALL vertical passes.
fn pil_box_blur(img: &DynamicImage, radius: f32, passes: u32) -> Result<DynamicImage, PilError> {
    pil_box_blur_xy(img, radius, radius, passes)
}

/// PIL-style box blur with independent horizontal and vertical radii.
///
/// Pillow's `ImagingBoxBlur` applies every horizontal pass with `xradius`,
/// then every vertical pass with `yradius`. Keeping the fixed-point weights
/// separate is required for `BoxBlur((xradius, yradius))`.
fn pil_box_blur_xy(
    img: &DynamicImage,
    radius_x: f32,
    radius_y: f32,
    passes: u32,
) -> Result<DynamicImage, PilError> {
    pil_box_blur_xy_impl(img, radius_x, radius_y, passes)
}

fn pil_box_blur_xy_impl(
    img: &DynamicImage,
    radius_x: f32,
    radius_y: f32,
    passes: u32,
) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let (w_u32, h_u32) = (img.width(), img.height());
    if w_u32 == 0 || h_u32 == 0 || (radius_x == 0.0 && radius_y == 0.0) {
        return Ok(img.clone());
    }

    let (width, height) = (w_u32 as usize, h_u32 as usize);

    // ImagingBoxBlur's replicated-edge average is identity for an image whose
    // every pixel has the same channel tuple. Detect that common small-image
    // case before allocating the horizontal/vertical work buffers; the
    // fixed-point weights below sum to exactly 1.0, including fractional
    // radii, so this preserves the byte result for every blur pass and native
    // mode. Larger frames use the linear sliding-window recurrence directly;
    // scanning and then cloning a full frame would add a memory-bandwidth pass
    // that costs more than the blur itself.
    if width.saturating_mul(height) <= UNIFORM_BOX_BLUR_MAX_PIXELS {
        let raw = img.as_bytes();
        let mut pixels = raw.chunks_exact(channels);
        if let Some(first_pixel) = pixels.next()
            && pixels.all(|pixel| pixel == first_pixel)
            && pixels.remainder().is_empty()
        {
            return Ok(img.clone());
        }
    }

    let blur_parameters = |radius: f32| {
        // Integer part of radius (PIL: (int)floatRadius).
        let integer_radius = radius as i32 as usize;
        // Number of pixels in the integer window.
        let window_pixels = (2 * integer_radius + 1) as u32;
        // Fixed-point weight: PIL uses f32 precision for ww computation
        // (UINT32)((1 << 24) / (floatRadius * 2 + 1)) — all in f32.
        let whole_weight = (BOX_BLUR_SCALE as f32 / (radius * 2.0 + 1.0)) as u32;
        // Fractional edge weight (PIL: fw = ((1 << 24) - window_pixels * ww) / 2).
        let fractional_weight =
            BOX_BLUR_SCALE.wrapping_sub(window_pixels.wrapping_mul(whole_weight)) / 2;
        (integer_radius, whole_weight, fractional_weight)
    };
    let (horizontal_radius, horizontal_weight, horizontal_fractional_weight) =
        blur_parameters(radius_x);
    let (vertical_radius, vertical_weight, vertical_fractional_weight) = blur_parameters(radius_y);

    let mut work = img.as_bytes().to_vec();
    let mut scratch = CheckedDims::new(w_u32, h_u32, channels as u8)?.alloc_buffer();

    // PIL does ALL horizontal passes first (matching ImagingBoxBlur order)
    for _ in 0..passes {
        blur_rows(
            &work,
            &mut scratch,
            width,
            height,
            channels,
            horizontal_radius,
            horizontal_weight,
            horizontal_fractional_weight,
        );
        std::mem::swap(&mut work, &mut scratch);
    }

    // Pillow transposes before its vertical passes. For large parallel jobs,
    // keep that layout explicit: one transpose makes every original column a
    // writable row, all vertical passes can then reuse the row-parallel blur,
    // and one final transpose restores the public row-major layout. The
    // serial/small-image path keeps the algebraically equivalent wide-row
    // recurrence and avoids paying for the extra data movement.
    #[cfg(feature = "parallel")]
    let use_transposed_vertical =
        width.saturating_mul(height) >= VERTICAL_BLUR_TRANSPOSE_THRESHOLD && passes > 0;
    #[cfg(not(feature = "parallel"))]
    let use_transposed_vertical = false;

    if use_transposed_vertical {
        #[cfg(feature = "parallel")]
        {
            transpose_interleaved_rows(&work, &mut scratch, width, height, channels);
            for pass in 0..passes {
                blur_rows(
                    &scratch,
                    &mut work,
                    height,
                    width,
                    channels,
                    vertical_radius,
                    vertical_weight,
                    vertical_fractional_weight,
                );
                // The freshly blurred result is in `work`.  Swap only when
                // another pass still needs to read it; swapping after the
                // final pass would make the final transpose consume the
                // pre-blur transposed buffer for odd pass counts.
                if pass + 1 < passes {
                    std::mem::swap(&mut work, &mut scratch);
                }
            }
            transpose_interleaved_rows(&work, &mut scratch, height, width, channels);
        }
    } else {
        let mut vertical_accumulator = vec![0u32; width * channels];
        for _ in 0..passes {
            blur_columns(
                &work,
                &mut scratch,
                width,
                height,
                channels,
                vertical_radius,
                vertical_weight,
                vertical_fractional_weight,
                &mut vertical_accumulator,
            );
            std::mem::swap(&mut work, &mut scratch);
        }
    }

    let output = if use_transposed_vertical {
        scratch
    } else {
        work
    };
    let result = raw_bytes_to_image(w_u32, h_u32, output, channels)?;
    Ok(preserve_mode(img, result))
}

// ── Rank filter ──

const SMALL_RANK_AREA: usize = 49;

#[inline]
fn select_rank_histogram(histogram: &[usize; 256], rank: usize) -> u8 {
    let mut seen = 0usize;
    for (value, count) in histogram.iter().enumerate() {
        seen += *count;
        if seen > rank {
            return value as u8;
        }
    }
    255
}

fn rank_filter_bytes_extreme_row(
    raw: &[u8],
    row: &mut [u8],
    w: i32,
    h: i32,
    channels: usize,
    half: i32,
    select_max: bool,
    y: i32,
) {
    for x in 0..w {
        let mut selected = if select_max { [0u8; 4] } else { [u8::MAX; 4] };
        for dy in -half..=half {
            for dx in -half..=half {
                let sx = (x + dx).clamp(0, w - 1);
                let sy = (y + dy).clamp(0, h - 1);
                let base = (sy * w + sx) as usize * channels;
                for c in 0..channels {
                    let value = raw[base + c];
                    if select_max {
                        selected[c] = selected[c].max(value);
                    } else {
                        selected[c] = selected[c].min(value);
                    }
                }
            }
        }
        let out_base = x as usize * channels;
        row[out_base..out_base + channels].copy_from_slice(&selected[..channels]);
    }
}

fn rank_filter_bytes_extreme(
    raw: &[u8],
    out: &mut [u8],
    w: i32,
    h: i32,
    channels: usize,
    half: i32,
    select_max: bool,
) {
    let row_stride = w.max(0) as usize * channels;
    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        out,
        row_stride,
        h.max(0) as usize,
        |_row_start, _row_end, y, row| {
            rank_filter_bytes_extreme_row(raw, row, w, h, channels, half, select_max, y as i32);
        }
    );
    #[cfg(not(feature = "parallel"))]
    for y in 0..h {
        let row_start = y as usize * row_stride;
        rank_filter_bytes_extreme_row(
            raw,
            &mut out[row_start..row_start + row_stride],
            w,
            h,
            channels,
            half,
            select_max,
            y,
        );
    }
}

fn rank_filter_bytes_small_row(
    raw: &[u8],
    row: &mut [u8],
    w: i32,
    h: i32,
    channels: usize,
    half: i32,
    area: usize,
    rank: usize,
    y: i32,
) {
    debug_assert!(area <= SMALL_RANK_AREA);
    for x in 0..w {
        let mut values = [[0u8; SMALL_RANK_AREA]; 4];
        let mut value_index = 0usize;
        for dy in -half..=half {
            for dx in -half..=half {
                let sx = (x + dx).clamp(0, w - 1);
                let sy = (y + dy).clamp(0, h - 1);
                let base = (sy * w + sx) as usize * channels;
                for c in 0..channels {
                    values[c][value_index] = raw[base + c];
                }
                value_index += 1;
            }
        }
        let out_base = x as usize * channels;
        for c in 0..channels {
            // Only the requested order statistic is observable. Selecting it
            // avoids sorting the complete byte neighborhood while preserving
            // the exact value returned for duplicate samples.
            let (_, selected, _) = values[c][..area].select_nth_unstable(rank);
            row[out_base + c] = *selected;
        }
    }
}

fn rank_filter_bytes_small(
    raw: &[u8],
    out: &mut [u8],
    w: i32,
    h: i32,
    channels: usize,
    half: i32,
    area: usize,
    rank: usize,
) {
    let row_stride = w.max(0) as usize * channels;
    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        out,
        row_stride,
        h.max(0) as usize,
        |_row_start, _row_end, y, row| {
            rank_filter_bytes_small_row(raw, row, w, h, channels, half, area, rank, y as i32);
        }
    );
    #[cfg(not(feature = "parallel"))]
    for y in 0..h {
        let row_start = y as usize * row_stride;
        rank_filter_bytes_small_row(
            raw,
            &mut out[row_start..row_start + row_stride],
            w,
            h,
            channels,
            half,
            area,
            rank,
            y,
        );
    }
}

fn rank_filter_bytes_histogram_row(
    raw: &[u8],
    row: &mut [u8],
    w: i32,
    h: i32,
    channels: usize,
    half: i32,
    rank: usize,
    y: i32,
) {
    let mut histogram = [[0usize; 256]; 4];
    for dy in -half..=half {
        let sy = (y + dy).clamp(0, h - 1);
        for dx in -half..=half {
            let sx = dx.clamp(0, w - 1);
            let base = (sy * w + sx) as usize * channels;
            for c in 0..channels {
                histogram[c][raw[base + c] as usize] += 1;
            }
        }
    }

    for x in 0..w {
        let out_base = x as usize * channels;
        for c in 0..channels {
            row[out_base + c] = select_rank_histogram(&histogram[c], rank);
        }

        if x + 1 < w {
            let remove_x = (x - half).clamp(0, w - 1);
            let add_x = (x + half + 1).clamp(0, w - 1);
            for dy in -half..=half {
                let sy = (y + dy).clamp(0, h - 1);
                let remove_base = (sy * w + remove_x) as usize * channels;
                let add_base = (sy * w + add_x) as usize * channels;
                for c in 0..channels {
                    histogram[c][raw[remove_base + c] as usize] -= 1;
                    histogram[c][raw[add_base + c] as usize] += 1;
                }
            }
        }
    }
}

fn rank_filter_bytes_histogram(
    raw: &[u8],
    out: &mut [u8],
    w: i32,
    h: i32,
    channels: usize,
    half: i32,
    rank: usize,
) {
    let row_stride = w.max(0) as usize * channels;
    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        out,
        row_stride,
        h.max(0) as usize,
        |_row_start, _row_end, y, row| {
            rank_filter_bytes_histogram_row(raw, row, w, h, channels, half, rank, y as i32);
        }
    );
    #[cfg(not(feature = "parallel"))]
    for y in 0..h {
        let row_start = y as usize * row_stride;
        rank_filter_bytes_histogram_row(
            raw,
            &mut out[row_start..row_start + row_stride],
            w,
            h,
            channels,
            half,
            rank,
            y,
        );
    }
}

#[cfg(not(feature = "parallel"))]
fn rank_filter_f_large_serial(
    raw: &[u8],
    out: &mut [u8],
    w: i32,
    h: i32,
    half: i32,
    area: usize,
    rank: usize,
) {
    let mut values = Vec::with_capacity(area);
    for y in 0..h {
        for x in 0..w {
            values.clear();
            for dy in -half..=half {
                for dx in -half..=half {
                    let sx = (x + dx).clamp(0, w - 1);
                    let sy = (y + dy).clamp(0, h - 1);
                    let base = (sy * w + sx) as usize * 4;
                    values.push(f32::from_le_bytes([
                        raw[base],
                        raw[base + 1],
                        raw[base + 2],
                        raw[base + 3],
                    ]));
                }
            }
            values.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let out_base = (y * w + x) as usize * 4;
            out[out_base..out_base + 4].copy_from_slice(&values[rank].to_le_bytes());
        }
    }
}

#[cfg(feature = "parallel")]
fn rank_filter_f_large_parallel(
    raw: &[u8],
    out: &mut [u8],
    w: i32,
    h: i32,
    half: i32,
    area: usize,
    rank: usize,
) {
    let width = w as usize;
    let height = h as usize;
    let row_stride = width * 4;
    crate::par_rows_mut!(out, row_stride, height, |row_start, _row_end, y, row| {
        let _ = row_start;
        let y = y as i32;
        // Rayon gives this closure a complete output row. Reusing the
        // window scratch for that row removes the old per-pixel
        // allocation while keeping each parallel worker independent.
        let mut values = Vec::with_capacity(area);
        for x in 0..w {
            values.clear();
            for dy in -half..=half {
                for dx in -half..=half {
                    let sx = (x + dx).clamp(0, w - 1);
                    let sy = (y + dy).clamp(0, h - 1);
                    let base = (sy * w + sx) as usize * 4;
                    values.push(f32::from_le_bytes([
                        raw[base],
                        raw[base + 1],
                        raw[base + 2],
                        raw[base + 3],
                    ]));
                }
            }
            values.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let out_base = x as usize * 4;
            row[out_base..out_base + 4].copy_from_slice(&values[rank].to_le_bytes());
        }
    });
}

/// Generic rank filter: sorts neighborhood values and picks the one at `rank`.
/// PIL uses clamping for border pixels.
/// Generalized to handle any number of channels (1-4).
/// For F-mode ("F"): treats 4 RGBA bytes as a single f32 value, sorts floats.
fn rank_filter_impl(
    img: &DynamicImage,
    size: u32,
    rank: u32,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w_u32, h_u32) = (img.width(), img.height());
    let (w, h) = (w_u32 as i32, h_u32 as i32);
    let half = (size / 2) as i32;
    let area = (size * size) as usize;
    let rank = rank.min((area - 1) as u32) as usize;

    // For F-mode: operate on f32 values stored as 4 RGBA bytes
    if mode == Some("F") {
        let rgba = img.to_rgba8();
        let raw = rgba.into_raw();
        let mut out = CheckedDims::new(w as u32, h as u32, 4)?.alloc_buffer();
        if area <= SMALL_RANK_AREA {
            for y in 0..h {
                for x in 0..w {
                    let mut values = [0f32; SMALL_RANK_AREA];
                    let mut value_index = 0usize;
                    for dy in -half..=half {
                        for dx in -half..=half {
                            let sx = (x + dx).clamp(0, w - 1);
                            let sy = (y + dy).clamp(0, h - 1);
                            let base = (sy * w + sx) as usize * 4;
                            values[value_index] = f32::from_le_bytes([
                                raw[base],
                                raw[base + 1],
                                raw[base + 2],
                                raw[base + 3],
                            ]);
                            value_index += 1;
                        }
                    }
                    values[..area].sort_unstable_by(|a, b| {
                        a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
                    });
                    let out_base = (y * w + x) as usize * 4;
                    out[out_base..out_base + 4].copy_from_slice(&values[rank].to_le_bytes());
                }
            }
        } else {
            #[cfg(feature = "parallel")]
            rank_filter_f_large_parallel(&raw, &mut out, w, h, half, area, rank);
            #[cfg(not(feature = "parallel"))]
            rank_filter_f_large_serial(&raw, &mut out, w, h, half, area, rank);
        }
        let result = DynamicImage::ImageRgba8(
            crate::raster::RgbaImage::from_raw(w_u32, h_u32, out)
                .ok_or_else(|| PilError::ValueError("rank_filter_impl(F): buffer error".into()))?,
        );
        return Ok(preserve_mode(img, result));
    }

    let channels = img.color().channel_count() as usize;
    let raw = img.as_bytes();
    let mut out = CheckedDims::new(w as u32, h as u32, channels as u8)?.alloc_buffer();

    let native_byte_layout = matches!(
        img,
        DynamicImage::ImageLuma8(_)
            | DynamicImage::ImageLumaA8(_)
            | DynamicImage::ImageRgb8(_)
            | DynamicImage::ImageRgba8(_)
    );
    let separable_extreme =
        native_byte_layout && size >= 5 && w.max(h) > 512 && (rank == 0 || rank == area - 1);
    if separable_extreme
        && rank_filter_bytes_extreme_separable(
            raw,
            &mut out,
            w,
            h,
            channels,
            half,
            rank == area - 1,
        )
    {
        // The separable path completed the output. Keep the original direct
        // path below for typed layouts and small windows where its lower setup
        // cost is preferable.
    } else if rank == 0 {
        rank_filter_bytes_extreme(raw, &mut out, w, h, channels, half, false);
    } else if rank == area - 1 {
        rank_filter_bytes_extreme(raw, &mut out, w, h, channels, half, true);
    } else if area <= SMALL_RANK_AREA {
        rank_filter_bytes_small(raw, &mut out, w, h, channels, half, area, rank);
    } else {
        rank_filter_bytes_histogram(raw, &mut out, w, h, channels, half, rank);
    }
    let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
    Ok(preserve_mode(img, result))
}

// ── Execute filter ops ──

/// Execute a 3x3 convolution filter.
pub fn execute_filter3x3(
    img: &DynamicImage,
    kernel: &[f32; 9],
    scale: f32,
    offset: i32,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // I-mode: operate directly on int32 pixel values (no [0,255] clipping)
    if explicit_mode == Some("I") {
        return filter_3x3_i32(img, kernel, scale, offset);
    }
    let channels = img.color().channel_count() as usize;
    let raw = img.as_bytes();
    let (w_u32, h_u32) = (img.width(), img.height());
    let (w, h) = (w_u32 as i32, h_u32 as i32);
    // The public kernel boundary clamps scales to a positive value before a
    // pipeline operation is created. Normalize once, outside the pixel loop.
    let normalized_kernel: [f32; 9] = std::array::from_fn(|index| kernel[index] / scale);
    let rounding_bias = offset as f32 + 0.5;
    let mut out = raw.to_vec();
    filter_3x3_byte_rows(
        raw,
        &mut out,
        w,
        h,
        channels,
        &normalized_kernel,
        rounding_bias,
    );
    let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
    Ok(preserve_mode(img, result))
}

/// Execute a 5x5 convolution filter.
pub fn execute_filter5x5(
    img: &DynamicImage,
    kernel: &[f32; 25],
    scale: f32,
    offset: i32,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // I-mode: operate directly on int32 pixel values (no [0,255] clipping)
    if explicit_mode == Some("I") {
        return filter_5x5_i32(img, kernel, scale, offset);
    }
    let channels = img.color().channel_count() as usize;
    let raw = img.as_bytes();
    let (w_u32, h_u32) = (img.width(), img.height());
    let (w, h) = (w_u32 as i32, h_u32 as i32);
    // Normalize once, outside the pixel loop. The row helper keeps all five
    // tap groups in the same order as the original scalar implementation.
    let normalized_kernel: [f32; 25] = std::array::from_fn(|index| kernel[index] / scale);
    let rounding_bias = offset as f32 + 0.5;
    let mut out = raw.to_vec();
    filter_5x5_byte_rows(
        raw,
        &mut out,
        w,
        h,
        channels,
        &normalized_kernel,
        rounding_bias,
    );
    let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
    Ok(preserve_mode(img, result))
}

/// Execute a Gaussian blur via 3 passes of PIL-style box blur.
pub fn execute_gaussian_blur(img: &DynamicImage, sigma: f32) -> Result<DynamicImage, PilError> {
    // PIL GaussianBlur: 3 passes of BoxBlur with computed fractional radius.
    // Uses the "From Box Blur to Gaussian Blur" algorithm (Gwosdek et al. 2011).
    // PIL's ImagingGaussianBlur uses f32 parameters but f64 in sqrt/promotion.
    let sigma = sigma.abs();
    if sigma == 0.0 {
        return Ok(img.clone());
    }
    let passes = 3.0f64;
    let sigma2 = sigma as f64 * sigma as f64 / passes;
    let l_val = ((12.0 * sigma2 + 1.0).sqrt() - 1.0) / 2.0;
    let l = l_val.floor();
    let l1 = l + 1.0;
    let a_num = (2.0 * l + 1.0) * (l * l1 - 3.0 * sigma2);
    // For each interval selected by `l = floor(l_val)`, sigma2 is strictly
    // below `(l + 1)^2`, so this denominator cannot be zero for sigma > 0.
    let a_den = 6.0 * (sigma2 - l1 * l1);
    let a = a_num / a_den;
    // Assign back to f32 (PIL: result is float)
    let blur_radius = (l + a) as f32;
    pil_box_blur(img, blur_radius, 3)
}

/// Execute a box blur with integer radius.
pub fn execute_box_blur(img: &DynamicImage, radius: u32) -> Result<DynamicImage, PilError> {
    if radius == 0 {
        return Ok(img.clone());
    }
    pil_box_blur(img, radius as f32, 1)
}

/// Execute a box blur with independent radii and a fixed number of passes.
pub fn execute_box_blur_xy_with_passes(
    img: &DynamicImage,
    radius_x: f32,
    radius_y: f32,
    passes: u32,
) -> Result<DynamicImage, PilError> {
    if radius_x == 0.0 && radius_y == 0.0 {
        return Ok(img.clone());
    }
    pil_box_blur_xy(img, radius_x, radius_y, passes)
}

/// Execute a median filter with explicit mode.
pub fn execute_median_filter_with_mode(
    img: &DynamicImage,
    size: u32,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    rank_filter_impl(img, size, size * size / 2, mode)
}

/// Execute a max filter with explicit mode.
pub fn execute_max_filter_with_mode(
    img: &DynamicImage,
    size: u32,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    rank_filter_impl(img, size, size * size - 1, mode)
}

/// Execute a min filter with explicit mode.
pub fn execute_min_filter_with_mode(
    img: &DynamicImage,
    size: u32,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    rank_filter_impl(img, size, 0, mode)
}

/// Execute a rank filter with explicit mode.
pub fn execute_rank_filter_with_mode(
    img: &DynamicImage,
    size: u32,
    rank: u32,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    rank_filter_impl(img, size, rank, mode)
}
