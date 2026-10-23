//! Signed-I implementation for the 5x5 convolution filter.

use super::super::convolution::pillow_kernel_row_5;
#[cfg(test)]
use super::execute_filter5x5;
use crate::error::PilError;
#[cfg(not(feature = "parallel"))]
use crate::ops::pil_resize::i32_samples_from_le_bytes;
use crate::raster::DynamicImage;
use std::borrow::Cow;

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

pub(super) fn filter_5x5_i32(
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

#[cfg(test)]
#[path = "int32/tests.rs"]
mod tests;
