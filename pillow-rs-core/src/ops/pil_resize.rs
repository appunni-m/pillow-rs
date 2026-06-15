//! PIL-compatible resize using two-pass separable interpolation.
//!
//! This implements PIL's exact two-pass approach:
//! 1. Horizontal pass: for each source row, compute weighted sum per output column,
//!    round to u8, store in intermediate image.
//! 2. Vertical pass: for each output column at each output row, compute weighted sum
//!    from intermediate rows, round to u8.
//!
//! PIL uses fixed-point arithmetic (PRECISION_BITS=22) but we use f64 with
//! intermediate rounding to match the two-pass quantization behavior.

use crate::pipeline::ResampleFilter;
use image::DynamicImage;

// ── Filter kernels ──

/// Box / Nearest-neighbor kernel.
fn kernel_box(x: f64) -> f64 {
    if x.abs() < 0.5 {
        1.0
    } else {
        0.0
    }
}

/// Triangle (bilinear) kernel.
fn kernel_triangle(x: f64) -> f64 {
    let a = x.abs();
    if a < 1.0 {
        1.0 - a
    } else {
        0.0
    }
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
fn kernel_hamming(x: f64) -> f64 {
    if x.abs() >= 1.0 {
        0.0
    } else {
        0.54 + 0.46 * (std::f64::consts::PI * x).cos()
    }
}

fn kernel_lanczos3(x: f64) -> f64 {
    kernel_lanczos(x, 3.0)
}

/// Choose kernel function and support based on filter type.
fn filter_from_resample(filter: ResampleFilter) -> (fn(f64) -> f64, f64) {
    match filter {
        ResampleFilter::Nearest => (kernel_box, 0.5),
        ResampleFilter::Bilinear => (kernel_triangle, 1.0),
        ResampleFilter::Bicubic => (kernel_catrom, 2.0),
        ResampleFilter::Lanczos => (kernel_lanczos3, 3.0),
        ResampleFilter::Box => (kernel_box, 0.5),
        ResampleFilter::Hamming => (kernel_hamming, 1.0),
    }
}

// ── Pixel access helpers ──

/// Get pixel as 4 f64 values (r, g, b, a). Grayscale replicates to RGB.
fn pixel_at(img: &DynamicImage, x: u32, y: u32) -> [f64; 4] {
    match img {
        DynamicImage::ImageLuma8(ref g) => {
            let v = g.get_pixel(x, y)[0] as f64;
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

/// Round a float to u8: truncate after adding 0.5, clipped to [0, 255].
fn pil_round(v: f64) -> u8 {
    let v = v + 0.5;
    if v <= 0.0 {
        0
    } else if v >= 256.0 {
        255
    } else {
        v as u8
    }
}

// ── PIL-compatible pixel range and weight computation ──

/// Precompute filter coefficients for one dimension.
///
/// For each output pixel, computes:
/// - xmin: first contributing source pixel index
/// - count: number of contributing source pixels
/// - weights: normalized filter weights for each source pixel
///
/// This matches PIL's `precompute_coeffs`:
///   center = (xx + 0.5) * scale
///   xmin = (int)(center - support + 0.5)
///   xmax = (int)(center + support + 0.5)
///   weight = kernel((sx + 0.5 - center) * ss)  where ss = 1.0 / filterscale
struct FilterCoeffs {
    xmin: Vec<i64>,
    count: Vec<usize>,
    weights: Vec<Vec<f64>>,
}

fn precompute_coeffs(
    out_size: u32,
    in_size: u32,
    kernel: fn(f64) -> f64,
    support: f64,
) -> FilterCoeffs {
    let scale = in_size as f64 / out_size as f64;
    let filterscale = scale.max(1.0);
    let ss = 1.0 / filterscale;
    let src_support = support * filterscale;

    let n = out_size as usize;
    let mut xmin = Vec::with_capacity(n);
    let mut count = Vec::with_capacity(n);
    let mut weights: Vec<Vec<f64>> = Vec::with_capacity(n);

    for ox in 0..n {
        // PIL center = (ox + 0.5) * scale
        let center = (ox as f64 + 0.5) * scale;

        // PIL: xmin = (int)(center - support + 0.5), xmax = (int)(center + support + 0.5)
        let mut x0 = (center - src_support + 0.5).trunc() as i64;
        let mut x1 = (center + src_support + 0.5).trunc() as i64;

        // Clamp to image bounds
        if x0 < 0 {
            x0 = 0;
        }
        if x1 > in_size as i64 {
            x1 = in_size as i64;
        }

        let cnt = (x1 - x0) as usize;
        xmin.push(x0);
        count.push(cnt);

        if cnt == 0 {
            weights.push(Vec::new());
            continue;
        }

        let mut w = Vec::with_capacity(cnt);
        let mut wsum = 0.0;
        for ix in 0..cnt {
            let sx = x0 + ix as i64;
            // PIL: kernel((sx + 0.5 - center) * ss)
            let val = kernel((sx as f64 + 0.5 - center) * ss);
            w.push(val);
            wsum += val;
        }

        // Normalize weights (PIL normalizes by the sum)
        if wsum > 0.0 {
            let inv = 1.0 / wsum;
            for wi in w.iter_mut() {
                *wi *= inv;
            }
        }
        weights.push(w);
    }

    FilterCoeffs {
        xmin,
        count,
        weights,
    }
}

// ── Alpha premultiplication ──

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

/// Resample one row of pixels into the output columns.
///
/// This is the horizontal pass: it computes one row of the intermediate image
/// from one row of the source image.
fn horizontal_pass_row(
    src_row: &[u8],
    _src_w: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    out_w: u32,
    intermediate_row: &mut [u8],
) {
    for ox in 0..out_w as usize {
        let x0 = coeffs.xmin[ox];
        let cnt = coeffs.count[ox];
        if cnt == 0 {
            continue;
        }
        let mut acc = vec![0.0f64; channels];
        for (cix, &w) in coeffs.weights[ox].iter().enumerate() {
            let sx = (x0 + cix as i64) as usize;
            for c in 0..channels {
                acc[c] += src_row[sx * channels + c] as f64 * w;
            }
        }
        let dest = &mut intermediate_row[ox * channels..(ox + 1) * channels];
        for c in 0..channels {
            dest[c] = pil_round(acc[c]);
        }
    }
}

/// Resample one column into the output rows.
///
/// This is the vertical pass: it computes one column of the final image
/// from one column of the intermediate image.
/// Returns a single value per channel for this (x, y) position.
fn vertical_pass_col(
    intermediate: &[u8],
    _src_rows: u32,
    out_x: u32,
    out_w: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    out_y: usize,
) -> Vec<u8> {
    let y0 = coeffs.xmin[out_y];
    let cnt = coeffs.count[out_y];
    if cnt == 0 {
        return vec![0u8; channels];
    }
    let mut acc = vec![0.0f64; channels];
    for (cix, &w) in coeffs.weights[out_y].iter().enumerate() {
        let sy = (y0 + cix as i64) as usize;
        let src_idx = (sy * out_w as usize + out_x as usize) * channels;
        for c in 0..channels {
            acc[c] += intermediate[src_idx + c] as f64 * w;
        }
    }
    let mut result = vec![0u8; channels];
    for c in 0..channels {
        result[c] = pil_round(acc[c]);
    }
    result
}

/// Preserve the original image's color mode.
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

/// PIL-compatible resize using two-pass separable interpolation.
///
/// PIL's approach:
/// 1. Precompute horizontal and vertical filter coefficients
/// 2. Horizontal pass: for each source row, compute each output column's
///    weighted sum, round to u8, and store in intermediate image
/// 3. Vertical pass: for each output column at each output row, compute
///    weighted sum from the intermediate rows, round to u8
///
/// For RGBA and LA modes, premultiplies alpha before resizing (matching PIL's
/// RGBa/La internal handling).
pub fn pil_resize(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    filter: ResampleFilter,
    explicit_mode: Option<&str>,
) -> DynamicImage {
    // Handle identity
    if (dst_w, dst_h) == (img.width(), img.height()) {
        return img.clone();
    }
    // Handle empty
    if dst_w == 0 || dst_h == 0 || img.width() == 0 || img.height() == 0 {
        return DynamicImage::new_rgba8(dst_w, dst_h);
    }

    // Retain original image for final mode preservation
    let orig_img = img;

    // CMYK/F/I stored as RGBA8 but 4th channel is NOT alpha (K/float/int byte).
    // PIL does NOT premultiply alpha for these modes.
    let is_cmyk = explicit_mode == Some("CMYK");
    let is_fi = explicit_mode == Some("F") || explicit_mode == Some("I");
    let needs_alpha = !is_cmyk
        && !is_fi
        && matches!(img.color(), image::ColorType::Rgba8 | image::ColorType::La8);
    let work = if needs_alpha {
        premultiply_alpha(img)
    } else {
        img.clone()
    };

    let (kernel_fn, support) = filter_from_resample(filter);
    let (sw, sh) = (work.width(), work.height());
    let (dw, dh) = (dst_w, dst_h);

    // Determine channel count
    let channels = match work.color() {
        image::ColorType::L8 => 1usize,
        image::ColorType::La8 => 2usize,
        image::ColorType::Rgb8 => 3usize,
        _ => 4usize,
    };

    // Handle NEAREST/Box: use direct nearest-neighbor sampling
    if matches!(filter, ResampleFilter::Nearest | ResampleFilter::Box) {
        let sw_f = sw as f64;
        let sh_f = sh as f64;
        let dw_f = dw as f64;
        let dh_f = dh as f64;
        let n = (dw * dh) as usize;
        let mut out_bytes: Vec<u8> = Vec::with_capacity(n * channels);
        for dy in 0..dh {
            for dx in 0..dw {
                // PIL: floor((dx + 0.5) * sw / dw)
                let sx = ((dx as f64 + 0.5) * sw_f / dw_f).floor() as u32;
                let sy = ((dy as f64 + 0.5) * sh_f / dh_f).floor() as u32;
                let sx = sx.min(sw - 1);
                let sy = sy.min(sh - 1);
                let p = pixel_at(&work, sx, sy);
                for &v in p[..channels].iter() {
                    out_bytes.push(pil_round(v));
                }
            }
        }
        // Build result from bytes
        let result = raw_to_dynamic(&out_bytes, dw, dh, channels);
        let result = if needs_alpha {
            unpremultiply_alpha(&result)
        } else {
            result
        };
        return pil_preserve_mode(orig_img, result);
    }

    // Precompute horizontal and vertical coefficients
    let h_coeffs = precompute_coeffs(dw, sw, kernel_fn, support);
    let v_coeffs = precompute_coeffs(dh, sh, kernel_fn, support);

    // Allocate intermediate image (sh rows × dw columns × channels)
    let mut intermediate = vec![0u8; (sh * dw) as usize * channels];

    // Horizontal pass: for each source row, compute all output columns
    let work_bytes = work.as_bytes();
    for sy in 0..sh {
        let src_start = (sy * sw) as usize * channels;
        let src_row = &work_bytes[src_start..src_start + sw as usize * channels];
        let inter_start = (sy * dw) as usize * channels;
        let inter_row = &mut intermediate[inter_start..inter_start + dw as usize * channels];
        horizontal_pass_row(src_row, sw, channels, &h_coeffs, dw, inter_row);
    }

    // Allocate output image
    let mut out_bytes = vec![0u8; (dw * dh) as usize * channels];

    // Vertical pass: for each output row, compute all output columns
    for dy in 0..dh {
        let out_start = (dy * dw) as usize * channels;
        let out_row = &mut out_bytes[out_start..out_start + dw as usize * channels];
        for dx in 0..dw {
            let vert_result =
                vertical_pass_col(&intermediate, sh, dx, dw, channels, &v_coeffs, dy as usize);
            let dest = &mut out_row[dx as usize * channels..(dx as usize + 1) * channels];
            dest[..channels].copy_from_slice(&vert_result[..channels]);
        }
    }

    // Build DynamicImage from bytes
    let result = raw_to_dynamic(&out_bytes, dw, dh, channels);

    // Un-premultiply alpha if needed
    let result = if needs_alpha {
        unpremultiply_alpha(&result)
    } else {
        result
    };

    pil_preserve_mode(orig_img, result)
}

/// Convert raw bytes to DynamicImage based on color type.
fn raw_to_dynamic(bytes: &[u8], w: u32, h: u32, channels: usize) -> DynamicImage {
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
