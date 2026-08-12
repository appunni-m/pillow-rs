//! Filter operations extracted from image.rs execute_op().
//!
//! These functions are standalone implementations of PIL-compatible filter
//! operations (Filter3x3, Filter5x5, GaussianBlur, BoxBlur, MedianFilter,
//! MaxFilter, MinFilter, RankFilter) that operate on DynamicImage and return
//! new DynamicImage instances.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::raster::DynamicImage;

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
            crate::raster::GrayImage::from_raw(w, h, data)
                .ok_or_else(|| PilError::ValueError("raw_bytes_to_image: buffer error".into()))?,
        )),
        2 => Ok(DynamicImage::ImageLumaA8(
            crate::raster::GrayAlphaImage::from_raw(w, h, data)
                .ok_or_else(|| PilError::ValueError("raw_bytes_to_image: buffer error".into()))?,
        )),
        3 => Ok(DynamicImage::ImageRgb8(
            crate::raster::RgbImage::from_raw(w, h, data)
                .ok_or_else(|| PilError::ValueError("raw_bytes_to_image: buffer error".into()))?,
        )),
        4 => Ok(DynamicImage::ImageRgba8(
            crate::raster::RgbaImage::from_raw(w, h, data)
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
        crate::raster::RgbaImage::from_raw(w_u32, h_u32, out)
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

    // See the 3x3 path: public kernel construction guarantees a positive
    // scale, and built-in 5x5 filters use positive scales as well.
    let s = scale;
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
        crate::raster::RgbaImage::from_raw(w_u32, h_u32, out)
            .ok_or_else(|| PilError::ValueError("filter_5x5_i32: buffer error".into()))?,
    ))
}

// ── PIL-style box blur ──

const BOX_BLUR_SCALE: u32 = 1 << 24;
const BOX_BLUR_BIAS: u32 = 1 << 23;

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
    let mut accumulator = [0u32; 4];
    for row in 0..height {
        blur_line(
            source,
            destination,
            row * row_length,
            width,
            channels,
            radius,
            whole_weight,
            fractional_weight,
            &mut accumulator,
        );
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

/// PIL-style box blur with fractional radius support.
/// Uses sliding-window accumulator with fixed-point (24-bit) arithmetic.
/// Matches PIL order: ALL horizontal passes first, then ALL vertical passes.
fn pil_box_blur(img: &DynamicImage, radius: f32, passes: u32) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let (w_u32, h_u32) = (img.width(), img.height());
    if w_u32 == 0 || h_u32 == 0 || radius == 0.0 {
        return Ok(img.clone());
    }
    let (width, height) = (w_u32 as usize, h_u32 as usize);

    // Integer part of radius (PIL: (int)floatRadius)
    let integer_radius = radius as i32 as usize;
    // Number of pixels in the integer window
    let window_pixels = (2 * integer_radius + 1) as u32;
    // Fixed-point weight: PIL uses f32 precision for ww computation
    // (UINT32)((1 << 24) / (floatRadius * 2 + 1)) — all in f32
    let whole_weight = (BOX_BLUR_SCALE as f32 / (radius * 2.0 + 1.0)) as u32;
    // Fractional edge weight (PIL: fw = ((1 << 24) - window_pixels * ww) / 2)
    let fractional_weight =
        BOX_BLUR_SCALE.wrapping_sub(window_pixels.wrapping_mul(whole_weight)) / 2;

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
            integer_radius,
            whole_weight,
            fractional_weight,
        );
        std::mem::swap(&mut work, &mut scratch);
    }

    // Pillow transposes before its vertical passes. Processing each complete
    // row as one vector is algebraically identical and keeps both sides of the
    // recurrence contiguous without two extra full-image copies.
    let mut vertical_accumulator = vec![0u32; width * channels];
    for _ in 0..passes {
        blur_columns(
            &work,
            &mut scratch,
            width,
            height,
            channels,
            integer_radius,
            whole_weight,
            fractional_weight,
            &mut vertical_accumulator,
        );
        std::mem::swap(&mut work, &mut scratch);
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
            crate::raster::RgbaImage::from_raw(w_u32, h_u32, out)
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
    // The public kernel boundary clamps scales to a positive value before a
    // pipeline operation is created.
    let s = scale;
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
    // The public kernel boundary clamps scales to a positive value before a
    // pipeline operation is created.
    let s = scale;
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
