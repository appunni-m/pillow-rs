//! Geometry operations extracted from image.rs execute_op().
//!
//! These functions are standalone implementations of PIL-compatible geometry
//! operations (Resize, Crop, Rotate, Transpose, Thumbnail, Reduce) that operate
//! on DynamicImage and return new DynamicImage instances.

use crate::raster::{DynamicImage, GenericImageView};
use std::f64;

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::ops::pil_resize::{pil_resize, precompute_coeffs_f64, round_up};
use crate::pipeline::{ResampleFilter, TransposeMethod};

// ── PIL-compatible filter kernels (f64 precision) ──

/// Box / Nearest-neighbor kernel.
fn f_kernel_box(x: f64) -> f64 {
    if x.abs() < 0.5 { 1.0 } else { 0.0 }
}

/// Triangle (bilinear) kernel.
fn f_kernel_triangle(x: f64) -> f64 {
    let a = x.abs();
    if a < 1.0 { 1.0 - a } else { 0.0 }
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
    } else if x.abs() < 1e-10 {
        1.0
    } else {
        // Keep the numeric F/I paths aligned with Pillow's Hamming
        // windowed-sinc resampler (Resample.c), including its sinc factor.
        let pix = std::f64::consts::PI * x;
        (pix.sin() / pix) * (0.54 + 0.46 * pix.cos())
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
    let out = crate::raster::RgbaImage::from_raw(dst_w, dst_h, rgba_bytes)
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
    let _sx_scale = (sw_f / dw_f).max(1.0);
    let _sy_scale = (sh_f / dh_f).max(1.0);

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
        let out =
            crate::raster::RgbaImage::from_raw(dst_w, dst_h, rgba_bytes).ok_or_else(|| {
                PilError::ValueError("resize_i: failed to create output buffer".into())
            })?;
        return Ok(DynamicImage::ImageRgba8(out));
    }

    // PIL: for INT32 images, coefficients stay as double-precision (not fixed-point).
    // Use f64 accumulation + ROUND_UP matching PIL's ImagingResample for 32-bit types.
    let h_coeffs_f64 = precompute_coeffs_f64(dst_w, sw, kernel, support);
    let v_coeffs_f64 = precompute_coeffs_f64(dst_h, sh, kernel, support);

    // Allocate intermediate buffer (sh rows x dw cols) as f64
    let mut intermediate: Vec<f64> = vec![0.0f64; (sh * dst_w) as usize];

    // Horizontal pass: f64 accumulation, matching PIL's double-precision path
    for sy in 0..sh {
        let src_row_base = (sy * sw) as usize;
        for dx in 0..dst_w {
            let x0 = h_coeffs_f64.xmin[dx as usize];
            let cnt = h_coeffs_f64.count[dx as usize];
            if cnt == 0 {
                continue;
            }
            let mut acc: f64 = 0.0;
            for (cix, &w) in h_coeffs_f64.weights[dx as usize].iter().enumerate() {
                let sx = (x0 + cix as i64) as usize;
                acc += w * src_ints[src_row_base + sx] as f64;
            }
            // PIL: ROUND_UP(ss)
            intermediate[(sy * dst_w + dx) as usize] = round_up(acc);
        }
    }

    // Vertical pass
    let mut out_ints: Vec<i32> = Vec::with_capacity(n);
    for dy in 0..dst_h {
        let y0 = v_coeffs_f64.xmin[dy as usize];
        let cnt = v_coeffs_f64.count[dy as usize];
        if cnt == 0 {
            for _ in 0..dst_w {
                out_ints.push(0i32);
            }
            continue;
        }
        for dx in 0..dst_w {
            let mut acc: f64 = 0.0;
            for (cix, &w) in v_coeffs_f64.weights[dy as usize].iter().enumerate() {
                let sy = (y0 + cix as i64) as usize;
                acc += w * intermediate[(sy * dst_w as usize) + dx as usize];
            }
            out_ints.push(round_up(acc) as i32);
        }
    }

    // Re-pack each i32 as 4 RGBA8 bytes (little-endian).
    let rgba_bytes: Vec<u8> = out_ints.iter().flat_map(|v| v.to_le_bytes()).collect();
    let out = crate::raster::RgbaImage::from_raw(dst_w, dst_h, rgba_bytes)
        .ok_or_else(|| PilError::ValueError("resize_i: failed to create output buffer".into()))?;
    Ok(DynamicImage::ImageRgba8(out))
}

// ── Generic rotation & transform helpers (mode-aware) ──

fn affine_nearest_fixed(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    channels: usize,
    affine: [f64; 6],
    fill: (u8, u8, u8, u8),
    output: &mut [u8],
) {
    // Pillow's ImagingTransformAffine nearest path rounds the six affine
    // coefficients to signed 16.16 values once, then advances those integers
    // across each output row. Recomputing the same coordinates as f64 changes
    // which source pixel wins at exact integer boundaries.
    let fixed = |value: f64| (value.mul_add(65_536.0, 0.5).floor()) as i64;
    let [a, b, c, d, e, f] = affine;
    let step_x_x = fixed(a);
    let step_y_x = fixed(b);
    let step_x_y = fixed(d);
    let step_y_y = fixed(e);
    let origin_x = fixed(c + a * 0.5 + b * 0.5);
    let origin_y = fixed(f + d * 0.5 + e * 0.5);
    let (source_width, source_height) = source_size;
    let (destination_width, destination_height) = destination_size;

    for y in 0..destination_height {
        let mut source_x = origin_x + i64::from(y) * step_y_x;
        let mut source_y = origin_y + i64::from(y) * step_y_y;
        for x in 0..destination_width {
            let input_x = source_x >> 16;
            let input_y = source_y >> 16;
            let output_index = (y * destination_width + x) as usize * channels;
            if input_x >= 0
                && input_x < i64::from(source_width)
                && input_y >= 0
                && input_y < i64::from(source_height)
            {
                let input_index =
                    (input_y as u32 * source_width + input_x as u32) as usize * channels;
                output[output_index..output_index + channels]
                    .copy_from_slice(&source[input_index..input_index + channels]);
            } else {
                for channel in 0..channels.min(4) {
                    output[output_index + channel] = match channel {
                        0 => fill.0,
                        1 => fill.1,
                        2 => fill.2,
                        _ => fill.3,
                    };
                }
            }
            source_x += step_x_x;
            source_y += step_x_y;
        }
    }
}

/// Rotate an image by an arbitrary angle, working on the native number of channels.
/// When `nearest` is true, uses nearest-neighbor sampling (required for P, 1, I, F modes).
fn rotate_arbitrary_generic(
    img: &DynamicImage,
    angle: f64,
    expand: bool,
    fill: Option<(u8, u8, u8, u8)>,
    nearest: bool,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let (w, h) = img.dimensions();
    let sw = w as f64;
    let sh = h as f64;
    // Pillow builds the reverse affine transform (destination -> source),
    // rounding the trigonometric coefficients to 15 decimal places before
    // calculating the expanded canvas.
    // Pillow's affine coefficients map destination pixels back into the
    // source image. The inverse mapping uses the negative angle; using the
    // forward sign mirrors the exposed fill region for arbitrary angles.
    let rad = -angle.to_radians();
    let round_15 = |value: f64| (value * 1_000_000_000_000_000.0).round() / 1_000_000_000_000_000.0;
    let aff_a = round_15(rad.cos());
    let aff_b = round_15(rad.sin());
    let aff_d = round_15(-rad.sin());
    let aff_e = aff_a;
    // Pillow's Image.rotate composes post-translation into the reverse affine
    // matrix before calculating expand bounds; applying it after sampling
    // changes both the canvas size and the selected source pixels.
    let (center_x, center_y) = center.unwrap_or((sw / 2.0, sh / 2.0));
    let (translate_x, translate_y) = translate.unwrap_or((0.0, 0.0));
    let mut aff_c =
        aff_a * (-center_x - translate_x) + aff_b * (-center_y - translate_y) + center_x;
    let mut aff_f =
        aff_d * (-center_x - translate_x) + aff_e * (-center_y - translate_y) + center_y;
    let transform =
        |x: f64, y: f64, c: f64, f: f64| (aff_a * x + aff_b * y + c, aff_d * x + aff_e * y + f);

    // Pillow rounds each outer edge independently. This differs from taking
    // ceil(max - min) whenever the transformed minimum is fractional.
    let corners = [(0.0, 0.0), (sw, 0.0), (sw, sh), (0.0, sh)];
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(cx, cy) in &corners {
        let (rx, ry) = transform(cx, cy, aff_c, aff_f);
        min_x = min_x.min(rx);
        max_x = max_x.max(rx);
        min_y = min_y.min(ry);
        max_y = max_y.max(ry);
    }
    let (dw, dh) = if expand {
        (
            (max_x.ceil() - min_x.floor()) as u32,
            (max_y.ceil() - min_y.floor()) as u32,
        )
    } else {
        (w, h)
    };

    if expand {
        let shift_x = -(dw as f64 - sw) / 2.0;
        let shift_y = -(dh as f64 - sh) / 2.0;
        (aff_c, aff_f) = transform(shift_x, shift_y, aff_c, aff_f);
    }

    let raw = img.as_bytes();
    let fill_color = fill.unwrap_or((0, 0, 0, 0));

    let mut out = CheckedDims::new(dw, dh, channels as u8)?.alloc_buffer();

    if nearest {
        affine_nearest_fixed(
            raw,
            (w, h),
            (dw, dh),
            channels,
            [aff_a, aff_b, aff_c, aff_d, aff_e, aff_f],
            fill_color,
            &mut out,
        );
    } else {
        for dy in 0..dh {
            for dx in 0..dw {
                // Pillow's ImagingTransformAffine bilinear path starts the
                // source y accumulator at f + 0.5 while the source x
                // accumulator starts at c. Keeping that asymmetric sample
                // origin matters at expanded boundaries: the unshifted
                // source-y form moves the 45-degree fill edge by one pixel.
                let (sx_rel, sy_rel) = transform(dx as f64, dy as f64, aff_c, aff_f);
                let sy_rel = sy_rel + 0.5;

                let out_idx = (dy * dw + dx) as usize * channels;

                if sx_rel >= 0.0 && sx_rel < sw && sy_rel >= 0.0 && sy_rel < sh {
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
    }

    raw_bytes_to_image(dw, dh, out, channels)
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
) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let raw = img.as_bytes();
    let (sw, sh) = img.dimensions();
    let fill_color = fill.unwrap_or((0, 0, 0, 255));

    let mut out = CheckedDims::new(dst_w, dst_h, channels as u8)?.alloc_buffer();

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

    raw_bytes_to_image(dst_w, dst_h, out, channels)
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
    // Only use F/I resize paths when the image is already stored as Rgba8
    // (4 bytes per pixel), meaning it has been converted to F/I mode already.
    // If the image is still RGB (3 bytes per pixel), use normal resize regardless
    // of explicit_mode, because the F/I convert hasn't happened yet in the pipeline.
    if explicit_mode == Some("F") && matches!(img, DynamicImage::ImageRgba8(_)) {
        return resize_f(img, w, h, filter);
    }
    if explicit_mode == Some("I") && matches!(img, DynamicImage::ImageRgba8(_)) {
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
        let mut out = crate::raster::GrayImage::new(rw, rh);
        for (op, ip) in out.pixels_mut().zip(gray.pixels()) {
            op[0] = if ip[0] >= 128 { 255 } else { 0 };
        }
        return Ok(preserve_mode(img, DynamicImage::ImageLuma8(out)));
    }
    let result = pil_resize(img, w, h, *filter, explicit_mode);
    Ok(preserve_mode(img, result))
}

/// Execute a Crop operation.
/// PIL's crop fills out-of-bounds source pixels with 0 (black).
pub fn execute_crop(
    img: &DynamicImage,
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
) -> Result<DynamicImage, PilError> {
    let w = right.saturating_sub(left);
    let h = bottom.saturating_sub(top);
    let (iw, ih) = (img.width(), img.height());
    // PIL: out-of-bounds source pixels are filled with 0.
    // If the crop region is fully within bounds, use the fast path.
    if left < iw && top < ih && right <= iw && bottom <= ih {
        return Ok(img.crop_imm(left, top, w, h));
    }
    // Out-of-bounds crop: fill missing pixels with 0.
    let channels = img.color().channel_count() as usize;
    let mut out = CheckedDims::new(w, h, channels as u8)?.alloc_buffer();
    let raw = img.as_bytes();
    for dy in 0..h {
        let sy = top + dy;
        if sy >= ih {
            continue; // row is entirely out of bounds, leave as 0
        }
        for dx in 0..w {
            let sx = left + dx;
            if sx >= iw {
                continue; // column is out of bounds, leave as 0
            }
            let src = ((sy * iw + sx) as usize) * channels;
            let dst = ((dy * w + dx) as usize) * channels;
            if src + channels <= raw.len() && dst + channels <= out.len() {
                out[dst..dst + channels].copy_from_slice(&raw[src..src + channels]);
            }
        }
    }
    // Use the module-level raw_bytes_to_image defined above
    let result = raw_bytes_to_image(w, h, out, channels)?;
    Ok(result)
}

/// Execute a Rotate operation.
/// Fast-path for 90-degree multiples; otherwise uses arbitrary rotation.
/// Rotate 90 or 270 degrees without expanding (clip to original size).
/// Matches PIL's behavior: rotate and center the result in the original canvas,
/// filling exposed areas with the fill color.
fn rotate_90_non_expand(
    img: &DynamicImage,
    clockwise_270: bool,
    fill: Option<(u8, u8, u8, u8)>,
) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let (w, h) = img.dimensions();
    let raw = img.as_bytes();
    let fill_color = fill.unwrap_or((0, 0, 0, 0));

    // Initialize output with fill color
    let fill_pixel: Vec<u8> = match channels {
        1 => vec![fill_color.0],
        2 => vec![fill_color.0, fill_color.3],
        3 => vec![fill_color.0, fill_color.1, fill_color.2],
        _ => vec![fill_color.0, fill_color.1, fill_color.2, fill_color.3],
    };
    let dims = CheckedDims::new(w, h, channels as u8)?;
    let mut out = fill_pixel.repeat(dims.total_pixels());

    let dx_off = (w as i32 - h as i32) / 2; // center offset in x
    let dy_off = (h as i32 - w as i32) / 2; // center offset in y

    for dy in 0..h {
        for dx in 0..w {
            let (sx, sy) = if clockwise_270 {
                // 270° CCW = 90° CW: expand_true uses input(dy, w-1-dx)
                // dx_true = dx - dx_off, dy_true = dy - dy_off
                // sx = dy_true = dy - dy_off, sy = w - 1 - dx_true = w - 1 - dx + dx_off
                (dy as i32 - dy_off, w as i32 - 1 - dx as i32 + dx_off)
            } else {
                // 90° CCW: expand_true uses input(w-1-dy, dx)
                // dx_true = dx - dx_off, dy_true = dy - dy_off
                // sx = w - 1 - dy_true = w - 1 - dy + dy_off, sy = dx_true = dx - dx_off
                (w as i32 - 1 - dy as i32 + dy_off, dx as i32 - dx_off)
            };
            if sx >= 0 && sx < w as i32 && sy >= 0 && sy < h as i32 {
                let in_idx = (sy as u32 * w + sx as u32) as usize * channels;
                let out_idx = (dy * w + dx) as usize * channels;
                out[out_idx..out_idx + channels].copy_from_slice(&raw[in_idx..in_idx + channels]);
            }
        }
    }

    // Create output dynamic image
    raw_bytes_to_image(w, h, out, channels)
}

pub fn execute_rotate(
    img: &DynamicImage,
    angle: f64,
    expand: bool,
    fill: Option<(u8, u8, u8, u8)>,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    requested_nearest: bool,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let deg = (angle.round() as i32).rem_euclid(360);
    let has_custom_transform = center.is_some() || translate.is_some();
    let nearest = requested_nearest
        || explicit_mode == Some("P")
        || explicit_mode == Some("1")
        || explicit_mode == Some("I")
        || explicit_mode == Some("F");
    // Fast path: exact 90-degree multiples
    // PIL rotates counterclockwise; image crate rotates clockwise.
    // PIL 90° CCW = image crate 270° CW, PIL 270° CCW = image crate 90° CW.
    // For 90/270 with expand=False, compute the clipped result directly
    // by pasting the expanded result centered in the original-sized canvas.
    let result = if !has_custom_transform && ((deg - 90).abs() < 2 || (deg - 90).abs() >= 358) {
        if expand {
            img.rotate270() // 270° CW = 90° CCW (PIL)
        } else {
            rotate_90_non_expand(img, false, fill)?
        }
    } else if !has_custom_transform && (deg - 180).abs() < 2 {
        img.rotate180()
    } else if !has_custom_transform && ((deg - 270).abs() < 2 || (deg - 270).abs() >= 358) {
        if expand {
            img.rotate90() // 90° CW = 270° CCW (PIL)
        } else {
            rotate_90_non_expand(img, true, fill)?
        }
    } else {
        // Multi-channel arbitrary rotation (no RGBA roundtrip)
        rotate_arbitrary_generic(img, angle, expand, fill, nearest, center, translate)?
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
        TransposeMethod::Transpose | TransposeMethod::Transverse => {
            let (width, height) = img.dimensions();
            let channels = img.color().channel_count() as usize;
            let source = img.as_bytes();
            let mut output = CheckedDims::new(height, width, channels as u8)?.alloc_buffer();
            for output_y in 0..width {
                for output_x in 0..height {
                    let (source_x, source_y) = match method {
                        TransposeMethod::Transpose => (output_y, output_x),
                        TransposeMethod::Transverse => {
                            (width - 1 - output_y, height - 1 - output_x)
                        }
                        _ => unreachable!(),
                    };
                    let source_index = (source_y * width + source_x) as usize * channels;
                    let output_index = (output_y * height + output_x) as usize * channels;
                    output[output_index..output_index + channels]
                        .copy_from_slice(&source[source_index..source_index + channels]);
                }
            }
            raw_bytes_to_image(height, width, output, channels)
        }
    }
}

/// PIL's round_aspect: picks floor or ceil of `number` based on `key` function.
/// Returns max(min(floor, ceil, key=key), 1). When floor == ceil (integer),
/// returns that integer.
fn round_aspect(number: f64, key: impl Fn(f64) -> f64) -> u32 {
    let floor = number.trunc();
    if number == floor {
        return floor as u32;
    }
    let ceil = floor + 1.0;
    let floor_key = key(floor);
    let ceil_key = key(ceil);
    let best = if floor_key <= ceil_key { floor } else { ceil };
    (best as u32).max(1)
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
    // PIL's thumbnail uses round_aspect(): picks floor or ceil based on
    // which better preserves the aspect ratio. This differs from simple
    // rounding — e.g. round(12.5)=12 but if ceil=13 gives better aspect
    // ratio preservation, PIL picks 13.
    // Exact PIL formula:
    //   if x / y >= aspect:
    //       x = round_aspect(y * aspect, key=lambda n: abs(aspect - n / y))
    //   else:
    //       y = round_aspect(x / aspect, key=lambda n: 0 if n == 0 else abs(aspect - x / n))
    let (new_w, new_h) = if w as f64 / h as f64 >= cur_w as f64 / cur_h as f64 {
        let adjusted = round_aspect(h as f64 * (cur_w as f64 / cur_h as f64), |n| {
            (cur_w as f64 / cur_h as f64 - n / h as f64).abs()
        });
        (adjusted, h)
    } else {
        let adjusted = round_aspect(w as f64 / (cur_w as f64 / cur_h as f64), |n| {
            if n == 0.0 {
                0.0
            } else {
                (cur_w as f64 / cur_h as f64 - w as f64 / n).abs()
            }
        });
        (w, adjusted)
    };
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
        && !matches!(
            img.color(),
            crate::raster::ColorType::La8 | crate::raster::ColorType::Rgba8
        );
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
            let mut out = CheckedDims::new(rw, rh, channels as u8)?.alloc_buffer();
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
                        let block_pixels = (factor.min(cur_h - y * factor)
                            * factor.min(cur_w - x * factor))
                            as u64;
                        let val = ((sum + block_pixels / 2) / block_pixels) as u8;
                        out[(y * rw + x) as usize * channels + c] = val;
                    }
                }
            }
            work_img = raw_to_dynimage(&out, rw, rh, channels);
        }
    }
    // Only use F/I thumbnail paths when the image is already stored as Rgba8
    // (4 bytes per pixel), meaning it has been converted to F/I mode already.
    // If the image is still RGB or other format, use normal thumbnail regardless
    // of explicit_mode, because the F/I convert hasn't happened yet in the pipeline.
    let result = match (explicit_mode, &work_img) {
        (Some("F"), DynamicImage::ImageRgba8(_)) => {
            resize_f(&work_img, new_w, new_h, &effective_filter)?
        }
        (Some("I"), DynamicImage::ImageRgba8(_)) => {
            resize_i(&work_img, new_w, new_h, &effective_filter)?
        }
        _ => pil_resize(&work_img, new_w, new_h, effective_filter, explicit_mode),
    };
    Ok(preserve_mode(img, result))
}

/// Helper: convert raw bytes + dimensions to DynamicImage.
fn raw_to_dynimage(bytes: &[u8], w: u32, h: u32, channels: usize) -> DynamicImage {
    match channels {
        1 => DynamicImage::ImageLuma8(
            crate::raster::GrayImage::from_raw(w, h, bytes.to_vec())
                .unwrap_or_else(|| crate::raster::GrayImage::new(w, h)),
        ),
        2 => DynamicImage::ImageLumaA8(
            crate::raster::GrayAlphaImage::from_raw(w, h, bytes.to_vec())
                .unwrap_or_else(|| crate::raster::GrayAlphaImage::new(w, h)),
        ),
        3 => DynamicImage::ImageRgb8(
            crate::raster::RgbImage::from_raw(w, h, bytes.to_vec())
                .unwrap_or_else(|| crate::raster::RgbImage::new(w, h)),
        ),
        _ => DynamicImage::ImageRgba8(
            crate::raster::RgbaImage::from_raw(w, h, bytes.to_vec())
                .unwrap_or_else(|| crate::raster::RgbaImage::new(w, h)),
        ),
    }
}

/// Execute a Reduce operation matching Pillow's `Reduce.c`.
///
/// Pillow computes ceil(w/xscale) x ceil(h/yscale) output pixels, averages
/// complete xscale×yscale blocks for the inner region, then fills the right
/// column, bottom row, and bottom-right corner from partial blocks using the
/// same multiplier/amend rounding (`(sum + amend) * multiplier >> 24`).
pub fn execute_reduce(
    img: &DynamicImage,
    x_factor: u32,
    y_factor: u32,
) -> Result<DynamicImage, PilError> {
    if x_factor < 2 && y_factor < 2 {
        return Ok(img.clone());
    }
    let fx = x_factor.max(1);
    let fy = y_factor.max(1);
    let channels = img.color().channel_count() as usize;
    let (w, h) = (img.width(), img.height());
    let new_w = w.div_ceil(fx);
    let new_h = h.div_ceil(fy);
    let raw = img.as_bytes().to_vec();
    let mut out = CheckedDims::new(new_w, new_h, channels as u8)?.alloc_buffer();

    let division_multiplier = |divider: u32| -> u64 {
        // division_UINT32(divider, 8): 2^32 / (256 * divider), truncated.
        ((1u128 << 32) / (u128::from(divider) * 256)) as u64
    };
    let block_average = |sum: u64, divider: u32, amend: u32| -> u8 {
        let m = division_multiplier(divider);
        (((sum + u64::from(amend)) * m) >> 24) as u8
    };

    // Main region: complete fx×fy blocks (floor division loop bounds).
    let main_w = w / fx;
    let main_h = h / fy;
    let block_area = fx * fy;
    let amend = block_area / 2;
    for y in 0..main_h {
        for x in 0..main_w {
            let mut sums = vec![0u64; channels];
            for dy in 0..fy {
                for dx in 0..fx {
                    let src_idx = ((y * fy + dy) * w + x * fx + dx) as usize * channels;
                    for c in 0..channels {
                        sums[c] += raw[src_idx + c] as u64;
                    }
                }
            }
            let dst_idx = (y * new_w + x) as usize * channels;
            for c in 0..channels {
                out[dst_idx + c] = block_average(sums[c], block_area, amend);
            }
        }
    }

    // Right column (partial x): scale = (w % fx) * fy.
    if w % fx != 0 {
        let scale = (w % fx) * fy;
        let m = division_multiplier(scale);
        let amend = scale / 2;
        let x = main_w;
        for y in 0..main_h {
            let mut sums = vec![0u64; channels];
            for dy in 0..fy {
                for dx in 0..(w % fx) {
                    let src_idx = ((y * fy + dy) * w + main_w * fx + dx) as usize * channels;
                    for c in 0..channels {
                        sums[c] += raw[src_idx + c] as u64;
                    }
                }
            }
            let dst_idx = (y * new_w + x) as usize * channels;
            for c in 0..channels {
                out[dst_idx + c] = (((sums[c] + u64::from(amend)) * m) >> 24) as u8;
            }
        }
    }

    // Bottom row (partial y): scale = fx * (h % fy).
    if h % fy != 0 {
        let scale = fx * (h % fy);
        let m = division_multiplier(scale);
        let amend = scale / 2;
        let y = main_h;
        for x in 0..main_w {
            let mut sums = vec![0u64; channels];
            for dy in 0..(h % fy) {
                for dx in 0..fx {
                    let src_idx = ((main_h * fy + dy) * w + x * fx + dx) as usize * channels;
                    for c in 0..channels {
                        sums[c] += raw[src_idx + c] as u64;
                    }
                }
            }
            let dst_idx = (y * new_w + x) as usize * channels;
            for c in 0..channels {
                out[dst_idx + c] = (((sums[c] + u64::from(amend)) * m) >> 24) as u8;
            }
        }
    }

    // Bottom-right corner: scale = (w % fx) * (h % fy).
    if w % fx != 0 && h % fy != 0 {
        let scale = (w % fx) * (h % fy);
        let m = division_multiplier(scale);
        let amend = scale / 2;
        let mut sums = vec![0u64; channels];
        for dy in 0..(h % fy) {
            for dx in 0..(w % fx) {
                let src_idx = ((main_h * fy + dy) * w + main_w * fx + dx) as usize * channels;
                for c in 0..channels {
                    sums[c] += raw[src_idx + c] as u64;
                }
            }
        }
        let dst_idx = (main_h * new_w + main_w) as usize * channels;
        for c in 0..channels {
            out[dst_idx + c] = (((sums[c] + u64::from(amend)) * m) >> 24) as u8;
        }
    }

    let result = raw_bytes_to_image(new_w, new_h, out, channels)?;
    Ok(result)
}
