use super::filters::filter_from_resample;
use crate::pipeline::ResampleFilter;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

/// PIL uses 22-bit fixed-point arithmetic (PRECISION_BITS=22) for weights
/// and intermediate accumulation. We match this exactly.
pub(super) const PRECISION_BITS: u32 = 22;
pub(super) const PRECISION: i64 = 1i64 << PRECISION_BITS; // 2^22
pub(super) const HALF_PRECISION: i64 = 1i64 << (PRECISION_BITS - 1); // 2^21
pub(super) const BOXED_ZERO_FAST_PATH_MAX_PIXELS: usize = 256 * 256;

/// Round a float to u8, matching PIL's fixed-point rounding:
///   `(int)(v + 0.5)` clamped to [0, 255]
pub(super) fn pil_round(v: f64) -> u8 {
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
pub(super) fn fixed_point_to_u8(sum: i64) -> u8 {
    let v = (sum + HALF_PRECISION) >> PRECISION_BITS;
    if v <= 0 {
        0
    } else if v >= 255 {
        255
    } else {
        v as u8
    }
}

#[cfg(not(feature = "parallel"))]
#[inline]
pub(super) fn fixed_point_to_u8_i32(sum: i32) -> u8 {
    let value = (sum + (1_i32 << (PRECISION_BITS - 1))) >> PRECISION_BITS;
    value.clamp(0, 255) as u8
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
    pub(super) fn weights_for(&self, index: usize) -> &[i64] {
        let start = self.offsets[index];
        &self.weights[start..start + self.count[index]]
    }
}

/// Prove every ordered byte-sample partial sum, including Pillow's rounding
/// bias, fits in i32 before selecting the narrow HSV resize kernel.
#[cfg(not(feature = "parallel"))]
pub(super) fn resize_u8_coefficients_fit_i32(coeffs: &FilterCoeffs) -> bool {
    if coeffs.count.len() != coeffs.offsets.len() {
        return false;
    }
    for index in 0..coeffs.count.len() {
        let Some(weights) = coeffs
            .offsets
            .get(index)
            .and_then(|&start| {
                start
                    .checked_add(*coeffs.count.get(index)?)
                    .map(|end| (start, end))
            })
            .and_then(|(start, end)| coeffs.weights.get(start..end))
        else {
            return false;
        };
        let Some(sum_abs) = weights.iter().try_fold(0u64, |sum, &weight| {
            i32::try_from(weight).ok()?;
            sum.checked_add(weight.unsigned_abs())
        }) else {
            return false;
        };
        let Some(bound) = sum_abs
            .checked_mul(255)
            .and_then(|bound| bound.checked_add(1 << (PRECISION_BITS - 1)))
        else {
            return false;
        };
        if bound > i32::MAX as u64 {
            return false;
        }
    }
    true
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
            // Resample.c evaluates `(x + xmin - center + 0.5)` with the
            // source index addition still integral.  Subtracting the center
            // before adding the half-pixel preserves its f64 rounding.
            let val = kernel((sx as f64 - center + 0.5) * ss);
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
            let value = kernel((x0 as f64 + tap as f64 - center + 0.5) * ss);
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

/// Rebase coefficient source rows to the smallest span that can contribute.
///
/// Boxed resizes often read only a fraction of the source's vertical extent.
/// The horizontal pass is independent for each source row, so rows outside
/// the vertical coefficient ranges can be omitted exactly. The returned
/// `(first_row, row_count)` addresses the original source; `xmin` values are
/// shifted to address an intermediate containing only that range. Invalid or
/// empty tables are left untouched and return `None`.
pub(crate) fn compact_resize_coeffs_to_source_span(
    coeffs: &mut FilterCoeffs,
    source_rows: u32,
) -> Option<(u32, u32)> {
    if coeffs.xmin.len() != coeffs.count.len() {
        return None;
    }
    let mut first = i64::MAX;
    let mut end = i64::MIN;
    for (&xmin, &count) in coeffs.xmin.iter().zip(&coeffs.count) {
        if count == 0 {
            continue;
        }
        let row_end = xmin.checked_add(i64::try_from(count).ok()?)?;
        if xmin < 0 || row_end > i64::from(source_rows) {
            return None;
        }
        first = first.min(xmin);
        end = end.max(row_end);
    }
    if first == i64::MAX || end <= first {
        return None;
    }
    for (xmin, &count) in coeffs.xmin.iter_mut().zip(&coeffs.count) {
        if count > 0 {
            *xmin -= first;
        }
    }
    Some((u32::try_from(first).ok()?, u32::try_from(end - first).ok()?))
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
