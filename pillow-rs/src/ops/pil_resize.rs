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
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

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
        // Resample.c declares the window constants with an `f` suffix.
        // Preserve that float-to-double promotion: using f64 literals moves
        // the final F-mode sample by one ULP.
        (pix.sin() / pix) * ((0.54_f32 as f64) + (0.46_f32 as f64) * pix.cos())
    }
}

fn kernel_lanczos3(x: f64) -> f64 {
    kernel_lanczos(x, 3.0)
}

/// Choose kernel function and support based on filter type.
/// Returns the kernel and support used by the generic scalar resampler.
/// Backend adapters use this only to build the same scalar coefficient table;
/// pixel accumulation remains in the selected backend.
pub(crate) fn filter_from_resample(filter: ResampleFilter) -> (fn(f64) -> f64, f64) {
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
pub(crate) fn luma16_resample_big_endian(mode: Option<&str>) -> bool {
    match mode {
        // Pillow's Resample.c deliberately selects the big-endian branch for
        // I;16N.  On little-endian hosts this is observable as the historical
        // native-mode byte ordering of the convolution path.
        Some("I;16N") => true,
        Some("I;16B") => true,
        _ => false,
    }
}

pub(crate) fn luma16_resample_read(sample: u16, big_endian: bool) -> u16 {
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

pub(crate) fn luma16_resample_write(value: f64, big_endian: bool) -> u16 {
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
    pub(crate) offsets: Vec<usize>,
    pub(crate) weights: Vec<i64>, // 22-bit fixed-point weights, flattened by output pixel
}

impl FilterCoeffs {
    #[inline]
    fn weights_for(&self, index: usize) -> &[i64] {
        let start = self.offsets[index];
        &self.weights[start..start + self.count[index]]
    }
}

pub(crate) struct FilterCoeffsF64 {
    pub(crate) xmin: Vec<i64>,
    pub(crate) count: Vec<usize>,
    pub(crate) weights: Vec<Vec<f64>>, // double-precision weights (for I/F modes)
}

struct FilterCoeffsF64Entry {
    key: FilterCoeffsF64Key,
    coeffs: Arc<FilterCoeffsF64>,
    bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FilterCoeffsF64Key {
    input_size: u32,
    output_size: u32,
    kernel: usize,
    support_bits: u64,
}

struct FilterCoeffsF64Cache {
    entries: VecDeque<FilterCoeffsF64Entry>,
    retained_bytes: usize,
}

static FILTER_COEFF_F64_CACHE: OnceLock<Mutex<FilterCoeffsF64Cache>> = OnceLock::new();

fn filter_coeff_f64_cache() -> &'static Mutex<FilterCoeffsF64Cache> {
    FILTER_COEFF_F64_CACHE.get_or_init(|| {
        Mutex::new(FilterCoeffsF64Cache {
            entries: VecDeque::new(),
            retained_bytes: 0,
        })
    })
}

/// Stable cache identity for the ordinary (unboxed) resize geometry.
///
/// Boxed resizes carry floating-point crop coordinates and keep their separate
/// exact path.  The common `Image.resize` path has only integer dimensions and
/// a finite filter enum, so this key is complete without hashing function
/// pointers or floating-point values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FilterCoeffsKey {
    input_size: u32,
    output_size: u32,
    filter: u8,
}

struct FilterCoeffsEntry {
    key: FilterCoeffsKey,
    coeffs: Arc<FilterCoeffs>,
    bytes: usize,
}

struct FilterCoeffsCache {
    entries: VecDeque<FilterCoeffsEntry>,
    retained_bytes: usize,
}

const FILTER_COEFF_CACHE_CAPACITY: usize = 16;
const FILTER_COEFF_CACHE_BYTES: usize = 8 * 1024 * 1024;

static FILTER_COEFF_CACHE: OnceLock<Mutex<FilterCoeffsCache>> = OnceLock::new();

fn filter_coeff_cache() -> &'static Mutex<FilterCoeffsCache> {
    FILTER_COEFF_CACHE.get_or_init(|| {
        Mutex::new(FilterCoeffsCache {
            entries: VecDeque::new(),
            retained_bytes: 0,
        })
    })
}

fn filter_cache_id(filter: ResampleFilter) -> u8 {
    match filter {
        ResampleFilter::Nearest => 0,
        ResampleFilter::Bilinear => 1,
        ResampleFilter::Bicubic => 2,
        ResampleFilter::Lanczos => 3,
        ResampleFilter::Box => 4,
        ResampleFilter::Hamming => 5,
    }
}

fn filter_coeff_bytes(coeffs: &FilterCoeffs) -> usize {
    coeffs
        .xmin
        .len()
        .saturating_mul(std::mem::size_of::<i64>())
        .saturating_add(
            coeffs
                .count
                .len()
                .saturating_mul(std::mem::size_of::<usize>()),
        )
        .saturating_add(
            coeffs
                .offsets
                .len()
                .saturating_mul(std::mem::size_of::<usize>()),
        )
        .saturating_add(
            coeffs
                .weights
                .len()
                .saturating_mul(std::mem::size_of::<i64>()),
        )
}

fn filter_coeff_f64_bytes(coeffs: &FilterCoeffsF64) -> usize {
    coeffs
        .xmin
        .len()
        .saturating_mul(std::mem::size_of::<i64>())
        .saturating_add(
            coeffs
                .count
                .len()
                .saturating_mul(std::mem::size_of::<usize>()),
        )
        .saturating_add(
            coeffs
                .weights
                .iter()
                .map(|weights| weights.len().saturating_mul(std::mem::size_of::<f64>()))
                .sum::<usize>(),
        )
}

fn cache_filter_coeffs(key: FilterCoeffsKey, coeffs: Arc<FilterCoeffs>) -> Arc<FilterCoeffs> {
    let bytes = filter_coeff_bytes(&coeffs);
    if bytes > FILTER_COEFF_CACHE_BYTES {
        return coeffs;
    }

    let Ok(mut cache) = filter_coeff_cache().lock() else {
        return coeffs;
    };
    while cache.entries.len() >= FILTER_COEFF_CACHE_CAPACITY
        || cache.retained_bytes.saturating_add(bytes) > FILTER_COEFF_CACHE_BYTES
    {
        let Some(entry) = cache.entries.pop_back() else {
            break;
        };
        cache.retained_bytes = cache.retained_bytes.saturating_sub(entry.bytes);
    }
    cache.retained_bytes = cache.retained_bytes.saturating_add(bytes);
    cache.entries.push_front(FilterCoeffsEntry {
        key,
        coeffs: Arc::clone(&coeffs),
        bytes,
    });
    coeffs
}

fn cached_filter_coeffs(
    input_size: u32,
    output_size: u32,
    filter: ResampleFilter,
) -> Arc<FilterCoeffs> {
    let key = FilterCoeffsKey {
        input_size,
        output_size,
        filter: filter_cache_id(filter),
    };
    if let Ok(mut cache) = filter_coeff_cache().lock() {
        if let Some(index) = cache.entries.iter().position(|entry| entry.key == key) {
            let entry = cache
                .entries
                .remove(index)
                .expect("resize coefficient cache entry disappeared");
            let coeffs = Arc::clone(&entry.coeffs);
            cache.entries.push_front(entry);
            crate::compute::record_pipeline_resize_coeff_cache_hit();
            return coeffs;
        }
    }

    crate::compute::record_pipeline_resize_coeff_cache_miss();
    let (kernel, support) = filter_from_resample(filter);
    let coeffs = Arc::new(_precompute_coeffs_impl(
        output_size,
        input_size,
        input_size as f64 / output_size as f64,
        kernel,
        support,
    ));
    cache_filter_coeffs(key, coeffs)
}

fn cache_filter_coeffs_f64(
    key: FilterCoeffsF64Key,
    coeffs: Arc<FilterCoeffsF64>,
) -> Arc<FilterCoeffsF64> {
    let bytes = filter_coeff_f64_bytes(&coeffs);
    if bytes > FILTER_COEFF_CACHE_BYTES {
        return coeffs;
    }

    let Ok(mut cache) = filter_coeff_f64_cache().lock() else {
        return coeffs;
    };
    while cache.entries.len() >= FILTER_COEFF_CACHE_CAPACITY
        || cache.retained_bytes.saturating_add(bytes) > FILTER_COEFF_CACHE_BYTES
    {
        let Some(entry) = cache.entries.pop_back() else {
            break;
        };
        cache.retained_bytes = cache.retained_bytes.saturating_sub(entry.bytes);
    }
    cache.retained_bytes = cache.retained_bytes.saturating_add(bytes);
    cache.entries.push_front(FilterCoeffsF64Entry {
        key,
        coeffs: Arc::clone(&coeffs),
        bytes,
    });
    coeffs
}

fn cached_filter_coeffs_f64(
    input_size: u32,
    output_size: u32,
    kernel: fn(f64) -> f64,
    support: f64,
) -> Arc<FilterCoeffsF64> {
    let key = FilterCoeffsF64Key {
        input_size,
        output_size,
        kernel: kernel as usize,
        support_bits: support.to_bits(),
    };
    if let Ok(mut cache) = filter_coeff_f64_cache().lock() {
        if let Some(index) = cache.entries.iter().position(|entry| entry.key == key) {
            let entry = cache
                .entries
                .remove(index)
                .expect("f64 resize coefficient cache entry disappeared");
            let coeffs = Arc::clone(&entry.coeffs);
            cache.entries.push_front(entry);
            crate::compute::record_pipeline_resize_coeff_cache_hit();
            return coeffs;
        }
    }

    crate::compute::record_pipeline_resize_coeff_cache_miss();
    let coeffs = Arc::new(_precompute_coeffs_f64_impl(
        output_size,
        input_size,
        kernel,
        support,
    ));
    cache_filter_coeffs_f64(key, coeffs)
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
fn _precompute_coeffs_f64_impl(
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

pub(crate) fn precompute_coeffs_f64(
    out_size: u32,
    in_size: u32,
    kernel: fn(f64) -> f64,
    support: f64,
) -> Arc<FilterCoeffsF64> {
    cached_filter_coeffs_f64(in_size, out_size, kernel, support)
}

/// Precompute double-precision coefficients for a resize with a fractional
/// source box. Pillow receives these boundaries as `float` before computing
/// the f64 kernel centers, just as it does for the byte boxed-resample path.
pub(crate) fn precompute_coeffs_f64_boxed(
    out_size: u32,
    in_size: u32,
    box_start: f64,
    box_end: f64,
    filter: ResampleFilter,
) -> FilterCoeffsF64 {
    let (kernel, support) = filter_from_resample(filter);
    let box_start = box_start as f32 as f64;
    let box_end = box_end as f32 as f64;
    let scale = (box_end as f32 - box_start as f32) as f64 / f64::from(out_size);
    let filterscale = scale.max(1.0);
    let src_support = support * filterscale;
    let source_size = i64::from(in_size);
    let mut xmin = Vec::with_capacity(out_size as usize);
    let mut count = Vec::with_capacity(out_size as usize);
    let mut weights = Vec::with_capacity(out_size as usize);
    for output in 0..out_size as usize {
        let center = box_start + (output as f64 + 0.5) * scale;
        let mut x0 = (center - src_support + 0.5).trunc() as i64;
        let mut x1 = (center + src_support + 0.5).trunc() as i64;
        x0 = x0.max(0);
        x1 = x1.min(source_size);
        let sample_count = (x1 - x0).max(0) as usize;
        xmin.push(x0);
        count.push(sample_count);
        let mut row_weights = Vec::with_capacity(sample_count);
        let mut sum = 0.0;
        let ss = 1.0 / filterscale;
        for tap in 0..sample_count {
            let value = kernel((x0 as f64 + tap as f64 + 0.5 - center) * ss);
            row_weights.push(value);
            sum += value;
        }
        if sum != 0.0 {
            for value in &mut row_weights {
                *value /= sum;
            }
        }
        weights.push(row_weights);
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
    filter: ResampleFilter,
) -> Arc<FilterCoeffs> {
    cached_filter_coeffs(in_size, out_size, filter)
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
    // Pillow's ImagingResample ABI receives the box coordinates as `float`
    // before `Resample.c::precompute_coeffs` computes its double-precision
    // centers. Preserve that first-divergence conversion: keeping the
    // Python-provided f64 values changes fixed-point weights at boundaries.
    let box_start_f32 = box_start as f32;
    let box_end_f32 = box_end as f32;
    let box_start = box_start_f32 as f64;
    // Scale = (float)(box_end - box_start) / output_size, as in Pillow.
    let box_length = (box_end_f32 - box_start_f32) as f64;
    let scale = box_length / out_size as f64;
    let filterscale = scale.max(1.0);
    let src_support = support * filterscale;

    let n = out_size as usize;
    let mut xmin = Vec::with_capacity(n);
    let mut count = Vec::with_capacity(n);
    let mut offsets = Vec::with_capacity(n);
    let mut weights: Vec<i64> = Vec::with_capacity(n * support.ceil() as usize);

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
        offsets.push(weights.len());

        if cnt == 0 {
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

        weights.extend(w_f64.iter().map(|&w| {
            let scaled = w * PRECISION as f64;
            let rounded = if w >= 0.0 { scaled + 0.5 } else { scaled - 0.5 };
            rounded as i64
        }));
    }

    FilterCoeffs {
        xmin,
        count,
        offsets,
        weights,
    }
}

/// Precompute box-resize coefficients for a public resampling filter.
///
/// The SIMD adapter uses the same Pillow-compatible coefficient builder as
/// the scalar resampler; keeping kernel selection here prevents a second
/// boxed-filter implementation from drifting at fixed-point boundaries.
pub(crate) fn precompute_coeffs_boxed_for_filter(
    out_size: u32,
    in_size: u32,
    box_start: f64,
    box_end: f64,
    filter: ResampleFilter,
) -> FilterCoeffs {
    let (kernel, support) = filter_from_resample(filter);
    precompute_coeffs_boxed(out_size, in_size, box_start, box_end, kernel, support)
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
    let mut offsets = Vec::with_capacity(n);
    let mut weights: Vec<i64> = Vec::with_capacity(n * support.ceil() as usize);

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
        offsets.push(weights.len());

        if cnt == 0 {
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
        let w_fixed = w_f64.iter().map(|&w| {
            let scaled = w * PRECISION as f64;
            let rounded = if w >= 0.0 { scaled + 0.5 } else { scaled - 0.5 };
            rounded as i64
        });

        // NOTE: PIL does NOT adjust the fixed-point weights to sum exactly to
        // PRECISION. The normalizes weights are converted to fixed-point with
        // rounding (+0.5 for positive, -0.5 for negative) and used as-is.
        // Any small discrepancy from the ideal sum is absorbed by the
        // HALF_PRECISION bias added during accumulation.
        weights.extend(w_fixed);
    }

    FilterCoeffs {
        xmin,
        count,
        offsets,
        weights,
    }
}

// ── Alpha premultiplication ──

pub(crate) fn premultiply_alpha(img: &DynamicImage) -> DynamicImage {
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

pub(crate) fn unpremultiply_alpha(img: &DynamicImage) -> DynamicImage {
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
        let weights = coeffs.weights_for(ox);
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

#[inline]
fn premultiply_channel(value: u8, alpha: u8) -> u8 {
    ((value as u16 * alpha as u16 + 127) / 255) as u8
}

/// Resample an alpha-bearing row while premultiplying color samples as they
/// enter the fixed-point accumulator.  This preserves the old
/// `premultiply_alpha` rounding contract without materializing a second image.
fn horizontal_pass_row_alpha(
    src_row: &[u8],
    channels: usize,
    coeffs: &FilterCoeffs,
    out_w: u32,
    intermediate_row: &mut [u8],
) {
    debug_assert!(matches!(channels, 2 | 4));
    let alpha_channel = channels - 1;
    for ox in 0..out_w as usize {
        let x0 = coeffs.xmin[ox];
        let cnt = coeffs.count[ox];
        if cnt == 0 {
            continue;
        }
        let weights = coeffs.weights_for(ox);
        for c in 0..channels {
            let mut acc: i64 = 0;
            for (cix, &w) in weights.iter().enumerate() {
                let sx = (x0 + cix as i64) as usize;
                let pixel_start = sx * channels;
                let source = src_row[pixel_start + c];
                let sample = if c == alpha_channel {
                    source
                } else {
                    premultiply_channel(source, src_row[pixel_start + alpha_channel])
                };
                acc += sample as i64 * w;
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
) -> [u8; 4] {
    let y0 = coeffs.xmin[out_y];
    let cnt = coeffs.count[out_y];
    if cnt == 0 {
        return [0u8; 4];
    }
    let weights = coeffs.weights_for(out_y);
    let mut result = [0u8; 4];
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

// A row-major intermediate makes the vertical pass revisit a different cache
// line for every source row and output column.  Above this size, one explicit
// transpose makes the samples for each output column contiguous.  Keep the
// small-image path unchanged: the extra allocation and copy are not amortized
// there, and preserving that path gives the benchmark a real crossover point.
const RESIZE_VERTICAL_TRANSPOSE_THRESHOLD: usize = 512 * 512;

#[inline]
fn should_transpose_vertical(source_rows: u32, output_width: u32, channels: usize) -> bool {
    matches!(channels, 1..=4)
        && source_rows > 1
        && output_width > 1
        && (source_rows as usize).saturating_mul(output_width as usize)
            >= RESIZE_VERTICAL_TRANSPOSE_THRESHOLD
}

/// Reorder the horizontal-pass result from `[source_y][output_x][channel]` to
/// `[output_x][source_y][channel]` for the cache-local vertical pass.
fn transpose_resize_intermediate(
    source: &[u8],
    source_rows: u32,
    output_width: u32,
    channels: usize,
) -> Vec<u8> {
    let source_row_stride = output_width as usize * channels;
    let destination_row_stride = source_rows as usize * channels;
    let mut destination = vec![0u8; source.len()];

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        &mut destination,
        destination_row_stride,
        output_width as usize,
        |_row_start, _row_end, x, row| {
            let x = x as usize;
            for y in 0..source_rows as usize {
                let source_start = y * source_row_stride + x * channels;
                let destination_start = y * channels;
                row[destination_start..destination_start + channels]
                    .copy_from_slice(&source[source_start..source_start + channels]);
            }
        }
    );

    #[cfg(not(feature = "parallel"))]
    for x in 0..output_width as usize {
        let row_start = x * destination_row_stride;
        let row = &mut destination[row_start..row_start + destination_row_stride];
        for y in 0..source_rows as usize {
            let source_start = y * source_row_stride + x * channels;
            let destination_start = y * channels;
            row[destination_start..destination_start + channels]
                .copy_from_slice(&source[source_start..source_start + channels]);
        }
    }

    destination
}

#[inline]
fn vertical_pass_col_transposed(
    intermediate: &[u8],
    source_rows: u32,
    out_x: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    out_y: usize,
) -> [u8; 4] {
    let y0 = coeffs.xmin[out_y];
    let cnt = coeffs.count[out_y];
    if cnt == 0 {
        return [0u8; 4];
    }
    let weights = coeffs.weights_for(out_y);
    let column_start = out_x as usize * source_rows as usize * channels;
    let mut result = [0u8; 4];
    for c in 0..channels {
        let mut acc: i64 = 0;
        for (cix, &w) in weights.iter().enumerate() {
            let sy = (y0 + cix as i64) as usize;
            let src_idx = column_start + sy * channels;
            acc += intermediate[src_idx + c] as i64 * w;
        }
        result[c] = fixed_point_to_u8(acc);
    }
    result
}

fn horizontal_pass_rows(
    work_bytes: &[u8],
    source_width: u32,
    source_height: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    output_width: u32,
    intermediate: &mut [u8],
) {
    let source_stride = source_width as usize * channels;
    let output_stride = output_width as usize * channels;
    if source_height == 0 || output_stride == 0 {
        return;
    }

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        intermediate,
        output_stride,
        source_height as usize,
        |row_start, _row_end, y, row| {
            let source_start = y as usize * source_stride;
            horizontal_pass_row(
                &work_bytes[source_start..source_start + source_stride],
                source_width,
                channels,
                coeffs,
                output_width,
                &mut row[..output_stride],
            );
            debug_assert_eq!(row_start, y as usize * output_stride);
        }
    );

    #[cfg(not(feature = "parallel"))]
    for y in 0..source_height as usize {
        let source_start = y * source_stride;
        let output_start = y * output_stride;
        horizontal_pass_row(
            &work_bytes[source_start..source_start + source_stride],
            source_width,
            channels,
            coeffs,
            output_width,
            &mut intermediate[output_start..output_start + output_stride],
        );
    }
}

fn horizontal_pass_rows_alpha(
    source: &[u8],
    source_width: u32,
    source_height: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    output_width: u32,
    intermediate: &mut [u8],
) {
    let source_stride = source_width as usize * channels;
    let output_stride = output_width as usize * channels;
    if source_height == 0 || output_stride == 0 {
        return;
    }

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        intermediate,
        output_stride,
        source_height as usize,
        |_row_start, _row_end, y, row| {
            let source_start = y as usize * source_stride;
            horizontal_pass_row_alpha(
                &source[source_start..source_start + source_stride],
                channels,
                coeffs,
                output_width,
                &mut row[..output_stride],
            );
        }
    );

    #[cfg(not(feature = "parallel"))]
    for y in 0..source_height as usize {
        let source_start = y * source_stride;
        let output_start = y * output_stride;
        horizontal_pass_row_alpha(
            &source[source_start..source_start + source_stride],
            channels,
            coeffs,
            output_width,
            &mut intermediate[output_start..output_start + output_stride],
        );
    }
}

fn vertical_pass_rows(
    intermediate: &[u8],
    source_rows: u32,
    output_width: u32,
    output_height: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    output: &mut [u8],
) {
    let output_stride = output_width as usize * channels;
    if output_height == 0 || output_stride == 0 {
        return;
    }

    if should_transpose_vertical(source_rows, output_width, channels) {
        let transposed =
            transpose_resize_intermediate(intermediate, source_rows, output_width, channels);
        vertical_pass_rows_transposed(
            &transposed,
            source_rows,
            output_width,
            output_height,
            channels,
            coeffs,
            output,
        );
        return;
    }

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        output,
        output_stride,
        output_height as usize,
        |_row_start, _row_end, y, row| {
            for dx in 0..output_width {
                let value = vertical_pass_col(
                    intermediate,
                    source_rows,
                    dx,
                    output_width,
                    channels,
                    coeffs,
                    y as usize,
                );
                let start = dx as usize * channels;
                row[start..start + channels].copy_from_slice(&value[..channels]);
            }
        }
    );

    #[cfg(not(feature = "parallel"))]
    for y in 0..output_height as usize {
        let output_start = y * output_stride;
        let row = &mut output[output_start..output_start + output_stride];
        for dx in 0..output_width {
            let value = vertical_pass_col(
                intermediate,
                source_rows,
                dx,
                output_width,
                channels,
                coeffs,
                y,
            );
            let start = dx as usize * channels;
            row[start..start + channels].copy_from_slice(&value[..channels]);
        }
    }
}

fn vertical_pass_rows_transposed(
    intermediate: &[u8],
    source_rows: u32,
    output_width: u32,
    output_height: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    output: &mut [u8],
) {
    let output_stride = output_width as usize * channels;

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        output,
        output_stride,
        output_height as usize,
        |_row_start, _row_end, y, row| {
            for dx in 0..output_width {
                let value = vertical_pass_col_transposed(
                    intermediate,
                    source_rows,
                    dx,
                    channels,
                    coeffs,
                    y as usize,
                );
                let start = dx as usize * channels;
                row[start..start + channels].copy_from_slice(&value[..channels]);
            }
        }
    );

    #[cfg(not(feature = "parallel"))]
    for y in 0..output_height as usize {
        let output_start = y * output_stride;
        let row = &mut output[output_start..output_start + output_stride];
        for dx in 0..output_width {
            let value =
                vertical_pass_col_transposed(intermediate, source_rows, dx, channels, coeffs, y);
            let start = dx as usize * channels;
            row[start..start + channels].copy_from_slice(&value[..channels]);
        }
    }
}

#[inline]
fn unpremultiply_channel(value: u8, alpha: u8) -> u8 {
    if alpha > 0 {
        // Preserve Pillow's truncating unpremultiply operation.
        (value as f64 * 255.0 / alpha as f64) as u8
    } else {
        value
    }
}

fn vertical_pass_rows_alpha(
    intermediate: &[u8],
    source_rows: u32,
    output_width: u32,
    output_height: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    output: &mut [u8],
) {
    debug_assert!(matches!(channels, 2 | 4));
    let output_stride = output_width as usize * channels;
    let alpha_channel = channels - 1;
    if output_height == 0 || output_stride == 0 {
        return;
    }

    if should_transpose_vertical(source_rows, output_width, channels) {
        let transposed =
            transpose_resize_intermediate(intermediate, source_rows, output_width, channels);
        vertical_pass_rows_alpha_transposed(
            &transposed,
            source_rows,
            output_width,
            output_height,
            channels,
            coeffs,
            output,
        );
        return;
    }

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        output,
        output_stride,
        output_height as usize,
        |_row_start, _row_end, y, row| {
            let y = y as usize;
            for dx in 0..output_width {
                let value = vertical_pass_col(
                    intermediate,
                    source_rows,
                    dx,
                    output_width,
                    channels,
                    coeffs,
                    y,
                );
                let start = dx as usize * channels;
                let alpha = value[alpha_channel];
                for c in 0..channels {
                    row[start + c] = if c == alpha_channel {
                        alpha
                    } else {
                        unpremultiply_channel(value[c], alpha)
                    };
                }
            }
        }
    );

    #[cfg(not(feature = "parallel"))]
    for y in 0..output_height as usize {
        let output_start = y * output_stride;
        let row = &mut output[output_start..output_start + output_stride];
        for dx in 0..output_width {
            let value = vertical_pass_col(
                intermediate,
                source_rows,
                dx,
                output_width,
                channels,
                coeffs,
                y,
            );
            let start = dx as usize * channels;
            let alpha = value[alpha_channel];
            for c in 0..channels {
                row[start + c] = if c == alpha_channel {
                    alpha
                } else {
                    unpremultiply_channel(value[c], alpha)
                };
            }
        }
    }
}

fn vertical_pass_rows_alpha_transposed(
    intermediate: &[u8],
    source_rows: u32,
    output_width: u32,
    output_height: u32,
    channels: usize,
    coeffs: &FilterCoeffs,
    output: &mut [u8],
) {
    debug_assert!(matches!(channels, 2 | 4));
    let output_stride = output_width as usize * channels;
    let alpha_channel = channels - 1;

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        output,
        output_stride,
        output_height as usize,
        |_row_start, _row_end, y, row| {
            let y = y as usize;
            for dx in 0..output_width {
                let value = vertical_pass_col_transposed(
                    intermediate,
                    source_rows,
                    dx,
                    channels,
                    coeffs,
                    y,
                );
                let start = dx as usize * channels;
                let alpha = value[alpha_channel];
                for c in 0..channels {
                    row[start + c] = if c == alpha_channel {
                        alpha
                    } else {
                        unpremultiply_channel(value[c], alpha)
                    };
                }
            }
        }
    );

    #[cfg(not(feature = "parallel"))]
    for y in 0..output_height as usize {
        let output_start = y * output_stride;
        let row = &mut output[output_start..output_start + output_stride];
        for dx in 0..output_width {
            let value =
                vertical_pass_col_transposed(intermediate, source_rows, dx, channels, coeffs, y);
            let start = dx as usize * channels;
            let alpha = value[alpha_channel];
            for c in 0..channels {
                row[start + c] = if c == alpha_channel {
                    alpha
                } else {
                    unpremultiply_channel(value[c], alpha)
                };
            }
        }
    }
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
    // Nearest-neighbour resizing copies one source sample directly.  Pillow's
    // ImagingScaleAffine path does not premultiply LA/RGBA for that filter;
    // doing so here would round a constant (173, 127) or (17, 83, 149, 127)
    // down by one during the unnecessary premultiply/unpremultiply cycle.
    let needs_alpha = !matches!(filter, ResampleFilter::Nearest)
        && !is_cmyk
        && !is_fi
        && explicit_mode != Some("RGBa")
        && explicit_mode != Some("PA")
        && matches!(
            img.color(),
            crate::raster::ColorType::Rgba8 | crate::raster::ColorType::La8
        );
    let (sw, sh) = (img.width(), img.height());
    let (dw, dh) = (dst_w, dst_h);

    // Determine channel count
    let channels = match img.color() {
        crate::raster::ColorType::L8 => 1usize,
        crate::raster::ColorType::La8 => 2usize,
        crate::raster::ColorType::Rgb8 => 3usize,
        _ => 4usize,
    };

    // Pillow's ImagingResample and ImagingScaleAffine both preserve an
    // all-zero native byte image exactly: every weighted sample is zero and
    // the destination has no edge or alpha work to perform.  The generic
    // two-pass loops still build and walk both coefficient tables, which was
    // the first CPU divergence in the small ImageOps.contain/cover rows.
    // Keep this bounded to byte-backed layouts; typed F/I paths have their
    // own representation-preserving fast paths below.
    let native_byte_image = matches!(
        img,
        DynamicImage::ImageLuma8(_)
            | DynamicImage::ImageLumaA8(_)
            | DynamicImage::ImageRgb8(_)
            | DynamicImage::ImageRgba8(_)
    );
    if native_byte_image && img.as_bytes().iter().all(|&value| value == 0) {
        let output_len = (dw as usize)
            .checked_mul(dh as usize)
            .and_then(|pixels| pixels.checked_mul(channels))
            .unwrap_or(0);
        let result = raw_to_dynamic(&vec![0; output_len], dw, dh, channels);
        return pil_preserve_mode(orig_img, result);
    }

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

        // Native byte images already have the exact sample layout required by
        // Pillow's affine nearest path. Copy complete source pixels directly
        // instead of expanding each one through `pixel_at`'s four-channel
        // f64 representation and then pushing bytes individually. This keeps
        // the coordinate calculation identical while making the common
        // byte-image path bandwidth-bound.
        if matches!(
            img,
            DynamicImage::ImageLuma8(_)
                | DynamicImage::ImageLumaA8(_)
                | DynamicImage::ImageRgb8(_)
                | DynamicImage::ImageRgba8(_)
        ) {
            let source = img.as_bytes();
            let mut out_bytes = vec![0u8; n * channels];
            let source_stride = sw as usize * channels;
            let destination_stride = dw as usize * channels;
            let mut yo = scale_y * 0.5;
            for dy in 0..dh as usize {
                let sy = if yo >= sh as f64 { sh - 1 } else { yo as u32 } as usize;
                let source_row = sy * source_stride;
                let destination_row = dy * destination_stride;
                if channels == 1 {
                    let destination =
                        &mut out_bytes[destination_row..destination_row + destination_stride];
                    for (destination_pixel, &sx) in destination.iter_mut().zip(&xintab) {
                        *destination_pixel = source[source_row + sx as usize];
                    }
                } else {
                    for (dx, &sx) in xintab.iter().enumerate() {
                        let source_start = source_row + sx as usize * channels;
                        let destination_start = destination_row + dx * channels;
                        out_bytes[destination_start..destination_start + channels]
                            .copy_from_slice(&source[source_start..source_start + channels]);
                    }
                }
                yo += scale_y;
            }
            let result = raw_to_dynamic_owned(out_bytes, dw, dh, channels);
            return pil_preserve_mode(orig_img, result);
        }
        // PIL also uses cumulative stepping for y: yo = a[4] * 0.5
        let mut yo = scale_y * 0.5;
        for _dy in 0..dh {
            let sy = if yo >= sh as f64 { sh - 1 } else { yo as u32 };
            for dx in 0..dw {
                let sx = xintab[dx as usize];
                let p = pixel_at(img, sx, sy);
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
        return pil_preserve_mode(orig_img, result);
    }

    // Precompute horizontal and vertical coefficients for two-pass pipeline
    let h_coeffs = precompute_coeffs(dw, sw, filter);
    let v_coeffs = precompute_coeffs(dh, sh, filter);

    // Allocate intermediate image (sh rows × dw columns × channels)
    let mut intermediate = vec![0u8; (sh * dw) as usize * channels];

    // Horizontal pass: each source row is independent and can be written into
    // its own intermediate row.
    if needs_alpha {
        horizontal_pass_rows_alpha(
            img.as_bytes(),
            sw,
            sh,
            channels,
            &h_coeffs,
            dw,
            &mut intermediate,
        );
    } else {
        horizontal_pass_rows(
            img.as_bytes(),
            sw,
            sh,
            channels,
            &h_coeffs,
            dw,
            &mut intermediate,
        );
    }

    // Allocate output image
    let mut out_bytes = vec![0u8; (dw * dh) as usize * channels];

    // Vertical output rows are also independent once the intermediate image
    // exists, so they use the same disjoint-row write helper.
    if needs_alpha {
        vertical_pass_rows_alpha(
            &intermediate,
            sh,
            dw,
            dh,
            channels,
            &v_coeffs,
            &mut out_bytes,
        );
    } else {
        vertical_pass_rows(
            &intermediate,
            sh,
            dw,
            dh,
            channels,
            &v_coeffs,
            &mut out_bytes,
        );
    }

    // Build DynamicImage from bytes
    let result = raw_to_dynamic(&out_bytes, dw, dh, channels);

    pil_preserve_mode(orig_img, result)
}

/// Resize an F-mode image through a fractional source box.
///
/// F samples are IEEE-754 values packed four bytes at a time.  They must be
/// decoded before resampling; treating the bytes as four independent image
/// channels is not Pillow-compatible.  The intermediate remains f32, while
/// each separable accumulation follows Pillow's f64-kernel/f32-store order.
fn pil_resize_f_boxed(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    filter: ResampleFilter,
) -> DynamicImage {
    let rgba = img.to_rgba8();
    let (source_width, source_height) = rgba.dimensions();
    let output_len = (dst_w as usize)
        .checked_mul(dst_h as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .unwrap_or(0);
    if dst_w == 0 || dst_h == 0 || source_width == 0 || source_height == 0 {
        return DynamicImage::ImageRgba8(
            crate::raster::RgbaImage::from_raw(dst_w, dst_h, vec![0; output_len])
                .unwrap_or_else(|| crate::raster::RgbaImage::new(dst_w, dst_h)),
        );
    }
    let source: Vec<f32> = rgba
        .as_raw()
        .chunks_exact(4)
        .map(|sample| f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]))
        .collect();
    let box_left = box_left as f32 as f64;
    let box_top = box_top as f32 as f64;
    let box_right = box_right as f32 as f64;
    let box_bottom = box_bottom as f32 as f64;

    if matches!(filter, ResampleFilter::Nearest) {
        let scale_x = (box_right as f32 - box_left as f32) as f64 / f64::from(dst_w);
        let scale_y = (box_bottom as f32 - box_top as f32) as f64 / f64::from(dst_h);
        let last_x = i64::from(source_width - 1);
        let last_y = i64::from(source_height - 1);
        let mut output = Vec::with_capacity(output_len);
        for dy in 0..dst_h {
            let source_y = (box_top + (f64::from(dy) + 0.5) * scale_y).floor() as i64;
            let source_y = source_y.clamp(0, last_y) as usize;
            for dx in 0..dst_w {
                let source_x = (box_left + (f64::from(dx) + 0.5) * scale_x).floor() as i64;
                let source_x = source_x.clamp(0, last_x) as usize;
                output.extend_from_slice(
                    &source[(source_y * source_width as usize + source_x)..][..1]
                        .first()
                        .copied()
                        .unwrap_or(0.0)
                        .to_le_bytes(),
                );
            }
        }
        return raw_to_dynamic(&output, dst_w, dst_h, 4);
    }

    let horizontal = precompute_coeffs_f64_boxed(dst_w, source_width, box_left, box_right, filter);
    let vertical = precompute_coeffs_f64_boxed(dst_h, source_height, box_top, box_bottom, filter);
    let mut intermediate = vec![0.0f32; source_height as usize * dst_w as usize];
    for source_y in 0..source_height as usize {
        let source_start = source_y * source_width as usize;
        let intermediate_start = source_y * dst_w as usize;
        for output_x in 0..dst_w as usize {
            let x0 = horizontal.xmin[output_x];
            let mut sum = 0.0;
            for (tap, &weight) in horizontal.weights[output_x].iter().enumerate() {
                let source_x = (x0 + tap as i64) as usize;
                sum += weight * f64::from(source[source_start + source_x]);
            }
            intermediate[intermediate_start + output_x] = if sum == 0.0 { 0.0 } else { sum as f32 };
        }
    }

    let mut output_floats = vec![0.0f32; dst_w as usize * dst_h as usize];
    for output_y in 0..dst_h as usize {
        let y0 = vertical.xmin[output_y];
        for output_x in 0..dst_w as usize {
            let mut sum = 0.0;
            for (tap, &weight) in vertical.weights[output_y].iter().enumerate() {
                let source_y = (y0 + tap as i64) as usize;
                sum += weight * f64::from(intermediate[source_y * dst_w as usize + output_x]);
            }
            output_floats[output_y * dst_w as usize + output_x] =
                if sum == 0.0 { 0.0 } else { sum as f32 };
        }
    }
    let output: Vec<u8> = output_floats
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect();
    raw_to_dynamic(&output, dst_w, dst_h, 4)
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
    if explicit_mode == Some("F") && matches!(img, DynamicImage::ImageRgba8(_)) {
        return pil_resize_f_boxed(
            img, dst_w, dst_h, box_left, box_top, box_right, box_bottom, filter,
        );
    }
    let is_cmyk = explicit_mode == Some("CMYK");
    let is_fi = explicit_mode == Some("F") || explicit_mode == Some("I");
    let needs_alpha = !is_cmyk
        && !is_fi
        && explicit_mode != Some("RGBa")
        && matches!(
            img.color(),
            crate::raster::ColorType::Rgba8 | crate::raster::ColorType::La8
        );
    let (kernel_fn, support) = filter_from_resample(filter);
    let (sw, sh) = (img.width(), img.height());

    let channels = match img.color() {
        crate::raster::ColorType::L8 => 1usize,
        crate::raster::ColorType::La8 => 2usize,
        crate::raster::ColorType::Rgb8 => 3usize,
        _ => 4usize,
    };

    // Pillow's boxed nearest path is an affine sample, not a one-tap box
    // convolution. This distinction matters for indexed ImageOps.fit: the
    // convolution-style coefficient builder can include two adjacent raw
    // palette samples at a boundary and produce an index that Pillow never
    // emits. Keep P/PA in their native sample layout and apply the exact
    // ``int(box_start + (x + 0.5) * scale)`` mapping used by ImagingTransform.
    if matches!(filter, ResampleFilter::Nearest) && matches!(explicit_mode, Some("P") | Some("PA"))
    {
        if sw == 0 || sh == 0 {
            return pil_preserve_mode(
                orig_img,
                raw_to_dynamic(
                    &vec![0; (dst_w as usize) * (dst_h as usize) * channels],
                    dst_w,
                    dst_h,
                    channels,
                ),
            );
        }
        // The native transform path receives the same float box record as the
        // resampler; keep its scale calculation at that boundary too.
        let box_left_f32 = box_left as f32;
        let box_top_f32 = box_top as f32;
        let box_right_f32 = box_right as f32;
        let box_bottom_f32 = box_bottom as f32;
        let box_left = box_left_f32 as f64;
        let box_top = box_top_f32 as f64;
        let scale_x = ((box_right_f32 - box_left_f32) as f64) / dst_w as f64;
        let scale_y = ((box_bottom_f32 - box_top_f32) as f64) / dst_h as f64;
        let source = img.as_bytes();
        let mut out_bytes = Vec::with_capacity((dst_w * dst_h) as usize * channels);
        for dy in 0..dst_h {
            let sy = (box_top + (dy as f64 + 0.5) * scale_y).floor();
            let sy = sy.clamp(0.0, (sh - 1) as f64) as usize;
            for dx in 0..dst_w {
                let sx = (box_left + (dx as f64 + 0.5) * scale_x).floor();
                let sx = sx.clamp(0.0, (sw - 1) as f64) as usize;
                let start = (sy * sw as usize + sx) * channels;
                out_bytes.extend_from_slice(&source[start..start + channels]);
            }
        }
        return pil_preserve_mode(orig_img, raw_to_dynamic(&out_bytes, dst_w, dst_h, channels));
    }

    // Use box-parameter coefficients for both passes
    let h_coeffs = precompute_coeffs_boxed(dst_w, sw, box_left, box_right, kernel_fn, support);
    let v_coeffs = precompute_coeffs_boxed(dst_h, sh, box_top, box_bottom, kernel_fn, support);

    // Allocate intermediate image (sh rows × dw columns × channels)
    let mut intermediate = vec![0u8; (sh * dst_w) as usize * channels];

    // Horizontal pass: each source row writes one independent intermediate row.
    if needs_alpha {
        horizontal_pass_rows_alpha(
            img.as_bytes(),
            sw,
            sh,
            channels,
            &h_coeffs,
            dst_w,
            &mut intermediate,
        );
    } else {
        horizontal_pass_rows(
            img.as_bytes(),
            sw,
            sh,
            channels,
            &h_coeffs,
            dst_w,
            &mut intermediate,
        );
    }

    // Allocate output image
    let mut out_bytes = vec![0u8; (dst_w * dst_h) as usize * channels];

    // Vertical output rows are independent after the horizontal pass.
    if needs_alpha {
        vertical_pass_rows_alpha(
            &intermediate,
            sh,
            dst_w,
            dst_h,
            channels,
            &v_coeffs,
            &mut out_bytes,
        );
    } else {
        vertical_pass_rows(
            &intermediate,
            sh,
            dst_w,
            dst_h,
            channels,
            &v_coeffs,
            &mut out_bytes,
        );
    }

    let result = raw_to_dynamic(&out_bytes, dst_w, dst_h, channels);

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

/// Convert an owned native-byte resize result without copying its backing
/// buffer. The borrowed helper remains for the convolution paths, which reuse
/// their intermediate/output slices after constructing the image.
fn raw_to_dynamic_owned(bytes: Vec<u8>, w: u32, h: u32, channels: usize) -> DynamicImage {
    match channels {
        1 => DynamicImage::ImageLuma8(
            crate::raster::GrayImage::from_raw(w, h, bytes)
                .unwrap_or_else(|| crate::raster::GrayImage::new(w, h)),
        ),
        2 => DynamicImage::ImageLumaA8(
            crate::raster::GrayAlphaImage::from_raw(w, h, bytes)
                .unwrap_or_else(|| crate::raster::GrayAlphaImage::new(w, h)),
        ),
        3 => DynamicImage::ImageRgb8(
            crate::raster::RgbImage::from_raw(w, h, bytes)
                .unwrap_or_else(|| crate::raster::RgbImage::new(w, h)),
        ),
        _ => DynamicImage::ImageRgba8(
            crate::raster::RgbaImage::from_raw(w, h, bytes)
                .unwrap_or_else(|| crate::raster::RgbaImage::new(w, h)),
        ),
    }
}
