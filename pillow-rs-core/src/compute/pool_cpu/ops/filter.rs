//! Filter operations extracted from image.rs execute_op().
//!
//! These functions are standalone implementations of PIL-compatible filter
//! operations (Filter3x3, Filter5x5, GaussianBlur, BoxBlur, MedianFilter,
//! MaxFilter, MinFilter, RankFilter) that operate on DynamicImage and return
//! new DynamicImage instances.

use image::DynamicImage;

use crate::error::PilError;
use crate::image::preserve_mode;

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
            image::GrayImage::from_raw(w, h, data)
                .ok_or_else(|| PilError::ValueError("raw_bytes_to_image: buffer error".into()))?,
        )),
        2 => Ok(DynamicImage::ImageLumaA8(
            image::GrayAlphaImage::from_raw(w, h, data)
                .ok_or_else(|| PilError::ValueError("raw_bytes_to_image: buffer error".into()))?,
        )),
        3 => Ok(DynamicImage::ImageRgb8(
            image::RgbImage::from_raw(w, h, data)
                .ok_or_else(|| PilError::ValueError("raw_bytes_to_image: buffer error".into()))?,
        )),
        4 => Ok(DynamicImage::ImageRgba8(
            image::RgbaImage::from_raw(w, h, data)
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
/// PIL applies the full kernel convolution with floating-point arithmetic,
/// then rounds to the nearest integer — NO clipping to [0,255].
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
    // PIL rounding: (INT32)(sum / scale + offset + 0.5)
    let rounding_bias = offset as f32 + 0.5;

    let mut out = raw.clone();

    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let base = |dx: i32, dy: i32| -> usize { ((y + dy) * w + (x + dx)) as usize * 4 };
            let v00 = i32::from_le_bytes([
                raw[base(-1, -1)],
                raw[base(-1, -1) + 1],
                raw[base(-1, -1) + 2],
                raw[base(-1, -1) + 3],
            ]);
            let v10 = i32::from_le_bytes([
                raw[base(0, -1)],
                raw[base(0, -1) + 1],
                raw[base(0, -1) + 2],
                raw[base(0, -1) + 3],
            ]);
            let v20 = i32::from_le_bytes([
                raw[base(1, -1)],
                raw[base(1, -1) + 1],
                raw[base(1, -1) + 2],
                raw[base(1, -1) + 3],
            ]);
            let v01 = i32::from_le_bytes([
                raw[base(-1, 0)],
                raw[base(-1, 0) + 1],
                raw[base(-1, 0) + 2],
                raw[base(-1, 0) + 3],
            ]);
            let v11 = i32::from_le_bytes([
                raw[base(0, 0)],
                raw[base(0, 0) + 1],
                raw[base(0, 0) + 2],
                raw[base(0, 0) + 3],
            ]);
            let v21 = i32::from_le_bytes([
                raw[base(1, 0)],
                raw[base(1, 0) + 1],
                raw[base(1, 0) + 2],
                raw[base(1, 0) + 3],
            ]);
            let v02 = i32::from_le_bytes([
                raw[base(-1, 1)],
                raw[base(-1, 1) + 1],
                raw[base(-1, 1) + 2],
                raw[base(-1, 1) + 3],
            ]);
            let v12 = i32::from_le_bytes([
                raw[base(0, 1)],
                raw[base(0, 1) + 1],
                raw[base(0, 1) + 2],
                raw[base(0, 1) + 3],
            ]);
            let v22 = i32::from_le_bytes([
                raw[base(1, 1)],
                raw[base(1, 1) + 1],
                raw[base(1, 1) + 2],
                raw[base(1, 1) + 3],
            ]);

            let ss = rounding_bias
                + v00 as f32 * k0
                + v10 as f32 * k1
                + v20 as f32 * k2
                + v01 as f32 * k3
                + v11 as f32 * k4
                + v21 as f32 * k5
                + v02 as f32 * k6
                + v12 as f32 * k7
                + v22 as f32 * k8;

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
        image::RgbaImage::from_raw(w_u32, h_u32, out)
            .ok_or_else(|| PilError::ValueError("filter_3x3_i32: buffer error".into()))?,
    ))
}

// ── 5x5 filter (I-mode) ──

/// Apply a 5x5 kernel filter on I-mode (32-bit signed integer) data.
/// Same approach as filter_3x3_i32 — no clipping to [0,255].
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
    // Pre-compute normalized kernel coefficients
    let kn: [f32; 25] = std::array::from_fn(|i| kernel[i] / s);

    let rounding_bias = offset as f32 + 0.5;
    let mut out = raw.clone();

    for y in 2..h - 2 {
        for x in 2..w - 2 {
            let base = |dx: i32, dy: i32| -> usize { ((y + dy) * w + (x + dx)) as usize * 4 };

            let mut ss = rounding_bias;
            for dy in -2..=2 {
                for dx in -2..=2 {
                    let bi = base(dx, dy);
                    let val = i32::from_le_bytes([raw[bi], raw[bi + 1], raw[bi + 2], raw[bi + 3]]);
                    let ki = ((dy + 2) * 5 + (dx + 2)) as usize;
                    ss += val as f32 * kn[ki];
                }
            }

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
        image::RgbaImage::from_raw(w_u32, h_u32, out)
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
        let mut hpass = vec![0u8; (w * h) as usize * channels];
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
        let mut vpass = vec![0u8; (w * h) as usize * channels];
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
fn rank_filter_impl(img: &DynamicImage, size: u32, rank: u32) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let raw = img.as_bytes();
    let (w_u32, h_u32) = (img.width(), img.height());
    let (w, h) = (w_u32 as i32, h_u32 as i32);
    let half = (size / 2) as i32;
    let area = (size * size) as usize;
    let rank = rank.min((area - 1) as u32) as usize;

    let mut out = vec![0u8; (w * h) as usize * channels];

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
    let k0 = kernel[0] / scale;
    let k1 = kernel[1] / scale;
    let k2 = kernel[2] / scale;
    let k3 = kernel[3] / scale;
    let k4 = kernel[4] / scale;
    let k5 = kernel[5] / scale;
    let k6 = kernel[6] / scale;
    let k7 = kernel[7] / scale;
    let k8 = kernel[8] / scale;
    let rounding_bias = offset as f32 + 0.5;
    let mut out = raw.to_vec();
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let base = |dx: i32, dy: i32| -> usize {
                ((y + dy) * w + (x + dx)) as usize * channels
            };
            for c in 0..channels {
                let row_b = raw[base(-1, 1) + c] as f32 * k0
                    + raw[base(0, 1) + c] as f32 * k1
                    + raw[base(1, 1) + c] as f32 * k2;
                let row_c = raw[base(-1, 0) + c] as f32 * k3
                    + raw[base(0, 0) + c] as f32 * k4
                    + raw[base(1, 0) + c] as f32 * k5;
                let row_t = raw[base(-1, -1) + c] as f32 * k6
                    + raw[base(0, -1) + c] as f32 * k7
                    + raw[base(1, -1) + c] as f32 * k8;
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
    let k00 = kernel[0] / scale;
    let k01 = kernel[1] / scale;
    let k02 = kernel[2] / scale;
    let k03 = kernel[3] / scale;
    let k04 = kernel[4] / scale;
    let k10 = kernel[5] / scale;
    let k11 = kernel[6] / scale;
    let k12 = kernel[7] / scale;
    let k13 = kernel[8] / scale;
    let k14 = kernel[9] / scale;
    let k20 = kernel[10] / scale;
    let k21 = kernel[11] / scale;
    let k22 = kernel[12] / scale;
    let k23 = kernel[13] / scale;
    let k24 = kernel[14] / scale;
    let k30 = kernel[15] / scale;
    let k31 = kernel[16] / scale;
    let k32 = kernel[17] / scale;
    let k33 = kernel[18] / scale;
    let k34 = kernel[19] / scale;
    let k40 = kernel[20] / scale;
    let k41 = kernel[21] / scale;
    let k42 = kernel[22] / scale;
    let k43 = kernel[23] / scale;
    let k44 = kernel[24] / scale;
    let rounding_bias = offset as f32 + 0.5;
    let mut out = raw.to_vec();
    for y in 2..h - 2 {
        for x in 2..w - 2 {
            let base = |dx: i32, dy: i32| -> usize {
                ((y + dy) * w + (x + dx)) as usize * channels
            };
            for c in 0..channels {
                let row0 = raw[base(-2, 2) + c] as f32 * k00
                    + raw[base(-1, 2) + c] as f32 * k01
                    + raw[base(0, 2) + c] as f32 * k02
                    + raw[base(1, 2) + c] as f32 * k03
                    + raw[base(2, 2) + c] as f32 * k04;
                let mut ss = rounding_bias;
                ss += row0;
                let row1 = raw[base(-2, 1) + c] as f32 * k10
                    + raw[base(-1, 1) + c] as f32 * k11
                    + raw[base(0, 1) + c] as f32 * k12
                    + raw[base(1, 1) + c] as f32 * k13
                    + raw[base(2, 1) + c] as f32 * k14;
                ss += row1;
                let row2 = raw[base(-2, 0) + c] as f32 * k20
                    + raw[base(-1, 0) + c] as f32 * k21
                    + raw[base(0, 0) + c] as f32 * k22
                    + raw[base(1, 0) + c] as f32 * k23
                    + raw[base(2, 0) + c] as f32 * k24;
                ss += row2;
                let row3 = raw[base(-2, -1) + c] as f32 * k30
                    + raw[base(-1, -1) + c] as f32 * k31
                    + raw[base(0, -1) + c] as f32 * k32
                    + raw[base(1, -1) + c] as f32 * k33
                    + raw[base(2, -1) + c] as f32 * k34;
                ss += row3;
                let row4 = raw[base(-2, -2) + c] as f32 * k40
                    + raw[base(-1, -2) + c] as f32 * k41
                    + raw[base(0, -2) + c] as f32 * k42
                    + raw[base(1, -2) + c] as f32 * k43
                    + raw[base(2, -2) + c] as f32 * k44;
                ss += row4;
                out[(y * w + x) as usize * channels + c] = clip8_filter(ss);
            }
        }
    }
    let result = raw_bytes_to_image(w_u32, h_u32, out, channels)?;
    Ok(preserve_mode(img, result))
}

/// Execute a Gaussian blur via 3 passes of PIL-style box blur.
pub fn execute_gaussian_blur(
    img: &DynamicImage,
    sigma: f32,
) -> Result<DynamicImage, PilError> {
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
pub fn execute_box_blur(
    img: &DynamicImage,
    radius: u32,
) -> Result<DynamicImage, PilError> {
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
    let mut out = vec![0u8; (w * h) as usize * channels];
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

/// Execute a median filter (rank = size*size/2).
pub fn execute_median_filter(
    img: &DynamicImage,
    size: u32,
) -> Result<DynamicImage, PilError> {
    rank_filter_impl(img, size, size * size / 2)
}

/// Execute a max filter (rank = size*size - 1).
pub fn execute_max_filter(
    img: &DynamicImage,
    size: u32,
) -> Result<DynamicImage, PilError> {
    rank_filter_impl(img, size, size * size - 1)
}

/// Execute a min filter (rank = 0).
pub fn execute_min_filter(
    img: &DynamicImage,
    size: u32,
) -> Result<DynamicImage, PilError> {
    rank_filter_impl(img, size, 0)
}

/// Execute a rank filter (rank = given rank).
pub fn execute_rank_filter(
    img: &DynamicImage,
    size: u32,
    rank: u32,
) -> Result<DynamicImage, PilError> {
    rank_filter_impl(img, size, rank)
}
