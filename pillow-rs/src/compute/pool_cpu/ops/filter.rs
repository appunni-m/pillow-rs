//! Filter operations extracted from image.rs execute_op().
//!
//! These functions are standalone implementations of PIL-compatible filter
//! operations (Filter3x3, Filter5x5, GaussianBlur, BoxBlur, MedianFilter,
//! MaxFilter, MinFilter, RankFilter) that operate on DynamicImage and return
//! new DynamicImage instances.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;

#[cfg(not(feature = "parallel"))]
#[path = "filter/uniform_box.rs"]
mod uniform_box;

#[cfg(not(feature = "parallel"))]
#[path = "filter/rgb_blur_ring.rs"]
mod rgb_blur_ring;

#[cfg(not(feature = "parallel"))]
#[path = "filter/kernels/mod.rs"]
mod kernels;

#[path = "filter/kernels/blur_line_steps.rs"]
mod blur_line_steps;

#[cfg(all(test, not(feature = "parallel")))]
#[path = "filter/uniform_box_tests.rs"]
mod uniform_box_tests;

use crate::image::preserve_mode;
use crate::image_utils::raw_bytes_to_image;
#[cfg(not(feature = "parallel"))]
use crate::ops::pil_resize::i32_samples_from_le_bytes;
use crate::raster::DynamicImage;
use std::borrow::Cow;

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

#[cfg(not(feature = "parallel"))]
trait F32MulAdd {
    fn mul_add(&self, sample: f32, weight: f32, accumulator: f32) -> f32;
}

#[cfg(not(feature = "parallel"))]
struct PortableF32Fma;

#[cfg(not(feature = "parallel"))]
impl F32MulAdd for PortableF32Fma {
    #[inline(always)]
    fn mul_add(&self, sample: f32, weight: f32, accumulator: f32) -> f32 {
        sample.mul_add(weight, accumulator)
    }
}

#[cfg(all(not(feature = "parallel"), target_arch = "x86_64"))]
struct X86F32FmaToken {
    _private: (),
}

#[cfg(all(not(feature = "parallel"), target_arch = "x86_64"))]
impl X86F32FmaToken {
    fn detect() -> Option<Self> {
        std::is_x86_feature_detected!("fma").then_some(Self { _private: () })
    }
}

#[cfg(all(not(feature = "parallel"), target_arch = "x86_64"))]
impl F32MulAdd for X86F32FmaToken {
    #[allow(unsafe_code)]
    #[inline(always)]
    fn mul_add(&self, sample: f32, weight: f32, accumulator: f32) -> f32 {
        let mut result = accumulator;
        // SAFETY: this token is created only after runtime FMA detection. The
        // I-mode ring selects it only when finite coefficients and the full
        // i32 input range prove that every intermediate stays finite.
        unsafe {
            std::arch::asm!(
                "vfmadd231ss {result}, {weight}, {sample}",
                result = inout(xmm_reg) result,
                weight = in(xmm_reg) weight,
                sample = in(xmm_reg) sample,
                options(pure, nomem, nostack, preserves_flags),
            );
        }
        result
    }
}

#[cfg(not(feature = "parallel"))]
#[inline(always)]
fn pillow_kernel_row_5_with_fma<F: F32MulAdd>(pixels: [f32; 5], kernel: &[f32], fma: &F) -> f32 {
    let sum = pixels[1] * kernel[1];
    let sum = fma.mul_add(pixels[0], kernel[0], sum);
    let sum = fma.mul_add(pixels[2], kernel[2], sum);
    let sum = fma.mul_add(pixels[3], kernel[3], sum);
    fma.mul_add(pixels[4], kernel[4], sum)
}

/// Prove that the normalized 5x5 binomial kernel keeps all ordered partial
/// sums finite and away from subnormals for every signed-I sample. Its exact
/// dyadic coefficients and zero offset let x86 use hardware FMA without
/// per-tap checks; arbitrary kernels keep the portable libm implementation.
#[cfg(all(not(feature = "parallel"), target_arch = "x86_64"))]
fn i32_filter_f32_fma_is_safe(kernel: &[f32; 25], offset: f32) -> bool {
    if offset != 0.0 {
        return false;
    }
    let factors = [1.0f32, 4.0, 6.0, 4.0, 1.0];
    kernel.iter().enumerate().all(|(index, coefficient)| {
        let expected = factors[index % 5] * factors[index / 5] / 256.0;
        coefficient.to_bits() == expected.to_bits()
    })
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
    offset: f32,
) {
    if y < 1 || y >= height - 1 || width < 3 {
        return;
    }
    let width = width as usize;
    let y = y as usize;
    let read_pixel = |row_start: usize, x: usize| -> i32 {
        let index = row_start + x * 4;
        i32::from_le_bytes([raw[index], raw[index + 1], raw[index + 2], raw[index + 3]])
    };
    let row_stride = width * 4;
    let top_start = (y - 1) * row_stride;
    let middle_start = y * row_stride;
    let bottom_start = (y + 1) * row_stride;

    // Slide three source-row windows across x. Each sample is decoded once as
    // it enters a window instead of decoding all nine neighbors per output.
    let (mut top_left, mut top_middle, mut top_right) = (
        read_pixel(top_start, 0),
        read_pixel(top_start, 1),
        read_pixel(top_start, 2),
    );
    let (mut middle_left, mut middle_center, mut middle_right) = (
        read_pixel(middle_start, 0),
        read_pixel(middle_start, 1),
        read_pixel(middle_start, 2),
    );
    let (mut bottom_left, mut bottom_middle, mut bottom_right) = (
        read_pixel(bottom_start, 0),
        read_pixel(bottom_start, 1),
        read_pixel(bottom_start, 2),
    );

    for x in 1..width - 1 {
        let bottom = pillow_kernel_row_3(
            [
                bottom_left as f32,
                bottom_middle as f32,
                bottom_right as f32,
            ],
            &kernel[0..3],
        );
        let middle = pillow_kernel_row_3(
            [
                middle_left as f32,
                middle_center as f32,
                middle_right as f32,
            ],
            &kernel[3..6],
        );
        let top = pillow_kernel_row_3(
            [top_left as f32, top_middle as f32, top_right as f32],
            &kernel[6..9],
        );
        let mut value = offset + 0.5;
        value += bottom;
        value += middle;
        value += top;
        let result = if value >= 0.0 { value as i32 } else { 0 };
        let output = x * 4;
        row[output..output + 4].copy_from_slice(&result.to_le_bytes());

        if x + 1 < width - 1 {
            let next = x + 2;
            top_left = top_middle;
            top_middle = top_right;
            top_right = read_pixel(top_start, next);
            middle_left = middle_center;
            middle_center = middle_right;
            middle_right = read_pixel(middle_start, next);
            bottom_left = bottom_middle;
            bottom_middle = bottom_right;
            bottom_right = read_pixel(bottom_start, next);
        }
    }
}

// The 3x3 I-mode filter can cheaply skip its per-pixel convolution when a
// bounded native image is uniform and the exact Pillow f32 result equals its
// source sample. Keep this limit aligned with the 5x5 proof below.
pub(crate) const I32_UNIFORM_FILTER3X3_MAX_PIXELS: usize = 1024 * 1024;

/// Compare one uniform sample's exact normalized 3x3 result with its source.
pub(crate) fn uniform_i32_filter3x3_sample_is_identity(
    sample_i32: i32,
    kernel: &[f32; 9],
    rounding_bias: f32,
) -> bool {
    let sample = sample_i32 as f32;
    let bottom = pillow_kernel_row_3([sample; 3], &kernel[0..3]);
    let middle = pillow_kernel_row_3([sample; 3], &kernel[3..6]);
    let top = pillow_kernel_row_3([sample; 3], &kernel[6..9]);
    let mut value = rounding_bias;
    value += bottom;
    value += middle;
    value += top;
    let filtered = if value >= 0.0 { value as i32 } else { 0 };
    filtered == sample_i32
}

/// Prove a uniform I image is unchanged by this normalized 3x3 convolution.
/// Use the same fused row expressions, vertical accumulation order, rounding
/// bias, and truncation as [`filter_3x3_i32_row`].
pub(crate) fn uniform_i32_filter3x3_is_identity(
    raw: &[u8],
    width: usize,
    height: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) -> bool {
    if width < 3 || height < 3 {
        return false;
    }
    let Some(pixel_count) = width.checked_mul(height) else {
        return false;
    };
    if pixel_count == 0 || pixel_count > I32_UNIFORM_FILTER3X3_MAX_PIXELS {
        return false;
    }
    let Some(expected_len) = pixel_count.checked_mul(4) else {
        return false;
    };
    if raw.len() != expected_len {
        return false;
    }
    let first = &raw[..4];
    if !raw.chunks_exact(4).skip(1).all(|pixel| pixel == first) {
        return false;
    }

    let sample_i32 = i32::from_le_bytes([first[0], first[1], first[2], first[3]]);
    uniform_i32_filter3x3_sample_is_identity(sample_i32, kernel, rounding_bias)
}

fn filter_3x3_i32(
    img: &DynamicImage,
    kernel: &[f32; 9],
    scale: f32,
    offset: f32,
) -> Result<DynamicImage, PilError> {
    // I-mode stores one little-endian i32 in each four-byte pixel. Borrow that
    // exact carrier instead of cloning it through the RGBA accessor; retain
    // the old widening route for callers with a different concrete variant.
    let (w_u32, h_u32) = (img.width(), img.height());
    let raw: Cow<'_, [u8]> = match img {
        DynamicImage::ImageRgba8(rgba) => Cow::Borrowed(rgba.as_raw()),
        _ => Cow::Owned(img.to_rgba8().into_raw()),
    };
    let raw = raw.as_ref();
    let (w, h) = (w_u32 as i32, h_u32 as i32);

    // The C filter receives the raw f32 divisor, including zero and negative
    // values. Rust's IEEE-754 division and saturating float-to-int cast match
    // the corresponding clip32 behavior at this boundary.
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

    let mut out = raw.to_vec();

    let row_stride = w_u32 as usize * 4;
    #[cfg(feature = "parallel")]
    if row_stride != 0 {
        crate::par_rows_mut!(
            &mut out,
            row_stride,
            h_u32 as usize,
            |_row_start, _row_end, y, row| {
                filter_3x3_i32_row(raw, row, y as i32, w, h, &kd, offset);
            }
        );
    } else {
        for y in 0..h {
            let row_start = y as usize * row_stride;
            filter_3x3_i32_row(
                raw,
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
    if kernels::matches(&kd, offset) {
        kernels::apply(raw, &mut out, w_u32 as usize, h_u32 as usize);
    } else {
        for y in 0..h {
            let row_start = y as usize * row_stride;
            filter_3x3_i32_row(
                raw,
                &mut out[row_start..row_start + row_stride],
                y,
                w,
                h,
                &kd,
                offset,
            );
        }
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
    offset: f32,
) {
    if y < 2 || y >= height - 2 || width < 5 {
        return;
    }
    let width = width as usize;
    let y = y as usize;
    let source_row_starts = [
        (y + 2) * width,
        (y + 1) * width,
        y * width,
        (y - 1) * width,
        (y - 2) * width,
    ];
    let mut windows = [[0.0f32; 5]; 5];
    for (window, source_row) in windows.iter_mut().zip(source_row_starts) {
        for (tap, sample) in window.iter_mut().enumerate() {
            *sample = read_i32_pixel_le(raw, source_row + tap) as f32;
        }
    }

    for x in 2..width - 2 {
        let bottom0 = pillow_kernel_row_5(windows[0], &kernel[0..5]);
        let bottom1 = pillow_kernel_row_5(windows[1], &kernel[5..10]);
        let middle = pillow_kernel_row_5(windows[2], &kernel[10..15]);
        let top1 = pillow_kernel_row_5(windows[3], &kernel[15..20]);
        let top0 = pillow_kernel_row_5(windows[4], &kernel[20..25]);
        let mut value = offset + 0.5;
        value += bottom0;
        value += bottom1;
        value += middle;
        value += top1;
        value += top0;
        let result = if value >= 0.0 { value as i32 } else { 0 };
        let output = x * 4;
        row[output..output + 4].copy_from_slice(&result.to_le_bytes());

        if x + 1 < width - 2 {
            let entering_x = x + 3;
            for (window, source_row) in windows.iter_mut().zip(source_row_starts) {
                window[0] = window[1];
                window[1] = window[2];
                window[2] = window[3];
                window[3] = window[4];
                window[4] = read_i32_pixel_le(raw, source_row + entering_x) as f32;
            }
        }
    }
}

// A native I-mode convolution can amortize a uniformity proof over a larger
// image than the byte filters: each interior output otherwise reads and
// converts 25 four-byte samples. Keep the proof bounded so a nearly-uniform
// large image cannot add an unbounded extra full-frame scan.
const I32_UNIFORM_FILTER5X5_MAX_PIXELS: usize = 1024 * 1024;
const I32_BINOMIAL_FILTER5X5_WEIGHTS: [i32; 5] = [1, 4, 6, 4, 1];
const I32_BINOMIAL_FILTER5X5_MAX_ABS_SAMPLE: u32 = 65_535;

fn i32_filter5x5_binomial_kernel_is_applicable(kernel: &[f32; 25]) -> bool {
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
fn i32_filter5x5_bounded_binomial_kernel_is_applicable(
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
fn filter_5x5_i32_bounded_binomial(raw: &[u8], width: usize, height: usize) -> Option<Vec<u8>> {
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

#[inline]
fn read_i32_pixel_le(raw: &[u8], pixel_index: usize) -> i32 {
    let byte = pixel_index * 4;
    i32::from_le_bytes([raw[byte], raw[byte + 1], raw[byte + 2], raw[byte + 3]])
}

/// Convert each I-mode sample to f32 once, then reuse five source rows while
/// evaluating the scalar convolution in Pillow's original FMA order.
#[cfg(not(feature = "parallel"))]
fn filter_5x5_i32_f32_ring(
    raw: &[u8],
    width: usize,
    height: usize,
    kernel: &[f32; 25],
    offset: f32,
) -> Option<Vec<u8>> {
    #[cfg(target_arch = "x86_64")]
    if i32_filter_f32_fma_is_safe(kernel, offset)
        && let Some(fma) = X86F32FmaToken::detect()
    {
        return filter_5x5_i32_f32_ring_with_fma(raw, width, height, kernel, offset, &fma);
    }

    filter_5x5_i32_f32_ring_with_fma(raw, width, height, kernel, offset, &PortableF32Fma)
}

#[cfg(not(feature = "parallel"))]
fn filter_5x5_i32_output_with_copied_border(
    raw: &[u8],
    width: usize,
    height: usize,
) -> Option<Vec<u8>> {
    let expected_len = width.checked_mul(height)?.checked_mul(4)?;
    if raw.len() != expected_len {
        return None;
    }

    // Every interior sample is replaced by the filter, while border bytes
    // must preserve the input exactly. Start from those input bytes rather
    // than zero-filling a destination whose full frame is then overwritten.
    let output = raw.to_vec();
    if width < 5 || height < 5 {
        return Some(output);
    }
    Some(output)
}

/// Reuse the exact horizontal FMA rows of the binomial kernel. Its vertical
/// weights are `[1, 4, 6, 4, 1]`: rows weighted by four are exact power-of-two
/// scalings of the weight-one row, and the weight-six row is twice a row
/// weighted by three. Power-of-two scaling commutes with these finite normal
/// f32 FMA results, while preserving Pillow's horizontal and vertical order.
#[cfg(not(feature = "parallel"))]
fn filter_5x5_i32_f32_binomial_scaled_rows_ring_with_fma<F: F32MulAdd>(
    raw: &[u8],
    samples: &[i32],
    width: usize,
    height: usize,
    kernel: &[f32; 25],
    offset: f32,
    fma: &F,
) -> Option<Vec<u8>> {
    let pixel_count = width.checked_mul(height)?;
    if samples.len() != pixel_count || raw.len() != pixel_count.checked_mul(4)? {
        return None;
    }
    if !i32_filter5x5_binomial_kernel_is_applicable(kernel) {
        return None;
    }

    let mut output = filter_5x5_i32_output_with_copied_border(raw, width, height)?;
    if width < 5 || height < 5 {
        return Some(output);
    }

    let ring_len = width.checked_mul(5)?;
    let row_stride = width.checked_mul(4)?;
    let mut horizontal_one_rows = vec![0.0f32; ring_len];
    let mut horizontal_three_rows = vec![0.0f32; ring_len];
    let horizontal_one_kernel = &kernel[0..5];
    let horizontal_three_kernel: [f32; 5] = std::array::from_fn(|tap| kernel[10 + tap] * 0.5);

    for source_y in 0..height {
        let input_row_start = source_y * width;
        let ring_row_start = source_y % 5 * width;
        let mut window = [0.0f32; 5];
        for (tap, value) in window.iter_mut().enumerate() {
            *value = samples[input_row_start + tap] as f32;
        }

        for x in 2..width - 2 {
            let index = ring_row_start + x;
            horizontal_one_rows[index] =
                pillow_kernel_row_5_with_fma(window, horizontal_one_kernel, fma);
            horizontal_three_rows[index] =
                pillow_kernel_row_5_with_fma(window, &horizontal_three_kernel, fma);

            if x + 1 < width - 2 {
                window = [
                    window[1],
                    window[2],
                    window[3],
                    window[4],
                    samples[input_row_start + x + 3] as f32,
                ];
            }
        }

        if source_y < 4 {
            continue;
        }

        let bottom = source_y % 5 * width;
        let lower = (source_y - 1) % 5 * width;
        let center = (source_y - 2) % 5 * width;
        let upper = (source_y - 3) % 5 * width;
        let top = (source_y - 4) % 5 * width;
        let output_row_start = (source_y - 2) * row_stride;
        for x in 2..width - 2 {
            let mut value = offset + 0.5;
            value += horizontal_one_rows[bottom + x];
            value += horizontal_one_rows[lower + x] * 4.0;
            value += horizontal_three_rows[center + x] * 2.0;
            value += horizontal_one_rows[upper + x] * 4.0;
            value += horizontal_one_rows[top + x];
            let result = if value >= 0.0 { value as i32 } else { 0 };
            let output_pixel = output_row_start + x * 4;
            output[output_pixel..output_pixel + 4].copy_from_slice(&result.to_le_bytes());
        }
    }

    Some(output)
}

#[cfg(not(feature = "parallel"))]
fn filter_5x5_i32_bounded_binomial_from_samples(
    raw: &[u8],
    samples: &[i32],
    width: usize,
    height: usize,
) -> Option<Vec<u8>> {
    let pixel_count = width.checked_mul(height)?;
    if samples.len() != pixel_count || raw.len() != pixel_count.checked_mul(4)? {
        return None;
    }

    let ring_len = width.checked_mul(5)?;
    let mut horizontal_rows = vec![0_i32; ring_len];
    let mut output = filter_5x5_i32_output_with_copied_border(raw, width, height)?;
    if width < 5 || height < 5 {
        return Some(output);
    }

    let horizontal_row = |source_y: usize, row: &mut [i32]| {
        let row_start = source_y * width;
        let mut window = [
            samples[row_start],
            samples[row_start + 1],
            samples[row_start + 2],
            samples[row_start + 3],
            samples[row_start + 4],
        ];
        if window
            .iter()
            .any(|sample| sample.unsigned_abs() > I32_BINOMIAL_FILTER5X5_MAX_ABS_SAMPLE)
        {
            return false;
        }
        for x in 2..width - 2 {
            row[x] = window[0] + window[1] * 4 + window[2] * 6 + window[3] * 4 + window[4];
            if x + 1 < width - 2 {
                let entering = samples[row_start + x + 3];
                if entering.unsigned_abs() > I32_BINOMIAL_FILTER5X5_MAX_ABS_SAMPLE {
                    return false;
                }
                window[0] = window[1];
                window[1] = window[2];
                window[2] = window[3];
                window[3] = window[4];
                window[4] = entering;
            }
        }
        true
    };

    for source_y in 0..5 {
        let slot = source_y % 5;
        if !horizontal_row(
            source_y,
            &mut horizontal_rows[slot * width..(slot + 1) * width],
        ) {
            return None;
        }
    }
    for y in 2..height - 2 {
        let bottom = (y + 2) % 5 * width;
        let lower = (y + 1) % 5 * width;
        let center = y % 5 * width;
        let upper = (y - 1) % 5 * width;
        let top = (y - 2) % 5 * width;
        let output_row_start = y * width * 4;
        for x in 2..width - 2 {
            let sum = horizontal_rows[bottom + x]
                + horizontal_rows[lower + x] * 4
                + horizontal_rows[center + x] * 6
                + horizontal_rows[upper + x] * 4
                + horizontal_rows[top + x];
            let filtered = if sum >= 0 {
                // The bounded-input proof keeps this rounded sum in range; for
                // nonnegative values, division by 256 is an exact eight-bit shift.
                ((sum as u32 + 128) >> 8) as i32
            } else {
                0
            };
            let output_pixel = output_row_start + x * 4;
            output[output_pixel..output_pixel + 4].copy_from_slice(&filtered.to_le_bytes());
        }

        let next_source_y = y + 3;
        if next_source_y < height {
            let slot = next_source_y % 5;
            if !horizontal_row(
                next_source_y,
                &mut horizontal_rows[slot * width..(slot + 1) * width],
            ) {
                return None;
            }
        }
    }
    Some(output)
}

/// Reuse horizontal sums when every kernel row has identical coefficients.
/// This preserves Pillow's per-row FMA and vertical accumulation order while
/// avoiding four repeated horizontal dot products for each output pixel.
#[cfg(not(feature = "parallel"))]
fn filter_5x5_i32_f32_equal_rows_ring_with_fma<F: F32MulAdd>(
    raw: &[u8],
    samples: &[i32],
    width: usize,
    height: usize,
    kernel: &[f32; 25],
    offset: f32,
    fma: &F,
) -> Option<Vec<u8>> {
    let mut output = filter_5x5_i32_output_with_copied_border(raw, width, height)?;
    if width < 5 || height < 5 {
        return Some(output);
    }

    let ring_len = width.checked_mul(5)?;
    let row_stride = width.checked_mul(4)?;
    let mut horizontal_rows = vec![0.0f32; ring_len];

    for source_y in 0..height {
        let input_row_start = source_y * width;
        let output_slot_start = source_y % 5 * width;
        let mut window = [0.0f32; 5];
        for (x, value) in window.iter_mut().enumerate() {
            *value = samples[input_row_start + x] as f32;
        }

        for x in 2..width - 2 {
            horizontal_rows[output_slot_start + x] =
                pillow_kernel_row_5_with_fma(window, &kernel[0..5], fma);
            if x + 1 < width - 2 {
                window = [
                    window[1],
                    window[2],
                    window[3],
                    window[4],
                    samples[input_row_start + x + 3] as f32,
                ];
            }
        }

        if source_y < 4 {
            continue;
        }

        let source_row_slots = [
            source_y % 5 * width,
            (source_y - 1) % 5 * width,
            (source_y - 2) % 5 * width,
            (source_y - 3) % 5 * width,
            (source_y - 4) % 5 * width,
        ];
        let output_row_start = (source_y - 2) * row_stride;
        for x in 2..width - 2 {
            let mut value = offset + 0.5;
            value += horizontal_rows[source_row_slots[0] + x];
            value += horizontal_rows[source_row_slots[1] + x];
            value += horizontal_rows[source_row_slots[2] + x];
            value += horizontal_rows[source_row_slots[3] + x];
            value += horizontal_rows[source_row_slots[4] + x];
            let result = if value >= 0.0 { value as i32 } else { 0 };
            let output_pixel = output_row_start + x * 4;
            output[output_pixel..output_pixel + 4].copy_from_slice(&result.to_le_bytes());
        }
    }

    Some(output)
}

#[cfg(not(feature = "parallel"))]
fn filter_5x5_i32_f32_ring_with_fma<F: F32MulAdd>(
    raw: &[u8],
    width: usize,
    height: usize,
    kernel: &[f32; 25],
    offset: f32,
    fma: &F,
) -> Option<Vec<u8>> {
    let pixel_count = width.checked_mul(height)?;
    if raw.len() != pixel_count.checked_mul(4)? {
        return None;
    }
    let samples = i32_samples_from_le_bytes(raw, pixel_count);
    if samples.len() != pixel_count {
        return None;
    }
    if i32_filter5x5_binomial_kernel_is_applicable(kernel) {
        return filter_5x5_i32_f32_binomial_scaled_rows_ring_with_fma(
            raw,
            samples.as_ref(),
            width,
            height,
            kernel,
            offset,
            fma,
        );
    }
    if kernel[0..5] == kernel[5..10]
        && kernel[0..5] == kernel[10..15]
        && kernel[0..5] == kernel[15..20]
        && kernel[0..5] == kernel[20..25]
    {
        return filter_5x5_i32_f32_equal_rows_ring_with_fma(
            raw,
            samples.as_ref(),
            width,
            height,
            kernel,
            offset,
            fma,
        );
    }

    let ring_len = width.checked_mul(5)?;
    let mut source_rows = vec![0.0f32; ring_len];
    let row_stride = width.checked_mul(4)?;
    let mut output = filter_5x5_i32_output_with_copied_border(raw, width, height)?;

    for source_y in 0..height {
        let slot_start = source_y % 5 * width;
        let input_row_start = source_y * width;
        for x in 0..width {
            source_rows[slot_start + x] = samples[input_row_start + x] as f32;
        }
        if source_y < 4 || width < 5 {
            continue;
        }

        let source_row_slots = [
            source_y % 5 * width,
            (source_y - 1) % 5 * width,
            (source_y - 2) % 5 * width,
            (source_y - 3) % 5 * width,
            (source_y - 4) % 5 * width,
        ];
        let output_row_start = (source_y - 2) * row_stride;
        let mut windows = [[0.0f32; 5]; 5];
        for (window, row_start) in windows.iter_mut().zip(source_row_slots) {
            window.copy_from_slice(&source_rows[row_start..row_start + 5]);
        }

        for x in 2..width - 2 {
            let bottom0 = pillow_kernel_row_5_with_fma(windows[0], &kernel[0..5], fma);
            let bottom1 = pillow_kernel_row_5_with_fma(windows[1], &kernel[5..10], fma);
            let middle = pillow_kernel_row_5_with_fma(windows[2], &kernel[10..15], fma);
            let top1 = pillow_kernel_row_5_with_fma(windows[3], &kernel[15..20], fma);
            let top0 = pillow_kernel_row_5_with_fma(windows[4], &kernel[20..25], fma);
            let mut value = offset + 0.5;
            value += bottom0;
            value += bottom1;
            value += middle;
            value += top1;
            value += top0;
            let result = if value >= 0.0 { value as i32 } else { 0 };
            let output_pixel = output_row_start + x * 4;
            output[output_pixel..output_pixel + 4].copy_from_slice(&result.to_le_bytes());

            if x + 1 < width - 2 {
                let entering_x = x + 3;
                for (window, row_start) in windows.iter_mut().zip(source_row_slots) {
                    window[0] = window[1];
                    window[1] = window[2];
                    window[2] = window[3];
                    window[3] = window[4];
                    window[4] = source_rows[row_start + entering_x];
                }
            }
        }
    }
    Some(output)
}

/// Prove that a uniform I image is unchanged by this exact normalized 5x5
/// convolution. The sample and the five row sums use the same f32 conversion,
/// FMA order, vertical accumulation, bias, and truncation as
/// [`filter_5x5_i32_row`]; only a proved byte-identical result may skip the
/// per-pixel kernel.
pub(crate) fn uniform_i32_filter5x5_is_identity(
    raw: &[u8],
    width: usize,
    height: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) -> bool {
    if width < 5 || height < 5 {
        return false;
    }
    let Some(pixel_count) = width.checked_mul(height) else {
        return false;
    };
    if pixel_count == 0 || pixel_count > I32_UNIFORM_FILTER5X5_MAX_PIXELS {
        return false;
    }
    let Some(expected_len) = pixel_count.checked_mul(4) else {
        return false;
    };
    if raw.len() != expected_len {
        return false;
    }
    let first = &raw[..4];
    if !raw.chunks_exact(4).skip(1).all(|pixel| pixel == first) {
        return false;
    }

    let sample_i32 = i32::from_le_bytes([first[0], first[1], first[2], first[3]]);
    let sample = sample_i32 as f32;
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
    let filtered = if value >= 0.0 { value as i32 } else { 0 };
    filtered == sample_i32
}

fn filter_5x5_i32(
    img: &DynamicImage,
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
) -> Result<DynamicImage, PilError> {
    // I-mode stores one little-endian i32 word per four-byte pixel. Borrow a
    // matching carrier directly, keeping the existing accessor fallback for
    // mismatched internal variants.
    let (w_u32, h_u32) = (img.width(), img.height());
    let raw: Cow<'_, [u8]> = match img {
        DynamicImage::ImageRgba8(rgba) => Cow::Borrowed(rgba.as_raw()),
        _ => Cow::Owned(img.to_rgba8().into_raw()),
    };
    let raw = raw.as_ref();
    let (w, h) = (w_u32 as i32, h_u32 as i32);

    // See the 3x3 path: preserve the raw C f32 divisor, including zero,
    // negative, and non-finite values.
    let s = scale;
    // Pre-compute normalized kernel coefficients using f32 (matching PIL C construction)
    let kd: [f32; 25] = std::array::from_fn(|i| kernel[i] / s);

    if uniform_i32_filter5x5_is_identity(raw, w_u32 as usize, h_u32 as usize, &kd, offset + 0.5) {
        return Ok(DynamicImage::ImageRgba8(
            crate::raster::RgbaImage::from_raw(w_u32, h_u32, raw.to_vec())
                .ok_or_else(|| PilError::ValueError("filter_5x5_i32: buffer error".into()))?,
        ));
    }

    // Keep the bounded-input proof on both CPU profiles. Parallel CPU uses its
    // integer factorization below; serial CPU checks an aligned sample view
    // and only selects the integer factorization when every sample is proven
    // to preserve Pillow's f32 arithmetic exactly.
    #[cfg(feature = "parallel")]
    if i32_filter5x5_bounded_binomial_is_applicable(
        raw,
        w_u32 as usize,
        h_u32 as usize,
        &kd,
        offset,
    ) {
        let output = filter_5x5_i32_bounded_binomial(raw, w_u32 as usize, h_u32 as usize)
            .ok_or_else(|| PilError::ValueError("filter_5x5_i32: buffer error".into()))?;
        return Ok(DynamicImage::ImageRgba8(
            crate::raster::RgbaImage::from_raw(w_u32, h_u32, output)
                .ok_or_else(|| PilError::ValueError("filter_5x5_i32: buffer error".into()))?,
        ));
    }

    #[cfg(not(feature = "parallel"))]
    {
        let first_sample_is_bounded = raw.get(..4).is_some_and(|sample| {
            i32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]).unsigned_abs()
                <= I32_BINOMIAL_FILTER5X5_MAX_ABS_SAMPLE
        });
        if first_sample_is_bounded {
            let pixel_count = (w_u32 as usize).checked_mul(h_u32 as usize);
            if let Some(pixel_count) = pixel_count {
                let samples = i32_samples_from_le_bytes(raw, pixel_count);
                if i32_filter5x5_bounded_binomial_kernel_is_applicable(
                    samples.len(),
                    w_u32 as usize,
                    h_u32 as usize,
                    &kd,
                    offset,
                ) {
                    if let Some(output) = filter_5x5_i32_bounded_binomial_from_samples(
                        raw,
                        samples.as_ref(),
                        w_u32 as usize,
                        h_u32 as usize,
                    ) {
                        return Ok(DynamicImage::ImageRgba8(
                            crate::raster::RgbaImage::from_raw(w_u32, h_u32, output).ok_or_else(
                                || PilError::ValueError("filter_5x5_i32: buffer error".into()),
                            )?,
                        ));
                    }
                }
            }
        }
    }

    #[cfg(not(feature = "parallel"))]
    if let Some(output) = filter_5x5_i32_f32_ring(raw, w_u32 as usize, h_u32 as usize, &kd, offset)
    {
        return Ok(DynamicImage::ImageRgba8(
            crate::raster::RgbaImage::from_raw(w_u32, h_u32, output)
                .ok_or_else(|| PilError::ValueError("filter_5x5_i32: buffer error".into()))?,
        ));
    }

    let mut out = raw.to_vec();

    let row_stride = w_u32 as usize * 4;
    #[cfg(feature = "parallel")]
    if row_stride != 0 {
        crate::par_rows_mut!(
            &mut out,
            row_stride,
            h_u32 as usize,
            |_row_start, _row_end, y, row| {
                filter_5x5_i32_row(raw, row, y as i32, w, h, &kd, offset);
            }
        );
    } else {
        for y in 0..h {
            let row_start = y as usize * row_stride;
            filter_5x5_i32_row(
                raw,
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
            raw,
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

/// Apply the exact native-LA binomial 3x3 kernel through three reusable
/// horizontally filtered rows.  The [1, 2, 1] weights and scale 16 keep every
/// intermediate integral, so `(sum + 8) >> 4` matches Pillow's `f32` result
/// without changing the copied one-pixel border.
#[cfg(not(feature = "parallel"))]
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

#[cfg_attr(not(feature = "parallel"), inline(never))]
fn filter_5x5_byte_rows(
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

pub(crate) fn binomial3x3_parameters_match(kernel: &[f32; 9], scale: f32, offset: f32) -> bool {
    const WEIGHTS: [f32; 9] = [1.0, 2.0, 1.0, 2.0, 4.0, 2.0, 1.0, 2.0, 1.0];
    scale == 16.0 && offset == 0.0 && kernel == &WEIGHTS
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
fn filter_5x5_constant_bytes(
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
fn filter_5x5_uniform_rgba_identity(
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
fn filter_5x5_la_single_outlier_rows(
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
fn filter_5x5_uniform_luma_rows(raw: &[u8], out: &mut [u8], width: usize, height: usize) -> bool {
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
fn filter_5x5_uniform_rgba_rows(raw: &[u8], out: &mut [u8], width: usize, height: usize) -> bool {
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
fn filter_5x5_binomial_luma_parallel_rows(
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
fn filter_5x5_binomial_la_parallel_rows(
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
fn filter_5x5_binomial_luma_rows(raw: &[u8], out: &mut [u8], width: usize, height: usize) -> bool {
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
fn filter_5x5_binomial_la_rows(raw: &[u8], out: &mut [u8], width: usize, height: usize) -> bool {
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

// ── PIL-style box blur ──

const BOX_BLUR_SCALE: u32 = 1 << 24;
const BOX_BLUR_BIAS: u32 = 1 << 23;
const UNIFORM_BOX_BLUR_MAX_PIXELS: usize = 64 * 64;

/// Return whether Pillow's fixed-point box pass leaves one constant byte
/// unchanged for every position in a replicated-edge line.
#[cfg(not(feature = "parallel"))]
#[inline]
fn box_blur_preserves_constant_byte(
    value: u8,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
) -> bool {
    let Some(window_pixels) = radius
        .checked_mul(2)
        .and_then(|diameter| diameter.checked_add(1))
        .and_then(|window| u32::try_from(window).ok())
    else {
        return false;
    };

    let mut weighted = u32::from(value)
        .wrapping_mul(window_pixels)
        .wrapping_mul(whole_weight);
    if fractional_weight != 0 {
        let far_pair = (u32::from(value) + u32::from(value)).wrapping_mul(fractional_weight);
        weighted = weighted.wrapping_add(far_pair);
    }
    (weighted.wrapping_add(BOX_BLUR_BIAS) >> 24) as u8 == value
}

/// Prove that a large RGBA image is uniform and that one horizontal and one
/// vertical fixed-point pass preserve each of its channel values. The full
/// scan is intentional: sampled probes cannot prove that an arbitrary input
/// is uniform.
#[cfg(not(feature = "parallel"))]
#[inline]
fn uniform_rgba_box_blur_is_identity(
    raw: &[u8],
    width: usize,
    height: usize,
    horizontal: (usize, u32, u32),
    vertical: (usize, u32, u32),
) -> bool {
    let Some(expected_len) = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
    else {
        return false;
    };
    if width == 0 || height == 0 || raw.len() != expected_len {
        return false;
    }
    let first_pixel = &raw[..4];
    if !first_pixel.iter().all(|&value| {
        box_blur_preserves_constant_byte(value, horizontal.0, horizontal.1, horizontal.2)
            && box_blur_preserves_constant_byte(value, vertical.0, vertical.1, vertical.2)
    }) {
        return false;
    }

    // Cheaply reject typical nonuniform images before the full proof scan.
    // These points can reject a candidate, but only the complete scan below
    // can prove uniformity.
    let last_x = width - 1;
    let last_y = height - 1;
    for row in 0..=4 {
        let y = (last_y / 4) * row + (last_y % 4) * row / 4;
        for column in 0..=6 {
            let x = (last_x / 6) * column + (last_x % 6) * column / 6;
            let start = (y * width + x) * 4;
            if &raw[start..start + 4] != first_pixel {
                return false;
            }
        }
    }
    raw.chunks_exact(4).all(|pixel| pixel == first_pixel)
}

#[inline(always)]
fn blur_line_step<const FRACTIONAL: bool>(
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

    match element_width {
        1 => blur_line_steps::l_step::<FRACTIONAL>(
            source,
            destination,
            accumulator,
            output_base,
            subtract_base,
            add_base,
            far_left_base,
            far_right_base,
            whole_weight,
            fractional_weight,
        ),
        2 => blur_line_steps::la_step::<FRACTIONAL>(
            source,
            destination,
            accumulator,
            output_base,
            subtract_base,
            add_base,
            far_left_base,
            far_right_base,
            whole_weight,
            fractional_weight,
        ),
        3 => blur_line_steps::rgb_step::<FRACTIONAL>(
            source,
            destination,
            accumulator,
            output_base,
            subtract_base,
            add_base,
            far_left_base,
            far_right_base,
            whole_weight,
            fractional_weight,
        ),
        _ => blur_line_steps::interleaved_step::<FRACTIONAL>(
            source,
            destination,
            accumulator,
            element_width,
            output_base,
            subtract_base,
            add_base,
            far_left_base,
            far_right_base,
            whole_weight,
            fractional_weight,
        ),
    }
}

/// Blur one contiguous interleaved row using Pillow's four edge regions.
///
/// `src/libImaging/BoxBlur.c::ImagingLineBoxBlur{8,32}` initializes the
/// accumulator for logical pixel `-1`, then advances it by exactly one
/// entering and one leaving sample for every output pixel. Keeping the same
/// regions avoids a radius-sized inner loop while preserving Pillow's edge
/// replication and unsigned 24-bit fixed-point arithmetic.
#[inline(always)]
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

    if fractional_weight == 0 {
        blur_line_steps::run::<false>(
            source,
            destination,
            line_start,
            element_width,
            radius,
            whole_weight,
            fractional_weight,
            accumulator,
            last,
            edge_a,
            edge_b,
        );
    } else {
        blur_line_steps::run::<true>(
            source,
            destination,
            line_start,
            element_width,
            radius,
            whole_weight,
            fractional_weight,
            accumulator,
            last,
            edge_a,
            edge_b,
        );
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
const PARALLEL_BLUR_MIN_BYTES: usize = 256 * 1024;

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
    // A Rayon split has measurable scheduling cost for the small frames that
    // are common at this boundary. Keep the recurrence and row order exactly
    // the same, but stay on the caller for small buffers; larger frames still
    // use the existing row-parallel path.
    if destination.len() >= PARALLEL_BLUR_MIN_BYTES && row_length != 0 {
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
    } else {
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

#[cfg(not(feature = "parallel"))]
#[inline(always)]
fn store_radius_one_fractional_sample(
    destination: &mut u8,
    whole_sum: u32,
    far_sum: u32,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let bulk = whole_sum
        .wrapping_mul(whole_weight)
        .wrapping_add(far_sum.wrapping_mul(fractional_weight))
        .wrapping_add(BOX_BLUR_BIAS);
    *destination = (bulk >> 24) as u8;
}

/// Blur one L row with Pillow's radius-one fractional taps.
#[cfg(not(feature = "parallel"))]
fn blur_luma_radius_one_fractional_line(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    debug_assert!(width > 0);
    debug_assert!(source.len() >= width);
    debug_assert!(destination.len() >= width);
    debug_assert_ne!(fractional_weight, 0);

    if width < 5 {
        let mut accumulator = [0u32; 4];
        blur_line(
            source,
            destination,
            0,
            width,
            1,
            1,
            whole_weight,
            fractional_weight,
            &mut accumulator,
        );
        return;
    }

    let first = u32::from(source[0]);
    store_radius_one_fractional_sample(
        &mut destination[0],
        first * 2 + u32::from(source[1]),
        first + u32::from(source[2]),
        whole_weight,
        fractional_weight,
    );
    store_radius_one_fractional_sample(
        &mut destination[1],
        first + u32::from(source[1]) + u32::from(source[2]),
        first + u32::from(source[3]),
        whole_weight,
        fractional_weight,
    );

    let interior_destination = &mut destination[2..width - 2];
    let far_left_samples = &source[..width - 4];
    let left_samples = &source[1..width - 3];
    let center_samples = &source[2..width - 2];
    let right_samples = &source[3..width - 1];
    let far_right_samples = &source[4..];
    for (((((output, &far_left), &left), &center), &right), &far_right) in interior_destination
        .iter_mut()
        .zip(far_left_samples)
        .zip(left_samples)
        .zip(center_samples)
        .zip(right_samples)
        .zip(far_right_samples)
    {
        let whole_sum = u32::from(left) + u32::from(center) + u32::from(right);
        let far_sum = u32::from(far_left) + u32::from(far_right);
        store_radius_one_fractional_sample(
            output,
            whole_sum,
            far_sum,
            whole_weight,
            fractional_weight,
        );
    }

    let last = width - 1;
    let penultimate = last - 1;
    store_radius_one_fractional_sample(
        &mut destination[penultimate],
        u32::from(source[penultimate - 1])
            + u32::from(source[penultimate])
            + u32::from(source[last]),
        u32::from(source[penultimate - 2]) + u32::from(source[last]),
        whole_weight,
        fractional_weight,
    );
    store_radius_one_fractional_sample(
        &mut destination[last],
        u32::from(source[last - 1]) + u32::from(source[last]) * 2,
        u32::from(source[last - 2]) + u32::from(source[last]),
        whole_weight,
        fractional_weight,
    );
}

/// Blur one RGB row for a radius-one fractional Gaussian box pass.
#[cfg(not(feature = "parallel"))]
fn blur_rgb_radius_one_fractional_line(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    debug_assert!(width > 0);
    debug_assert!(source.len() >= width * 3);
    debug_assert!(destination.len() >= width * 3);
    debug_assert_ne!(fractional_weight, 0);

    if width < 5 {
        let mut accumulator = [0u32; 3];
        blur_line(
            source,
            destination,
            0,
            width,
            3,
            1,
            whole_weight,
            fractional_weight,
            &mut accumulator,
        );
        return;
    }

    for channel in 0..3 {
        let first = u32::from(source[channel]);
        store_radius_one_fractional_sample(
            &mut destination[channel],
            first * 2 + u32::from(source[3 + channel]),
            first + u32::from(source[6 + channel]),
            whole_weight,
            fractional_weight,
        );
        store_radius_one_fractional_sample(
            &mut destination[3 + channel],
            first + u32::from(source[3 + channel]) + u32::from(source[6 + channel]),
            first + u32::from(source[9 + channel]),
            whole_weight,
            fractional_weight,
        );
    }

    let interior_destination = &mut destination[6..(width - 2) * 3];
    let far_left_samples = &source[..(width - 4) * 3];
    let left_samples = &source[3..(width - 3) * 3];
    let center_samples = &source[6..(width - 2) * 3];
    let right_samples = &source[9..(width - 1) * 3];
    let far_right_samples = &source[12..];
    for (((((output, &far_left), &left), &center), &right), &far_right) in interior_destination
        .iter_mut()
        .zip(far_left_samples)
        .zip(left_samples)
        .zip(center_samples)
        .zip(right_samples)
        .zip(far_right_samples)
    {
        let whole_sum = u32::from(left) + u32::from(center) + u32::from(right);
        let far_sum = u32::from(far_left) + u32::from(far_right);
        store_radius_one_fractional_sample(
            output,
            whole_sum,
            far_sum,
            whole_weight,
            fractional_weight,
        );
    }

    let penultimate = (width - 2) * 3;
    let last = (width - 1) * 3;
    for channel in 0..3 {
        store_radius_one_fractional_sample(
            &mut destination[penultimate + channel],
            u32::from(source[penultimate - 3 + channel])
                + u32::from(source[penultimate + channel])
                + u32::from(source[last + channel]),
            u32::from(source[penultimate - 6 + channel]) + u32::from(source[last + channel]),
            whole_weight,
            fractional_weight,
        );
        store_radius_one_fractional_sample(
            &mut destination[last + channel],
            u32::from(source[penultimate + channel]) + u32::from(source[last + channel]) * 2,
            u32::from(source[penultimate - 3 + channel]) + u32::from(source[last + channel]),
            whole_weight,
            fractional_weight,
        );
    }
}

/// Blur one RGBA row for a radius-one fractional Gaussian box pass.
#[cfg(not(feature = "parallel"))]
fn blur_rgba_radius_one_fractional_line(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    debug_assert!(width > 0);
    debug_assert!(source.len() >= width * 4);
    debug_assert!(destination.len() >= width * 4);
    debug_assert_ne!(fractional_weight, 0);

    if width < 5 {
        let mut accumulator = [0u32; 4];
        blur_line(
            source,
            destination,
            0,
            width,
            4,
            1,
            whole_weight,
            fractional_weight,
            &mut accumulator,
        );
        return;
    }

    for channel in 0..4 {
        let first = u32::from(source[channel]);
        store_radius_one_fractional_sample(
            &mut destination[channel],
            first * 2 + u32::from(source[4 + channel]),
            first + u32::from(source[8 + channel]),
            whole_weight,
            fractional_weight,
        );
        store_radius_one_fractional_sample(
            &mut destination[4 + channel],
            first + u32::from(source[4 + channel]) + u32::from(source[8 + channel]),
            first + u32::from(source[12 + channel]),
            whole_weight,
            fractional_weight,
        );
    }

    let interior_destination = &mut destination[8..(width - 2) * 4];
    let far_left_samples = &source[..(width - 4) * 4];
    let left_samples = &source[4..(width - 3) * 4];
    let center_samples = &source[8..(width - 2) * 4];
    let right_samples = &source[12..(width - 1) * 4];
    let far_right_samples = &source[16..];
    for (((((output, far_left), left), center), right), far_right) in interior_destination
        .chunks_exact_mut(4)
        .zip(far_left_samples.chunks_exact(4))
        .zip(left_samples.chunks_exact(4))
        .zip(center_samples.chunks_exact(4))
        .zip(right_samples.chunks_exact(4))
        .zip(far_right_samples.chunks_exact(4))
    {
        for channel in 0..4 {
            let whole_sum =
                u32::from(left[channel]) + u32::from(center[channel]) + u32::from(right[channel]);
            let far_sum = u32::from(far_left[channel]) + u32::from(far_right[channel]);
            store_radius_one_fractional_sample(
                &mut output[channel],
                whole_sum,
                far_sum,
                whole_weight,
                fractional_weight,
            );
        }
    }

    let penultimate = (width - 2) * 4;
    let last = (width - 1) * 4;
    for channel in 0..4 {
        store_radius_one_fractional_sample(
            &mut destination[penultimate + channel],
            u32::from(source[penultimate - 4 + channel])
                + u32::from(source[penultimate + channel])
                + u32::from(source[last + channel]),
            u32::from(source[penultimate - 8 + channel]) + u32::from(source[last + channel]),
            whole_weight,
            fractional_weight,
        );
        store_radius_one_fractional_sample(
            &mut destination[last + channel],
            u32::from(source[penultimate + channel]) + u32::from(source[last + channel]) * 2,
            u32::from(source[penultimate - 4 + channel]) + u32::from(source[last + channel]),
            whole_weight,
            fractional_weight,
        );
    }
}

/// Run the three horizontal Gaussian box passes one interleaved row at a time.
///
/// Horizontal filtering never reads another image row, so the intermediate
/// byte rounding can stay in two reusable row buffers instead of two extra
/// full-frame buffers. The same per-pass `blur_line` operation preserves the
/// order and byte result of three image-wide horizontal passes.
#[cfg(not(feature = "parallel"))]
fn blur_interleaved_rows_three_passes<const CHANNELS: usize>(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let row_length = width * CHANNELS;
    let mut first_intermediate = vec![0u8; row_length];
    let mut second_intermediate = vec![0u8; row_length];
    let mut accumulator = [0u32; 4];
    let rgb_radius_one_fractional =
        CHANNELS == 3 && radius == 1 && fractional_weight != 0 && width >= 5;
    let rgba_radius_one_fractional =
        CHANNELS == 4 && radius == 1 && fractional_weight != 0 && width >= 5;

    for row in 0..height {
        let start = row * row_length;
        let source_row = &source[start..start + row_length];
        let destination_row = &mut destination[start..start + row_length];
        if rgb_radius_one_fractional {
            blur_rgb_radius_one_fractional_line(
                source_row,
                &mut first_intermediate,
                width,
                whole_weight,
                fractional_weight,
            );
            blur_rgb_radius_one_fractional_line(
                &first_intermediate,
                &mut second_intermediate,
                width,
                whole_weight,
                fractional_weight,
            );
            blur_rgb_radius_one_fractional_line(
                &second_intermediate,
                destination_row,
                width,
                whole_weight,
                fractional_weight,
            );
        } else if rgba_radius_one_fractional {
            blur_rgba_radius_one_fractional_line(
                source_row,
                &mut first_intermediate,
                width,
                whole_weight,
                fractional_weight,
            );
            blur_rgba_radius_one_fractional_line(
                &first_intermediate,
                &mut second_intermediate,
                width,
                whole_weight,
                fractional_weight,
            );
            blur_rgba_radius_one_fractional_line(
                &second_intermediate,
                destination_row,
                width,
                whole_weight,
                fractional_weight,
            );
        } else {
            blur_line(
                source_row,
                &mut first_intermediate,
                0,
                width,
                CHANNELS,
                radius,
                whole_weight,
                fractional_weight,
                &mut accumulator,
            );
            blur_line(
                &first_intermediate,
                &mut second_intermediate,
                0,
                width,
                CHANNELS,
                radius,
                whole_weight,
                fractional_weight,
                &mut accumulator,
            );
            blur_line(
                &second_intermediate,
                destination_row,
                0,
                width,
                CHANNELS,
                radius,
                whole_weight,
                fractional_weight,
                &mut accumulator,
            );
        }
    }
}

/// Apply one fractional radius-one Gaussian box pass to an LA row. The three
/// regions keep the replicated edge indices out of the interior loop.
#[cfg(not(feature = "parallel"))]
fn blur_la_radius_one_fractional_line(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    debug_assert!(width >= 5);
    debug_assert!(source.len() >= width * 2);
    debug_assert!(destination.len() >= width * 2);
    debug_assert_ne!(fractional_weight, 0);

    for channel in 0..2 {
        let first = u32::from(source[channel]);
        store_radius_one_fractional_sample(
            &mut destination[channel],
            first * 2 + u32::from(source[2 + channel]),
            first + u32::from(source[4 + channel]),
            whole_weight,
            fractional_weight,
        );
        store_radius_one_fractional_sample(
            &mut destination[2 + channel],
            first + u32::from(source[2 + channel]) + u32::from(source[4 + channel]),
            first + u32::from(source[6 + channel]),
            whole_weight,
            fractional_weight,
        );
    }

    let interior_end = (width - 2) * 2;
    let interior_destination = &mut destination[4..interior_end];
    let far_left_samples = &source[..(width - 4) * 2];
    let left_samples = &source[2..(width - 3) * 2];
    let center_samples = &source[4..interior_end];
    let right_samples = &source[6..(width - 1) * 2];
    let far_right_samples = &source[8..];
    for (((((output, &far_left), &left), &center), &right), &far_right) in interior_destination
        .iter_mut()
        .zip(far_left_samples)
        .zip(left_samples)
        .zip(center_samples)
        .zip(right_samples)
        .zip(far_right_samples)
    {
        let whole_sum = u32::from(left) + u32::from(center) + u32::from(right);
        let far_sum = u32::from(far_left) + u32::from(far_right);
        store_radius_one_fractional_sample(
            output,
            whole_sum,
            far_sum,
            whole_weight,
            fractional_weight,
        );
    }

    let last = width - 1;
    let penultimate = last - 1;
    for channel in 0..2 {
        let penultimate_base = penultimate * 2 + channel;
        let last_base = last * 2 + channel;
        store_radius_one_fractional_sample(
            &mut destination[penultimate_base],
            u32::from(source[penultimate_base - 2])
                + u32::from(source[penultimate_base])
                + u32::from(source[penultimate_base + 2]),
            u32::from(source[penultimate_base - 4]) + u32::from(source[penultimate_base + 2]),
            whole_weight,
            fractional_weight,
        );
        store_radius_one_fractional_sample(
            &mut destination[last_base],
            u32::from(source[last_base - 2]) + u32::from(source[last_base]) * 2,
            u32::from(source[last_base - 4]) + u32::from(source[last_base]),
            whole_weight,
            fractional_weight,
        );
    }
}

/// Keep the exact byte rounding between the three horizontal passes while
/// specializing the common LA GaussianBlur(radius=2) box radius.
#[cfg(not(feature = "parallel"))]
fn blur_la_rows_three_passes_radius_one(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let row_length = width * 2;
    let mut first_intermediate = vec![0u8; row_length];
    let mut second_intermediate = vec![0u8; row_length];

    for row in 0..height {
        let start = row * row_length;
        let source_row = &source[start..start + row_length];
        let destination_row = &mut destination[start..start + row_length];
        blur_la_radius_one_fractional_line(
            source_row,
            &mut first_intermediate,
            width,
            whole_weight,
            fractional_weight,
        );
        blur_la_radius_one_fractional_line(
            &first_intermediate,
            &mut second_intermediate,
            width,
            whole_weight,
            fractional_weight,
        );
        blur_la_radius_one_fractional_line(
            &second_intermediate,
            destination_row,
            width,
            whole_weight,
            fractional_weight,
        );
    }
}

/// Run the three horizontal Gaussian box passes one L row at a time.
///
/// Each row is independent for a horizontal pass, so the exact intermediate
/// byte rounds can stay in two row buffers. This avoids writing and rereading
/// two full-frame horizontal intermediates while retaining Pillow's pass
/// order.
#[cfg(not(feature = "parallel"))]
fn blur_luma_rows_three_passes(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let mut first_intermediate = vec![0u8; width];
    let mut second_intermediate = vec![0u8; width];
    let mut accumulator = [0u32; 4];
    let radius_one_fractional = radius == 1 && fractional_weight != 0 && width >= 5;

    for row in 0..height {
        let start = row * width;
        let source_row = &source[start..start + width];
        let destination_row = &mut destination[start..start + width];
        if radius_one_fractional {
            blur_luma_radius_one_fractional_line(
                source_row,
                &mut first_intermediate,
                width,
                whole_weight,
                fractional_weight,
            );
            blur_luma_radius_one_fractional_line(
                &first_intermediate,
                &mut second_intermediate,
                width,
                whole_weight,
                fractional_weight,
            );
            blur_luma_radius_one_fractional_line(
                &second_intermediate,
                destination_row,
                width,
                whole_weight,
                fractional_weight,
            );
        } else {
            blur_line(
                source_row,
                &mut first_intermediate,
                0,
                width,
                1,
                radius,
                whole_weight,
                fractional_weight,
                &mut accumulator,
            );
            blur_line(
                &first_intermediate,
                &mut second_intermediate,
                0,
                width,
                1,
                radius,
                whole_weight,
                fractional_weight,
                &mut accumulator,
            );
            blur_line(
                &second_intermediate,
                destination_row,
                0,
                width,
                1,
                radius,
                whole_weight,
                fractional_weight,
                &mut accumulator,
            );
        }
    }
}

/// Apply one L vertical Gaussian box pass with five replicated-edge taps.
/// The direct row-major form keeps every source row contiguous and avoids the
/// generic transposed-line accumulator for this common radius.
#[cfg(not(feature = "parallel"))]
fn blur_luma_vertical_radius_one_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    debug_assert_eq!(source.len(), width * height);
    debug_assert_eq!(destination.len(), source.len());
    if width == 0 || height == 0 {
        return;
    }

    let last_y = height - 1;
    for y in 0..height {
        let top_two = y.saturating_sub(2) * width;
        let top_one = y.saturating_sub(1) * width;
        let center = y * width;
        let bottom_one = y.saturating_add(1).min(last_y) * width;
        let bottom_two = y.saturating_add(2).min(last_y) * width;
        let top_two_row = &source[top_two..top_two + width];
        let top_one_row = &source[top_one..top_one + width];
        let center_row = &source[center..center + width];
        let bottom_one_row = &source[bottom_one..bottom_one + width];
        let bottom_two_row = &source[bottom_two..bottom_two + width];
        let destination_row = &mut destination[center..center + width];

        if fractional_weight == 0 {
            for (((output, &top), &middle), &bottom) in destination_row
                .iter_mut()
                .zip(top_one_row)
                .zip(center_row)
                .zip(bottom_one_row)
            {
                let neighbors = u32::from(top) + u32::from(middle) + u32::from(bottom);
                let weighted = neighbors
                    .wrapping_mul(whole_weight)
                    .wrapping_add(BOX_BLUR_BIAS);
                *output = (weighted >> 24) as u8;
            }
        } else {
            for (((((output, &far_top), &top), &middle), &bottom), &far_bottom) in destination_row
                .iter_mut()
                .zip(top_two_row)
                .zip(top_one_row)
                .zip(center_row)
                .zip(bottom_one_row)
                .zip(bottom_two_row)
            {
                let neighbors = u32::from(top) + u32::from(middle) + u32::from(bottom);
                let far_edges = u32::from(far_top) + u32::from(far_bottom);
                let weighted = neighbors
                    .wrapping_mul(whole_weight)
                    .wrapping_add(far_edges.wrapping_mul(fractional_weight))
                    .wrapping_add(BOX_BLUR_BIAS);
                *output = (weighted >> 24) as u8;
            }
        }
    }
}

/// Apply one interleaved vertical Gaussian box pass with five replicated-edge taps.
#[cfg(not(feature = "parallel"))]
fn blur_interleaved_vertical_radius_one_fractional_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let row_stride = width * channels;
    debug_assert_eq!(source.len(), row_stride * height);
    debug_assert_eq!(destination.len(), source.len());
    debug_assert_ne!(fractional_weight, 0);
    if width == 0 || height == 0 {
        return;
    }

    let last_y = height - 1;
    for y in 0..height {
        let far_top = y.saturating_sub(2) * row_stride;
        let top = y.saturating_sub(1) * row_stride;
        let center = y * row_stride;
        let bottom = y.saturating_add(1).min(last_y) * row_stride;
        let far_bottom = y.saturating_add(2).min(last_y) * row_stride;
        let far_top_row = &source[far_top..far_top + row_stride];
        let top_row = &source[top..top + row_stride];
        let center_row = &source[center..center + row_stride];
        let bottom_row = &source[bottom..bottom + row_stride];
        let far_bottom_row = &source[far_bottom..far_bottom + row_stride];
        let destination_row = &mut destination[center..center + row_stride];

        for (((((output, &far_top), &top), &center), &bottom), &far_bottom) in destination_row
            .iter_mut()
            .zip(far_top_row)
            .zip(top_row)
            .zip(center_row)
            .zip(bottom_row)
            .zip(far_bottom_row)
        {
            let whole = u32::from(top) + u32::from(center) + u32::from(bottom);
            let fractional = u32::from(far_top) + u32::from(far_bottom);
            let weighted = whole
                .wrapping_mul(whole_weight)
                .wrapping_add(fractional.wrapping_mul(fractional_weight))
                .wrapping_add(BOX_BLUR_BIAS);
            *output = (weighted >> 24) as u8;
        }
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

#[cfg(feature = "parallel")]
fn blur_columns_radius_one_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    whole_weight: u32,
) {
    let row_stride = width * channels;
    crate::par_rows_mut!(
        destination,
        row_stride,
        height,
        |_row_start, _row_end, y, row| {
            let y = y as usize;
            let above = y.saturating_sub(1) * row_stride;
            let center = y * row_stride;
            let below = y.saturating_add(1).min(height - 1) * row_stride;
            for (index, output) in row.iter_mut().enumerate() {
                let sum = u32::from(source[above + index])
                    + u32::from(source[center + index])
                    + u32::from(source[below + index]);
                *output = (sum.wrapping_mul(whole_weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
            }
        }
    );
}

#[cfg(all(test, feature = "parallel"))]
mod parallel_radius_one_vertical_tests {
    use super::{blur_columns, blur_columns_radius_one_rows, blur_parameters};

    #[test]
    fn row_parallel_radius_one_matches_fixed_point_column_blur() {
        let (radius, whole_weight, fractional_weight) = blur_parameters(1.0);
        assert_eq!(radius, 1);
        assert_eq!(fractional_weight, 0);

        for (width, height) in [(1, 1), (1, 7), (2, 3), (17, 9), (65, 33)] {
            for channels in [1, 2, 3, 4] {
                let len = width * height * channels;
                let source = (0..len)
                    .map(|index| ((index * 73 + index / 11 * 29 + 37) % 256) as u8)
                    .collect::<Vec<_>>();
                let mut actual = vec![0; len];
                let mut expected = vec![0; len];

                blur_columns_radius_one_rows(
                    &source,
                    &mut actual,
                    width,
                    height,
                    channels,
                    whole_weight,
                );
                blur_columns(
                    &source,
                    &mut expected,
                    width,
                    height,
                    channels,
                    radius,
                    whole_weight,
                    fractional_weight,
                    &mut vec![0; width * channels],
                );

                assert_eq!(actual, expected, "{width}x{height}, channels={channels}");
            }
        }
    }
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

/// Apply a single native-byte BoxBlur(1) pass with a three-row horizontal
/// scratch ring. This keeps the horizontal intermediate in cache and writes
/// each final row directly, avoiding a full-frame source clone and vertical
/// scratch buffer for the common one-pass case.
#[cfg(not(feature = "parallel"))]
#[inline]
fn blur_box_radius_one_horizontal_row(
    source: &[u8],
    ring: &mut [u8],
    row: usize,
    row_bytes: usize,
    width: usize,
    channels: usize,
    horizontal_weight: u32,
) {
    if channels == 1 {
        let source_start = row * row_bytes;
        blur_box_radius_one_horizontal_l_row(
            &source[source_start..source_start + row_bytes],
            ring,
            width,
            horizontal_weight,
        );
        return;
    }
    if channels == 3 {
        let source_start = row * row_bytes;
        blur_box_radius_one_horizontal_rgb_row(
            &source[source_start..source_start + row_bytes],
            ring,
            width,
            horizontal_weight,
        );
        return;
    }
    blur_one_row(
        source,
        ring,
        row * row_bytes,
        width,
        channels,
        1,
        horizontal_weight,
        0,
    );
}

#[cfg(not(feature = "parallel"))]
#[inline]
fn blur_box_radius_one_horizontal_l_row(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    weight: u32,
) {
    debug_assert!(width > 0);
    debug_assert_eq!(source.len(), width);
    debug_assert_eq!(destination.len(), width);

    let last = width - 1;
    let mut sum = u32::from(source[0]) * 2 + u32::from(source[usize::from(width > 1)]);
    destination[0] = (sum.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
    if width == 1 {
        return;
    }

    sum = sum
        .wrapping_sub(u32::from(source[0]))
        .wrapping_add(u32::from(source[2.min(last)]));
    destination[1] = (sum.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
    if width == 2 {
        return;
    }

    for center in 2..last {
        sum = sum
            .wrapping_sub(u32::from(source[center - 2]))
            .wrapping_add(u32::from(source[center + 1]));
        destination[center] = (sum.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
    }

    sum = sum
        .wrapping_sub(u32::from(source[width - 3]))
        .wrapping_add(u32::from(source[last]));
    destination[last] = (sum.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
}

#[cfg(not(feature = "parallel"))]
#[inline]
fn blur_box_radius_one_horizontal_rgb_row(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    weight: u32,
) {
    debug_assert!(width > 0);
    debug_assert_eq!(source.len(), width * 3);
    debug_assert_eq!(destination.len(), width * 3);

    let last = width - 1;
    let last_base = last * 3;
    let first_right = usize::from(width > 1) * 3;
    let mut red = u32::from(source[0]) * 2 + u32::from(source[first_right]);
    let mut green = u32::from(source[1]) * 2 + u32::from(source[first_right + 1]);
    let mut blue = u32::from(source[2]) * 2 + u32::from(source[first_right + 2]);

    destination[0] = (red.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
    destination[1] = (green.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
    destination[2] = (blue.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
    if width == 1 {
        return;
    }

    let first_entering = (2.min(last)) * 3;
    red = red
        .wrapping_sub(u32::from(source[0]))
        .wrapping_add(u32::from(source[first_entering]));
    green = green
        .wrapping_sub(u32::from(source[1]))
        .wrapping_add(u32::from(source[first_entering + 1]));
    blue = blue
        .wrapping_sub(u32::from(source[2]))
        .wrapping_add(u32::from(source[first_entering + 2]));

    if width > 2 {
        for center in 1..width - 2 {
            let output_base = center * 3;
            destination[output_base] =
                (red.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
            destination[output_base + 1] =
                (green.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
            destination[output_base + 2] =
                (blue.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;

            let leaving_base = (center - 1) * 3;
            let entering_base = (center + 2) * 3;
            red = red
                .wrapping_sub(u32::from(source[leaving_base]))
                .wrapping_add(u32::from(source[entering_base]));
            green = green
                .wrapping_sub(u32::from(source[leaving_base + 1]))
                .wrapping_add(u32::from(source[entering_base + 1]));
            blue = blue
                .wrapping_sub(u32::from(source[leaving_base + 2]))
                .wrapping_add(u32::from(source[entering_base + 2]));
        }

        let penultimate = width - 2;
        let output_base = penultimate * 3;
        destination[output_base] =
            (red.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
        destination[output_base + 1] =
            (green.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
        destination[output_base + 2] =
            (blue.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;

        let leaving_base = (width - 3) * 3;
        red = red
            .wrapping_sub(u32::from(source[leaving_base]))
            .wrapping_add(u32::from(source[last_base]));
        green = green
            .wrapping_sub(u32::from(source[leaving_base + 1]))
            .wrapping_add(u32::from(source[last_base + 1]));
        blue = blue
            .wrapping_sub(u32::from(source[leaving_base + 2]))
            .wrapping_add(u32::from(source[last_base + 2]));
    }

    destination[last_base] = (red.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
    destination[last_base + 1] =
        (green.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
    destination[last_base + 2] =
        (blue.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
}

#[cfg(not(feature = "parallel"))]
fn blur_box_radius_one_row_ring(
    source: &[u8],
    width: u32,
    height: u32,
    channels: usize,
    horizontal_weight: u32,
    vertical_weight: u32,
) -> Result<Vec<u8>, PilError> {
    let dimensions = CheckedDims::new(width, height, channels as u8)?;
    if source.len() != dimensions.total_bytes() {
        return Err(PilError::DimensionError(
            "BoxBlur source byte length does not match its dimensions".into(),
        ));
    }
    let ring_height = dimensions.height.min(3);
    let ring_dimensions = CheckedDims::new(dimensions.width, ring_height, channels as u8)?;
    let row_bytes = dimensions.row_stride();
    let width = dimensions.width as usize;
    let height = dimensions.height as usize;
    let ring_slots = ring_height as usize;
    let mut output = dimensions.alloc_buffer();
    let mut ring = ring_dimensions.alloc_buffer();

    for row in 0..height.min(2) {
        let start = (row % ring_slots) * row_bytes;
        blur_box_radius_one_horizontal_row(
            source,
            &mut ring[start..start + row_bytes],
            row,
            row_bytes,
            width,
            channels,
            horizontal_weight,
        );
    }

    for y in 0..height {
        let center_slot = y % ring_slots;
        let bottom_y = (y + 1).min(height - 1);
        let bottom_slot = bottom_y % ring_slots;
        let center_start = center_slot * row_bytes;
        let top_y = y.saturating_sub(1);
        let top_start = (top_y % ring_slots) * row_bytes;
        let output_start = y * row_bytes;
        let output_row = &mut output[output_start..output_start + row_bytes];
        let top_row = &ring[top_start..top_start + row_bytes];
        let center_row = &ring[center_start..center_start + row_bytes];
        let bottom_start = bottom_slot * row_bytes;
        let bottom_row = &ring[bottom_start..bottom_start + row_bytes];
        for ((output, &top), (&center, &bottom)) in output_row
            .iter_mut()
            .zip(top_row)
            .zip(center_row.iter().zip(bottom_row))
        {
            let sum = u32::from(top) + u32::from(center) + u32::from(bottom);
            *output = (sum
                .wrapping_mul(vertical_weight)
                .wrapping_add(BOX_BLUR_BIAS)
                >> 24) as u8;
        }

        let next_row = y + 2;
        if next_row < height {
            let next_start = (next_row % ring_slots) * row_bytes;
            blur_box_radius_one_horizontal_row(
                source,
                &mut ring[next_start..next_start + row_bytes],
                next_row,
                row_bytes,
                width,
                channels,
                horizontal_weight,
            );
        }
    }

    Ok(output)
}

#[cfg(all(test, not(feature = "parallel")))]
mod serial_box_blur_radius_one_row_ring_tests {
    use super::{
        CheckedDims, blur_box_radius_one_row_ring, blur_columns, blur_parameters, blur_rows,
    };

    #[test]
    fn row_ring_matches_fixed_point_two_passes_at_edges_and_tails() {
        let (radius, weight, fractional_weight) = blur_parameters(1.0);
        assert_eq!((radius, fractional_weight), (1, 0));

        for (width, height) in [
            (1, 1),
            (1, 7),
            (2, 3),
            (3, 2),
            (4, 5),
            (5, 9),
            (17, 9),
            (65, 33),
        ] {
            for channels in [1, 2, 3, 4] {
                let dimensions = CheckedDims::new(width, height, channels).unwrap();
                let source = (0..dimensions.total_bytes())
                    .map(|index| ((index * 73 + index / 11 * 29 + 37) % 256) as u8)
                    .collect::<Vec<_>>();
                let actual = blur_box_radius_one_row_ring(
                    &source,
                    width,
                    height,
                    channels as usize,
                    weight,
                    weight,
                )
                .unwrap();
                let mut horizontal = dimensions.alloc_buffer();
                let mut expected = dimensions.alloc_buffer();
                blur_rows(
                    &source,
                    &mut horizontal,
                    width as usize,
                    height as usize,
                    channels as usize,
                    radius,
                    weight,
                    fractional_weight,
                );
                blur_columns(
                    &horizontal,
                    &mut expected,
                    width as usize,
                    height as usize,
                    channels as usize,
                    radius,
                    weight,
                    fractional_weight,
                    &mut vec![0; dimensions.row_stride()],
                );

                assert_eq!(actual, expected, "{width}x{height}, channels={channels}");
            }
        }
    }
}

#[inline]
fn blur_parameters(radius: f32) -> (usize, u32, u32) {
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

    // ImagingBoxBlur's replicated-edge average is identity for a uniform
    // image when its rounded fixed-point result preserves every channel byte.
    // Keep this small-image check before allocating the work buffers.
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

    if passes == 0 {
        return Ok(img.clone());
    }

    let (horizontal_radius, horizontal_weight, horizontal_fractional_weight) =
        blur_parameters(radius_x);
    let (vertical_radius, vertical_weight, vertical_fractional_weight) = blur_parameters(radius_y);

    // A full uniformity scan is worthwhile for the large RGBA Gaussian path
    // only when its exact fixed-point transform is an identity for the input
    // tuple. In particular, fractional weights may sum to 2^24 - 1; checking
    // the actual rounded result avoids assuming that they sum exactly to 1.
    #[cfg(not(feature = "parallel"))]
    if passes == 3
        && width.saturating_mul(height) > UNIFORM_BOX_BLUR_MAX_PIXELS
        && matches!(img, DynamicImage::ImageRgba8(_))
        && uniform_rgba_box_blur_is_identity(
            img.as_bytes(),
            width,
            height,
            (
                horizontal_radius,
                horizontal_weight,
                horizontal_fractional_weight,
            ),
            (vertical_radius, vertical_weight, vertical_fractional_weight),
        )
    {
        return Ok(img.clone());
    }

    #[cfg(not(feature = "parallel"))]
    if passes == 1
        && horizontal_radius == 1
        && vertical_radius == 1
        && horizontal_fractional_weight == 0
        && vertical_fractional_weight == 0
        && matches!(
            img,
            DynamicImage::ImageLuma8(_)
                | DynamicImage::ImageLumaA8(_)
                | DynamicImage::ImageRgb8(_)
                | DynamicImage::ImageRgba8(_)
        )
    {
        let output = blur_box_radius_one_row_ring(
            img.as_bytes(),
            w_u32,
            h_u32,
            channels,
            horizontal_weight,
            vertical_weight,
        )?;
        let result = raw_bytes_to_image(w_u32, h_u32, output, channels)?;
        return Ok(preserve_mode(img, result));
    }

    #[cfg(not(feature = "parallel"))]
    let rgb_gaussian_row_fusion = passes == 3 && matches!(img, DynamicImage::ImageRgb8(_));
    #[cfg(feature = "parallel")]
    let rgb_gaussian_row_fusion = false;
    #[cfg(not(feature = "parallel"))]
    let luma_gaussian_row_fusion = passes == 3 && matches!(img, DynamicImage::ImageLuma8(_));
    #[cfg(feature = "parallel")]
    let luma_gaussian_row_fusion = false;
    #[cfg(not(feature = "parallel"))]
    let la_gaussian_row_fusion = passes == 3 && matches!(img, DynamicImage::ImageLumaA8(_));
    #[cfg(feature = "parallel")]
    let la_gaussian_row_fusion = false;
    #[cfg(not(feature = "parallel"))]
    let rgba_gaussian_row_fusion = passes == 3 && matches!(img, DynamicImage::ImageRgba8(_));
    #[cfg(feature = "parallel")]
    let rgba_gaussian_row_fusion = false;

    let mut work = if rgb_gaussian_row_fusion
        || luma_gaussian_row_fusion
        || la_gaussian_row_fusion
        || rgba_gaussian_row_fusion
    {
        CheckedDims::new(w_u32, h_u32, channels as u8)?.alloc_buffer()
    } else {
        img.as_bytes().to_vec()
    };
    let mut scratch = CheckedDims::new(w_u32, h_u32, channels as u8)?.alloc_buffer();

    // PIL does ALL horizontal passes first (matching ImagingBoxBlur order)
    if rgb_gaussian_row_fusion {
        #[cfg(not(feature = "parallel"))]
        blur_interleaved_rows_three_passes::<3>(
            img.as_bytes(),
            &mut work,
            width,
            height,
            horizontal_radius,
            horizontal_weight,
            horizontal_fractional_weight,
        );
    } else if luma_gaussian_row_fusion {
        #[cfg(not(feature = "parallel"))]
        blur_luma_rows_three_passes(
            img.as_bytes(),
            &mut work,
            width,
            height,
            horizontal_radius,
            horizontal_weight,
            horizontal_fractional_weight,
        );
    } else if la_gaussian_row_fusion {
        #[cfg(not(feature = "parallel"))]
        if horizontal_radius == 1 && horizontal_fractional_weight != 0 && width >= 5 {
            blur_la_rows_three_passes_radius_one(
                img.as_bytes(),
                &mut work,
                width,
                height,
                horizontal_weight,
                horizontal_fractional_weight,
            );
        } else {
            blur_interleaved_rows_three_passes::<2>(
                img.as_bytes(),
                &mut work,
                width,
                height,
                horizontal_radius,
                horizontal_weight,
                horizontal_fractional_weight,
            );
        }
    } else if rgba_gaussian_row_fusion {
        #[cfg(not(feature = "parallel"))]
        blur_interleaved_rows_three_passes::<4>(
            img.as_bytes(),
            &mut work,
            width,
            height,
            horizontal_radius,
            horizontal_weight,
            horizontal_fractional_weight,
        );
    } else {
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
    }

    // Pillow transposes before its vertical passes. For large parallel jobs,
    // keep that layout explicit: one transpose makes every original column a
    // writable row, all vertical passes can then reuse the row-parallel blur,
    // and one final transpose restores the public row-major layout. The
    // serial/small-image path keeps the algebraically equivalent wide-row
    // recurrence and avoids paying for the extra data movement.
    #[cfg(feature = "parallel")]
    let use_parallel_radius_one_vertical = width.saturating_mul(height)
        >= VERTICAL_BLUR_TRANSPOSE_THRESHOLD
        && passes > 0
        && vertical_radius == 1
        && vertical_fractional_weight == 0;
    #[cfg(not(feature = "parallel"))]
    let use_parallel_radius_one_vertical = false;
    #[cfg(feature = "parallel")]
    let use_transposed_vertical = width.saturating_mul(height) >= VERTICAL_BLUR_TRANSPOSE_THRESHOLD
        && passes > 0
        && !use_parallel_radius_one_vertical;
    #[cfg(not(feature = "parallel"))]
    let use_transposed_vertical = false;

    if use_parallel_radius_one_vertical {
        #[cfg(feature = "parallel")]
        for _ in 0..passes {
            blur_columns_radius_one_rows(
                &work,
                &mut scratch,
                width,
                height,
                channels,
                vertical_weight,
            );
            std::mem::swap(&mut work, &mut scratch);
        }
    } else if use_transposed_vertical {
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
    } else if luma_gaussian_row_fusion && vertical_radius == 1 {
        #[cfg(not(feature = "parallel"))]
        for _ in 0..passes {
            blur_luma_vertical_radius_one_rows(
                &work,
                &mut scratch,
                width,
                height,
                vertical_weight,
                vertical_fractional_weight,
            );
            std::mem::swap(&mut work, &mut scratch);
        }
    } else if rgba_gaussian_row_fusion && vertical_radius == 1 && vertical_fractional_weight != 0 {
        #[cfg(not(feature = "parallel"))]
        for _ in 0..passes {
            blur_interleaved_vertical_radius_one_fractional_rows(
                &work,
                &mut scratch,
                width,
                height,
                channels,
                vertical_weight,
                vertical_fractional_weight,
            );
            std::mem::swap(&mut work, &mut scratch);
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
const SMALL_UNIFORM_BYTE_MAX_PIXELS: usize = 64 * 64;

/// Return whether a small native-byte image contains one repeated channel tuple.
///
/// A replicated-edge rank filter leaves a constant image unchanged.  Keep the
/// scan bounded so the fast path cannot add a full-frame pass to large or
/// non-uniform inputs.
#[inline]
fn native_small_uniform_byte_image(img: &DynamicImage, channels: usize, max_pixels: usize) -> bool {
    if channels == 0 || img.width() == 0 || img.height() == 0 {
        return false;
    }
    let Some(pixel_count) = (img.width() as usize).checked_mul(img.height() as usize) else {
        return false;
    };
    if pixel_count > max_pixels {
        return false;
    }
    let raw = img.as_bytes();
    let Some(first) = raw.get(..channels) else {
        return false;
    };
    raw.chunks_exact(channels).all(|pixel| pixel == first)
}

#[inline]
fn native_uniform_f_mode_image(raw: &[u8], width: usize, height: usize) -> bool {
    let Some(pixel_count) = width.checked_mul(height) else {
        return false;
    };
    let Some(byte_count) = pixel_count.checked_mul(4) else {
        return false;
    };
    if pixel_count == 0 || raw.len() != byte_count {
        return false;
    }
    let Some(first) = raw.get(..4) else {
        return false;
    };
    raw.chunks_exact(4).skip(1).all(|pixel| pixel == first)
}

fn rank_filter_f_requires_legacy_sort(raw: &[u8]) -> bool {
    let mut saw_negative_zero = false;
    let mut saw_positive_zero = false;
    for bytes in raw.chunks_exact(4) {
        let value = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if value.is_nan() {
            // The existing partial_cmp fallback treats every NaN as equal to
            // every value. Keep its exact sort behavior for these inputs.
            return true;
        }
        if value == 0.0 {
            saw_negative_zero |= value.is_sign_negative();
            saw_positive_zero |= !value.is_sign_negative();
            if saw_negative_zero && saw_positive_zero {
                // partial_cmp considers the two zero signs equal, though
                // their output bytes differ.
                return true;
            }
        }
    }
    false
}

#[inline]
fn select_rank_filter_f_value(values: &mut [f32], rank: usize, legacy_sort: bool) -> f32 {
    let compare = |a: &f32, b: &f32| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal);
    if legacy_sort {
        values.sort_unstable_by(compare);
        values[rank]
    } else {
        *values.select_nth_unstable_by(rank, compare).1
    }
}

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

/// Apply a 3x3 byte rank filter without constructing the maximum-sized
/// 7x7 scratch array for every output pixel. The clamped coordinates preserve
/// Pillow's replicated-edge window, and only the requested order statistic is
/// selected from the nine samples per channel.
#[inline(always)]
fn compare_swap_median9(values: &mut [u8; 9], left: usize, right: usize) {
    let a = values[left];
    let b = values[right];
    values[left] = a.min(b);
    values[right] = a.max(b);
}

#[inline(always)]
fn median9_u8(mut values: [u8; 9]) -> u8 {
    // A fixed 19-comparator selection network leaves the median at index 4.
    compare_swap_median9(&mut values, 1, 2);
    compare_swap_median9(&mut values, 4, 5);
    compare_swap_median9(&mut values, 7, 8);
    compare_swap_median9(&mut values, 0, 1);
    compare_swap_median9(&mut values, 3, 4);
    compare_swap_median9(&mut values, 6, 7);
    compare_swap_median9(&mut values, 0, 3);
    compare_swap_median9(&mut values, 1, 2);
    compare_swap_median9(&mut values, 4, 5);
    compare_swap_median9(&mut values, 7, 8);
    compare_swap_median9(&mut values, 3, 6);
    compare_swap_median9(&mut values, 4, 7);
    compare_swap_median9(&mut values, 5, 8);
    compare_swap_median9(&mut values, 1, 4);
    compare_swap_median9(&mut values, 2, 5);
    compare_swap_median9(&mut values, 4, 7);
    compare_swap_median9(&mut values, 4, 2);
    compare_swap_median9(&mut values, 6, 4);
    compare_swap_median9(&mut values, 4, 2);
    values[4]
}

fn rank_filter_bytes_3x3_row(
    raw: &[u8],
    row: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    rank: usize,
    y: usize,
) {
    let row_stride = width * channels;
    let top = y.saturating_sub(1) * row_stride;
    let middle = y * row_stride;
    let bottom = (y + 1).min(height - 1) * row_stride;

    for x in 0..width {
        let left = x.saturating_sub(1) * channels;
        let center = x * channels;
        let right = (x + 1).min(width - 1) * channels;
        let output = center;
        for channel in 0..channels {
            let mut values = [
                raw[top + left + channel],
                raw[top + center + channel],
                raw[top + right + channel],
                raw[middle + left + channel],
                raw[middle + center + channel],
                raw[middle + right + channel],
                raw[bottom + left + channel],
                raw[bottom + center + channel],
                raw[bottom + right + channel],
            ];
            row[output + channel] = if rank == 4 {
                median9_u8(values)
            } else if matches!(channels, 1 | 2) && rank == 1 {
                // Native L/LA rank 1 only needs the two smallest samples, not
                // a complete introselect partition. This min/max reduction
                // keeps duplicate minima exact and avoids data-dependent
                // selection.
                let mut smallest = values[0].min(values[1]);
                let mut second = values[0].max(values[1]);
                for value in values.into_iter().skip(2) {
                    let lower = smallest.min(value);
                    let upper = smallest.max(value);
                    second = second.min(upper);
                    smallest = lower;
                }
                second
            } else {
                let (_, selected, _) = values.select_nth_unstable(rank);
                *selected
            };
        }
    }
}

fn rank_filter_bytes_3x3(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    rank: usize,
) {
    let row_stride = width * channels;
    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(out, row_stride, height, |_row_start, _row_end, y, row| {
        rank_filter_bytes_3x3_row(raw, row, width, height, channels, rank, y as usize);
    });
    #[cfg(not(feature = "parallel"))]
    for y in 0..height {
        let row_start = y * row_stride;
        rank_filter_bytes_3x3_row(
            raw,
            &mut out[row_start..row_start + row_stride],
            width,
            height,
            channels,
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
    legacy_sort: bool,
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
            let selected = select_rank_filter_f_value(&mut values, rank, legacy_sort);
            let out_base = (y * w + x) as usize * 4;
            out[out_base..out_base + 4].copy_from_slice(&selected.to_le_bytes());
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
    legacy_sort: bool,
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
            let selected = select_rank_filter_f_value(&mut values, rank, legacy_sort);
            let out_base = x as usize * 4;
            row[out_base..out_base + 4].copy_from_slice(&selected.to_le_bytes());
        }
    });
}

/// Generic rank filter: sorts neighborhood values and picks the one at `rank`.
/// PIL uses clamping for border pixels.
/// Generalized to handle any number of channels (1-4).
/// For F-mode ("F"): treats each little-endian four-byte sample as one f32.
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

    // F-mode stores each scalar as one little-endian f32 in the four-byte
    // carrier. Borrow that storage directly and retain the accessor only for
    // mismatched internal variants.
    if mode == Some("F") {
        let raw: Cow<'_, [u8]> = match img {
            DynamicImage::ImageRgba8(rgba) => Cow::Borrowed(rgba.as_raw()),
            _ => Cow::Owned(img.to_rgba8().into_raw()),
        };
        let raw = raw.as_ref();
        if area > 1 && native_uniform_f_mode_image(raw, w_u32 as usize, h_u32 as usize) {
            let result = match img {
                DynamicImage::ImageRgba8(_) => img.clone(),
                _ => DynamicImage::ImageRgba8(
                    crate::raster::RgbaImage::from_raw(w_u32, h_u32, raw.to_vec()).ok_or_else(
                        || PilError::ValueError("rank_filter_impl(F): buffer error".into()),
                    )?,
                ),
            };
            return Ok(preserve_mode(img, result));
        }
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
            let legacy_sort = rank_filter_f_requires_legacy_sort(raw);
            #[cfg(feature = "parallel")]
            rank_filter_f_large_parallel(raw, &mut out, w, h, half, area, rank, legacy_sort);
            #[cfg(not(feature = "parallel"))]
            rank_filter_f_large_serial(raw, &mut out, w, h, half, area, rank, legacy_sort);
        }
        let result = DynamicImage::ImageRgba8(
            crate::raster::RgbaImage::from_raw(w_u32, h_u32, out)
                .ok_or_else(|| PilError::ValueError("rank_filter_impl(F): buffer error".into()))?,
        );
        return Ok(preserve_mode(img, result));
    }

    let channels = img.color().channel_count() as usize;
    let raw = img.as_bytes();

    let native_byte_layout = matches!(
        img,
        DynamicImage::ImageLuma8(_)
            | DynamicImage::ImageLumaA8(_)
            | DynamicImage::ImageRgb8(_)
            | DynamicImage::ImageRgba8(_)
    );
    if native_byte_layout
        && native_small_uniform_byte_image(img, channels, SMALL_UNIFORM_BYTE_MAX_PIXELS)
    {
        // Every rank of a uniform byte image is the same source tuple. The
        // bounded scan avoids gathering and selecting an identical window at
        // every output pixel, and cloning preserves the source's native mode.
        return Ok(img.clone());
    }
    let mut out = CheckedDims::new(w as u32, h as u32, channels as u8)?.alloc_buffer();
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
    } else if size == 3 {
        rank_filter_bytes_3x3(
            raw,
            &mut out,
            w_u32 as usize,
            h_u32 as usize,
            channels,
            rank,
        );
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
    offset: f32,
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
    #[cfg(all(
        not(feature = "parallel"),
        target_arch = "aarch64",
        target_feature = "neon"
    ))]
    if matches!(img, DynamicImage::ImageLumaA8(_))
        && w_u32 >= 16
        && h_u32 >= 3
        && binomial3x3_parameters_match(kernel, scale, offset)
    {
        let mut out = raw.to_vec();
        if let Some((vector_blocks, scalar_tail)) =
            crate::compute::pool_simd::ops::adapters::native_filter_3x3_la_binomial_rows(
                raw,
                &mut out,
                w_u32 as usize,
                h_u32 as usize,
            )
        {
            crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
            crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
            let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
            return Ok(preserve_mode(img, result));
        }
    }
    #[cfg(not(feature = "parallel"))]
    if matches!(img, DynamicImage::ImageLumaA8(_))
        && binomial3x3_parameters_match(kernel, scale, offset)
    {
        let mut out = raw.to_vec();
        if filter_3x3_binomial_la_rows(raw, &mut out, w_u32 as usize, h_u32 as usize) {
            let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
            return Ok(preserve_mode(img, result));
        }
    }
    // Normalize once, outside the pixel loop, with the same raw f32 divisor
    // that Pillow passes to `ImagingFilter`.
    let normalized_kernel: [f32; 9] = std::array::from_fn(|index| kernel[index] / scale);
    let rounding_bias = offset + 0.5;
    let mut out = raw.to_vec();
    let native_byte_layout = matches!(
        img,
        DynamicImage::ImageLuma8(_)
            | DynamicImage::ImageLumaA8(_)
            | DynamicImage::ImageRgb8(_)
            | DynamicImage::ImageRgba8(_)
    );
    if native_byte_layout
        && native_small_uniform_byte_image(img, channels, SMALL_UNIFORM_BYTE_MAX_PIXELS)
    {
        filter_3x3_constant_bytes(
            raw,
            &mut out,
            w_u32 as usize,
            h_u32 as usize,
            channels,
            &normalized_kernel,
            rounding_bias,
        );
        let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
        return Ok(preserve_mode(img, result));
    }
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
    offset: f32,
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
    let rounding_bias = offset + 0.5;
    #[cfg(not(feature = "parallel"))]
    if matches!(img, DynamicImage::ImageRgb8(_))
        && matches!(explicit_mode, None | Some("RGB"))
        && rgb_blur_ring::parameters_match(kernel, scale, offset)
        && let Some(out) = rgb_blur_ring::apply(raw, w_u32 as usize, h_u32 as usize)
    {
        let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
        return Ok(preserve_mode(img, result));
    }
    #[cfg(not(feature = "parallel"))]
    if matches!(img, DynamicImage::ImageRgba8(_))
        && matches!(explicit_mode, None | Some("RGBA"))
        && uniform5x5_parameters_match(kernel, scale, offset)
        && filter_5x5_uniform_rgba_identity(
            raw,
            w_u32 as usize,
            h_u32 as usize,
            &normalized_kernel,
            rounding_bias,
        )
    {
        let result = raw_bytes_to_image(w_u32, h_u32, raw.to_vec(), channels)?;
        return Ok(preserve_mode(img, result));
    }
    #[cfg(feature = "parallel")]
    if matches!(explicit_mode, None | Some("L"))
        && matches!(img, DynamicImage::ImageLuma8(_))
        && binomial5x5_parameters_match(kernel, scale, offset)
    {
        if let Some(out) =
            filter_5x5_binomial_luma_parallel_rows(raw, w_u32 as usize, h_u32 as usize)
        {
            let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
            return Ok(preserve_mode(img, result));
        }
    }
    #[cfg(feature = "parallel")]
    if matches!(explicit_mode, None | Some("LA"))
        && matches!(img, DynamicImage::ImageLumaA8(_))
        && binomial5x5_parameters_match(kernel, scale, offset)
    {
        if let Some(out) = filter_5x5_binomial_la_parallel_rows(raw, w_u32 as usize, h_u32 as usize)
        {
            let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
            return Ok(preserve_mode(img, result));
        }
    }
    let mut out = raw.to_vec();
    #[cfg(not(feature = "parallel"))]
    if matches!(explicit_mode, None | Some("LA"))
        && matches!(img, DynamicImage::ImageLumaA8(_))
        && la_single_outlier5x5_parameters_match(kernel, scale, offset)
        && filter_5x5_la_single_outlier_rows(raw, &mut out, w_u32 as usize, h_u32 as usize)
    {
        let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
        return Ok(preserve_mode(img, result));
    }
    #[cfg(not(feature = "parallel"))]
    if matches!(img, DynamicImage::ImageRgba8(_))
        && matches!(explicit_mode, None | Some("RGBA"))
        && uniform5x5_parameters_match(kernel, scale, offset)
        && filter_5x5_uniform_rgba_rows(raw, &mut out, w_u32 as usize, h_u32 as usize)
    {
        let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
        return Ok(preserve_mode(img, result));
    }
    let native_byte_layout = matches!(
        img,
        DynamicImage::ImageLuma8(_)
            | DynamicImage::ImageLumaA8(_)
            | DynamicImage::ImageRgb8(_)
            | DynamicImage::ImageRgba8(_)
    );
    if native_byte_layout
        && native_small_uniform_byte_image(img, channels, SMALL_UNIFORM_BYTE_MAX_PIXELS)
    {
        filter_5x5_constant_bytes(
            raw,
            &mut out,
            w_u32 as usize,
            h_u32 as usize,
            channels,
            &normalized_kernel,
            rounding_bias,
        );
        let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
        return Ok(preserve_mode(img, result));
    }
    if matches!(explicit_mode, None | Some("L"))
        && matches!(img, DynamicImage::ImageLuma8(_))
        && uniform5x5_parameters_match(kernel, scale, offset)
        && filter_5x5_uniform_luma_rows(raw, &mut out, w_u32 as usize, h_u32 as usize)
    {
        let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
        return Ok(preserve_mode(img, result));
    }
    #[cfg(not(feature = "parallel"))]
    if matches!(explicit_mode, None | Some("L"))
        && matches!(img, DynamicImage::ImageLuma8(_))
        && binomial5x5_parameters_match(kernel, scale, offset)
        && filter_5x5_binomial_luma_rows(raw, &mut out, w_u32 as usize, h_u32 as usize)
    {
        let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
        return Ok(preserve_mode(img, result));
    }
    #[cfg(not(feature = "parallel"))]
    #[cfg(target_arch = "aarch64")]
    #[cfg(target_feature = "neon")]
    if matches!(img, DynamicImage::ImageLumaA8(_))
        && w_u32 >= 13
        && h_u32 >= 5
        && binomial5x5_parameters_match(kernel, scale, offset)
    {
        // Reuse the exact packed-LA vector kernel for serial CPU on AArch64.
        // Keep this out of Parallel CPU so Rayon remains a separate profile.
        let mut out = raw.to_vec();
        if let Some((vector_blocks, scalar_tail)) =
            crate::compute::pool_simd::ops::native_filter::native_filter_5x5_binomial_la_rows(
                raw,
                &mut out,
                w_u32 as usize,
                h_u32 as usize,
            )
        {
            crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
            crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
            let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
            return Ok(preserve_mode(img, result));
        }
    }
    #[cfg(not(feature = "parallel"))]
    if matches!(explicit_mode, None | Some("LA"))
        && matches!(img, DynamicImage::ImageLumaA8(_))
        && binomial5x5_parameters_match(kernel, scale, offset)
        && filter_5x5_binomial_la_rows(raw, &mut out, w_u32 as usize, h_u32 as usize)
    {
        let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
        return Ok(preserve_mode(img, result));
    }
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
    let sigma = sigma.abs();
    if sigma == 0.0 {
        return Ok(img.clone());
    }
    pil_box_blur(img, gaussian_blur_box_radius(sigma), 3)
}

/// Match Pillow's three-pass Gaussian-to-box radius calculation.
fn gaussian_blur_box_radius(sigma: f32) -> f32 {
    // Uses the "From Box Blur to Gaussian Blur" algorithm (Gwosdek et al. 2011).
    // Pillow's ImagingGaussianBlur uses f32 parameters but f64 in sqrt/promotion.
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
    (l + a) as f32
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

#[cfg(all(test, not(feature = "parallel")))]
mod gaussian_blur_row_fusion_tests {
    use super::{
        blur_columns, blur_interleaved_rows_three_passes,
        blur_interleaved_vertical_radius_one_fractional_rows, blur_la_radius_one_fractional_line,
        blur_line, blur_luma_radius_one_fractional_line, blur_luma_rows_three_passes,
        blur_luma_vertical_radius_one_rows, blur_parameters, blur_rgb_radius_one_fractional_line,
        blur_rows, execute_gaussian_blur, gaussian_blur_box_radius,
        uniform_rgba_box_blur_is_identity,
    };
    use crate::raster::{DynamicImage, GrayAlphaImage, RgbaImage};

    fn three_pass_rgba_reference(
        source: &[u8],
        width: usize,
        height: usize,
        radius: f32,
    ) -> Vec<u8> {
        let (integer_radius, whole_weight, fractional_weight) = blur_parameters(radius);
        let mut work = source.to_vec();
        let mut scratch = vec![0; source.len()];
        for _ in 0..3 {
            blur_rows(
                &work,
                &mut scratch,
                width,
                height,
                4,
                integer_radius,
                whole_weight,
                fractional_weight,
            );
            std::mem::swap(&mut work, &mut scratch);
        }
        let mut accumulator = vec![0u32; width * 4];
        for _ in 0..3 {
            blur_columns(
                &work,
                &mut scratch,
                width,
                height,
                4,
                integer_radius,
                whole_weight,
                fractional_weight,
                &mut accumulator,
            );
            std::mem::swap(&mut work, &mut scratch);
        }
        work
    }

    #[test]
    fn large_uniform_rgba_gaussian_blur_returns_exact_input() {
        let (width, height) = (1024usize, 768usize);
        let sample = [83, 149, 17, 191];
        let raw = sample.repeat(width * height);
        let radius = gaussian_blur_box_radius(7.5);
        let (integer_radius, whole_weight, fractional_weight) = blur_parameters(radius);
        assert_eq!(integer_radius, 7);
        assert_eq!(whole_weight, 1_116_425);
        assert_eq!(fractional_weight, 15_420);
        assert!(uniform_rgba_box_blur_is_identity(
            &raw,
            width,
            height,
            (integer_radius, whole_weight, fractional_weight),
            (integer_radius, whole_weight, fractional_weight),
        ));

        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(width as u32, height as u32, raw.clone())
                .expect("valid uniform RGBA image"),
        );
        let actual = execute_gaussian_blur(&image, 7.5).expect("GaussianBlur");
        assert_eq!(actual.as_bytes(), raw);
    }

    #[test]
    fn large_rgba_gaussian_blur_full_scan_rejects_an_unsampled_pixel() {
        let (width, height) = (67usize, 67usize);
        let sample = [83, 149, 17, 191];
        let mut raw = sample.repeat(width * height);
        // This point is outside the 7-by-5 spaced rejection probes. The
        // complete scan must still find it and let the real blur run.
        raw[(width + 1) * 4..(width + 2) * 4].copy_from_slice(&[255, 0, 0, 0]);
        let radius = gaussian_blur_box_radius(7.5);
        let (integer_radius, whole_weight, fractional_weight) = blur_parameters(radius);
        assert!(!uniform_rgba_box_blur_is_identity(
            &raw,
            width,
            height,
            (integer_radius, whole_weight, fractional_weight),
            (integer_radius, whole_weight, fractional_weight),
        ));

        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(width as u32, height as u32, raw.clone())
                .expect("valid adversarial RGBA image"),
        );
        let actual = execute_gaussian_blur(&image, 7.5).expect("GaussianBlur");
        let expected = three_pass_rgba_reference(&raw, width, height, radius);
        assert_eq!(actual.as_bytes(), expected);
        assert_ne!(actual.as_bytes(), raw);
    }

    #[test]
    fn two_channel_step_matches_direct_box_blur_reference() {
        for width in [1, 2, 5, 17] {
            let source = (0..width * 2)
                .map(|index| ((index * 47 + index / 2 * 31 + 17) % 256) as u8)
                .collect::<Vec<_>>();
            for radius in [0.0, 0.25, 1.0, 1.375, 4.75, 30.0] {
                let (integer_radius, whole_weight, fractional_weight) = blur_parameters(radius);
                let mut actual = vec![0u8; source.len()];
                let mut accumulator = [0u32; 2];
                blur_line(
                    &source,
                    &mut actual,
                    0,
                    width,
                    2,
                    integer_radius,
                    whole_weight,
                    fractional_weight,
                    &mut accumulator,
                );

                let mut expected = vec![0u8; source.len()];
                let last = width as isize - 1;
                for x in 0..width {
                    for channel in 0..2 {
                        let mut whole_sum = 0u32;
                        for offset in -(integer_radius as isize)..=(integer_radius as isize) {
                            let sample_x = (x as isize + offset).clamp(0, last) as usize;
                            whole_sum =
                                whole_sum.wrapping_add(u32::from(source[sample_x * 2 + channel]));
                        }
                        let mut weighted_sum = whole_sum.wrapping_mul(whole_weight);
                        if fractional_weight != 0 {
                            let left_x =
                                (x as isize - integer_radius as isize - 1).clamp(0, last) as usize;
                            let right_x =
                                (x as isize + integer_radius as isize + 1).clamp(0, last) as usize;
                            let far_sum = u32::from(source[left_x * 2 + channel])
                                + u32::from(source[right_x * 2 + channel]);
                            weighted_sum =
                                weighted_sum.wrapping_add(far_sum.wrapping_mul(fractional_weight));
                        }
                        expected[x * 2 + channel] =
                            (weighted_sum.wrapping_add(super::BOX_BLUR_BIAS) >> 24) as u8;
                    }
                }

                assert_eq!(
                    actual, expected,
                    "width={width}, radius={radius}, integer_radius={integer_radius}"
                );
            }
        }
    }

    #[test]
    fn la_radius_one_fractional_line_matches_generic_fixed_point_blur() {
        let (radius, whole_weight, fractional_weight) = blur_parameters(1.375);
        assert_eq!(radius, 1);
        assert_ne!(fractional_weight, 0);

        for width in [5, 6, 7, 17, 65, 1024] {
            let source = (0..width * 2)
                .map(|index| ((index * 73 + index / 7 * 19 + 31) % 256) as u8)
                .collect::<Vec<_>>();
            let mut actual = vec![0u8; source.len()];
            let mut expected = vec![0u8; source.len()];
            blur_la_radius_one_fractional_line(
                &source,
                &mut actual,
                width,
                whole_weight,
                fractional_weight,
            );
            blur_line(
                &source,
                &mut expected,
                0,
                width,
                2,
                radius,
                whole_weight,
                fractional_weight,
                &mut [0u32; 4],
            );

            assert_eq!(actual, expected, "LA row width={width}");
        }
    }

    #[test]
    fn rgb_radius_one_fractional_line_matches_generic_fixed_point_blur() {
        let (radius, whole_weight, fractional_weight) = blur_parameters(1.375);
        assert_eq!(radius, 1);
        assert_ne!(fractional_weight, 0);

        for width in [5, 6, 7, 17, 65, 1024] {
            let source = (0..width * 3)
                .map(|index| ((index * 73 + index / 7 * 19 + 31) % 256) as u8)
                .collect::<Vec<_>>();
            let mut actual = vec![0u8; source.len()];
            let mut expected = vec![0u8; source.len()];
            blur_rgb_radius_one_fractional_line(
                &source,
                &mut actual,
                width,
                whole_weight,
                fractional_weight,
            );
            blur_line(
                &source,
                &mut expected,
                0,
                width,
                3,
                radius,
                whole_weight,
                fractional_weight,
                &mut [0u32; 4],
            );

            assert_eq!(actual, expected, "RGB row width={width}");
        }
    }

    #[test]
    fn luma_radius_one_fractional_line_matches_generic_fixed_point_blur() {
        let (radius, whole_weight, fractional_weight) = blur_parameters(1.375);
        assert_eq!(radius, 1);
        assert_ne!(fractional_weight, 0);

        for width in [1, 2, 3, 4, 5, 6, 7, 17, 65, 1024] {
            let source = (0..width)
                .map(|index| ((index * 73 + index / 7 * 19 + 31) % 256) as u8)
                .collect::<Vec<_>>();
            let mut actual = vec![0u8; width];
            let mut expected = vec![0u8; width];
            blur_luma_radius_one_fractional_line(
                &source,
                &mut actual,
                width,
                whole_weight,
                fractional_weight,
            );
            blur_line(
                &source,
                &mut expected,
                0,
                width,
                1,
                radius,
                whole_weight,
                fractional_weight,
                &mut [0u32; 4],
            );

            assert_eq!(actual, expected, "L row width={width}");
        }
    }

    #[test]
    fn luma_three_pass_row_fusion_matches_full_frame_passes() {
        for (width, height, radius) in [
            (1, 1, 1.375),
            (2, 3, 1.375),
            (5, 7, 0.25),
            (17, 9, 1.375),
            (65, 47, 2.5),
        ] {
            let source = (0..width * height)
                .map(|index| ((index * 73 + index / 7 * 19 + 31) % 256) as u8)
                .collect::<Vec<_>>();
            let (integer_radius, whole_weight, fractional_weight) = blur_parameters(radius);
            let mut fused = vec![0u8; source.len()];
            let mut reference_a = source.clone();
            let mut reference_b = vec![0u8; source.len()];

            blur_luma_rows_three_passes(
                &source,
                &mut fused,
                width,
                height,
                integer_radius,
                whole_weight,
                fractional_weight,
            );
            for _ in 0..3 {
                blur_rows(
                    &reference_a,
                    &mut reference_b,
                    width,
                    height,
                    1,
                    integer_radius,
                    whole_weight,
                    fractional_weight,
                );
                std::mem::swap(&mut reference_a, &mut reference_b);
            }

            assert_eq!(
                fused, reference_a,
                "L rows {width}x{height}, radius={radius}"
            );
        }
    }

    #[test]
    fn luma_vertical_radius_one_rows_match_fixed_point_column_reference() {
        for (width, height) in [(1, 1), (1, 7), (2, 3), (17, 9), (65, 47)] {
            for radius in [1.0, 1.375] {
                let source = (0..width * height)
                    .map(|index| ((index * 73 + index / 7 * 19 + 31) % 256) as u8)
                    .collect::<Vec<_>>();
                let mut actual = vec![0u8; source.len()];
                let mut expected = vec![0u8; source.len()];
                let (integer_radius, whole_weight, fractional_weight) = blur_parameters(radius);

                blur_luma_vertical_radius_one_rows(
                    &source,
                    &mut actual,
                    width,
                    height,
                    whole_weight,
                    fractional_weight,
                );
                blur_columns(
                    &source,
                    &mut expected,
                    width,
                    height,
                    1,
                    integer_radius,
                    whole_weight,
                    fractional_weight,
                    &mut vec![0u32; width],
                );

                assert_eq!(
                    actual, expected,
                    "L vertical {width}x{height}, radius={radius}"
                );
            }
        }
    }

    #[test]
    fn interleaved_vertical_radius_one_fractional_rows_match_fixed_point_reference() {
        let (radius, whole_weight, fractional_weight) = blur_parameters(1.375);
        assert_eq!(radius, 1);
        assert_ne!(fractional_weight, 0);

        for (width, height) in [(1, 1), (1, 7), (2, 3), (5, 4), (17, 9), (65, 47)] {
            for channels in [2, 3, 4] {
                let len = width * height * channels;
                let source = (0..len)
                    .map(|index| ((index * 73 + index / 11 * 29 + 37) % 256) as u8)
                    .collect::<Vec<_>>();
                let mut actual = vec![0u8; len];
                let mut expected = vec![0u8; len];

                blur_interleaved_vertical_radius_one_fractional_rows(
                    &source,
                    &mut actual,
                    width,
                    height,
                    channels,
                    whole_weight,
                    fractional_weight,
                );
                blur_columns(
                    &source,
                    &mut expected,
                    width,
                    height,
                    channels,
                    radius,
                    whole_weight,
                    fractional_weight,
                    &mut vec![0u32; width * channels],
                );

                assert_eq!(actual, expected, "{width}x{height}, channels={channels}");
            }
        }
    }

    #[test]
    fn single_channel_step_matches_two_component_generic_reference() {
        for (width, height, radius) in [
            (1, 1, 1.375),
            (2, 3, 1.375),
            (5, 7, 0.25),
            (17, 9, 1.375),
            (65, 47, 2.5),
        ] {
            let source = (0..width * height)
                .map(|index| ((index * 73 + index / 7 * 19 + 31) % 256) as u8)
                .collect::<Vec<_>>();
            let mut specialized = vec![0u8; source.len()];
            let mut generic = vec![0u8; source.len()];
            let (integer_radius, whole_weight, fractional_weight) = blur_parameters(radius);

            blur_rows(
                &source,
                &mut specialized,
                width,
                height,
                1,
                integer_radius,
                whole_weight,
                fractional_weight,
            );

            // A two-byte pixel takes the generic recurrence while each byte
            // evolves independently. Duplicate every L sample and use the
            // first output byte as the one-channel reference.
            for row in 0..height {
                let row_start = row * width;
                let mut paired_source = Vec::with_capacity(width * 2);
                for &sample in &source[row_start..row_start + width] {
                    paired_source.extend_from_slice(&[sample, sample]);
                }
                let mut paired_output = vec![0u8; width * 2];
                let mut accumulator = [0u32; 4];
                blur_line(
                    &paired_source,
                    &mut paired_output,
                    0,
                    width,
                    2,
                    integer_radius,
                    whole_weight,
                    fractional_weight,
                    &mut accumulator,
                );
                for (x, pair) in paired_output.chunks_exact(2).enumerate() {
                    assert_eq!(pair[0], pair[1]);
                    generic[row_start + x] = pair[0];
                }
            }

            assert_eq!(
                specialized, generic,
                "L rows {width}x{height}, radius={radius}"
            );
        }
    }

    #[test]
    fn interleaved_gaussian_horizontal_row_fusion_matches_three_full_frame_passes() {
        for channels in [2, 3, 4] {
            for (width, height, radius) in
                [(1, 1, 1.375), (2, 3, 1.375), (7, 5, 1.375), (65, 47, 1.375)]
            {
                let pixel_count = width * height;
                let source = (0..pixel_count * channels)
                    .map(|index| ((index * 73 + index / 7 * 19 + 31) % 256) as u8)
                    .collect::<Vec<_>>();
                let (integer_radius, whole_weight, fractional_weight) = blur_parameters(radius);
                let mut fused = vec![0; source.len()];
                match channels {
                    2 => blur_interleaved_rows_three_passes::<2>(
                        &source,
                        &mut fused,
                        width,
                        height,
                        integer_radius,
                        whole_weight,
                        fractional_weight,
                    ),
                    3 => blur_interleaved_rows_three_passes::<3>(
                        &source,
                        &mut fused,
                        width,
                        height,
                        integer_radius,
                        whole_weight,
                        fractional_weight,
                    ),
                    4 => blur_interleaved_rows_three_passes::<4>(
                        &source,
                        &mut fused,
                        width,
                        height,
                        integer_radius,
                        whole_weight,
                        fractional_weight,
                    ),
                    _ => unreachable!("test channels are 2, 3, or 4"),
                }

                let mut work = source.clone();
                let mut scratch = vec![0; source.len()];
                for _ in 0..3 {
                    blur_rows(
                        &work,
                        &mut scratch,
                        width,
                        height,
                        channels,
                        integer_radius,
                        whole_weight,
                        fractional_weight,
                    );
                    std::mem::swap(&mut work, &mut scratch);
                }

                assert_eq!(
                    fused, work,
                    "fused {channels}-channel passes differ at {width}×{height}"
                );
            }
        }
    }

    #[test]
    fn la_gaussian_blur_row_fusion_matches_full_frame_passes() {
        for (width, height) in [(1, 1), (2, 3), (7, 5), (65, 47)] {
            let pixel_count = width * height;
            let source = (0..pixel_count * 2)
                .map(|index| ((index * 73 + index / 7 * 19 + 31) % 256) as u8)
                .collect::<Vec<_>>();
            let image = DynamicImage::ImageLumaA8(
                GrayAlphaImage::from_raw(width as u32, height as u32, source.clone())
                    .expect("valid LA fixture"),
            );
            let actual = execute_gaussian_blur(&image, 2.0).expect("LA GaussianBlur");

            let (integer_radius, whole_weight, fractional_weight) = blur_parameters(1.375);
            let mut work = source;
            let mut scratch = vec![0; work.len()];
            for _ in 0..3 {
                blur_rows(
                    &work,
                    &mut scratch,
                    width,
                    height,
                    2,
                    integer_radius,
                    whole_weight,
                    fractional_weight,
                );
                std::mem::swap(&mut work, &mut scratch);
            }

            let mut accumulator = vec![0u32; width * 2];
            for _ in 0..3 {
                blur_columns(
                    &work,
                    &mut scratch,
                    width,
                    height,
                    2,
                    integer_radius,
                    whole_weight,
                    fractional_weight,
                    &mut accumulator,
                );
                std::mem::swap(&mut work, &mut scratch);
            }

            assert_eq!(
                actual.as_bytes(),
                work,
                "fused LA GaussianBlur differs at {width}×{height}"
            );
        }
    }
}

#[cfg(test)]
mod i32_filter5x5_uniform_tests {
    use super::{execute_filter5x5, uniform_i32_filter5x5_is_identity};
    use crate::raster::{DynamicImage, RgbaImage};

    #[test]
    fn uniform_binomial_filter_returns_the_exact_input() {
        let (width, height, sample) = (32, 24, 37i32);
        let pixel = sample.to_le_bytes();
        let raw = pixel.repeat(width * height);
        let kernel = [
            1.0, 4.0, 6.0, 4.0, 1.0, 4.0, 16.0, 24.0, 16.0, 4.0, 6.0, 24.0, 36.0, 24.0, 6.0, 4.0,
            16.0, 24.0, 16.0, 4.0, 1.0, 4.0, 6.0, 4.0, 1.0,
        ];
        let normalized = std::array::from_fn(|index| kernel[index] / 256.0);
        assert!(uniform_i32_filter5x5_is_identity(
            &raw,
            width,
            height,
            &normalized,
            0.5,
        ));

        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(width as u32, height as u32, raw.clone()).unwrap(),
        );
        let result = execute_filter5x5(&image, &kernel, 256.0, 0.0, Some("I")).unwrap();
        assert_eq!(result.as_bytes(), raw);
    }

    #[test]
    fn uniformity_proof_rejects_nonidentity_and_nonuniform_inputs() {
        let (width, height, sample) = (7, 6, 11i32);
        let pixel = sample.to_le_bytes();
        let raw = pixel.repeat(width * height);
        let mut identity_kernel = [0.0; 25];
        identity_kernel[12] = 1.0;
        assert!(uniform_i32_filter5x5_is_identity(
            &raw,
            width,
            height,
            &identity_kernel,
            0.5,
        ));

        let mut nonidentity_kernel = identity_kernel;
        nonidentity_kernel[12] = 2.0;
        assert!(!uniform_i32_filter5x5_is_identity(
            &raw,
            width,
            height,
            &nonidentity_kernel,
            0.5,
        ));

        let mut varied = raw;
        varied[4 * (width + 3)..4 * (width + 3) + 4].copy_from_slice(&12i32.to_le_bytes());
        assert!(!uniform_i32_filter5x5_is_identity(
            &varied,
            width,
            height,
            &identity_kernel,
            0.5,
        ));
    }
}

#[cfg(all(test, not(feature = "parallel")))]
mod i32_filter5x5_sliding_window_tests {
    use super::PortableF32Fma;
    use super::{
        DynamicImage, filter_5x5_i32, filter_5x5_i32_bounded_binomial_from_samples,
        filter_5x5_i32_f32_binomial_scaled_rows_ring_with_fma, filter_5x5_i32_f32_ring,
        i32_filter5x5_binomial_kernel_is_applicable,
        i32_filter5x5_bounded_binomial_kernel_is_applicable, pillow_kernel_row_5,
    };

    fn scalar_reference_row(
        raw: &[u8],
        row: &mut [u8],
        y: i32,
        width: i32,
        height: i32,
        kernel: &[f32; 25],
        offset: f32,
    ) {
        if y < 2 || y >= height - 2 || width < 5 {
            return;
        }
        for x in 2..width - 2 {
            let mut value = offset + 0.5;
            for (row_index, dy) in [2, 1, 0, -1, -2].into_iter().enumerate() {
                let samples = std::array::from_fn(|tap| {
                    let pixel = ((y + dy) * width + x + tap as i32 - 2) as usize;
                    i32::from_le_bytes([
                        raw[pixel * 4],
                        raw[pixel * 4 + 1],
                        raw[pixel * 4 + 2],
                        raw[pixel * 4 + 3],
                    ]) as f32
                });
                value += pillow_kernel_row_5(samples, &kernel[row_index * 5..row_index * 5 + 5]);
            }
            let result = if value >= 0.0 { value as i32 } else { 0 };
            let output = x as usize * 4;
            row[output..output + 4].copy_from_slice(&result.to_le_bytes());
        }
    }

    #[test]
    fn sliding_windows_preserve_scalar_fma_results_for_signed_i32_samples() {
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let mut values = vec![i32::MIN, i32::MAX, -16_777_217, 16_777_217, -1, 0, 1];
        for _ in values.len()..19 * 13 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            values.push(state as u32 as i32);
        }

        let kernel = std::array::from_fn(|index| ((index as i32 * 17 % 23) - 11) as f32 / 16.0);
        for (width, height) in [(5usize, 5usize), (6, 7), (19, 13)] {
            let mut raw = Vec::with_capacity(width * height * 4);
            for index in 0..width * height {
                raw.extend_from_slice(&values[index].to_le_bytes());
            }
            for offset in [0.0f32, 0.5, -17.25, 93.125] {
                let row_stride = width * 4;
                let mut expected = raw.clone();
                for y in 0..height {
                    let row_start = y * row_stride;
                    scalar_reference_row(
                        &raw,
                        &mut expected[row_start..row_start + row_stride],
                        y as i32,
                        width as i32,
                        height as i32,
                        &kernel,
                        offset,
                    );
                }
                let actual = filter_5x5_i32_f32_ring(&raw, width, height, &kernel, offset).unwrap();
                for pixel in 0..width * height {
                    let start = pixel * 4;
                    assert_eq!(
                        &actual[start..start + 4],
                        &expected[start..start + 4],
                        "sample mismatch at ({}, {}) for offset {offset}",
                        pixel % width,
                        pixel / width,
                    );
                }
            }
        }
    }

    #[test]
    fn bounded_binomial_integer_ring_matches_f32_at_minimum_and_odd_dimensions() {
        let weights = [1i32, 4, 6, 4, 1];
        let kernel =
            std::array::from_fn(|index| (weights[index / 5] * weights[index % 5]) as f32 / 256.0);
        let values: [i32; 7] = [-65_535, -32_768, -1, 0, 1, 32_767, 65_535];

        for (width, height) in [(5usize, 5usize), (6, 6), (7, 8), (8, 5)] {
            let samples = (0..width * height)
                .map(|index| values[(index * 5 + index / width) % values.len()])
                .collect::<Vec<_>>();
            let raw = samples
                .iter()
                .flat_map(|sample| sample.to_le_bytes())
                .collect::<Vec<_>>();
            assert!(i32_filter5x5_bounded_binomial_kernel_is_applicable(
                samples.len(),
                width,
                height,
                &kernel,
                0.0,
            ));

            let actual =
                filter_5x5_i32_bounded_binomial_from_samples(&raw, &samples, width, height)
                    .unwrap();
            let reference = filter_5x5_i32_f32_ring(&raw, width, height, &kernel, 0.0).unwrap();
            assert_eq!(
                actual, reference,
                "bounded kernel mismatch at {width}×{height}"
            );

            for out_of_bound_index in [0, samples.len() / 2, samples.len() - 1] {
                let mut out_of_bound = samples.clone();
                out_of_bound[out_of_bound_index] = 65_536;
                let out_of_bound_raw = out_of_bound
                    .iter()
                    .flat_map(|sample| sample.to_le_bytes())
                    .collect::<Vec<_>>();
                assert!(i32_filter5x5_bounded_binomial_kernel_is_applicable(
                    out_of_bound.len(),
                    width,
                    height,
                    &kernel,
                    0.0,
                ));
                assert!(
                    filter_5x5_i32_bounded_binomial_from_samples(
                        &out_of_bound_raw,
                        &out_of_bound,
                        width,
                        height,
                    )
                    .is_none()
                );

                let image = DynamicImage::ImageRgba8(
                    crate::raster::RgbaImage::from_raw(
                        width as u32,
                        height as u32,
                        out_of_bound_raw.clone(),
                    )
                    .unwrap(),
                );
                let actual = filter_5x5_i32(&image, &kernel, 1.0, 0.0).unwrap();
                let DynamicImage::ImageRgba8(actual) = actual else {
                    panic!("I-mode filter must preserve its scalar carrier");
                };
                let reference =
                    filter_5x5_i32_f32_ring(&out_of_bound_raw, width, height, &kernel, 0.0)
                        .unwrap();
                assert_eq!(actual.as_raw(), &reference);
            }
        }
    }

    #[test]
    fn binomial_scaled_rows_ring_matches_pillow_fma_for_full_i32_range() {
        let weights = [1i32, 4, 6, 4, 1];
        let kernel =
            std::array::from_fn(|index| (weights[index / 5] * weights[index % 5]) as f32 / 256.0);
        assert!(i32_filter5x5_binomial_kernel_is_applicable(&kernel));

        let mut state = 0x6a09_e667_f3bc_c909u64;
        let mut values = vec![i32::MIN, i32::MAX, -999_997, 999_996, -1, 0, 1];
        for _ in values.len()..19 * 13 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            values.push(state as u32 as i32);
        }

        for (width, height) in [(5usize, 5usize), (6, 7), (19, 13)] {
            let samples = (0..width * height)
                .map(|index| values[index])
                .collect::<Vec<_>>();
            let raw = samples
                .iter()
                .flat_map(|sample| sample.to_le_bytes())
                .collect::<Vec<_>>();
            for offset in [0.0f32, 0.5, -17.25] {
                let mut expected = raw.clone();
                let row_stride = width * 4;
                for y in 0..height {
                    let row_start = y * row_stride;
                    scalar_reference_row(
                        &raw,
                        &mut expected[row_start..row_start + row_stride],
                        y as i32,
                        width as i32,
                        height as i32,
                        &kernel,
                        offset,
                    );
                }

                let actual = filter_5x5_i32_f32_binomial_scaled_rows_ring_with_fma(
                    &raw,
                    &samples,
                    width,
                    height,
                    &kernel,
                    offset,
                    &PortableF32Fma,
                )
                .unwrap();
                assert_eq!(
                    actual, expected,
                    "FMA mismatch at {width}×{height}, offset {offset}"
                );

                let image = DynamicImage::ImageRgba8(
                    crate::raster::RgbaImage::from_raw(width as u32, height as u32, raw.clone())
                        .unwrap(),
                );
                let DynamicImage::ImageRgba8(actual) = filter_5x5_i32(
                    &image,
                    &kernel.map(|coefficient| coefficient * 256.0),
                    256.0,
                    offset,
                )
                .unwrap() else {
                    panic!("I-mode filter must preserve its scalar carrier");
                };
                assert_eq!(
                    actual.as_raw(),
                    &expected,
                    "dispatch mismatch at {width}×{height}, offset {offset}"
                );
            }
        }
    }

    #[test]
    fn unaligned_i32_source_preserves_filter_output() {
        let width = 9usize;
        let height = 7usize;
        let values = [i32::MIN, -16_777_217, -1, 0, 1, 65_535, 65_536, i32::MAX];
        let mut raw = Vec::with_capacity(width * height * 4);
        for index in 0..width * height {
            raw.extend_from_slice(&values[index % values.len()].to_le_bytes());
        }
        let mut storage = vec![0; raw.len() + std::mem::align_of::<i32>()];
        let base = storage.as_ptr() as usize;
        let offset = (0..std::mem::align_of::<i32>())
            .find(|offset| (base + offset) % std::mem::align_of::<i32>() != 0)
            .expect("an unaligned byte offset must exist");
        storage[offset..offset + raw.len()].copy_from_slice(&raw);
        let unaligned = &storage[offset..offset + raw.len()];

        let kernel = std::array::from_fn(|index| ((index as i32 * 13 % 19) - 9) as f32 / 8.0);
        let offset = -3.25f32;
        let row_stride = width * 4;
        let mut expected = raw.clone();
        for y in 0..height {
            let row_start = y * row_stride;
            scalar_reference_row(
                &raw,
                &mut expected[row_start..row_start + row_stride],
                y as i32,
                width as i32,
                height as i32,
                &kernel,
                offset,
            );
        }
        let actual = filter_5x5_i32_f32_ring(unaligned, width, height, &kernel, offset).unwrap();
        assert_eq!(actual, expected);
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn runtime_x86_f32_fma_matches_portable_fused_operation() {
        use super::{F32MulAdd, X86F32FmaToken, i32_filter_f32_fma_is_safe};

        let Some(x86_fma) = X86F32FmaToken::detect() else {
            return;
        };
        let portable = PortableF32Fma;
        for (sample, weight, accumulator) in [
            (0.1f32, 0.2, 0.3),
            (1.0 + f32::EPSILON, 1.0 - f32::EPSILON, -1.0),
            (f32::MIN_POSITIVE, 1.0, -0.0),
            (-0.0, 1.0, -0.0),
            (f32::MAX * 0.125, 1.5, -f32::MAX * 0.125),
        ] {
            assert_eq!(
                x86_fma.mul_add(sample, weight, accumulator).to_bits(),
                portable.mul_add(sample, weight, accumulator).to_bits(),
                "FMA result differs for {sample:?} * {weight:?} + {accumulator:?}",
            );
        }

        let factors = [1.0f32, 4.0, 6.0, 4.0, 1.0];
        let kernel = std::array::from_fn(|index| factors[index % 5] * factors[index / 5] / 256.0);
        assert!(i32_filter_f32_fma_is_safe(&kernel, 0.0));
        assert!(i32_filter_f32_fma_is_safe(&kernel, -0.0));

        let mut nonfinite = kernel;
        nonfinite[7] = f32::NAN;
        assert!(!i32_filter_f32_fma_is_safe(&nonfinite, 0.0));
        assert!(!i32_filter_f32_fma_is_safe(&kernel, f32::INFINITY));

        let mut excessive = [0.0f32; 25];
        excessive[12] = f32::MAX;
        assert!(!i32_filter_f32_fma_is_safe(&excessive, 0.0));
    }
}

#[cfg(test)]
mod i32_filter3x3_uniform_tests {
    use super::{
        I32_UNIFORM_FILTER3X3_MAX_PIXELS, execute_filter3x3, uniform_i32_filter3x3_is_identity,
    };
    use crate::raster::{DynamicImage, RgbaImage};

    #[test]
    fn uniform_cross_smoothing_filter_returns_the_exact_input() {
        let (width, height, sample) = (32, 24, 37i32);
        let raw = sample.to_le_bytes().repeat(width * height);
        let kernel = [0.0, 1.0, 0.0, 1.0, 4.0, 1.0, 0.0, 1.0, 0.0];
        let normalized = std::array::from_fn(|index| kernel[index] / 8.0);
        assert!(uniform_i32_filter3x3_is_identity(
            &raw,
            width,
            height,
            &normalized,
            0.5,
        ));

        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(width as u32, height as u32, raw.clone()).unwrap(),
        );
        let result = execute_filter3x3(&image, &kernel, 8.0, 0.0, Some("I")).unwrap();
        assert_eq!(result.as_bytes(), raw);
    }

    #[test]
    fn uniformity_proof_rejects_nonidentity_nonuniform_and_oversized_inputs() {
        let (width, height, sample) = (7, 6, 11i32);
        let raw = sample.to_le_bytes().repeat(width * height);
        let mut identity_kernel = [0.0; 9];
        identity_kernel[4] = 1.0;
        assert!(uniform_i32_filter3x3_is_identity(
            &raw,
            width,
            height,
            &identity_kernel,
            0.5,
        ));

        let mut nonidentity_kernel = identity_kernel;
        nonidentity_kernel[4] = 2.0;
        assert!(!uniform_i32_filter3x3_is_identity(
            &raw,
            width,
            height,
            &nonidentity_kernel,
            0.5,
        ));

        let mut varied = raw.clone();
        varied[4 * (width + 3)..4 * (width + 3) + 4].copy_from_slice(&12i32.to_le_bytes());
        assert!(!uniform_i32_filter3x3_is_identity(
            &varied,
            width,
            height,
            &identity_kernel,
            0.5,
        ));
        assert!(!uniform_i32_filter3x3_is_identity(
            &raw,
            I32_UNIFORM_FILTER3X3_MAX_PIXELS + 1,
            3,
            &identity_kernel,
            0.5,
        ));
    }
}

#[cfg(test)]
mod f_mode_rank_filter_uniform_tests {
    use super::{
        native_uniform_f_mode_image, rank_filter_f_requires_legacy_sort, rank_filter_impl,
        select_rank_filter_f_value,
    };
    use crate::raster::{DynamicImage, RgbaImage};

    fn raw_f32(values: &[f32]) -> Vec<u8> {
        let mut raw = Vec::with_capacity(values.len() * 4);
        for value in values {
            raw.extend_from_slice(&value.to_le_bytes());
        }
        raw
    }

    #[test]
    fn uniform_f_mode_rank_filter_preserves_exact_source_bytes() {
        let (width, height, sample) = (256, 256, 37.0f32);
        let raw = sample.to_le_bytes().repeat(width * height);
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(width as u32, height as u32, raw.clone()).unwrap(),
        );

        let result = rank_filter_impl(&image, 9, 40, Some("F")).unwrap();
        assert_eq!(result.as_bytes(), raw);
    }

    #[test]
    fn uniform_f_mode_detection_is_bit_exact() {
        assert!(native_uniform_f_mode_image(
            &37.0f32.to_le_bytes().repeat(4),
            2,
            2
        ));
        assert!(!native_uniform_f_mode_image(
            &[0.0f32.to_le_bytes(), (-0.0f32).to_le_bytes()].concat(),
            2,
            1,
        ));
        assert!(!native_uniform_f_mode_image(&37.0f32.to_le_bytes(), 0, 1));
    }

    #[test]
    fn f_mode_selection_matches_full_sort_for_finite_ties() {
        let original = [8.0, -3.0, 8.0, 0.5, 11.0, -3.0, 4.0, 1.0, 8.0];
        assert!(!rank_filter_f_requires_legacy_sort(&raw_f32(&original)));
        for rank in 0..original.len() {
            let mut sorted = original;
            sorted.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let expected = sorted[rank].to_bits();
            let mut selected = original.to_vec();
            assert_eq!(
                select_rank_filter_f_value(&mut selected, rank, false).to_bits(),
                expected,
            );
        }
    }

    #[test]
    fn f_mode_non_total_values_keep_legacy_sort() {
        assert!(rank_filter_f_requires_legacy_sort(&raw_f32(&[
            1.0,
            f32::from_bits(0x7fc0_1234),
            2.0,
        ])));
        assert!(rank_filter_f_requires_legacy_sort(&raw_f32(&[
            -0.0, 1.0, 0.0
        ])));
        assert!(!rank_filter_f_requires_legacy_sort(&raw_f32(&[
            -0.0, -0.0, 1.0
        ])));
    }
}

#[cfg(all(test, not(feature = "parallel")))]
mod binomial3x3_native_la_tests {
    use super::{binomial3x3_parameters_match, filter_3x3_binomial_la_rows, filter_3x3_byte_rows};

    #[test]
    fn native_la_ring_matches_ordered_float_kernel_and_copied_borders() {
        let kernel = [1.0, 2.0, 1.0, 2.0, 4.0, 2.0, 1.0, 2.0, 1.0];
        assert!(binomial3x3_parameters_match(&kernel, 16.0, 0.0));
        assert!(!binomial3x3_parameters_match(&kernel, 15.0, 0.0));
        assert!(!binomial3x3_parameters_match(&kernel, 16.0, 1.0));

        let normalized = std::array::from_fn(|index| kernel[index] / 16.0);
        for (width, height) in [
            (1usize, 1usize),
            (2, 5),
            (3, 3),
            (4, 7),
            (7, 5),
            (9, 7),
            (17, 9),
            (33, 35),
        ] {
            let raw: Vec<u8> = (0..width * height * 2)
                .map(|index| ((index * 73 + index / (width * 2) * 29 + 17) % 256) as u8)
                .collect();
            let mut expected = raw.clone();
            filter_3x3_byte_rows(
                &raw,
                &mut expected,
                width as i32,
                height as i32,
                2,
                &normalized,
                0.5,
            );

            let mut actual = raw.clone();
            assert!(filter_3x3_binomial_la_rows(
                &raw,
                &mut actual,
                width,
                height,
            ));
            assert_eq!(actual, expected, "native LA {width}x{height}");
        }
    }
}

#[cfg(all(test, not(feature = "parallel")))]
mod binomial5x5_native_byte_tests {
    use super::{filter_5x5_binomial_la_rows, filter_5x5_binomial_luma_rows, filter_5x5_byte_rows};

    #[test]
    fn integer_separable_path_matches_ordered_float_kernel_and_copied_borders() {
        let weights = [1.0, 4.0, 6.0, 4.0, 1.0];
        let kernel: [f32; 25] =
            std::array::from_fn(|index| weights[index / 5] * weights[index % 5]);
        let normalized = std::array::from_fn(|index| kernel[index] / 256.0);
        for (width, height) in [(8, 6), (9, 7), (5, 5), (4, 8), (8, 4)] {
            let raw: Vec<u8> = (0..width * height)
                .map(|index| ((index * 73 + index / width * 29 + 17) % 256) as u8)
                .collect();
            let mut expected = raw.clone();
            filter_5x5_byte_rows(
                &raw,
                &mut expected,
                width as i32,
                height as i32,
                1,
                &normalized,
                0.5,
            );

            let mut actual = raw.clone();
            assert!(filter_5x5_binomial_luma_rows(
                &raw,
                &mut actual,
                width,
                height,
            ));
            assert_eq!(actual, expected, "dimension {width}x{height}");
        }
    }

    #[test]
    fn native_la_ring_matches_ordered_float_kernel_for_each_interleaved_band() {
        let weights = [1.0, 4.0, 6.0, 4.0, 1.0];
        let kernel: [f32; 25] =
            std::array::from_fn(|index| weights[index / 5] * weights[index % 5]);
        let normalized = std::array::from_fn(|index| kernel[index] / 256.0);
        for (width, height) in [
            (1usize, 1usize),
            (4, 7),
            (5, 5),
            (7, 5),
            (9, 7),
            (17, 9),
            (33, 35),
        ] {
            let raw: Vec<u8> = (0..width * height * 2)
                .map(|index| ((index * 73 + index / (width * 2) * 29 + 17) % 256) as u8)
                .collect();
            let mut expected = raw.clone();
            filter_5x5_byte_rows(
                &raw,
                &mut expected,
                width as i32,
                height as i32,
                2,
                &normalized,
                0.5,
            );

            let mut actual = raw.clone();
            assert!(filter_5x5_binomial_la_rows(
                &raw,
                &mut actual,
                width,
                height,
            ));
            assert_eq!(actual, expected, "native LA {width}x{height}");
        }
    }

    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    #[test]
    fn native_la_vector_ring_matches_ordered_float_kernel_across_vector_tails() {
        let weights = [1.0, 4.0, 6.0, 4.0, 1.0];
        let kernel: [f32; 25] =
            std::array::from_fn(|index| weights[index / 5] * weights[index % 5]);
        let normalized = std::array::from_fn(|index| kernel[index] / 256.0);
        for (width, height) in [(5usize, 5usize), (13, 7), (19, 11), (20, 13), (31, 17)] {
            let raw: Vec<u8> = (0..width * height * 2)
                .map(|index| ((index * 73 + index / (width * 2) * 29 + 17) % 256) as u8)
                .collect();
            let mut expected = raw.clone();
            filter_5x5_byte_rows(
                &raw,
                &mut expected,
                width as i32,
                height as i32,
                2,
                &normalized,
                0.5,
            );

            let mut actual = raw.clone();
            let (vector_blocks, scalar_tail) =
                crate::compute::pool_simd::ops::native_filter::native_filter_5x5_binomial_la_rows(
                    &raw,
                    &mut actual,
                    width,
                    height,
                )
                .expect("valid native LA vector dimensions");
            if width >= 13 && height >= 5 {
                assert!(vector_blocks > 0, "{width}x{height} should vectorize");
            }
            if width == 13 || width == 19 || width == 31 {
                assert!(scalar_tail > 0, "{width}x{height} should have a tail");
            }
            assert_eq!(actual, expected, "native LA {width}x{height}");
        }
    }
}

#[cfg(all(test, not(feature = "parallel")))]
mod la_single_outlier5x5_tests {
    use super::{
        filter_5x5_byte_rows, filter_5x5_la_single_outlier_rows,
        la_single_outlier5x5_parameters_match,
    };

    #[test]
    fn rolling_integer_sums_match_ordered_float_for_la_pixels_and_borders() {
        let mut kernel = [1.0f32; 25];
        kernel[24] = 2.0;
        let normalized = std::array::from_fn(|index| kernel[index] / 25.0);
        assert!(la_single_outlier5x5_parameters_match(&kernel, 25.0, 0.0));
        assert!(!la_single_outlier5x5_parameters_match(&kernel, 24.0, 0.0));
        assert!(!la_single_outlier5x5_parameters_match(&kernel, 25.0, 1.0));

        let mut misplaced_outlier = kernel;
        misplaced_outlier[23] = 2.0;
        misplaced_outlier[24] = 1.0;
        assert!(!la_single_outlier5x5_parameters_match(
            &misplaced_outlier,
            25.0,
            0.0
        ));

        for (width, height) in [
            (1usize, 1usize),
            (4, 7),
            (7, 4),
            (5, 5),
            (7, 5),
            (13, 7),
            (19, 11),
            (71, 67),
        ] {
            for pattern in 0..4 {
                let mut state = 0x9e37_79b9u32;
                let raw: Vec<u8> = (0..width * height * 2)
                    .map(|index| match pattern {
                        0 => 0,
                        1 => 255,
                        2 => ((index * 73 + index / (width * 2) * 29 + 17) % 256) as u8,
                        _ => {
                            state ^= state << 13;
                            state ^= state >> 17;
                            state ^= state << 5;
                            state as u8
                        }
                    })
                    .collect();
                let mut expected = raw.clone();
                filter_5x5_byte_rows(
                    &raw,
                    &mut expected,
                    width as i32,
                    height as i32,
                    2,
                    &normalized,
                    0.5,
                );

                let mut actual = raw.clone();
                assert!(filter_5x5_la_single_outlier_rows(
                    &raw,
                    &mut actual,
                    width,
                    height,
                ));
                assert_eq!(
                    actual, expected,
                    "native LA {width}x{height}, pattern {pattern}"
                );
            }
        }
    }
}

#[cfg(all(test, feature = "parallel"))]
mod parallel_binomial5x5_native_la_tests {
    use super::{
        binomial5x5_parameters_match, filter_5x5_binomial_la_parallel_rows, filter_5x5_byte_rows,
    };

    #[test]
    fn banded_native_la_ring_matches_ordered_float_kernel_and_copied_borders() {
        let weights = [1.0f32, 4.0, 6.0, 4.0, 1.0];
        let kernel: [f32; 25] =
            std::array::from_fn(|index| weights[index / 5] * weights[index % 5]);
        let normalized = std::array::from_fn(|index| kernel[index] / 256.0);
        assert!(binomial5x5_parameters_match(&kernel, 256.0, 0.0));
        assert!(!binomial5x5_parameters_match(&kernel, 256.0, 1.0));

        for (width, height) in [
            (1usize, 1usize),
            (4, 7),
            (5, 5),
            (7, 5),
            (17, 67),
            (19, 68),
            (21, 69),
            (23, 132),
            (25, 133),
        ] {
            for pattern in 0..3 {
                let raw: Vec<u8> = (0..width * height * 2)
                    .map(|index| match pattern {
                        0 => 0,
                        1 => 255,
                        _ => ((index * 73 + index / (width * 2) * 29 + 17) % 256) as u8,
                    })
                    .collect();
                let mut expected = raw.clone();
                filter_5x5_byte_rows(
                    &raw,
                    &mut expected,
                    width as i32,
                    height as i32,
                    2,
                    &normalized,
                    0.5,
                );

                let actual = filter_5x5_binomial_la_parallel_rows(&raw, width, height)
                    .expect("valid native LA dimensions");
                assert_eq!(
                    actual, expected,
                    "native LA {width}x{height}, pattern {pattern}"
                );
            }
        }
    }
}

#[cfg(test)]
mod uniform5x5_native_luma_tests {
    use super::{filter_5x5_byte_rows, filter_5x5_uniform_luma_rows, uniform5x5_parameters_match};

    #[test]
    fn rolling_integer_rows_match_ordered_float_for_all_sums_and_borders() {
        let kernel = [1.0f32; 25];
        let normalized = [1.0f32 / 25.0; 25];
        assert!(uniform5x5_parameters_match(&kernel, 25.0, 0.0));
        assert!(!uniform5x5_parameters_match(&kernel, 24.0, 0.0));
        assert!(!uniform5x5_parameters_match(&kernel, 25.0, 1.0));
        let mut almost_uniform = kernel;
        almost_uniform[12] = f32::from_bits(1.0f32.to_bits() + 1);
        assert!(!uniform5x5_parameters_match(&almost_uniform, 25.0, 0.0));

        // Every integer numerator from 0 through 25*255 is representable by
        // 25 byte samples. Compare the integer path against Pillow's ordered
        // multiply/FMA/row-add path at every possible byte-rounding boundary.
        for sum in 0..=25 * 255 {
            let mut raw = [0u8; 25];
            let mut remaining = sum;
            for sample in &mut raw {
                let value = remaining.min(255);
                *sample = value as u8;
                remaining -= value;
            }
            let mut expected = raw;
            filter_5x5_byte_rows(&raw, &mut expected, 5, 5, 1, &normalized, 0.5);
            let mut actual = raw;
            assert!(filter_5x5_uniform_luma_rows(&raw, &mut actual, 5, 5));
            assert_eq!(actual[12], expected[12], "window sum {sum}");
        }

        for (width, height) in [(13usize, 7usize), (4, 7), (11, 13), (19, 11)] {
            let raw = (0..width * height)
                .map(|index| ((index * 73 + index / width * 29 + 17) % 256) as u8)
                .collect::<Vec<_>>();
            let mut expected = raw.clone();
            filter_5x5_byte_rows(
                &raw,
                &mut expected,
                width as i32,
                height as i32,
                1,
                &normalized,
                0.5,
            );
            let mut actual = raw.clone();
            assert!(filter_5x5_uniform_luma_rows(
                &raw,
                &mut actual,
                width,
                height
            ));
            assert_eq!(actual, expected, "{width}x{height}");
        }
    }
}

#[cfg(all(test, not(feature = "parallel")))]
mod uniform5x5_native_rgba_tests {
    use super::{filter_5x5_byte_rows, filter_5x5_uniform_rgba_rows};

    #[test]
    fn rolling_integer_rows_match_ordered_float_for_odd_width_and_copied_borders() {
        let (width, height) = (67usize, 63usize);
        let raw = (0..width * height * 4)
            .map(|index| ((index * 73 + index / (width * 4) * 29 + 17) % 256) as u8)
            .collect::<Vec<_>>();
        let normalized = [1.0f32 / 25.0; 25];

        let mut expected = raw.clone();
        filter_5x5_byte_rows(
            &raw,
            &mut expected,
            width as i32,
            height as i32,
            4,
            &normalized,
            0.5,
        );

        let mut actual = raw.clone();
        assert!(filter_5x5_uniform_rgba_rows(
            &raw,
            &mut actual,
            width,
            height,
        ));
        assert_eq!(actual, expected, "native RGBA {width}x{height}");
    }
}

#[cfg(test)]
mod native_l_rank_filter_second_min_tests {
    use super::{rank_filter_bytes_3x3, rank_filter_bytes_small};

    #[test]
    fn rank_one_second_min_matches_selection_for_ties_borders_and_tails() {
        for (width, height) in [(1usize, 1usize), (1, 11), (2, 3), (17, 9), (33, 35)] {
            let raw = (0..width * height)
                .map(|index| ((index * 37 + index / width * 11 + 3) % 5) as u8)
                .collect::<Vec<_>>();
            let mut actual = vec![0u8; raw.len()];
            let mut selected = vec![0u8; raw.len()];

            rank_filter_bytes_3x3(&raw, &mut actual, width, height, 1, 1);
            rank_filter_bytes_small(&raw, &mut selected, width as i32, height as i32, 1, 1, 9, 1);

            assert_eq!(actual, selected, "native L {width}x{height}");
        }
    }
}

#[cfg(all(
    test,
    not(feature = "parallel"),
    any(
        all(target_arch = "aarch64", target_feature = "neon"),
        all(target_arch = "x86_64", target_feature = "sse2")
    )
))]
mod native_i32_vector_filter_tests {
    use super::filter_5x5_i32_row;

    #[test]
    fn vector_i32_kernel_matches_ordered_f32_across_width_tails_and_borders() {
        let factors = [1i32, 4, 6, 4, 1];
        let kernel =
            std::array::from_fn(|index| (factors[index / 5] * factors[index % 5]) as f32 / 256.0);
        let patterns = [
            i32::MIN,
            i32::MAX,
            -16_777_217,
            -65_536,
            -1,
            0,
            1,
            65_535,
            65_536,
            16_777_219,
        ];

        for (width, height) in [(5usize, 5usize), (13, 7), (19, 11), (20, 13), (31, 17)] {
            let raw = (0..width * height)
                .flat_map(|index| {
                    patterns[(index * 7 + index / width * 3) % patterns.len()].to_le_bytes()
                })
                .collect::<Vec<_>>();
            for offset in [0.0f32, 0.5, -17.25] {
                let mut expected = raw.clone();
                let row_stride = width * 4;
                for y in 0..height {
                    let row_start = y * row_stride;
                    filter_5x5_i32_row(
                        &raw,
                        &mut expected[row_start..row_start + row_stride],
                        y as i32,
                        width as i32,
                        height as i32,
                        &kernel,
                        offset,
                    );
                }

                let mut actual = raw.clone();
                let (vector_blocks, scalar_tail) =
                    crate::compute::pool_simd::ops::native_filter::native_filter_5x5_i32_rows(
                        &raw,
                        &mut actual,
                        width,
                        height,
                        &kernel,
                        offset + 0.5,
                    )
                    .expect("valid native I-mode vector dimensions");
                assert!(vector_blocks > 0);
                assert_eq!(scalar_tail, 0);
                assert_eq!(actual, expected, "I-mode {width}x{height}, offset {offset}");
            }
        }
    }
}
