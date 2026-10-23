//! Filter operations extracted from image.rs execute_op().
//!
//! These functions are standalone implementations of PIL-compatible filter
//! operations (Filter3x3, Filter5x5, GaussianBlur, BoxBlur, MedianFilter,
//! MaxFilter, MinFilter, RankFilter) that operate on DynamicImage and return
//! new DynamicImage instances.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;

#[path = "filter/uniform_image.rs"]
mod uniform_image;
use uniform_image::{SMALL_UNIFORM_BYTE_MAX_PIXELS, native_small_uniform_byte_image};

#[path = "filter/blur.rs"]
mod blur;
pub(crate) use blur::{execute_box_blur, execute_box_blur_xy_with_passes, execute_gaussian_blur};

#[path = "filter/rank.rs"]
mod rank;
pub(crate) use rank::{
    execute_max_filter_with_mode, execute_median_filter_with_mode, execute_min_filter_with_mode,
    execute_rank_filter_with_mode,
};

#[cfg(not(feature = "parallel"))]
#[path = "filter/uniform_box.rs"]
mod uniform_box;

#[cfg(not(feature = "parallel"))]
#[path = "filter/rgb_blur_ring.rs"]
mod rgb_blur_ring;

#[cfg(not(feature = "parallel"))]
#[path = "filter/kernels/mod.rs"]
mod kernels;

#[cfg(all(test, not(feature = "parallel")))]
#[path = "filter/uniform_box_tests.rs"]
mod uniform_box_tests;

use crate::image::preserve_mode;
use crate::image_utils::raw_bytes_to_image;
#[cfg(not(feature = "parallel"))]
use crate::ops::pil_resize::i32_samples_from_le_bytes;
use crate::raster::DynamicImage;
use std::borrow::Cow;

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
