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
use crate::raster::{DynamicImage, ImageBuffer, Luma};

// ── Filter kernels ──

/// Box / Nearest-neighbor kernel.
fn kernel_box(x: f64) -> f64 {
    if x > -0.5 && x <= 0.5 { 1.0 } else { 0.0 }
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
    } else if x.abs() < 1e-10 {
        1.0
    } else {
        // Pillow's Hamming resampler is a windowed sinc, not only the
        // cosine window. This mirrors the Hamming branch in Pillow's
        // Resample.c and is observable on downsampled impulses.
        let pix = std::f64::consts::PI * x;
        (pix.sin() / pix) * (0.54 + 0.46 * pix.cos())
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
        DynamicImage::ImageLuma8(g) => {
            let v = g.get_pixel(x, y)[0] as f64;
            [v, v, v, 255.0]
        }
        DynamicImage::ImageLumaA8(ga) => {
            let p = ga.get_pixel(x, y);
            let v = p[0] as f64;
            [v, v, v, p[1] as f64]
        }
        DynamicImage::ImageRgb8(rgb) => {
            let p = rgb.get_pixel(x, y);
            [p[0] as f64, p[1] as f64, p[2] as f64, 255.0]
        }
        DynamicImage::ImageRgba8(rgba) => {
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

/// Resize a native 16-bit grayscale image through PIL's nearest-neighbor path.
///
/// `I;16*` images carry unsigned 16-bit samples.  Keeping them in the native
/// buffer is required because converting through `to_rgba8()` changes both the
/// sample width and the bytes returned by `tobytes()`.
fn pil_resize_luma16_nearest(
    img: &ImageBuffer<Luma<u16>, Vec<u16>>,
    dst_w: u32,
    dst_h: u32,
) -> DynamicImage {
    let sw = img.width();
    let sh = img.height();
    let scale_x = sw as f64 / dst_w as f64;
    let scale_y = sh as f64 / dst_h as f64;
    let mut result = ImageBuffer::new(dst_w, dst_h);

    let mut xintab = Vec::with_capacity(dst_w as usize);
    let mut xo = scale_x * 0.5;
    for _ in 0..dst_w {
        let xi = xo as u32;
        xintab.push(if xi >= sw { sw - 1 } else { xi });
        xo += scale_x;
    }

    let mut yo = scale_y * 0.5;
    for dy in 0..dst_h {
        let sy = if yo >= sh as f64 { sh - 1 } else { yo as u32 };
        for dx in 0..dst_w {
            let sx = xintab[dx as usize];
            result.put_pixel(dx, dy, *img.get_pixel(sx, sy));
        }
        yo += scale_y;
    }

    DynamicImage::ImageLuma16(result)
}

/// Pillow's `Resample.c` uses a separate byte-oriented implementation for
/// `I;16*` images.  The source and destination images are native `u16`
/// buffers, but the C implementation reads and writes their two bytes using
/// the mode-dependent `bigendian` flag.  Keep that ABI-visible behavior at the
/// core boundary instead of feeding a 16-bit buffer into the u8 resampler.
fn luma16_resample_big_endian(mode: Option<&str>) -> bool {
    match mode {
        // Pillow's Resample.c deliberately selects the big-endian branch for
        // I;16N.  On little-endian hosts this is observable as the historical
        // native-mode byte ordering of the convolution path.
        Some("I;16N") => true,
        Some("I;16B") => true,
        _ => false,
    }
}

fn luma16_resample_read(sample: u16, big_endian: bool) -> u16 {
    let native_bytes = sample.to_ne_bytes();
    if big_endian {
        u16::from_be_bytes(native_bytes)
    } else {
        u16::from_le_bytes(native_bytes)
    }
}

fn clip_u8(value: i64) -> u8 {
    value.clamp(0, 255) as u8
}

fn luma16_resample_write(value: f64, big_endian: bool) -> u16 {
    let rounded = round_up(value) as i64;
    // Resample.c writes each byte through CLIP8 rather than clipping the
    // complete 16-bit result.  Keeping the byte-level operation matters for
    // negative-filter overshoot and for the exact I;16* output bytes.
    let low = clip_u8(rounded % 256);
    let high = clip_u8(rounded >> 8);
    let bytes = if big_endian { [high, low] } else { [low, high] };
    u16::from_ne_bytes(bytes)
}

fn horizontal_pass_luma16(
    src_row: &[u16],
    coeffs: &FilterCoeffsF64,
    out_w: u32,
    big_endian: bool,
    intermediate_row: &mut [u16],
) {
    for ox in 0..out_w as usize {
        let x0 = coeffs.xmin[ox];
        let cnt = coeffs.count[ox];
        if cnt == 0 {
            continue;
        }
        let mut acc = 0.0f64;
        for (cix, &weight) in coeffs.weights[ox].iter().enumerate() {
            let sx = (x0 + cix as i64) as usize;
            acc += luma16_resample_read(src_row[sx], big_endian) as f64 * weight;
        }
        intermediate_row[ox] = luma16_resample_write(acc, big_endian);
    }
}

fn vertical_pass_luma16(
    intermediate: &[u16],
    out_x: u32,
    out_w: u32,
    coeffs: &FilterCoeffsF64,
    out_y: usize,
    big_endian: bool,
) -> u16 {
    let y0 = coeffs.xmin[out_y];
    let cnt = coeffs.count[out_y];
    if cnt == 0 {
        return 0;
    }
    let mut acc = 0.0f64;
    for (cix, &weight) in coeffs.weights[out_y].iter().enumerate() {
        let sy = (y0 + cix as i64) as usize;
        let source = intermediate[sy * out_w as usize + out_x as usize];
        acc += luma16_resample_read(source, big_endian) as f64 * weight;
    }
    luma16_resample_write(acc, big_endian)
}

/// Resize `I;16*` through Pillow's native 16-bit two-pass resampler.
fn pil_resize_luma16(
    img: &ImageBuffer<Luma<u16>, Vec<u16>>,
    dst_w: u32,
    dst_h: u32,
    filter: ResampleFilter,
    explicit_mode: Option<&str>,
) -> DynamicImage {
    if matches!(filter, ResampleFilter::Nearest) {
        return pil_resize_luma16_nearest(img, dst_w, dst_h);
    }

    let (kernel, support) = filter_from_resample(filter);
    let h_coeffs = precompute_coeffs_f64(dst_w, img.width(), kernel, support);
    let v_coeffs = precompute_coeffs_f64(dst_h, img.height(), kernel, support);
    let big_endian = luma16_resample_big_endian(explicit_mode);

    let mut intermediate = ImageBuffer::<Luma<u16>, Vec<u16>>::new(img.height(), dst_w);
    {
        let intermediate_data: &mut [u16] = &mut *intermediate;
        for sy in 0..img.height() {
            let src_start = (sy * img.width()) as usize;
            let src_end = src_start + img.width() as usize;
            let intermediate_start = (sy * dst_w) as usize;
            let intermediate_end = intermediate_start + dst_w as usize;
            horizontal_pass_luma16(
                &img.as_raw()[src_start..src_end],
                &h_coeffs,
                dst_w,
                big_endian,
                &mut intermediate_data[intermediate_start..intermediate_end],
            );
        }
    }

    let mut output = ImageBuffer::<Luma<u16>, Vec<u16>>::new(dst_w, dst_h);
    for dy in 0..dst_h {
        for dx in 0..dst_w {
            let value = vertical_pass_luma16(
                intermediate.as_raw(),
                dx,
                dst_w,
                &v_coeffs,
                dy as usize,
                big_endian,
            );
            output.put_pixel(dx, dy, Luma([value]));
        }
    }

    DynamicImage::ImageLuma16(output)
}

/// PIL uses 22-bit fixed-point arithmetic (PRECISION_BITS=22) for weights
/// and intermediate accumulation. We match this exactly.
const PRECISION_BITS: u32 = 22;
const PRECISION: i64 = 1i64 << PRECISION_BITS; // 2^22
const HALF_PRECISION: i64 = 1i64 << (PRECISION_BITS - 1); // 2^21

/// Round a float to u8, matching PIL's fixed-point rounding:
///   `(int)(v + 0.5)` clamped to [0, 255]
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

/// Convert a fixed-point sum to u8, matching PIL's:
///   `(UINT8)((sum + (1 << (PRECISION_BITS - 1))) >> PRECISION_BITS)`
fn fixed_point_to_u8(sum: i64) -> u8 {
    let v = (sum + HALF_PRECISION) >> PRECISION_BITS;
    if v <= 0 {
        0
    } else if v >= 255 {
        255
    } else {
        v as u8
    }
}

// ── PIL-compatible pixel range and weight computation ──

/// Precompute filter coefficients for one dimension, using PIL's exact
/// fixed-point arithmetic (PRECISION_BITS=22).
///
/// For each output pixel, computes:
/// - xmin: first contributing source pixel index
/// - count: number of contributing source pixels
/// - weights: normalized filter weights in 22-bit fixed-point
///
/// This matches PIL's `precompute_coeffs`:
///   center = (xx + 0.5) * scale
///   xmin = (int)(center - support + 0.5)
///   xmax = (int)(center + support + 0.5)
///   weight = kernel((sx + 0.5 - center) * ss)  where ss = 1.0 / filterscale
pub(crate) struct FilterCoeffs {
    pub(crate) xmin: Vec<i64>,
    pub(crate) count: Vec<usize>,
    pub(crate) weights: Vec<Vec<i64>>, // 22-bit fixed-point weights
}

pub(crate) struct FilterCoeffsF64 {
    pub(crate) xmin: Vec<i64>,
    pub(crate) count: Vec<usize>,
    pub(crate) weights: Vec<Vec<f64>>, // double-precision weights (for I/F modes)
}

/// PIL's ROUND_UP: (int)((f) >= 0.0 ? (f) + 0.5 : (f) - 0.5)
pub(crate) fn round_up(f: f64) -> f64 {
    if f >= 0.0 {
        (f + 0.5).trunc()
    } else {
        (f - 0.5).trunc()
    }
}

/// Precompute f64 (double-precision) coefficients matching PIL's 32-bit image resample.
pub(crate) fn precompute_coeffs_f64(
    out_size: u32,
    in_size: u32,
    kernel: fn(f64) -> f64,
    support: f64,
) -> FilterCoeffsF64 {
    let scale = in_size as f64 / out_size as f64;
    let filterscale = scale.max(1.0);
    let ss = 1.0 / filterscale;
    let src_support = support * filterscale;
    let n = out_size as usize;
    let mut xmin = Vec::with_capacity(n);
    let mut count = Vec::with_capacity(n);
    let mut weights: Vec<Vec<f64>> = Vec::with_capacity(n);
    for ox in 0..n {
        let center = (ox as f64 + 0.5) * scale;
        let mut x0 = (center - src_support + 0.5).trunc() as i64;
        let mut x1 = (center + src_support + 0.5).trunc() as i64;
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
        let mut w: Vec<f64> = Vec::with_capacity(cnt);
        let mut wsum = 0.0;
        for ix in 0..cnt {
            let sx = x0 + ix as i64;
            let val = kernel((sx as f64 + 0.5 - center) * ss);
            w.push(val);
            wsum += val;
        }
        if wsum != 0.0 {
            for val in &mut w {
                *val /= wsum;
            }
        }
        weights.push(w);
    }
    FilterCoeffsF64 {
        xmin,
        count,
        weights,
    }
}

pub(crate) fn precompute_coeffs(
    out_size: u32,
    in_size: u32,
    kernel: fn(f64) -> f64,
    support: f64,
) -> FilterCoeffs {
    let scale = in_size as f64 / out_size as f64;
    _precompute_coeffs_impl(out_size, in_size, scale, kernel, support)
}

/// Precompute coefficients for a box-based resize (PIL's box parameter).
/// The box is a (left, top, right, bottom) tuple in source coordinates,
/// mapping to the output size. All coordinates are in the source image
/// coordinate system (floating point).
pub(crate) fn precompute_coeffs_boxed(
    out_size: u32,
    in_size: u32,
    box_start: f64,
    box_end: f64,
    kernel: fn(f64) -> f64,
    support: f64,
) -> FilterCoeffs {
    // Scale = box_length / output_size
    let box_length = box_end - box_start;
    let scale = box_length / out_size as f64;
    let filterscale = scale.max(1.0);
    let src_support = support * filterscale;

    let n = out_size as usize;
    let mut xmin = Vec::with_capacity(n);
    let mut count = Vec::with_capacity(n);
    let mut weights: Vec<Vec<i64>> = Vec::with_capacity(n);

    for ox in 0..n {
        // PIL: center = box_start + (ox + 0.5) * scale
        let center = box_start + (ox as f64 + 0.5) * scale;

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

        // Compute f64 weights, then convert to fixed-point
        let ss = 1.0 / filterscale;
        let mut w_f64 = Vec::with_capacity(cnt);
        let mut wsum = 0.0;
        for ix in 0..cnt {
            let sx = x0 + ix as i64;
            let val = kernel((sx as f64 + 0.5 - center) * ss);
            w_f64.push(val);
            wsum += val;
        }

        if wsum != 0.0 {
            for wi in w_f64.iter_mut() {
                *wi /= wsum;
            }
        }

        let w_fixed: Vec<i64> = w_f64
            .iter()
            .map(|&w| {
                let scaled = w * PRECISION as f64;
                let rounded = if w >= 0.0 { scaled + 0.5 } else { scaled - 0.5 };
                rounded as i64
            })
            .collect();

        weights.push(w_fixed);
    }

    FilterCoeffs {
        xmin,
        count,
        weights,
    }
}

/// Internal implementation with explicit scale, called by pil_resize (double scale).
fn _precompute_coeffs_impl(
    out_size: u32,
    in_size: u32,
    scale: f64,
    kernel: fn(f64) -> f64,
    support: f64,
) -> FilterCoeffs {
    let filterscale = scale.max(1.0);
    let ss = 1.0 / filterscale;
    let src_support = support * filterscale;

    let n = out_size as usize;
    let mut xmin = Vec::with_capacity(n);
    let mut count = Vec::with_capacity(n);
    let mut weights: Vec<Vec<i64>> = Vec::with_capacity(n);

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

        // Compute f64 weights, then convert to fixed-point
        let mut w_f64 = Vec::with_capacity(cnt);
        let mut wsum = 0.0;
        for ix in 0..cnt {
            let sx = x0 + ix as i64;
            // PIL: kernel((sx + 0.5 - center) * ss)
            let val = kernel((sx as f64 + 0.5 - center) * ss);
            w_f64.push(val);
            wsum += val;
        }

        // Normalize weights. PIL's C code divides each weight by the sum
        // in-place on the double buffer: kk[offset + i] /= wsum.
        // This is subtly different from multiplication by 1/wsum due to
        // floating-point rounding (one ULP difference).
        if wsum != 0.0 {
            for wi in w_f64.iter_mut() {
                *wi /= wsum;
            }
        }

        // Convert to fixed-point: (int)(weight * (1 << PRECISION_BITS) + (weight >= 0 ? 0.5 : -0.5))
        // PIL uses different rounding for positive and negative weights.
        let w_fixed: Vec<i64> = w_f64
            .iter()
            .map(|&w| {
                let scaled = w * PRECISION as f64;
                let rounded = if w >= 0.0 { scaled + 0.5 } else { scaled - 0.5 };
                rounded as i64
            })
            .collect();

        // NOTE: PIL does NOT adjust the fixed-point weights to sum exactly to
        // PRECISION. The normalizes weights are converted to fixed-point with
        // rounding (+0.5 for positive, -0.5 for negative) and used as-is.
        // Any small discrepancy from the ideal sum is absorbed by the
        // HALF_PRECISION bias added during accumulation.
        weights.push(w_fixed);
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
        DynamicImage::ImageRgba8(rgba) => {
            let mut out = rgba.clone();
            for p in out.pixels_mut() {
                let a = p[3] as f64 / 255.0;
                p[0] = (p[0] as f64 * a + 0.5) as u8;
                p[1] = (p[1] as f64 * a + 0.5) as u8;
                p[2] = (p[2] as f64 * a + 0.5) as u8;
            }
            DynamicImage::ImageRgba8(out)
        }
        DynamicImage::ImageLumaA8(la) => {
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
        DynamicImage::ImageRgba8(rgba) => {
            let mut out = rgba.clone();
            for p in out.pixels_mut() {
                let a = p[3] as f64;
                if a > 0.0 {
                    let inv = 255.0 / a;
                    // Pillow's RGBa -> RGBA conversion truncates the
                    // unpremultiplied channel; it does not round to nearest.
                    p[0] = (p[0] as f64 * inv) as u8;
                    p[1] = (p[1] as f64 * inv) as u8;
                    p[2] = (p[2] as f64 * inv) as u8;
                }
            }
            DynamicImage::ImageRgba8(out)
        }
        DynamicImage::ImageLumaA8(la) => {
            let mut out = la.clone();
            for p in out.pixels_mut() {
                let a = p[1] as f64;
                if a > 0.0 {
                    // Match Pillow's La -> LA conversion truncation.
                    p[0] = (p[0] as f64 * 255.0 / a) as u8;
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
/// from one row of the source image. Uses PIL's fixed-point arithmetic:
///   `result = (sum + (1 << (PRECISION_BITS - 1))) >> PRECISION_BITS`
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
        let weights = &coeffs.weights[ox];
        for c in 0..channels {
            let mut acc: i64 = 0;
            for (cix, &w) in weights.iter().enumerate() {
                let sx = (x0 + cix as i64) as usize;
                acc += src_row[sx * channels + c] as i64 * w;
            }
            intermediate_row[ox * channels + c] = fixed_point_to_u8(acc);
        }
    }
}

/// Resample one column into the output rows.
///
/// This is the vertical pass: it computes one column of the final image
/// from one column of the intermediate image. Uses PIL's fixed-point arithmetic.
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
    let weights = &coeffs.weights[out_y];
    let mut result = vec![0u8; channels];
    for c in 0..channels {
        let mut acc: i64 = 0;
        for (cix, &w) in weights.iter().enumerate() {
            let sy = (y0 + cix as i64) as usize;
            let src_idx = (sy * out_w as usize + out_x as usize) * channels;
            acc += intermediate[src_idx + c] as i64 * w;
        }
        result[c] = fixed_point_to_u8(acc);
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
        crate::raster::ColorType::L8 => DynamicImage::ImageLuma8(result.to_luma8()),
        crate::raster::ColorType::La8 => DynamicImage::ImageLumaA8(result.to_luma_alpha8()),
        crate::raster::ColorType::Rgb8 => DynamicImage::ImageRgb8(result.to_rgb8()),
        crate::raster::ColorType::Rgba8 => DynamicImage::ImageRgba8(result.to_rgba8()),
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

    // Pillow keeps I;16* samples in the native 16-bit resize path.  The
    // generic pixel accessor below is byte-oriented and would otherwise
    // convert this mode to RGBA8 before preserving only its mode label.
    if let DynamicImage::ImageLuma16(luma) = img {
        return pil_resize_luma16(luma, dst_w, dst_h, filter, explicit_mode);
    }

    // Retain original image for final mode preservation
    let orig_img = img;

    // CMYK/F/I stored as RGBA8 but 4th channel is NOT alpha (K/float/int byte).
    // RGBa is already premultiplied storage, so PIL resamples its channels
    // directly rather than premultiplying them a second time.
    // PIL does NOT premultiply alpha for these modes.
    let is_cmyk = explicit_mode == Some("CMYK");
    let is_fi = explicit_mode == Some("F") || explicit_mode == Some("I");
    let needs_alpha = !is_cmyk
        && !is_fi
        && explicit_mode != Some("RGBa")
        && matches!(
            img.color(),
            crate::raster::ColorType::Rgba8 | crate::raster::ColorType::La8
        );
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
        crate::raster::ColorType::L8 => 1usize,
        crate::raster::ColorType::La8 => 2usize,
        crate::raster::ColorType::Rgb8 => 3usize,
        _ => 4usize,
    };

    // PIL's _resize C code uses ImagingTransform with AFFINE for NEAREST filter
    // (single-pixel sampling), NOT the two-pass pipeline. Box and all other filters
    // go through ImagingResample (two-pass convolution).
    // The AFFINE formula is:
    //   xin = a[0] * (x + 0.5) + a[2]   (a[2] = box[0] = 0)
    //   ix = (int)floor(xin)
    // A tiny epsilon is subtracted because the C code computes the scale factor
    // using float then double promotion, causing exact-integer boundaries to
    // nudge down by ~1e-15.
    if matches!(filter, ResampleFilter::Nearest) {
        // PIL's NEAREST resize uses ImagingScaleAffine with cumulative f64 stepping.
        // From _imaging.c for NEAREST filter:
        //   a[0] = (double)(box[2] - box[0]) / xsize   (= sw / dw)
        //   a[2] = box[0]                                (= 0)
        // Then: xo = a[2] + a[0] * 0.5
        //       for each x: xin = (int)(xo); xo += a[0]
        let scale_x = sw as f64 / dw as f64;
        let scale_y = sh as f64 / dh as f64;
        let n = (dw * dh) as usize;
        let mut out_bytes: Vec<u8> = Vec::with_capacity(n * channels);
        // Precompute x-mapping table matching PIL's xintab approach
        let mut xintab: Vec<u32> = Vec::with_capacity(dw as usize);
        let mut xo = scale_x * 0.5;
        for _dx in 0..dw {
            let xi = xo as u32;
            xintab.push(if xi >= sw { sw - 1 } else { xi });
            xo += scale_x;
        }
        // PIL also uses cumulative stepping for y: yo = a[4] * 0.5
        let mut yo = scale_y * 0.5;
        for _dy in 0..dh {
            let sy = if yo >= sh as f64 { sh - 1 } else { yo as u32 };
            for dx in 0..dw {
                let sx = xintab[dx as usize];
                let p = pixel_at(&work, sx, sy);
                for c in 0..channels {
                    // `pixel_at` exposes grayscale-alpha pixels as RGBA
                    // (`[luma, luma, luma, alpha]`), while the resize buffer
                    // keeps their native two-byte `[luma, alpha]` layout.
                    // Select the alpha lane explicitly so PA/LA nearest
                    // resize does not copy luma into the alpha sample.
                    let rgba_channel = if channels == 2 && c == 1 { 3 } else { c };
                    out_bytes.push(pil_round(p[rgba_channel]));
                }
            }
            yo += scale_y;
        }
        let result = raw_to_dynamic(&out_bytes, dw, dh, channels);
        let result = if needs_alpha {
            unpremultiply_alpha(&result)
        } else {
            result
        };
        return pil_preserve_mode(orig_img, result);
    }

    // Precompute horizontal and vertical coefficients for two-pass pipeline
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

/// Box-based resize: maps source region [box_left, box_right] × [box_top, box_bottom]
/// to the output (dst_w, dst_h). All box coordinates are in source pixel coordinates.
pub fn pil_resize_boxed(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    filter: ResampleFilter,
    explicit_mode: Option<&str>,
) -> DynamicImage {
    let orig_img = img;
    let is_cmyk = explicit_mode == Some("CMYK");
    let is_fi = explicit_mode == Some("F") || explicit_mode == Some("I");
    let needs_alpha = !is_cmyk
        && !is_fi
        && explicit_mode != Some("RGBa")
        && matches!(
            img.color(),
            crate::raster::ColorType::Rgba8 | crate::raster::ColorType::La8
        );
    let work = if needs_alpha {
        premultiply_alpha(img)
    } else {
        img.clone()
    };

    let (kernel_fn, support) = filter_from_resample(filter);
    let (sw, sh) = (work.width(), work.height());

    let channels = match work.color() {
        crate::raster::ColorType::L8 => 1usize,
        crate::raster::ColorType::La8 => 2usize,
        crate::raster::ColorType::Rgb8 => 3usize,
        _ => 4usize,
    };

    // Use box-parameter coefficients for both passes
    let h_coeffs = precompute_coeffs_boxed(dst_w, sw, box_left, box_right, kernel_fn, support);
    let v_coeffs = precompute_coeffs_boxed(dst_h, sh, box_top, box_bottom, kernel_fn, support);

    // Allocate intermediate image (sh rows × dw columns × channels)
    let mut intermediate = vec![0u8; (sh * dst_w) as usize * channels];

    // Horizontal pass
    let work_bytes = work.as_bytes();
    for sy in 0..sh {
        let src_start = (sy * sw) as usize * channels;
        let src_row = &work_bytes[src_start..src_start + sw as usize * channels];
        let inter_start = (sy * dst_w) as usize * channels;
        let inter_row = &mut intermediate[inter_start..inter_start + dst_w as usize * channels];
        horizontal_pass_row(src_row, sw, channels, &h_coeffs, dst_w, inter_row);
    }

    // Allocate output image
    let mut out_bytes = vec![0u8; (dst_w * dst_h) as usize * channels];

    // Vertical pass
    for dy in 0..dst_h {
        let out_start = (dy * dst_w) as usize * channels;
        let out_row = &mut out_bytes[out_start..out_start + dst_w as usize * channels];
        for dx in 0..dst_w {
            let vert_result = vertical_pass_col(
                &intermediate,
                sh,
                dx,
                dst_w,
                channels,
                &v_coeffs,
                dy as usize,
            );
            let dest = &mut out_row[dx as usize * channels..(dx as usize + 1) * channels];
            dest[..channels].copy_from_slice(&vert_result[..channels]);
        }
    }

    let result = raw_to_dynamic(&out_bytes, dst_w, dst_h, channels);
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
