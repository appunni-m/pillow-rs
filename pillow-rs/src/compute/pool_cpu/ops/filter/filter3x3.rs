//! CPU implementation for Pillow's 3x3 convolution filter.

use super::convolution::{clip8_filter, pillow_kernel_row_3};
use super::uniform_image::{SMALL_UNIFORM_BYTE_MAX_PIXELS, native_small_uniform_byte_image};
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::image_utils::raw_bytes_to_image;
use crate::raster::DynamicImage;

#[path = "kernels/mod.rs"]
mod kernels;

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
    // I-mode stores one little-endian i32 in each four-byte carrier. A
    // mismatched storage variant is not widened into this mode's kernel.
    let (w_u32, h_u32) = (img.width(), img.height());
    let DynamicImage::ImageRgba8(rgba) = img else {
        return Err(PilError::ValueError(
            "I-mode filter requires four-byte scalar storage".into(),
        ));
    };
    let raw = rgba.as_raw();
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

/// Apply one row of the byte 3x3 convolution without changing Pillow's
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

pub(crate) fn binomial3x3_parameters_match(kernel: &[f32; 9], scale: f32, offset: f32) -> bool {
    const WEIGHTS: [f32; 9] = [1.0, 2.0, 1.0, 2.0, 4.0, 2.0, 1.0, 2.0, 1.0];
    scale == 16.0 && offset == 0.0 && kernel == &WEIGHTS
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
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
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

#[cfg(test)]
#[path = "filter3x3/tests.rs"]
mod tests;
