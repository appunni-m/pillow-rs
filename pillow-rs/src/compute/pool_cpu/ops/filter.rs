//! Filter operations extracted from image.rs execute_op().
//!
//! These functions are standalone implementations of PIL-compatible filter
//! operations (Filter3x3, Filter5x5, GaussianBlur, BoxBlur, MedianFilter,
//! MaxFilter, MinFilter, RankFilter) that operate on DynamicImage and return
//! new DynamicImage instances.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::preserve_mode;
use image_slash_star::DynamicImage;

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

// ── Raw bytes to image ──

/// Convert raw flat bytes back to a DynamicImage based on channel count.
pub fn raw_bytes_to_image(
    w: u32,
    h: u32,
    data: Vec<u8>,
    channels: usize,
) -> Result<DynamicImage, PilError> {
    match channels {
        1 => Ok(DynamicImage::ImageLuma8(
            image_slash_star::GrayImage::from_raw(w, h, data)
                .ok_or_else(|| PilError::ValueError("raw_bytes_to_image: buffer error".into()))?,
        )),
        2 => Ok(DynamicImage::ImageLumaA8(
            image_slash_star::GrayAlphaImage::from_raw(w, h, data)
                .ok_or_else(|| PilError::ValueError("raw_bytes_to_image: buffer error".into()))?,
        )),
        3 => Ok(DynamicImage::ImageRgb8(
            image_slash_star::RgbImage::from_raw(w, h, data)
                .ok_or_else(|| PilError::ValueError("raw_bytes_to_image: buffer error".into()))?,
        )),
        4 => Ok(DynamicImage::ImageRgba8(
            image_slash_star::RgbaImage::from_raw(w, h, data)
                .ok_or_else(|| PilError::ValueError("raw_bytes_to_image: buffer error".into()))?,
        )),
        _ => Err(PilError::ValueError(format!(
            "raw_bytes_to_image: unsupported channel count {}",
            channels
        ))),
    }
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

    // Pre-divide kernel by scale (matching PIL C construction)
    let s = if scale.abs() < 1e-10 { 1.0 } else { scale };
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

    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let base = |dx: i32, dy: i32| -> usize { ((y + dy) * w + (x + dx)) as usize * 4 };
            let read_pixel = |dx: i32, dy: i32| -> i32 {
                let bi = base(dx, dy);
                i32::from_le_bytes([raw[bi], raw[bi + 1], raw[bi + 2], raw[bi + 3]])
            };

            // PIL reverses Y-axis: k[0] on bottom row, k[8] on top row
            let bot_row = pillow_kernel_row_3(
                [
                    read_pixel(-1, 1) as f32,
                    read_pixel(0, 1) as f32,
                    read_pixel(1, 1) as f32,
                ],
                &kd[0..3],
            );
            let mid_row = pillow_kernel_row_3(
                [
                    read_pixel(-1, 0) as f32,
                    read_pixel(0, 0) as f32,
                    read_pixel(1, 0) as f32,
                ],
                &kd[3..6],
            );
            let top_row = pillow_kernel_row_3(
                [
                    read_pixel(-1, -1) as f32,
                    read_pixel(0, -1) as f32,
                    read_pixel(1, -1) as f32,
                ],
                &kd[6..9],
            );

            // PIL I-mode: with +0.5 rounding bias
            let mut ss = offset as f32 + 0.5;
            ss += bot_row;
            ss += mid_row;
            ss += top_row;

            // PIL: clip negative to 0 (allow values > 255)
            let result = if ss >= 0.0 { ss as i32 } else { 0 };
            let out_idx = (y * w + x) as usize * 4;
            let le = result.to_le_bytes();
            out[out_idx] = le[0];
            out[out_idx + 1] = le[1];
            out[out_idx + 2] = le[2];
            out[out_idx + 3] = le[3];
        }
    }

    Ok(DynamicImage::ImageRgba8(
        image_slash_star::RgbaImage::from_raw(w_u32, h_u32, out)
            .ok_or_else(|| PilError::ValueError("filter_3x3_i32: buffer error".into()))?,
    ))
}

// ── 5x5 filter (I-mode) ──

/// Apply a 5x5 kernel filter on I-mode (32-bit signed integer) data.
/// Same approach as filter_3x3_i32 — f32, reversed Y-axis, +0.5 rounding.
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

    let s = if scale.abs() < 1e-10 { 1.0 } else { scale };
    // Pre-compute normalized kernel coefficients using f32 (matching PIL C construction)
    let kd: [f32; 25] = std::array::from_fn(|i| kernel[i] / s);

    let mut out = raw.clone();

    for y in 2..h - 2 {
        for x in 2..w - 2 {
            let base = |dx: i32, dy: i32| -> usize { ((y + dy) * w + (x + dx)) as usize * 4 };
            let read_pixel = |dx: i32, dy: i32| -> i32 {
                let bi = base(dx, dy);
                i32::from_le_bytes([raw[bi], raw[bi + 1], raw[bi + 2], raw[bi + 3]])
            };

            // Reversed Y-axis: k[0..4] on bottom row (y+2), k[20..24] on top row (y-2)
            let bot_row0 = pillow_kernel_row_5(
                [
                    read_pixel(-2, 2) as f32,
                    read_pixel(-1, 2) as f32,
                    read_pixel(0, 2) as f32,
                    read_pixel(1, 2) as f32,
                    read_pixel(2, 2) as f32,
                ],
                &kd[0..5],
            );
            let bot_row1 = pillow_kernel_row_5(
                [
                    read_pixel(-2, 1) as f32,
                    read_pixel(-1, 1) as f32,
                    read_pixel(0, 1) as f32,
                    read_pixel(1, 1) as f32,
                    read_pixel(2, 1) as f32,
                ],
                &kd[5..10],
            );
            let mid_row = pillow_kernel_row_5(
                [
                    read_pixel(-2, 0) as f32,
                    read_pixel(-1, 0) as f32,
                    read_pixel(0, 0) as f32,
                    read_pixel(1, 0) as f32,
                    read_pixel(2, 0) as f32,
                ],
                &kd[10..15],
            );
            let top_row1 = pillow_kernel_row_5(
                [
                    read_pixel(-2, -1) as f32,
                    read_pixel(-1, -1) as f32,
                    read_pixel(0, -1) as f32,
                    read_pixel(1, -1) as f32,
                    read_pixel(2, -1) as f32,
                ],
                &kd[15..20],
            );
            let top_row0 = pillow_kernel_row_5(
                [
                    read_pixel(-2, -2) as f32,
                    read_pixel(-1, -2) as f32,
                    read_pixel(0, -2) as f32,
                    read_pixel(1, -2) as f32,
                    read_pixel(2, -2) as f32,
                ],
                &kd[20..25],
            );

            // PIL I-mode: with +0.5 rounding bias
            let mut ss = offset as f32 + 0.5;
            ss += bot_row0;
            ss += bot_row1;
            ss += mid_row;
            ss += top_row1;
            ss += top_row0;

            // PIL clips I-mode filter results to 0 (no negative values)
            let result = if ss >= 0.0 { ss as i32 } else { 0 };
            let out_idx = (y * w + x) as usize * 4;
            let le = result.to_le_bytes();
            out[out_idx] = le[0];
            out[out_idx + 1] = le[1];
            out[out_idx + 2] = le[2];
            out[out_idx + 3] = le[3];
        }
    }

    Ok(DynamicImage::ImageRgba8(
        image_slash_star::RgbaImage::from_raw(w_u32, h_u32, out)
            .ok_or_else(|| PilError::ValueError("filter_5x5_i32: buffer error".into()))?,
    ))
}

// ── PIL-style box blur ──

/// PIL-style box blur with fractional radius support.
/// Uses sliding-window accumulator with fixed-point (24-bit) arithmetic.
/// Matches PIL order: ALL horizontal passes first, then ALL vertical passes.
pub fn pil_box_blur(
    img: &DynamicImage,
    radius: f32,
    passes: u32,
) -> Result<DynamicImage, PilError> {
    if radius <= 0.0 {
        return Ok(img.clone());
    }
    let channels = img.color().channel_count() as usize;
    let raw = img.as_bytes();
    let (w_u32, h_u32) = (img.width(), img.height());
    let w = w_u32 as i32;
    let h = h_u32 as i32;

    // Integer part of radius (PIL: (int)floatRadius)
    let r_int = radius as i32;
    // Number of pixels in the integer window
    let window_pixels = (2 * r_int + 1) as u32;
    // Fixed-point weight: PIL uses f32 precision for ww computation
    // (UINT32)((1 << 24) / (floatRadius * 2 + 1)) — all in f32
    let ww = ((1u64 << 24) as f32 / (radius * 2.0 + 1.0)) as u32;
    // Fractional edge weight (PIL: fw = ((1 << 24) - window_pixels * ww) / 2)
    let fw = ((1u64 << 24) - window_pixels as u64 * ww as u64) as u32 / 2;
    let bias = 1u32 << 23;

    let mut work = raw.to_vec();

    // PIL does ALL horizontal passes first (matching ImagingBoxBlur order)
    for _pass in 0..passes {
        let mut hpass = CheckedDims::new(w as u32, h as u32, channels as u8)?.alloc_buffer();
        for y in 0..h {
            for x in 0..w {
                for c in 0..channels {
                    let mut acc = 0u64;
                    for dx in -r_int..=r_int {
                        let sx = (x + dx).clamp(0, w - 1);
                        let idx = (y * w + sx) as usize * channels + c;
                        acc += work[idx] as u64;
                    }
                    let left_x = (x - r_int - 1).clamp(0, w - 1);
                    let right_x = (x + r_int + 1).clamp(0, w - 1);
                    let lv = work[(y * w + left_x) as usize * channels + c] as u64;
                    let rv = work[(y * w + right_x) as usize * channels + c] as u64;
                    let bulk = acc * ww as u64 + (lv + rv) * fw as u64 + bias as u64;
                    hpass[(y * w + x) as usize * channels + c] = (bulk >> 24) as u8;
                }
            }
        }
        work = hpass;
    }

    // PIL does ALL vertical passes after all horizontal passes
    for _pass in 0..passes {
        let mut vpass = CheckedDims::new(w as u32, h as u32, channels as u8)?.alloc_buffer();
        for x in 0..w {
            for y in 0..h {
                for c in 0..channels {
                    let mut acc = 0u64;
                    for dy in -r_int..=r_int {
                        let sy = (y + dy).clamp(0, h - 1);
                        let idx = (sy * w + x) as usize * channels + c;
                        acc += work[idx] as u64;
                    }
                    let top_y = (y - r_int - 1).clamp(0, h - 1);
                    let bot_y = (y + r_int + 1).clamp(0, h - 1);
                    let tv = work[(top_y * w + x) as usize * channels + c] as u64;
                    let bv = work[(bot_y * w + x) as usize * channels + c] as u64;
                    let bulk = acc * ww as u64 + (tv + bv) * fw as u64 + bias as u64;
                    vpass[(y * w + x) as usize * channels + c] = (bulk >> 24) as u8;
                }
            }
        }
        work = vpass;
    }

    let result = raw_bytes_to_image(w_u32, h_u32, work, channels)?;
    Ok(preserve_mode(img, result))
}

// ── Rank filter ──

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

        for y in 0..h {
            for x in 0..w {
                let mut vals: Vec<f32> = Vec::with_capacity(area);
                for dy in -half..=half {
                    for dx in -half..=half {
                        let sx = (x + dx).clamp(0, w - 1);
                        let sy = (y + dy).clamp(0, h - 1);
                        let base = (sy * w + sx) as usize * 4;
                        let val = f32::from_le_bytes([
                            raw[base],
                            raw[base + 1],
                            raw[base + 2],
                            raw[base + 3],
                        ]);
                        vals.push(val);
                    }
                }
                vals.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let result = vals[rank];
                let out_base = (y * w + x) as usize * 4;
                let le = result.to_le_bytes();
                out[out_base] = le[0];
                out[out_base + 1] = le[1];
                out[out_base + 2] = le[2];
                out[out_base + 3] = le[3];
            }
        }
        let result = DynamicImage::ImageRgba8(
            image_slash_star::RgbaImage::from_raw(w_u32, h_u32, out)
                .ok_or_else(|| PilError::ValueError("rank_filter_impl(F): buffer error".into()))?,
        );
        return Ok(preserve_mode(img, result));
    }

    let channels = img.color().channel_count() as usize;
    let raw = img.as_bytes();
    let mut out = CheckedDims::new(w as u32, h as u32, channels as u8)?.alloc_buffer();

    for y in 0..h {
        for x in 0..w {
            let mut chan_vals: Vec<Vec<u8>> = vec![Vec::with_capacity(area); channels];
            for dy in -half..=half {
                for dx in -half..=half {
                    let sx = (x + dx).clamp(0, w - 1);
                    let sy = (y + dy).clamp(0, h - 1);
                    let base = (sy * w + sx) as usize * channels;
                    for c in 0..channels {
                        chan_vals[c].push(raw[base + c]);
                    }
                }
            }
            for c in 0..channels {
                chan_vals[c].sort_unstable();
                out[(y * w + x) as usize * channels + c] = chan_vals[c][rank];
            }
        }
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
    let s = if scale.abs() < 1e-10 { 1.0 } else { scale };
    let k0 = kernel[0] / s;
    let k1 = kernel[1] / s;
    let k2 = kernel[2] / s;
    let k3 = kernel[3] / s;
    let k4 = kernel[4] / s;
    let k5 = kernel[5] / s;
    let k6 = kernel[6] / s;
    let k7 = kernel[7] / s;
    let k8 = kernel[8] / s;
    let rounding_bias = offset as f32 + 0.5;
    let mut out = raw.to_vec();
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let base =
                |dx: i32, dy: i32| -> usize { ((y + dy) * w + (x + dx)) as usize * channels };
            for c in 0..channels {
                let row_b = pillow_kernel_row_3(
                    [
                        raw[base(-1, 1) + c] as f32,
                        raw[base(0, 1) + c] as f32,
                        raw[base(1, 1) + c] as f32,
                    ],
                    &[k0, k1, k2],
                );
                let row_c = pillow_kernel_row_3(
                    [
                        raw[base(-1, 0) + c] as f32,
                        raw[base(0, 0) + c] as f32,
                        raw[base(1, 0) + c] as f32,
                    ],
                    &[k3, k4, k5],
                );
                let row_t = pillow_kernel_row_3(
                    [
                        raw[base(-1, -1) + c] as f32,
                        raw[base(0, -1) + c] as f32,
                        raw[base(1, -1) + c] as f32,
                    ],
                    &[k6, k7, k8],
                );
                let mut ss = rounding_bias;
                ss += row_b;
                ss += row_c;
                ss += row_t;
                out[(y * w + x) as usize * channels + c] = clip8_filter(ss);
            }
        }
    }
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
    let s = if scale.abs() < 1e-10 { 1.0 } else { scale };
    let k00 = kernel[0] / s;
    let k01 = kernel[1] / s;
    let k02 = kernel[2] / s;
    let k03 = kernel[3] / s;
    let k04 = kernel[4] / s;
    let k10 = kernel[5] / s;
    let k11 = kernel[6] / s;
    let k12 = kernel[7] / s;
    let k13 = kernel[8] / s;
    let k14 = kernel[9] / s;
    let k20 = kernel[10] / s;
    let k21 = kernel[11] / s;
    let k22 = kernel[12] / s;
    let k23 = kernel[13] / s;
    let k24 = kernel[14] / s;
    let k30 = kernel[15] / s;
    let k31 = kernel[16] / s;
    let k32 = kernel[17] / s;
    let k33 = kernel[18] / s;
    let k34 = kernel[19] / s;
    let k40 = kernel[20] / s;
    let k41 = kernel[21] / s;
    let k42 = kernel[22] / s;
    let k43 = kernel[23] / s;
    let k44 = kernel[24] / s;
    let rounding_bias = offset as f32 + 0.5;
    let mut out = raw.to_vec();
    for y in 2..h - 2 {
        for x in 2..w - 2 {
            let base =
                |dx: i32, dy: i32| -> usize { ((y + dy) * w + (x + dx)) as usize * channels };
            for c in 0..channels {
                let row0 = pillow_kernel_row_5(
                    [
                        raw[base(-2, 2) + c] as f32,
                        raw[base(-1, 2) + c] as f32,
                        raw[base(0, 2) + c] as f32,
                        raw[base(1, 2) + c] as f32,
                        raw[base(2, 2) + c] as f32,
                    ],
                    &[k00, k01, k02, k03, k04],
                );
                let mut ss = rounding_bias;
                ss += row0;
                let row1 = pillow_kernel_row_5(
                    [
                        raw[base(-2, 1) + c] as f32,
                        raw[base(-1, 1) + c] as f32,
                        raw[base(0, 1) + c] as f32,
                        raw[base(1, 1) + c] as f32,
                        raw[base(2, 1) + c] as f32,
                    ],
                    &[k10, k11, k12, k13, k14],
                );
                ss += row1;
                let row2 = pillow_kernel_row_5(
                    [
                        raw[base(-2, 0) + c] as f32,
                        raw[base(-1, 0) + c] as f32,
                        raw[base(0, 0) + c] as f32,
                        raw[base(1, 0) + c] as f32,
                        raw[base(2, 0) + c] as f32,
                    ],
                    &[k20, k21, k22, k23, k24],
                );
                ss += row2;
                let row3 = pillow_kernel_row_5(
                    [
                        raw[base(-2, -1) + c] as f32,
                        raw[base(-1, -1) + c] as f32,
                        raw[base(0, -1) + c] as f32,
                        raw[base(1, -1) + c] as f32,
                        raw[base(2, -1) + c] as f32,
                    ],
                    &[k30, k31, k32, k33, k34],
                );
                ss += row3;
                let row4 = pillow_kernel_row_5(
                    [
                        raw[base(-2, -2) + c] as f32,
                        raw[base(-1, -2) + c] as f32,
                        raw[base(0, -2) + c] as f32,
                        raw[base(1, -2) + c] as f32,
                        raw[base(2, -2) + c] as f32,
                    ],
                    &[k40, k41, k42, k43, k44],
                );
                ss += row4;
                out[(y * w + x) as usize * channels + c] = clip8_filter(ss);
            }
        }
    }
    let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
    Ok(preserve_mode(img, result))
}

/// Execute a Gaussian blur via 3 passes of PIL-style box blur.
pub fn execute_gaussian_blur(img: &DynamicImage, sigma: f32) -> Result<DynamicImage, PilError> {
    // PIL GaussianBlur: 3 passes of BoxBlur with computed fractional radius.
    // Uses the "From Box Blur to Gaussian Blur" algorithm (Gwosdek et al. 2011).
    // PIL's ImagingGaussianBlur uses f32 parameters but f64 in sqrt/promotion.
    if sigma <= 0.0 {
        return Ok(img.clone());
    }
    let passes = 3.0f64;
    let sigma2 = sigma as f64 * sigma as f64 / passes;
    let l_val = ((12.0 * sigma2 + 1.0).sqrt() - 1.0) / 2.0;
    let l = l_val.floor();
    let l1 = l + 1.0;
    let a_num = (2.0 * l + 1.0) * (l * l1 - 3.0 * sigma2);
    let a_den = 6.0 * (sigma2 - l1 * l1);
    let a = if a_den.abs() > 1e-10 {
        a_num / a_den
    } else {
        0.0
    };
    // Assign back to f32 (PIL: result is float)
    let blur_radius = (l + a) as f32;
    pil_box_blur(img, blur_radius, 3)
}

/// Execute a box blur with integer radius.
pub fn execute_box_blur(img: &DynamicImage, radius: u32) -> Result<DynamicImage, PilError> {
    let r = radius as i32;
    if r <= 0 {
        return Ok(img.clone());
    }
    let channels = img.color().channel_count() as usize;
    let raw = img.as_bytes();
    let (w, h) = (img.width(), img.height());
    let window = (2 * r + 1) as u32;
    let ww: u32 = ((1u64 << 24) / window as u64) as u32;
    let bias: u32 = 1u32 << 23;

    let mut hpass = vec![0u8; (w * h) as usize * channels];
    for y in 0..h {
        for x in 0..w {
            for c in 0..channels {
                let mut acc: u64 = 0;
                for dx in -r..=r {
                    let sx = (x as i32 + dx).clamp(0, w as i32 - 1) as u32;
                    let idx = (y * w + sx) as usize * channels + c;
                    acc += raw[idx] as u64;
                }
                hpass[(y * w + x) as usize * channels + c] =
                    ((acc * ww as u64 + bias as u64) >> 24) as u8;
            }
        }
    }
    let mut out = CheckedDims::new(w, h, channels as u8)?.alloc_buffer();
    for y in 0..h {
        for x in 0..w {
            for c in 0..channels {
                let mut acc: u64 = 0;
                for dy in -r..=r {
                    let sy = (y as i32 + dy).clamp(0, h as i32 - 1) as u32;
                    let idx = (sy * w + x) as usize * channels + c;
                    acc += hpass[idx] as u64;
                }
                out[(y * w + x) as usize * channels + c] =
                    ((acc * ww as u64 + bias as u64) >> 24) as u8;
            }
        }
    }
    let result = raw_bytes_to_image(w, h, out, channels)?;
    Ok(preserve_mode(img, result))
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

#[cfg(test)]
mod tests {
    use super::{execute_filter3x3, execute_filter5x5};
    use image_slash_star::{DynamicImage, GrayImage};

    #[test]
    fn detail_uses_pillow_fused_row_evaluation() {
        // Pillow 12.2.0 returns 90 for this exact first-divergence
        // neighborhood; separately rounded products return 89.
        let pixels = vec![95, 95, 96, 96, 96, 106, 126, 126, 137];
        let image = DynamicImage::ImageLuma8(
            GrayImage::from_raw(3, 3, pixels).expect("3x3 fixture dimensions must match"),
        );
        let kernel = [0.0, -1.0, 0.0, -1.0, 10.0, -1.0, 0.0, -1.0, 0.0];

        let filtered =
            execute_filter3x3(&image, &kernel, 6.0, 0, None).expect("filter must succeed");

        assert_eq!(filtered.to_luma8().get_pixel(1, 1)[0], 90);
    }

    #[test]
    fn smooth_more_uses_pillow_fused_row_evaluation() {
        // Pillow 12.2.0 returns 93 for this exact first-divergence
        // neighborhood; separately rounded products return 94.
        let pixels = vec![
            91, 92, 92, 92, 93, 92, 92, 93, 93, 93, 93, 93, 93, 94, 94, 93, 94, 94, 94, 95, 94, 94,
            95, 105, 115,
        ];
        let image = DynamicImage::ImageLuma8(
            GrayImage::from_raw(5, 5, pixels).expect("5x5 fixture dimensions must match"),
        );
        let kernel = [
            1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 5.0, 5.0, 5.0, 1.0, 1.0, 5.0, 44.0, 5.0, 1.0, 1.0, 5.0,
            5.0, 5.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
        ];

        let filtered =
            execute_filter5x5(&image, &kernel, 100.0, 0, None).expect("filter must succeed");

        assert_eq!(filtered.to_luma8().get_pixel(2, 2)[0], 93);
    }
}
