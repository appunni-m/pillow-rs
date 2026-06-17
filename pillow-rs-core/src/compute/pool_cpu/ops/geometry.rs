//! Geometry operations extracted from image.rs execute_op().
//!
//! These functions are standalone implementations of PIL-compatible geometry
//! operations (Resize, Crop, Rotate, Transpose, Thumbnail, Reduce) that operate
//! on DynamicImage and return new DynamicImage instances.

use image::{DynamicImage, GenericImageView};
use std::f64;

use crate::error::PilError;
use crate::image::preserve_mode;
use crate::ops::pil_resize::{pil_resize, precompute_coeffs, precompute_coeffs_float, FilterCoeffs};
use crate::pipeline::{ResampleFilter, TransposeMethod};

// ── Resample filter conversion ──

#[allow(dead_code)]
/// Convert ResampleFilter to image crate's FilterType.
fn to_image_filter(f: &ResampleFilter) -> image::imageops::FilterType {
    match f {
        ResampleFilter::Nearest => image::imageops::FilterType::Nearest,
        ResampleFilter::Bilinear => image::imageops::FilterType::Triangle,
        ResampleFilter::Bicubic => image::imageops::FilterType::CatmullRom,
        ResampleFilter::Lanczos => image::imageops::FilterType::Lanczos3,
        ResampleFilter::Box => image::imageops::FilterType::Gaussian,
        ResampleFilter::Hamming => image::imageops::FilterType::Lanczos3,
    }
}

// ── PIL-compatible filter kernels (f64 precision) ──

/// Box / Nearest-neighbor kernel.
fn f_kernel_box(x: f64) -> f64 {
    if x.abs() < 0.5 {
        1.0
    } else {
        0.0
    }
}

/// Triangle (bilinear) kernel.
fn f_kernel_triangle(x: f64) -> f64 {
    let a = x.abs();
    if a < 1.0 {
        1.0 - a
    } else {
        0.0
    }
}

/// Catmull-Rom (bicubic) kernel.
fn f_kernel_catrom(x: f64) -> f64 {
    let a = x.abs();
    if a < 1.0 {
        1.5 * a.powi(3) - 2.5 * a.powi(2) + 1.0
    } else if a < 2.0 {
        -0.5 * a.powi(3) + 2.5 * a.powi(2) - 4.0 * a + 2.0
    } else {
        0.0
    }
}

/// Lanczos kernel with window `a`.
fn f_kernel_lanczos(x: f64, a: f64) -> f64 {
    if x.abs() >= a {
        return 0.0;
    }
    if x.abs() < 1e-10 {
        return 1.0;
    }
    let pix = std::f64::consts::PI * x;
    let sa = pix.sin() / pix;
    let s = (std::f64::consts::PI * x / a).sin() / (std::f64::consts::PI * x / a);
    sa * s
}

/// Hamming kernel.
fn f_kernel_hamming(x: f64) -> f64 {
    if x.abs() >= 1.0 {
        0.0
    } else {
        0.54 + 0.46 * (std::f64::consts::PI * x).cos()
    }
}

fn f_kernel_lanczos3(x: f64) -> f64 {
    f_kernel_lanczos(x, 3.0)
}

fn resample_kernel(filter: &ResampleFilter) -> (fn(f64) -> f64, f64) {
    match filter {
        ResampleFilter::Nearest => (f_kernel_box, 0.5),
        ResampleFilter::Bilinear => (f_kernel_triangle, 1.0),
        ResampleFilter::Bicubic => (f_kernel_catrom, 2.0),
        ResampleFilter::Lanczos => (f_kernel_lanczos3, 3.0),
        ResampleFilter::Box => (f_kernel_box, 0.5),
        ResampleFilter::Hamming => (f_kernel_hamming, 1.0),
    }
}

// ── Helpers ──

/// Clamp an integer to [0, max).
fn clamp_idx(v: i64, max: u32) -> u32 {
    if v < 0 {
        0
    } else if v as u32 >= max {
        max - 1
    } else {
        v as u32
    }
}

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

// ── F-mode / I-mode resize ──

/// Resize an F-mode image (32-bit floats stored as RGBA8 bytes).
/// Uses PIL-compatible direct 2D interpolation with f64 precision,
/// so the result matches PIL's Image.resize() on mode F images.
fn resize_f(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    filter: &ResampleFilter,
) -> Result<DynamicImage, PilError> {
    let rgba = img.to_rgba8();
    let (sw, sh) = rgba.dimensions();

    if dst_w == 0 || dst_h == 0 || sw == 0 || sh == 0 {
        return Ok(DynamicImage::new_rgba8(dst_w, dst_h));
    }
    if (dst_w, dst_h) == (sw, sh) {
        return Ok(img.clone());
    }

    // Reinterpret each 4 RGBA bytes as a f32 (little-endian).
    let src_floats: Vec<f32> = rgba
        .pixels()
        .map(|p| f32::from_le_bytes([p[0], p[1], p[2], p[3]]))
        .collect();

    let (kernel, support) = resample_kernel(filter);
    let sw_f = sw as f64;
    let sh_f = sh as f64;
    let dw_f = dst_w as f64;
    let dh_f = dst_h as f64;

    // PIL-compatible scale factor for kernel widening during downscaling
    let sx_scale = (sw_f / dw_f).max(1.0);
    let sy_scale = (sh_f / dh_f).max(1.0);

    let n = (dst_w * dst_h) as usize;
    let mut out_floats: Vec<f32> = Vec::with_capacity(n);

    // PIL AFFINE transform: sx = (int)((dx + 1) * sw/dw - 0.5)
    if matches!(filter, ResampleFilter::Nearest | ResampleFilter::Box) {
        for dy in 0..dst_h {
            for dx in 0..dst_w {
                let sx = ((dx as f64 + 1.0) * sw_f / dw_f - 0.5) as i64;
                let sy = ((dy as f64 + 1.0) * sh_f / dh_f - 0.5) as i64;
                let sx = clamp_idx(sx, sw);
                let sy = clamp_idx(sy, sh);
                let idx = (sy * sw + sx) as usize;
                out_floats.push(src_floats[idx]);
            }
        }
    } else {
        for dy in 0..dst_h {
            for dx in 0..dst_w {
                // PIL exact: center = (xx + 0.5) * scale [no -0.5]
                // filterscale = max(1.0, scale)
                // xmin = (int)(center - support*filterscale + 0.5) [round to nearest]
                // weight = kernel((sx + 0.5 - center) / filterscale)
                let cx = (dx as f64 + 0.5) * sw_f / dw_f;
                let cy = (dy as f64 + 0.5) * sh_f / dh_f;

                let left = (cx - support * sx_scale + 0.5).trunc() as i64;
                let right = (cx + support * sx_scale + 0.5).trunc() as i64;
                let top = (cy - support * sy_scale + 0.5).trunc() as i64;
                let bottom = (cy + support * sy_scale + 0.5).trunc() as i64;

                let mut acc = 0.0f64;
                let mut wsum = 0.0f64;

                for iy in top..bottom {
                    let sy = clamp_idx(iy, sh);
                    // PIL: weight = kernel((sx + 0.5 - center) / filterscale)
                    let wy = kernel((iy as f64 + 0.5 - cy) / sy_scale);
                    if wy.abs() < 1e-15 {
                        continue;
                    }
                    for ix in left..right {
                        let sx = clamp_idx(ix, sw);
                        let wx = kernel((ix as f64 + 0.5 - cx) / sx_scale);
                        let w = wx * wy;
                        if w.abs() < 1e-15 {
                            continue;
                        }
                        let idx = (sy * sw + sx) as usize;
                        let val = src_floats[idx] as f64;
                        acc += w * val;
                        wsum += w;
                    }
                }

                let out_val = if wsum > 0.0 {
                    (acc / wsum) as f32
                } else {
                    // fallback: nearest pixel
                    let sx = clamp_idx(cx.floor() as i64, sw);
                    let sy = clamp_idx(cy.floor() as i64, sh);
                    src_floats[(sy * sw + sx) as usize]
                };

                out_floats.push(out_val);
            }
        }
    }

    // Re-pack each f32 as 4 RGBA8 bytes (little-endian).
    let rgba_bytes: Vec<u8> = out_floats.iter().flat_map(|f| f.to_le_bytes()).collect();
    let out = image::RgbaImage::from_raw(dst_w, dst_h, rgba_bytes)
        .ok_or_else(|| PilError::ValueError("resize_f: failed to create output buffer".into()))?;
    Ok(DynamicImage::ImageRgba8(out))
}

/// Resize an I-mode image (32-bit signed integers stored as RGBA8 bytes LE).
/// Uses PIL's two-pass separable approach matching ImagingResample.
fn resize_i(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    filter: &ResampleFilter,
) -> Result<DynamicImage, PilError> {
    let rgba = img.to_rgba8();
    let (sw, sh) = rgba.dimensions();

    if dst_w == 0 || dst_h == 0 || sw == 0 || sh == 0 {
        return Ok(DynamicImage::new_rgba8(dst_w, dst_h));
    }
    if (dst_w, dst_h) == (sw, sh) {
        return Ok(img.clone());
    }

    // Reinterpret each 4 RGBA bytes as i32 (little-endian).
    let src_ints: Vec<i32> = rgba
        .pixels()
        .map(|p| i32::from_le_bytes([p[0], p[1], p[2], p[3]]))
        .collect();

    let (kernel, support) = resample_kernel(filter);
    let sw_f = sw as f64;
    let sh_f = sh as f64;
    let dw_f = dst_w as f64;
    let dh_f = dst_h as f64;

    // PIL-compatible scale factor for kernel widening during downscaling
    let sx_scale = (sw_f / dw_f).max(1.0);
    let sy_scale = (sh_f / dh_f).max(1.0);

    let n = (dst_w * dst_h) as usize;

    // NEAREST: PIL uses ImagingTransform (AFFINE) with formula:
    //   sx = (int)((dx + 1.0) * sw/dw - 0.5)
    //   sy = (int)((dy + 1.0) * sh/dh - 0.5)
    if matches!(filter, ResampleFilter::Nearest) {
        let mut out_ints: Vec<i32> = Vec::with_capacity(n);
        for dy in 0..dst_h {
            for dx in 0..dst_w {
                let sx = ((dx as f64 + 1.0) * sw_f / dw_f - 0.5).floor() as i64;
                let sy = ((dy as f64 + 1.0) * sh_f / dh_f - 0.5).floor() as i64;
                let sx = sx.clamp(0, sw as i64 - 1) as u32;
                let sy = sy.clamp(0, sh as i64 - 1) as u32;
                let idx = (sy * sw + sx) as usize;
                out_ints.push(src_ints[idx]);
            }
        }
        let rgba_bytes: Vec<u8> = out_ints.iter().flat_map(|v| v.to_le_bytes()).collect();
        let out = image::RgbaImage::from_raw(dst_w, dst_h, rgba_bytes)
            .ok_or_else(|| PilError::ValueError("resize_i: failed to create output buffer".into()))?;
        return Ok(DynamicImage::ImageRgba8(out));
    }

    // Two-pass separable approach for non-NEAREST filters (matching PIL's ImagingResample)
    // Use PIL's standard double-precision precomputation for exact weight matching.
    let h_coeffs = precompute_coeffs(dst_w, sw, kernel, support);
    let v_coeffs = precompute_coeffs(dst_h, sh, kernel, support);

    const PRECISION_BITS: i64 = 22;
    const HALF_PRECISION: i64 = 1 << (PRECISION_BITS - 1);

    // Allocate intermediate buffer (sh rows x dw cols) as i32, matching PIL's
    // ImagingResample behavior (horizontal pass rounds to output type).
    let mut intermediate: Vec<i32> = vec![0i32; (sh * dst_w) as usize];

    // Horizontal pass: for each source row, compute each output column's weighted sum,
    // round to i32 (matching PIL's intermediate quantization with fixed-point weights).
    for sy in 0..sh {
        let src_row_base = (sy * sw) as usize;
        for dx in 0..dst_w {
            let x0 = h_coeffs.xmin[dx as usize];
            let cnt = h_coeffs.count[dx as usize];
            if cnt == 0 {
                continue;
            }
            let mut acc: i64 = 0;
            for (cix, &w) in h_coeffs.weights[dx as usize].iter().enumerate() {
                let sx = (x0 + cix as i64) as usize;
                acc += w * src_ints[src_row_base + sx] as i64;
            }
            // Round fixed-point sum to i32
            let val = ((acc + HALF_PRECISION) >> PRECISION_BITS) as i32;
            intermediate[(sy * dst_w + dx) as usize] = val;
        }
    }

    // Vertical pass: for each output column, compute each output row's weighted sum,
    // round to i32 (final output).
    let mut out_ints: Vec<i32> = Vec::with_capacity(n);
    for dy in 0..dst_h {
        let y0 = v_coeffs.xmin[dy as usize];
        let cnt = v_coeffs.count[dy as usize];
        if cnt == 0 {
            for _ in 0..dst_w {
                out_ints.push(0i32);
            }
            continue;
        }
        for dx in 0..dst_w {
            let mut acc: i64 = 0;
            for (cix, &w) in v_coeffs.weights[dy as usize].iter().enumerate() {
                let sy = (y0 + cix as i64) as usize;
                acc += w * intermediate[(sy * dst_w as usize) + dx as usize] as i64;
            }
            let out_val = ((acc + HALF_PRECISION) >> PRECISION_BITS) as i32;
            out_ints.push(out_val);
        }
    }

    // Re-pack each i32 as 4 RGBA8 bytes (little-endian).
    let rgba_bytes: Vec<u8> = out_ints.iter().flat_map(|v| v.to_le_bytes()).collect();
    let out = image::RgbaImage::from_raw(dst_w, dst_h, rgba_bytes)
        .ok_or_else(|| PilError::ValueError("resize_i: failed to create output buffer".into()))?;
    Ok(DynamicImage::ImageRgba8(out))
}

// ── Generic rotation & transform helpers (mode-aware) ──

/// Rotate an image by an arbitrary angle, working on the native number of channels.
/// When `nearest` is true, uses nearest-neighbor sampling (required for P, 1, I, F modes).
fn rotate_arbitrary_generic(
    img: &DynamicImage,
    angle: f64,
    expand: bool,
    fill: Option<(u8, u8, u8, u8)>,
    nearest: bool,
) -> DynamicImage {
    let channels = img.color().channel_count() as usize;
    let (w, h) = img.dimensions();
    let sw = w as f64;
    let sh = h as f64;
    let rad = angle.to_radians();
    let (cos, sin) = (rad.cos(), rad.sin());

    // Compute bounding box of rotated image
    let corners = [(0.0, 0.0), (sw, 0.0), (sw, sh), (0.0, sh)];
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(cx, cy) in &corners {
        let rx = cx * cos - cy * sin;
        let ry = cx * sin + cy * cos;
        min_x = min_x.min(rx);
        max_x = max_x.max(rx);
        min_y = min_y.min(ry);
        max_y = max_y.max(ry);
    }
    let (dw, dh) = if expand {
        ((max_x - min_x).ceil() as u32, (max_y - min_y).ceil() as u32)
    } else {
        (w, h)
    };

    let raw = img.as_bytes();
    let fill_color = fill.unwrap_or((0, 0, 0, 0));

    let (ox, oy) = if expand { (-min_x, -min_y) } else { (0.0, 0.0) };
    let cx_src = sw / 2.0;
    let cy_src = sh / 2.0;
    let cx_dst = dw as f64 / 2.0;
    let cy_dst = dh as f64 / 2.0;

    let mut out = vec![0u8; (dw * dh) as usize * channels];

    for dy in 0..dh {
        for dx in 0..dw {
            // Map destination pixel to source coordinate (inverse rotation)
            let sx_rel = (dx as f64 + ox - cx_dst) * cos + (dy as f64 + oy - cy_dst) * sin + cx_src;
            let sy_rel =
                -(dx as f64 + ox - cx_dst) * sin + (dy as f64 + oy - cy_dst) * cos + cy_src;

            let out_idx = (dy * dw + dx) as usize * channels;

            if nearest {
                let ix = (sx_rel + 0.5).floor() as i64;
                let iy = (sy_rel + 0.5).floor() as i64;
                if ix >= 0 && ix < w as i64 && iy >= 0 && iy < h as i64 {
                    let in_idx = (iy as u32 * w + ix as u32) as usize * channels;
                    out[out_idx..out_idx + channels]
                        .copy_from_slice(&raw[in_idx..in_idx + channels]);
                } else {
                    for c in 0..channels.min(4) {
                        out[out_idx + c] = match c {
                            0 => fill_color.0,
                            1 => fill_color.1,
                            2 => fill_color.2,
                            _ => fill_color.3,
                        };
                    }
                }
            } else if sx_rel >= 0.0 && sx_rel < sw && sy_rel >= 0.0 && sy_rel < sh {
                let sx = sx_rel.floor() as u32;
                let sy = sy_rel.floor() as u32;
                let fx = sx_rel - sx as f64;
                let fy = sy_rel - sy as f64;
                let sx1 = (sx + 1).min(w - 1);
                let sy1 = (sy + 1).min(h - 1);
                for c in 0..channels {
                    let p00 = raw[(sy * w + sx) as usize * channels + c] as f64;
                    let p10 = raw[(sy * w + sx1) as usize * channels + c] as f64;
                    let p01 = raw[(sy1 * w + sx) as usize * channels + c] as f64;
                    let p11 = raw[(sy1 * w + sx1) as usize * channels + c] as f64;
                    let v = (1.0 - fx) * (1.0 - fy) * p00
                        + fx * (1.0 - fy) * p10
                        + (1.0 - fx) * fy * p01
                        + fx * fy * p11;
                    out[out_idx + c] = v.round() as u8;
                }
            } else {
                for c in 0..channels.min(4) {
                    out[out_idx + c] = match c {
                        0 => fill_color.0,
                        1 => fill_color.1,
                        2 => fill_color.2,
                        _ => fill_color.3,
                    };
                }
            }
        }
    }

    match channels {
        1 => DynamicImage::ImageLuma8(
            image::GrayImage::from_raw(dw, dh, out)
                .expect("rotate_arbitrary: buffer size mismatch"),
        ),
        2 => DynamicImage::ImageLumaA8(
            image::GrayAlphaImage::from_raw(dw, dh, out)
                .expect("rotate_arbitrary: buffer size mismatch"),
        ),
        3 => DynamicImage::ImageRgb8(
            image::RgbImage::from_raw(dw, dh, out).expect("rotate_arbitrary: buffer size mismatch"),
        ),
        4 => DynamicImage::ImageRgba8(
            image::RgbaImage::from_raw(dw, dh, out)
                .expect("rotate_arbitrary: buffer size mismatch"),
        ),
        _ => unreachable!(),
    }
}

#[allow(dead_code)]
/// Apply an affine transform working on the native number of channels.
/// When `nearest` is true, uses nearest-neighbor sampling.
fn transform_affine_generic(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    aff_a: f64,
    aff_b: f64,
    aff_c: f64,
    aff_d: f64,
    aff_e: f64,
    aff_f: f64,
    fill: Option<(u8, u8, u8, u8)>,
    nearest: bool,
) -> DynamicImage {
    let channels = img.color().channel_count() as usize;
    let raw = img.as_bytes();
    let (sw, sh) = img.dimensions();
    let fill_color = fill.unwrap_or((0, 0, 0, 255));

    let mut out = vec![0u8; (dst_w * dst_h) as usize * channels];

    for dy in 0..dst_h {
        for dx in 0..dst_w {
            let sx = aff_a * dx as f64 + aff_b * dy as f64 + aff_c;
            let sy = aff_d * dx as f64 + aff_e * dy as f64 + aff_f;
            let out_idx = (dy * dst_w + dx) as usize * channels;

            if nearest {
                let ix = (sx + 0.5).floor() as i64;
                let iy = (sy + 0.5).floor() as i64;
                if ix >= 0 && ix < sw as i64 && iy >= 0 && iy < sh as i64 {
                    let in_idx = (iy as u32 * sw + ix as u32) as usize * channels;
                    out[out_idx..out_idx + channels]
                        .copy_from_slice(&raw[in_idx..in_idx + channels]);
                } else {
                    for ch in 0..channels.min(4) {
                        out[out_idx + ch] = match ch {
                            0 => fill_color.0,
                            1 => fill_color.1,
                            2 => fill_color.2,
                            _ => fill_color.3,
                        };
                    }
                }
            } else if sx >= 0.0 && sx < sw as f64 && sy >= 0.0 && sy < sh as f64 {
                let x0 = sx.floor() as u32;
                let y0 = sy.floor() as u32;
                let x1 = (x0 + 1).min(sw - 1);
                let y1 = (y0 + 1).min(sh - 1);
                let fx = sx - x0 as f64;
                let fy = sy - y0 as f64;
                for ch in 0..channels {
                    let p00 = raw[(y0 * sw + x0) as usize * channels + ch] as f64;
                    let p10 = raw[(y0 * sw + x1) as usize * channels + ch] as f64;
                    let p01 = raw[(y1 * sw + x0) as usize * channels + ch] as f64;
                    let p11 = raw[(y1 * sw + x1) as usize * channels + ch] as f64;
                    let v = (1.0 - fx) * (1.0 - fy) * p00
                        + fx * (1.0 - fy) * p10
                        + (1.0 - fx) * fy * p01
                        + fx * fy * p11;
                    out[out_idx + ch] = v.round() as u8;
                }
            } else {
                for ch in 0..channels.min(4) {
                    out[out_idx + ch] = match ch {
                        0 => fill_color.0,
                        1 => fill_color.1,
                        2 => fill_color.2,
                        _ => fill_color.3,
                    };
                }
            }
        }
    }

    match channels {
        1 => DynamicImage::ImageLuma8(
            image::GrayImage::from_raw(dst_w, dst_h, out)
                .expect("transform_affine: buffer size mismatch"),
        ),
        2 => DynamicImage::ImageLumaA8(
            image::GrayAlphaImage::from_raw(dst_w, dst_h, out)
                .expect("transform_affine: buffer size mismatch"),
        ),
        3 => DynamicImage::ImageRgb8(
            image::RgbImage::from_raw(dst_w, dst_h, out)
                .expect("transform_affine: buffer size mismatch"),
        ),
        4 => DynamicImage::ImageRgba8(
            image::RgbaImage::from_raw(dst_w, dst_h, out)
                .expect("transform_affine: buffer size mismatch"),
        ),
        _ => unreachable!(),
    }
}

// ── Execute geometry ops ──

/// Execute a Resize operation.
/// F-mode uses float interpolation; I-mode uses int32 interpolation;
/// all other modes use the standard PIL-compatible two-pass resize.
pub fn execute_resize(
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: &ResampleFilter,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if explicit_mode == Some("F") {
        return resize_f(img, w, h, filter);
    }
    if explicit_mode == Some("I") {
        return resize_i(img, w, h, filter);
    }
    // Mode "1": convert to L, resize, then convert back to "1" by thresholding at 128.
    // PIL's C extension handles mode "1" internally with bit-unpacking, but our
    // pil_resize works on Luma8 which has equivalent data. The two-pass BOX filter
    // (NEAREST) produces averages; the conversion back to "1" thresholds them.
    if explicit_mode == Some("1") {
        // Image is already Luma8 with {0,255}. Resize via pil_resize (which uses
        // the BOX filter for NEAREST, matching PIL's behavior for mode "1").
        let result = pil_resize(img, w, h, *filter, explicit_mode);
        // After resize, threshold back to binary {0, 255}: pixel >= 128 => 255 else 0
        let gray = result.to_luma8();
        let (rw, rh) = gray.dimensions();
        let mut out = image::GrayImage::new(rw, rh);
        for (op, ip) in out.pixels_mut().zip(gray.pixels()) {
            op[0] = if ip[0] >= 128 { 255 } else { 0 };
        }
        return Ok(preserve_mode(img, DynamicImage::ImageLuma8(out)));
    }
    let result = pil_resize(img, w, h, *filter, explicit_mode);
    Ok(preserve_mode(img, result))
}

/// Execute a Crop operation.
pub fn execute_crop(
    img: &DynamicImage,
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
) -> Result<DynamicImage, PilError> {
    let w = right.saturating_sub(left);
    let h = bottom.saturating_sub(top);
    Ok(img.crop_imm(left, top, w, h))
}

/// Execute a Rotate operation.
/// Fast-path for 90-degree multiples; otherwise uses arbitrary rotation.
pub fn execute_rotate(
    img: &DynamicImage,
    angle: f64,
    expand: bool,
    fill: Option<(u8, u8, u8, u8)>,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let deg = (angle.round() as i32).rem_euclid(360);
    // Fast path: exact 90-degree multiples
    // PIL rotates counterclockwise; image crate rotates clockwise.
    // PIL 90° CCW = image crate 270° CW, PIL 270° CCW = image crate 90° CW.
    let result = if (deg - 90).abs() < 2 || (deg - 90).abs() >= 358 {
        img.rotate270() // 270° CW = 90° CCW (PIL)
    } else if (deg - 180).abs() < 2 {
        img.rotate180()
    } else if (deg - 270).abs() < 2 || (deg - 270).abs() >= 358 {
        img.rotate90() // 90° CW = 270° CCW (PIL)
    } else {
        // Multi-channel arbitrary rotation (no RGBA roundtrip)
        let nearest = explicit_mode == Some("P")
            || explicit_mode == Some("1")
            || explicit_mode == Some("I")
            || explicit_mode == Some("F");
        rotate_arbitrary_generic(img, angle, expand, fill, nearest)
    };
    Ok(preserve_mode(img, result))
}

/// Execute a Transpose operation.
pub fn execute_transpose(
    img: &DynamicImage,
    method: &TransposeMethod,
) -> Result<DynamicImage, PilError> {
    match method {
        TransposeMethod::FlipLeftRight => Ok(img.fliph()),
        TransposeMethod::FlipTopBottom => Ok(img.flipv()),
        // PIL rotates counter-clockwise; image crate rotates clockwise.
        // PIL ROTATE_90 (CCW) = image crate rotate270 (CW)
        // PIL ROTATE_270 (CCW) = image crate rotate90 (CW)
        TransposeMethod::Rotate90 => Ok(img.rotate270()),
        TransposeMethod::Rotate180 => Ok(img.rotate180()),
        TransposeMethod::Rotate270 => Ok(img.rotate90()),
        // PIL TRANSPOSE = ROTATE_90 (CCW) then FLIP_LEFT_RIGHT
        // With corrected ROTATE_90 = rotate270:
        //   TRANSPOSE = rotate270().fliph()
        // PIL TRANSVERSE = ROTATE_270 (CCW) then FLIP_LEFT_RIGHT
        // With corrected ROTATE_270 = rotate90:
        //   TRANSVERSE = rotate90().fliph()
        TransposeMethod::Transpose => Ok(img.rotate270().fliph()),
        TransposeMethod::Transverse => Ok(img.rotate90().fliph()),
    }
}

/// Execute a Thumbnail operation.
/// Computes the scale factor to fit within the given box, preserving aspect ratio.
/// Matches PIL's thumbnail behavior including the reducing_gap optimization
/// (default reducing_gap=2.0) for non-NEAREST filters.
pub fn execute_thumbnail(
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: &ResampleFilter,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (cur_w, cur_h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return Err(PilError::ValueError("thumbnail size must be > 0".into()));
    }
    let scale = (w as f64 / cur_w as f64).min(h as f64 / cur_h as f64);
    let new_w = (cur_w as f64 * scale) as u32;
    let new_h = (cur_h as f64 * scale) as u32;
    let new_w = new_w.max(1);
    let new_h = new_h.max(1);
    // PIL forces NEAREST for mode "1" and "P" to avoid non-binary/interpolated values
    let effective_filter = match explicit_mode {
        Some("1") | Some("P") => ResampleFilter::Nearest,
        _ => *filter,
    };
    // PIL's thumbnail uses reducing_gap=2.0 by default: first integer-reduce
    // by up to scale/reducing_gap, then resize the rest.
    // This matches PIL's ImagingReduce then ImagingResample two-step.
    // Skip reducing_gap for modes with alpha (LA, RGBA) to avoid premultiply issues.
    let needs_reduce = !matches!(effective_filter, ResampleFilter::Nearest)
        && !matches!(img.color(), image::ColorType::La8 | image::ColorType::Rgba8);
    let mut work_img = img.clone();
    if needs_reduce {
        let scale_x = cur_w as f64 / new_w as f64;
        let scale_y = cur_h as f64 / new_h as f64;
        let factor = ((scale_x.max(scale_y)) / 2.0) as u32;
        let factor = factor.max(1);
        if factor > 1 {
            let (rw, rh) = (cur_w / factor, cur_h / factor);
            // Average each factor×factor block per-channel (matching PIL's ImagingReduce)
            let channels = work_img.color().channel_count() as usize;
            let raw = work_img.as_bytes();
            let mut out = vec![0u8; (rw * rh * channels as u32) as usize];
            for y in 0..rh {
                for x in 0..rw {
                    for c in 0..channels {
                        let mut sum = 0u64;
                        for dy in 0..factor {
                            let sy = (y * factor + dy).min(cur_h - 1);
                            for dx in 0..factor {
                                let sx = (x * factor + dx).min(cur_w - 1);
                                let idx = (sy * cur_w + sx) as usize * channels + c;
                                sum += raw[idx] as u64;
                            }
                        }
                        let block_pixels = (factor.min(cur_h - y * factor) * factor.min(cur_w - x * factor)) as u64;
                        let val = ((sum + block_pixels / 2) / block_pixels) as u8;
                        out[(y * rw + x) as usize * channels + c] = val;
                    }
                }
            }
            work_img = raw_to_dynimage(&out, rw, rh, channels);
        }
    }
    let result = match explicit_mode {
        Some("F") => resize_f(&work_img, new_w, new_h, &effective_filter)?,
        Some("I") => resize_i(&work_img, new_w, new_h, &effective_filter)?,
        _ => pil_resize(&work_img, new_w, new_h, effective_filter, explicit_mode),
    };
    Ok(preserve_mode(img, result))
}

/// Helper: convert raw bytes + dimensions to DynamicImage.
fn raw_to_dynimage(bytes: &[u8], w: u32, h: u32, channels: usize) -> DynamicImage {
    match channels {
        1 => DynamicImage::ImageLuma8(
            image::GrayImage::from_raw(w, h, bytes.to_vec())
                .unwrap_or_else(|| image::GrayImage::new(w, h)),
        ),
        2 => DynamicImage::ImageLumaA8(
            image::GrayAlphaImage::from_raw(w, h, bytes.to_vec())
                .unwrap_or_else(|| image::GrayAlphaImage::new(w, h)),
        ),
        3 => DynamicImage::ImageRgb8(
            image::RgbImage::from_raw(w, h, bytes.to_vec())
                .unwrap_or_else(|| image::RgbImage::new(w, h)),
        ),
        _ => DynamicImage::ImageRgba8(
            image::RgbaImage::from_raw(w, h, bytes.to_vec())
                .unwrap_or_else(|| image::RgbaImage::new(w, h)),
        ),
    }
}

/// Execute a Reduce operation.
/// Averages each factor×factor block per-channel, preserving mode.
pub fn execute_reduce(img: &DynamicImage, factor: u32) -> Result<DynamicImage, PilError> {
    if factor < 2 {
        return Ok(img.clone());
    }
    let f = factor;
    // PIL reduce: average each factor×factor block per-channel, preserving mode
    let channels = img.color().channel_count() as usize;
    let (w, h) = (img.width(), img.height());
    let new_w = w / f;
    let new_h = h / f;
    let raw = img.as_bytes().to_vec();
    let mut out = vec![0u8; (new_w * new_h * channels as u32) as usize];
    for y in 0..new_h {
        for x in 0..new_w {
            let mut sums = vec![0u64; channels];
            let mut count = 0u32;
            for dy in 0..f {
                for dx in 0..f {
                    let px = x * f + dx;
                    let py = y * f + dy;
                    if px < w && py < h {
                        let src_idx = (py * w + px) as usize * channels;
                        for c in 0..channels {
                            sums[c] += raw[src_idx + c] as u64;
                        }
                        count += 1;
                    }
                }
            }
            if count > 0 {
                let half = count as u64 / 2;
                let dst_idx = (y * new_w + x) as usize * channels;
                for c in 0..channels {
                    out[dst_idx + c] = ((sums[c] + half) / count as u64) as u8;
                }
            }
        }
    }
    let result = raw_bytes_to_image(new_w, new_h, out, channels)?;
    Ok(result)
}
