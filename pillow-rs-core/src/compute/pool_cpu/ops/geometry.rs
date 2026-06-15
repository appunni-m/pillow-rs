//! Geometry operations extracted from image.rs execute_op().
//!
//! These functions are standalone implementations of PIL-compatible geometry
//! operations (Resize, Crop, Rotate, Transpose, Thumbnail, Reduce) that operate
//! on DynamicImage and return new DynamicImage instances.

use image::{DynamicImage, GenericImageView};
use std::f64;

use crate::error::PilError;
use crate::image::preserve_mode;
use crate::ops::pil_resize::pil_resize;
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

    // Handle NEAREST/Box separately: PIL uses floor((dx+0.5)*sw/dw) without -0.5
    if matches!(filter, ResampleFilter::Nearest | ResampleFilter::Box) {
        for dy in 0..dst_h {
            for dx in 0..dst_w {
                let cx = (dx as f64 + 0.5) * sw_f / dw_f;
                let cy = (dy as f64 + 0.5) * sh_f / dh_f;
                let sx = clamp_idx(cx.floor() as i64, sw);
                let sy = clamp_idx(cy.floor() as i64, sh);
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

#[allow(dead_code)]
/// Resize an I-mode image (32-bit signed integers stored as RGBA8 bytes LE).
/// Uses PIL-compatible direct 2D interpolation with f64 precision and i32 rounding.
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
    let mut out_ints: Vec<i32> = Vec::with_capacity(n);

    // Handle NEAREST/Box separately: PIL uses floor((dx+0.5)*sw/dw)
    if matches!(filter, ResampleFilter::Nearest | ResampleFilter::Box) {
        for dy in 0..dst_h {
            for dx in 0..dst_w {
                let cx = (dx as f64 + 0.5) * sw_f / dw_f;
                let cy = (dy as f64 + 0.5) * sh_f / dh_f;
                let sx = clamp_idx(cx.floor() as i64, sw);
                let sy = clamp_idx(cy.floor() as i64, sh);
                let idx = (sy * sw + sx) as usize;
                out_ints.push(src_ints[idx]);
            }
        }
    } else {
        for dy in 0..dst_h {
            for dx in 0..dst_w {
                // PIL exact: center = (xx + 0.5) * scale [no -0.5]
                let cx = (dx as f64 + 0.5) * sw_f / dw_f;
                let cy = (dy as f64 + 0.5) * sh_f / dh_f;

                // PIL: xmin = (int)(center - support*filterscale + 0.5)
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
                    if wy.abs() < 1e-15 { continue; }
                    for ix in left..right {
                        let sx = clamp_idx(ix, sw);
                        let wx = kernel((ix as f64 + 0.5 - cx) / sx_scale);
                        let w = wx * wy;
                        if w.abs() < 1e-15 { continue; }
                        let idx = (sy * sw + sx) as usize;
                        acc += w * src_ints[idx] as f64;
                        wsum += w;
                    }
                }

                // PIL rounds to nearest int32
                let out_val = if wsum > 0.0 {
                    (acc / wsum + 0.5).trunc() as i32
                } else {
                    let sx = clamp_idx(cx.floor() as i64, sw);
                    let sy = clamp_idx(cy.floor() as i64, sh);
                    src_ints[(sy * sw + sx) as usize]
                };

                out_ints.push(out_val);
            }
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
        let result = pil_resize(img, w, h, *filter, explicit_mode);
        return Ok(preserve_mode(img, result));
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
        TransposeMethod::Rotate90 => Ok(img.rotate90()),
        TransposeMethod::Rotate180 => Ok(img.rotate180()),
        TransposeMethod::Rotate270 => Ok(img.rotate270()),
        TransposeMethod::Transpose => Ok(img.rotate90().fliph()),
        TransposeMethod::Transverse => Ok(img.rotate270().fliph()),
    }
}

/// Execute a Thumbnail operation.
/// Computes the scale factor to fit within the given box, preserving aspect ratio.
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
    let result = pil_resize(img, new_w.max(1), new_h.max(1), *filter, explicit_mode);
    Ok(preserve_mode(img, result))
}

/// Execute a Reduce operation.
/// Averages each factor×factor block per-channel, preserving mode.
pub fn execute_reduce(
    img: &DynamicImage,
    factor: u32,
) -> Result<DynamicImage, PilError> {
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
