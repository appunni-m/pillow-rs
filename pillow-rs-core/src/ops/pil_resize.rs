//! PIL-compatible resize using direct 2D interpolation.
//!
//! The image crate's two-pass separable resize produces different results from
//! PIL's Per-Kernel resize due to f32 precision and pass ordering. This module
//! implements direct per-pixel bilinear/bicubic/lanczos/nearest interpolation
//! with f64 precision, matching PIL's output.

use crate::pipeline::ResampleFilter;
use image::DynamicImage;

// ── Filter kernels ──

/// Box / Nearest-neighbor kernel.
fn kernel_box(x: f64) -> f64 {
    if x.abs() < 0.5 { 1.0 } else { 0.0 }
}

/// Triangle (bilinear) kernel.
fn kernel_triangle(x: f64) -> f64 {
    let a = x.abs();
    if a < 1.0 { 1.0 - a } else { 0.0 }
}

/// Catmull-Rom (bicubic) kernel.
fn kernel_catrom(x: f64) -> f64 {
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
fn kernel_lanczos(x: f64, a: f64) -> f64 {
    if x.abs() >= a { return 0.0; }
    if x.abs() < 1e-10 { return 1.0; }
    let pix = std::f64::consts::PI * x;
    let sa = pix.sin() / pix;
    let s = (std::f64::consts::PI * x / a).sin() / (std::f64::consts::PI * x / a);
    sa * s
}

/// Hamming kernel (approximated as Lanczos-like window).
/// PIL's Hamming: 0.54 + 0.46 * cos(pi * x) for |x| < 1, else 0.
fn kernel_hamming(x: f64) -> f64 {
    if x.abs() >= 1.0 { 0.0 }
    else { 0.54 + 0.46 * (std::f64::consts::PI * x).cos() }
}

// ── Per-channel interpolation ──

/// Get pixel channels at (x, y) from RGBA or grayscale image.
/// Returns [r, g, b, a] for all modes. For L/LA, R=G=B=L.
fn get_channels(img: &DynamicImage, x: u32, y: u32) -> [f64; 4] {
    match img {
        DynamicImage::ImageLuma8(ref g) => {
            let p = g.get_pixel(x, y);
            let v = p[0] as f64;
            [v, v, v, 255.0]
        }
        DynamicImage::ImageLumaA8(ref ga) => {
            let p = ga.get_pixel(x, y);
            let v = p[0] as f64;
            [v, v, v, p[1] as f64]
        }
        DynamicImage::ImageRgb8(ref rgb) => {
            let p = rgb.get_pixel(x, y);
            [p[0] as f64, p[1] as f64, p[2] as f64, 255.0]
        }
        DynamicImage::ImageRgba8(ref rgba) => {
            let p = rgba.get_pixel(x, y);
            [p[0] as f64, p[1] as f64, p[2] as f64, p[3] as f64]
        }
        _ => {
            let rgba = img.to_rgba8();
            let p = rgba.get_pixel(x, y);
            [p[0] as f64, p[1] as f64, p[2] as f64, p[3] as f64]
        }
    }
}

/// Clamp an integer to [0, max].
fn clamp_idx(v: i64, max: u32) -> u32 {
    if v < 0 { 0 }
    else if v as u32 >= max { max - 1 }
    else { v as u32 }
}

/// Round a float to u8 using PIL's rounding: truncate after adding 0.5.
fn pil_round(v: f64) -> u8 {
    let v = v + 0.5;
    if v <= 0.0 { 0 }
    else if v >= 256.0 { 255 }
    else { v as u8 }
}

// ── Main interpolation function ──

/// Interpolate pixel value at (cx, cy) in source coordinates.
/// Uses PIL-compatible sratio-adjusted kernel:
///   weight = kernel((pixel_pos - center) / scale)
/// where scale = max(ratio, 1.0) for each dimension.
fn interpolate(
    img: &DynamicImage,
    cx: f64,
    cy: f64,
    sx_scale: f64,
    sy_scale: f64,
    kernel: fn(f64) -> f64,
    support: f64,
) -> [f64; 4] {
    let (sw, sh) = (img.width(), img.height());
    // PIL uses widened support for downscaling
    let src_support_x = support * sx_scale;
    let src_support_y = support * sy_scale;
    let left = (cx - src_support_x + 1e-9).ceil() as i64;
    let right = (cx + src_support_x - 1e-9).floor() as i64;
    let top = (cy - src_support_y + 1e-9).ceil() as i64;
    let bottom = (cy + src_support_y - 1e-9).floor() as i64;

    let mut acc = [0.0f64; 4];
    let mut wsum = 0.0f64;

    for iy in top..=bottom {
        let sy = clamp_idx(iy, sh);
        let wy = kernel((iy as f64 - cy) / sy_scale);
        if wy.abs() < 1e-15 { continue; }
        for ix in left..=right {
            let sx = clamp_idx(ix, sw);
            let wx = kernel((ix as f64 - cx) / sx_scale);
            let w = wx * wy;
            if w.abs() < 1e-15 { continue; }
            let p = get_channels(img, sx, sy);
            acc[0] += w * p[0];
            acc[1] += w * p[1];
            acc[2] += w * p[2];
            acc[3] += w * p[3];
            wsum += w;
        }
    }

    if wsum > 0.0 {
        let inv = 1.0 / wsum;
        [acc[0] * inv, acc[1] * inv, acc[2] * inv, acc[3] * inv]
    } else {
        // fallback: nearest pixel
        let sx = clamp_idx(cx.round() as i64, sw);
        let sy = clamp_idx(cy.round() as i64, sh);
        get_channels(img, sx, sy)
    }
}

// ── Alpha premultiplication (PIL-compatible: RGBA -> RGBa, LA -> La) ──

fn premultiply_alpha(img: &DynamicImage) -> DynamicImage {
    match img {
        DynamicImage::ImageRgba8(ref rgba) => {
            let mut out = rgba.clone();
            for p in out.pixels_mut() {
                let a = p[3] as f64 / 255.0;
                p[0] = (p[0] as f64 * a + 0.5) as u8;
                p[1] = (p[1] as f64 * a + 0.5) as u8;
                p[2] = (p[2] as f64 * a + 0.5) as u8;
            }
            DynamicImage::ImageRgba8(out)
        }
        DynamicImage::ImageLumaA8(ref la) => {
            let mut out = la.clone();
            for p in out.pixels_mut() {
                let a = p[1] as f64 / 255.0;
                p[0] = (p[0] as f64 * a + 0.5) as u8;
            }
            DynamicImage::ImageLumaA8(out)
        }
        _ => img.clone(),
    }
}

fn unpremultiply_alpha(img: &DynamicImage) -> DynamicImage {
    match img {
        DynamicImage::ImageRgba8(ref rgba) => {
            let mut out = rgba.clone();
            for p in out.pixels_mut() {
                let a = p[3] as f64;
                if a > 0.0 {
                    let inv = 255.0 / a;
                    p[0] = (p[0] as f64 * inv + 0.5) as u8;
                    p[1] = (p[1] as f64 * inv + 0.5) as u8;
                    p[2] = (p[2] as f64 * inv + 0.5) as u8;
                }
            }
            DynamicImage::ImageRgba8(out)
        }
        DynamicImage::ImageLumaA8(ref la) => {
            let mut out = la.clone();
            for p in out.pixels_mut() {
                let a = p[1] as f64;
                if a > 0.0 {
                    p[0] = (p[0] as f64 * 255.0 / a + 0.5) as u8;
                }
            }
            DynamicImage::ImageLumaA8(out)
        }
        _ => img.clone(),
    }
}

// ── Pixel-type-specific output builders ──

/// Create a Luma8 image from RGBA f64 pixel data (averaging R=G=B).
fn build_luma8(pixels: &[[f64; 4]], w: u32, h: u32) -> image::GrayImage {
    let mut out = image::GrayImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            let v = pil_round(pixels[idx][0]);
            out.put_pixel(x, y, image::Luma([v]));
        }
    }
    out
}

fn build_luma_alpha8(pixels: &[[f64; 4]], w: u32, h: u32) -> image::GrayAlphaImage {
    let mut out = image::GrayAlphaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            let v = pil_round(pixels[idx][0]);
            let a = pil_round(pixels[idx][3]);
            out.put_pixel(x, y, image::LumaA([v, a]));
        }
    }
    out
}

fn build_rgb8(pixels: &[[f64; 4]], w: u32, h: u32) -> image::RgbImage {
    let mut out = image::RgbImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            out.put_pixel(x, y, image::Rgb([
                pil_round(pixels[idx][0]),
                pil_round(pixels[idx][1]),
                pil_round(pixels[idx][2]),
            ]));
        }
    }
    out
}

fn build_rgba8(pixels: &[[f64; 4]], w: u32, h: u32) -> image::RgbaImage {
    let mut out = image::RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            out.put_pixel(x, y, image::Rgba([
                pil_round(pixels[idx][0]),
                pil_round(pixels[idx][1]),
                pil_round(pixels[idx][2]),
                pil_round(pixels[idx][3]),
            ]));
        }
    }
    out
}

// Lanczos3 wrapper
fn kernel_lanczos3(x: f64) -> f64 {
    kernel_lanczos(x, 3.0)
}

/// Choose kernel function and support based on filter type.
fn filter_from_resample(
    filter: ResampleFilter,
) -> (fn(f64) -> f64, f64) {
    match filter {
        ResampleFilter::Nearest => (kernel_box, 0.5),
        ResampleFilter::Bilinear => (kernel_triangle, 1.0),
        ResampleFilter::Bicubic => (kernel_catrom, 2.0),
        ResampleFilter::Lanczos => (kernel_lanczos3, 3.0),
        ResampleFilter::Box => (kernel_box, 0.5),
        ResampleFilter::Hamming => (kernel_hamming, 1.0),
    }
}

/// Preserve the original image's color mode (local copy for pil_resize).
pub(crate) fn pil_preserve_mode(original: &DynamicImage, result: DynamicImage) -> DynamicImage {
    let orig_color = original.color();
    let res_color = result.color();
    if orig_color == res_color {
        return result;
    }
    match orig_color {
        image::ColorType::L8 => DynamicImage::ImageLuma8(result.to_luma8()),
        image::ColorType::La8 => DynamicImage::ImageLumaA8(result.to_luma_alpha8()),
        image::ColorType::Rgb8 => DynamicImage::ImageRgb8(result.to_rgb8()),
        image::ColorType::Rgba8 => DynamicImage::ImageRgba8(result.to_rgba8()),
        _ => result,
    }
}

/// PIL-compatible resize.
///
/// Uses direct 2D interpolation with f64 precision and PIL's exact coordinate
/// mapping: `center = (output + 0.5) * src_size / dst_size - 0.5`.
///
/// For RGBA and LA modes, premultiplies alpha before resizing (matching PIL's
/// RGBa/La internal handling).
pub fn pil_resize(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    filter: ResampleFilter,
) -> DynamicImage {
    // Handle identity
    if (dst_w, dst_h) == (img.width(), img.height()) {
        return img.clone();
    }
    // Handle empty
    if dst_w == 0 || dst_h == 0 || img.width() == 0 || img.height() == 0 {
        return DynamicImage::new_rgba8(dst_w, dst_h);
    }

    // Premultiply alpha for RGBA/LA modes (PIL-compatible)
    let needs_alpha = matches!(
        img.color(),
        image::ColorType::Rgba8 | image::ColorType::La8
    );
    let work = if needs_alpha {
        premultiply_alpha(img)
    } else {
        img.clone()
    };

    let (kernel_fn, support) = filter_from_resample(filter);
    let (sw, sh) = (work.width() as f64, work.height() as f64);
    let (dw, dh) = (dst_w, dst_h);

    // PIL-compatible scale factor for kernel widening during downscaling
    let sx_scale = (sw / dw as f64).max(1.0);
    let sy_scale = (sh / dh as f64).max(1.0);

    // Pre-allocate pixel buffer
    let n = (dw * dh) as usize;
    let mut pixels: Vec<[f64; 4]> = Vec::with_capacity(n);

    // Handle NEAREST/Box separately: PIL uses floor((dx+0.5)*sw/dw) NOT subtracting 0.5
    if matches!(filter, ResampleFilter::Nearest | ResampleFilter::Box) {
        for dy in 0..dh {
            for dx in 0..dw {
                let cx = (dx as f64 + 0.5) * sw / dw as f64;
                let cy = (dy as f64 + 0.5) * sh / dh as f64;
                let sx = clamp_idx(cx.floor() as i64, img.width());
                let sy = clamp_idx(cy.floor() as i64, img.height());
                let p = get_channels(&work, sx, sy);
                pixels.push(p);
            }
        }
    } else {
        for dy in 0..dh {
            for dx in 0..dw {
                // PIL coordinate mapping: center = (output + 0.5) * src_size / dst_size - 0.5
                let cx = (dx as f64 + 0.5) * sw / dw as f64 - 0.5;
                let cy = (dy as f64 + 0.5) * sh / dh as f64 - 0.5;
                let p = interpolate(&work, cx, cy, sx_scale, sy_scale, kernel_fn, support);
                pixels.push(p);
            }
        }
    }

    // Un-premultiply alpha if needed
    let result_pixels = if needs_alpha {
        // For RGBA/LA modes, we built the result and need to un-premultiply
        // Create the result image first
        let result = match img.color() {
            image::ColorType::Rgba8 => DynamicImage::ImageRgba8(build_rgba8(&pixels, dw, dh)),
            image::ColorType::La8 => DynamicImage::ImageLumaA8(build_luma_alpha8(&pixels, dw, dh)),
            _ => unreachable!(),
        };
        unpremultiply_alpha(&result)
    } else {
        match img.color() {
            image::ColorType::L8 => DynamicImage::ImageLuma8(build_luma8(&pixels, dw, dh)),
            image::ColorType::Rgb8 => DynamicImage::ImageRgb8(build_rgb8(&pixels, dw, dh)),
            image::ColorType::Rgba8 => DynamicImage::ImageRgba8(build_rgba8(&pixels, dw, dh)),
            _ => {
                // Fallback: resize as RGBA, then convert back
                let rgba = build_rgba8(&pixels, dw, dh);
                pil_preserve_mode(img, DynamicImage::ImageRgba8(rgba))
            }
        }
    };

    pil_preserve_mode(img, result_pixels)
}
