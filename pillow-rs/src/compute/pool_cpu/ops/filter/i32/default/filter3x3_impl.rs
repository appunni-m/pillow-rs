//! Native signed-I 3x3 convolution strategy.

use crate::compute::pool_cpu::ops::filter::helpers::pillow_kernel_row_3;
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};

#[path = "i32_cross_smooth_kernel.rs"]
mod i32_cross_smooth_kernel;

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
pub(in crate::compute::pool_cpu::ops::filter::i32) const I32_UNIFORM_FILTER3X3_MAX_PIXELS: usize =
    1024 * 1024;

/// Compare one uniform sample's exact normalized 3x3 result with its source.
pub(in crate::compute::pool_cpu::ops::filter::i32) fn uniform_i32_filter3x3_sample_is_identity(
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
pub(in crate::compute::pool_cpu::ops::filter::i32) fn uniform_i32_filter3x3_is_identity(
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

fn execute(
    img: &DynamicImage,
    kernel: &[f32; 9],
    scale: f32,
    offset: f32,
) -> Result<DynamicImage, PilError> {
    // I-mode stores one little-endian i32 in each four-byte pixel. Borrow that
    // exact carrier instead of cloning it through the RGBA accessor; retain
    // the old widening route for callers with a different concrete variant.
    let (w_u32, h_u32) = (img.width(), img.height());
    let DynamicImage::ImageI32(image) = img else {
        return Err(PilError::InternalError(
            "Filter3x3 I strategy received a non-I raster".into(),
        ));
    };
    let raw = image.as_raw();
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
    
    
    if i32_cross_smooth_kernel::matches(&kd, offset) {
        i32_cross_smooth_kernel::apply(raw, &mut out, w_u32 as usize, h_u32 as usize);
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

    Ok(DynamicImage::ImageI32(
        crate::raster::RgbaImage::from_raw(w_u32, h_u32, out)
            .ok_or_else(|| PilError::ValueError("filter_3x3_i32: buffer error".into()))?,
    ))
}


#[cfg(test)]
#[path = "filter3x3_tests.rs"]
mod tests;
