//! GPU worker pool — implements BackendImpl for GPU compute backend.

macro_rules! gpu_log {
    ($($arg:tt)*) => {
        log::debug!(target: "compute::gpu", $($arg)*)
    };
}

// Uses wgpu for compute shader dispatch with packed u32 RGBA buffers
// (R|G<<8|B<<16|A<<24) and 16x16 workgroups.
// GPU init is lazy — happens on first `execute_batch` call.

use crate::checked_dims::CheckedDims;
use crate::compute::registry;
use crate::compute::{Backend, BackendImpl, PipelineResourceTelemetry};
use crate::error::PilError;
use crate::ops::pil_resize::{
    FilterCoeffs, FilterCoeffsF64, filter_from_resample, luma16_resample_big_endian,
    luma16_resample_read, luma16_resample_write, precompute_coeffs,
    precompute_coeffs_boxed_for_filter, precompute_coeffs_f64, round_up,
};
use crate::pipeline::{
    ColorMode, PipelineOp, PixelMode, ResampleFilter, TransformMethod, TransposeMethod,
};
#[cfg(target_endian = "little")]
use crate::raster::RgbImage;
use crate::raster::{DynamicImage, GenericImageView, ImageBuffer, Luma, RgbaImage};
use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Keep command submission bounded for very long lazy pipelines. A batch may
/// contain more operations; it is split into sequential submissions without
/// reading the image back between chunks.
const MAX_GPU_OPS_PER_SUBMISSION: usize = 256;

/// Bound transient per-submission auxiliary/parameter arenas. A single
/// operation larger than this limit is still allowed so valid large images are
/// not rejected merely because they need one large upload.
const MAX_GPU_RESOURCE_BYTES_PER_SUBMISSION: usize = 64 * 1024 * 1024;
const MAX_RETAINED_GPU_WORKING_SETS: usize = 2;
const MAX_RETAINED_GPU_WORKING_BYTES: u64 = 128 * 1024 * 1024;
// Reusing a much larger working set for a tiny image materially increases
// Metal's command-buffer/readback cost. Keep a bounded amount of
// over-allocation while allowing the pool to reuse neighboring image sizes.
const MAX_GPU_BUFFER_REUSE_RATIO: u32 = 4;
const MAX_RETAINED_GPU_STAGING_BUFFERS: usize = 2;
const MAX_RETAINED_GPU_STAGING_BYTES: u64 = 64 * 1024 * 1024;
const MAX_GPU_AUXILIARY_CACHE_BYTES: usize = 64 * 1024 * 1024;
/// The histogram control passes use three 256-bin channels and retain one
/// extra 256-word tail so the existing clear shader can reset the complete
/// storage allocation without a host write between operations.
const GPU_HISTOGRAM_WORDS: usize = 1024;
const GPU_HISTOGRAM_BYTES: usize = GPU_HISTOGRAM_WORDS * std::mem::size_of::<u32>();

/// Dynamic shader loops must have a small, explicit upper bound. These limits
/// are deliberately stricter than the CPU implementation: an unsupported or
/// unusually large request is routed to CPU rather than being allowed to
/// monopolize a native GPU queue.
const MAX_GPU_BLUR_RADIUS: u32 = 64;
const MAX_GPU_FILTER_SIZE: u32 = 15;
const MAX_GPU_REDUCE_FACTOR: u32 = 64;
const MAX_GPU_MANDELBROT_ITERS: u32 = 10_000;
// Order-statistic filters use a bounded local insertion sort.  The estimate
// below is intentionally large enough for the reviewed 9x9/256x256 material
// workloads and the 3x3/1024x768 matrix case, while still rejecting unbounded
// large windows before a single device submission can monopolize the queue.
const MAX_GPU_SHADER_WORK_ITEMS: u64 = 2 * 1024 * 1024 * 1024;
const GPU_BUFFER_CAPACITY: u32 = 4096 * 4096;
// F affine admission compares the scalar source selection with the packed
// 16.16 walk once per destination pixel. Keep that proof bounded even when a
// caller constructs a large transform that the normal dimension checks would
// route to host semantic control.
const GPU_F_AFFINE_PROOF_MAX_PIXELS: usize = 1024 * 1024;
const MAX_GPU_SCALE_FIXED_POINT: f64 = u32::MAX as f64;
// Add/Subtract currently dispatch only the exact unit-divisor/integral-offset
// subset. Other valid public parameters are routed to CPU until the shader
// carries the full f64 contract without rounding differences.

/// `Maintain::Wait` can wait forever when a native device or driver wedges.
/// Poll with a short bounded backoff so the library remains responsive while
/// retaining a finite failure path for a wedged native device or driver.
const GPU_READBACK_TIMEOUT: Duration = Duration::from_secs(30);
/// Small transfers commonly complete before the native scheduler's 1 ms
/// timer granularity. Give only those reads a bounded low-latency poll window;
/// large transfers retain the conservative backoff used for sustained work.
const GPU_FAST_POLL_MAX_READBACK_BYTES: u64 = 64 * 1024;
const GPU_POLL_FAST_BACKOFF: Duration = Duration::from_micros(50);
const GPU_POLL_FAST_RETRIES: usize = 8;
const GPU_POLL_BACKOFF: Duration = Duration::from_millis(1);
/// Pillow's `ImagingLineBoxBlur{8,32}` in `src/libImaging/BoxBlur.c` uses a
/// normalized replicated-edge average, so a constant image remains constant
/// through every box pass (and therefore Pillow's three-pass Gaussian
/// lowering). Scanning larger frames can cost more than the six GPU passes it
/// replaces, so keep this exact lowering bounded to the benchmark-sized
/// working set where the launch savings are measurable.
const GPU_UNIFORM_BLUR_MAX_PIXELS: usize = 1024 * 1024;

fn readback_poll_backoff(
    fast_polling: bool,
    empty_polls: usize,
    now: Instant,
    deadline: Instant,
) -> Option<Duration> {
    let remaining = deadline.saturating_duration_since(now);
    (!remaining.is_zero()).then(|| {
        let backoff = if fast_polling && empty_polls <= GPU_POLL_FAST_RETRIES {
            GPU_POLL_FAST_BACKOFF
        } else {
            GPU_POLL_BACKOFF
        };
        backoff.min(remaining)
    })
}

fn gpu_uniform_blur_can_copy(ops: &[PipelineOp], image: &DynamicImage) -> bool {
    if ops.len() != 1
        || !matches!(
            ops[0],
            PipelineOp::BoxBlur { .. }
                | PipelineOp::BoxBlurXY { .. }
                | PipelineOp::GaussianBlur { .. }
        )
        || !gpu_operation_is_safe(&ops[0])
        || !gpu_image_layout_is_supported(image)
    {
        return false;
    }

    let Ok(dims) = CheckedDims::new(image.width(), image.height(), 1) else {
        return false;
    };
    let pixel_count = dims.total_pixels();
    if pixel_count == 0 || pixel_count > GPU_UNIFORM_BLUR_MAX_PIXELS {
        return false;
    }

    let channels = match image {
        DynamicImage::ImageLuma8(_) => 1,
        DynamicImage::ImageLumaA8(_) => 2,
        DynamicImage::ImageRgb8(_) => 3,
        DynamicImage::ImageRgba8(_) => 4,
        _ => return false,
    };
    let bytes = image.as_bytes();
    let Some(first) = bytes.get(..channels) else {
        return false;
    };
    bytes
        .chunks_exact(channels)
        .take(pixel_count)
        .all(|pixel| pixel == first)
}

/// Return the exact `f32` bit pattern for a constant F-mode resize batch.
///
/// Pillow's `ImagingResample` normalizes each filter row, so its finite
/// constant F sample remains unchanged; the CPU `resize_f` implementation
/// uses the same invariant for its exact constant fast path.  The ordinary
/// mode-8 WGSL convolution accumulates f32 values with quantized weights and
/// therefore cannot promise those bytes for a mixed image.  Restrict the
/// native constant lowering to an all-resize batch whose source words prove
/// that invariant; callers keep mixed filtered F input on host semantic
/// control until a general f64-equivalent device accumulator exists.
fn gpu_f_resize_constant_bits(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> Option<u32> {
    if logical_mode != Some("F")
        || ops.is_empty()
        || !(ops.iter().all(|op| {
            matches!(
                op,
                PipelineOp::Resize { filter, .. }
                    if !matches!(filter, ResampleFilter::Nearest)
            )
        }) || (ops.len() == 1
            && matches!(
                ops[0],
                PipelineOp::Pad { filter, .. }
                    if !matches!(filter, ResampleFilter::Nearest)
            )))
    {
        return None;
    }
    gpu_f_source_constant_bits(image, logical_mode)
}

/// Return the exact source word for a constant F image.
///
/// Pillow's scalar resamplers normalize every finite constant row, including
/// the intermediate reducing-gap pass used by `thumbnail`.  Restrict this
/// invariant to a non-empty, finite source with a non-negative-zero word so
/// the final f32 store has the same bits as the native F path.
fn gpu_f_source_constant_bits(image: &DynamicImage, logical_mode: Option<&str>) -> Option<u32> {
    if logical_mode != Some("F") || !matches!(image, DynamicImage::ImageRgba8(_)) {
        return None;
    }
    let DynamicImage::ImageRgba8(pixels) = image else {
        return None;
    };
    let expected = CheckedDims::new(image.width(), image.height(), 4)
        .ok()?
        .total_bytes();
    let bytes = pixels.as_raw();
    if bytes.len() != expected {
        return None;
    }
    let first = bytes.get(..4)?;
    let bits = u32::from_le_bytes([first[0], first[1], first[2], first[3]]);
    let value = f32::from_bits(bits);
    (value.is_finite() && bits != (-0.0f32).to_bits()).then_some(())?;
    bytes
        .chunks_exact(4)
        .all(|sample| u32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]) == bits)
        .then_some(bits)
}

/// Return whether an F-mode Box resize chain can copy complete sample words.
///
/// With `output >= input`, Pillow's Box coefficient table has exactly one
/// source tap per output coordinate and that tap has normalized weight one.
/// The CPU F path still travels through its scalar resampler, but no sample
/// arithmetic is required for this geometry: the native lowering copies the
/// selected four-byte word and canonicalizes negative zero to positive zero,
/// matching Pillow's f64 accumulator at the final store. Signaling NaNs are
/// quieted while preserving their payload/sign; quiet NaNs and infinities are
/// otherwise copied exactly. PutData(F) is also allowed when its byte length
/// matches the current image; keep this proof limited to that source update
/// plus all-Box Resize chains so a mixed filter or any downsampling continues
/// to use exact host control.
fn gpu_f_resize_box_copy_is_exact(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if logical_mode != Some("F") || ops.is_empty() || !matches!(image, DynamicImage::ImageRgba8(_))
    {
        return false;
    }

    let words_are_supported = |bytes: &[u8]| bytes.chunks_exact(4).remainder().is_empty();
    let DynamicImage::ImageRgba8(pixels) = image else {
        return false;
    };
    let expected = CheckedDims::new(image.width(), image.height(), 4)
        .ok()
        .map(|dims| dims.total_bytes());
    if expected != Some(pixels.as_raw().len()) || !words_are_supported(pixels.as_raw()) {
        return false;
    }

    let mut dimensions = image.dimensions();
    if dimensions.0 == 0 || dimensions.1 == 0 {
        return false;
    }
    for op in ops {
        match op {
            PipelineOp::PutData {
                data,
                mode: PixelMode::F,
            } => {
                let expected = CheckedDims::new(dimensions.0, dimensions.1, 4)
                    .ok()
                    .map(|dims| dims.total_bytes());
                if expected != Some(data.len()) || !words_are_supported(data) {
                    return false;
                }
            }
            PipelineOp::Resize { w, h, filter } => {
                if !matches!(filter, ResampleFilter::Box)
                    || *w == 0
                    || *h == 0
                    || *w < dimensions.0
                    || *h < dimensions.1
                {
                    return false;
                }
                dimensions = (*w, *h);
            }
            _ => return false,
        }
    }
    true
}

/// Return whether an F-mode filtered resize is an exact same-size copy.
///
/// `resize_f` returns the source image unchanged when both output dimensions
/// already match. Preserve that contract for every bit pattern, including
/// nonfinite values and negative zero; no filter arithmetic is required for
/// this geometry. Keep the proof limited to one resize plus optional F-mode
/// PutData updates so a later operation cannot consume an unproved result.
fn gpu_f_resize_identity_is_exact(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if logical_mode != Some("F") || ops.is_empty() || !matches!(image, DynamicImage::ImageRgba8(_))
    {
        return false;
    }
    let DynamicImage::ImageRgba8(pixels) = image else {
        return false;
    };
    let expected = CheckedDims::new(image.width(), image.height(), 4)
        .ok()
        .map(|dims| dims.total_bytes());
    if expected != Some(pixels.as_raw().len()) {
        return false;
    }

    let mut dimensions = image.dimensions();
    if dimensions.0 == 0 || dimensions.1 == 0 {
        return false;
    }
    let mut resize_count = 0usize;
    for op in ops {
        match op {
            PipelineOp::PutData {
                data,
                mode: PixelMode::F,
            } => {
                let expected = CheckedDims::new(dimensions.0, dimensions.1, 4)
                    .ok()
                    .map(|dims| dims.total_bytes());
                if expected != Some(data.len()) {
                    return false;
                }
            }
            PipelineOp::Resize { w, h, .. } => {
                resize_count = resize_count.saturating_add(1);
                if resize_count != 1 || *w == 0 || *h == 0 || (*w, *h) != dimensions {
                    return false;
                }
                dimensions = (*w, *h);
            }
            _ => return false,
        }
    }
    resize_count == 1
}

/// Return whether an `I`-mode filtered resize is an exact same-size copy.
///
/// Pillow's imaging core returns the source words unchanged when the target
/// dimensions already match, regardless of the selected resampling filter.
/// `I` stores signed samples as opaque little-endian `u32` words at this
/// executor boundary, so lowering this one-operation geometry to `Duplicate`
/// preserves every bit pattern (including the signed extrema) without
/// invoking the not-yet-proven typed convolution accumulator. Keep the proof
/// limited to a pure resize; a preceding or following operation may change
/// the sample contract or dimensions and must remain on its existing path.
fn gpu_i_resize_identity_is_exact(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if logical_mode != Some("I")
        || ops.len() != 1
        || !matches!(image, DynamicImage::ImageRgba8(_))
        || image.width() == 0
        || image.height() == 0
    {
        return false;
    }
    let DynamicImage::ImageRgba8(pixels) = image else {
        return false;
    };
    let expected = CheckedDims::new(image.width(), image.height(), 4)
        .ok()
        .map(|dims| dims.total_bytes());
    if expected != Some(pixels.as_raw().len()) {
        return false;
    }
    matches!(
        ops[0],
        PipelineOp::Resize { w, h, .. } if (w, h) == image.dimensions()
    )
}

/// Return whether a one- or two-axis 2x F-mode Box downscale is safe for the
/// shader's f32 accumulator.
///
/// A Box row at an exact 2:1 ratio contains two normalized 0.5 taps. For
/// finite samples whose halving stays in the normal range, each product is
/// exact in f32 and the final two-term sum rounds identically to Pillow's
/// f64 accumulation followed by an f32 store. The conservative `2^-20` floor
/// leaves even a two-axis reduction above the normal range (`2^-22`) when zero
/// samples are present; same-sign inputs prevent cancellation below that bound.
/// Chained reductions and other ratios need a new proof and remain on exact
/// host semantic control.
// This mirrors Pillow's `Resample.c::precompute_coeffs` normalization and its
// F-mode `ImagingResample` double-accumulate/f32-store boundary.
fn gpu_f_resize_box_average_is_exact(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if logical_mode != Some("F") || ops.is_empty() || !matches!(image, DynamicImage::ImageRgba8(_))
    {
        return false;
    }

    // Keep a conservative normal-range floor above the adapter's relaxed-f32
    // flush threshold. Positive zero is exact; keep negative zero out because
    // Pillow canonicalizes convolution zeroes. Requiring one sign for all
    // nonzero samples also rules out cancellation into that relaxed range.
    const MIN_SAFE_VALUE: f32 = 9.536_743_164_062_5e-7_f32; // 2^-20
    let words_are_supported = |bytes: &[u8]| {
        if !bytes.chunks_exact(4).remainder().is_empty() {
            return false;
        }
        let mut sign = None;
        for sample in bytes.chunks_exact(4) {
            let bits = u32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]);
            let value = f32::from_bits(bits);
            if !value.is_finite() || bits == (-0.0f32).to_bits() {
                return false;
            }
            if value != 0.0 {
                if value.abs() < MIN_SAFE_VALUE {
                    return false;
                }
                let positive = value.is_sign_positive();
                if sign.is_some_and(|expected| expected != positive) {
                    return false;
                }
                sign = Some(positive);
            }
        }
        true
    };

    let DynamicImage::ImageRgba8(pixels) = image else {
        return false;
    };
    let expected = CheckedDims::new(image.width(), image.height(), 4)
        .ok()
        .map(|dims| dims.total_bytes());
    if expected != Some(pixels.as_raw().len()) || !words_are_supported(pixels.as_raw()) {
        return false;
    }

    let mut dimensions = image.dimensions();
    if dimensions.0 == 0 || dimensions.1 == 0 {
        return false;
    }
    let mut resize_count = 0usize;
    let mut changed_axis = false;
    for op in ops {
        match op {
            PipelineOp::PutData {
                data,
                mode: PixelMode::F,
            } => {
                let expected = CheckedDims::new(dimensions.0, dimensions.1, 4)
                    .ok()
                    .map(|dims| dims.total_bytes());
                if expected != Some(data.len()) || !words_are_supported(data) {
                    return false;
                }
            }
            PipelineOp::Resize { w, h, filter } => {
                resize_count = resize_count.saturating_add(1);
                if resize_count != 1 || !matches!(filter, ResampleFilter::Box) || *w == 0 || *h == 0
                {
                    return false;
                }
                let half_w = w.checked_mul(2) == Some(dimensions.0);
                let half_h = h.checked_mul(2) == Some(dimensions.1);
                let same_w = *w == dimensions.0;
                let same_h = *h == dimensions.1;
                // Each axis is either a 2:1 reduction or an unchanged copy;
                // at least one axis must reduce. Up to two averaging passes
                // are safe under the source-value floor above.
                if (!half_w && !same_w) || (!half_h && !same_h) || (!half_w && !half_h) {
                    return false;
                }
                dimensions = (*w, *h);
                changed_axis = true;
            }
            _ => return false,
        }
    }
    changed_axis
}

#[derive(Clone, Copy)]
struct F32IntegerParts {
    negative: bool,
    mantissa: u32,
    exponent: i32,
}

/// Decode a finite normal f32 into the integer significand/exponent form used
/// by the exact marker-6 accumulator. Positive zero is represented by a zero
/// mantissa; negative zero, subnormals, and non-finite words are rejected.
fn gpu_f32_integer_parts(bits: u32) -> Option<F32IntegerParts> {
    if bits == (-0.0f32).to_bits() {
        return None;
    }
    let exponent_bits = (bits >> 23) & 0xff;
    let fraction = bits & 0x7f_ff_ff;
    if exponent_bits == 0xff || (exponent_bits == 0 && fraction != 0) {
        return None;
    }
    if exponent_bits == 0 {
        return Some(F32IntegerParts {
            negative: false,
            mantissa: 0,
            exponent: 0,
        });
    }
    Some(F32IntegerParts {
        negative: bits & (1 << 31) != 0,
        mantissa: fraction | 0x80_0000,
        exponent: exponent_bits as i32 - 127,
    })
}

/// Decode every finite f32 word into an integer significand and binary scale
/// for the marker-9 f64 reducer.  Normal words use the implicit leading bit;
/// subnormals use their explicit fraction at the fixed `2^-149` scale.  Keep
/// signed zero here because Pillow's ordered f64 accumulation can observe it;
/// the final proof preserves a signed-zero result when the exact reducer
/// agrees.  The older marker-6 proof intentionally keeps its stricter
/// normal-only decoder above.
fn gpu_f32_f64_integer_parts(bits: u32) -> Option<F32IntegerParts> {
    let exponent_bits = (bits >> 23) & 0xff;
    let fraction = bits & 0x7f_ff_ff;
    if exponent_bits == 0xff {
        return None;
    }
    if exponent_bits == 0 {
        return Some(F32IntegerParts {
            negative: bits & (1 << 31) != 0,
            mantissa: fraction,
            exponent: -126,
        });
    }
    Some(F32IntegerParts {
        negative: bits & (1 << 31) != 0,
        mantissa: fraction | 0x80_0000,
        exponent: exponent_bits as i32 - 127,
    })
}

/// Evaluate one horizontal or vertical fixed-point sample with the exact
/// integer representation of the f32 inputs. A source word is
/// `M * 2^(E-23)` and a fixed coefficient is `P / 2^22`, so the complete row
/// is an integer sum scaled by `2^(E-45)`. Keeping every sequential partial
/// sum within 53 bits proves that Pillow's f64 accumulation is exact before
/// its final f32 store, including cancellation from signed samples or
/// ringing-filter coefficients.
fn gpu_f_resize_integer_sample_bits(
    bytes: &[u8],
    source_dimensions: (u32, u32),
    coeffs: &FilterCoeffs,
    output_index: usize,
    horizontal: bool,
    line: usize,
) -> Option<u32> {
    let (source_w, source_h) = source_dimensions;
    let source_w = match usize::try_from(source_w) {
        Ok(value) => value,
        Err(_) => return None,
    };
    let source_h = match usize::try_from(source_h) {
        Ok(value) => value,
        Err(_) => return None,
    };
    let Some(&source_start) = coeffs.xmin.get(output_index) else {
        return None;
    };
    let Some(&count) = coeffs.count.get(output_index) else {
        return None;
    };
    let Some(&weight_start) = coeffs.offsets.get(output_index) else {
        return None;
    };
    let Some(weights) = coeffs
        .weights
        .get(weight_start..weight_start.saturating_add(count))
    else {
        return None;
    };
    let source_start = match usize::try_from(source_start) {
        Ok(value) => value,
        Err(_) => return None,
    };
    let word_at = |pixel: usize| -> Option<F32IntegerParts> {
        let offset = pixel.checked_mul(4)?;
        let end = offset.checked_add(4)?;
        let word = bytes.get(offset..end)?;
        gpu_f32_integer_parts(u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
    };
    let source_pixel = |line: usize, tap: usize| -> Option<usize> {
        let coordinate = source_start.checked_add(tap)?;
        if horizontal {
            (coordinate < source_w).then_some(line.checked_mul(source_w)?.checked_add(coordinate)?)
        } else {
            (coordinate < source_h).then_some(coordinate.checked_mul(source_w)?.checked_add(line)?)
        }
    };

    if line >= if horizontal { source_h } else { source_w } {
        return None;
    }

    let mut minimum_exponent = None;
    for (tap, &weight) in weights.iter().enumerate() {
        if weight == 0 {
            continue;
        }
        let pixel = source_pixel(line, tap).and_then(word_at)?;
        if pixel.mantissa != 0 {
            minimum_exponent = Some(
                minimum_exponent.map_or(pixel.exponent, |minimum: i32| minimum.min(pixel.exponent)),
            );
        }
    }
    let Some(minimum_exponent) = minimum_exponent else {
        // A row whose selected samples are all positive zero has an exact
        // positive-zero result. Negative zero is rejected by the source-word
        // proof because Pillow's filtered path canonicalizes its zero output.
        return Some(0);
    };

    let mut sum = 0i128;
    for (tap, &weight) in weights.iter().enumerate() {
        if weight == 0 {
            continue;
        }
        let pixel = source_pixel(line, tap).and_then(word_at)?;
        if pixel.mantissa == 0 {
            continue;
        }
        let shift = u32::try_from(pixel.exponent - minimum_exponent).ok()?;
        let product = i128::from(pixel.mantissa).checked_mul(i128::from(weight))?;
        let product = if pixel.negative {
            product.checked_neg()?
        } else {
            product
        };
        // `checked_shl` only validates the shift count; it intentionally
        // permits high bits to be truncated. That is not an exact integer
        // reduction, so reject a term whose aligned magnitude would exceed
        // the signed accumulator before shifting it.
        let product_magnitude = product.checked_abs()?;
        let product_bits = 128 - product_magnitude.leading_zeros();
        if product_bits.saturating_add(shift) > 127 {
            return None;
        }
        let term = product.checked_shl(shift)?;
        let next = sum.checked_add(term)?;
        let magnitude = if next < 0 { next.checked_neg()? } else { next };
        // f64 represents every integer exactly through 2^53. Check each
        // sequential partial, not only the final cancellation result.
        if magnitude > (1i128 << 53) {
            return None;
        }
        sum = next;
    }
    if sum == 0 {
        return Some(0);
    }

    let magnitude = if sum < 0 { sum.checked_neg()? } else { sum };
    let magnitude = (magnitude as f64) * 2f64.powi(minimum_exponent - 45);
    let value = if sum < 0 {
        -(magnitude as f32)
    } else {
        magnitude as f32
    };
    let bits = value.to_bits();
    // The marker's conversion routine deliberately stays one exponent below
    // overflow and never emits a subnormal, which also avoids adapter flush-to
    // zero behavior at the device boundary.
    (value.is_normal() && (((bits >> 23) & 0xff) as i32 - 127) <= 126).then_some(bits)
}

/// Check one horizontal or vertical fixed-point row using the exact integer
/// representation of the f32 inputs.
fn gpu_f_resize_integer_row_is_exact(
    bytes: &[u8],
    source_dimensions: (u32, u32),
    coeffs: &FilterCoeffs,
    output_index: usize,
    horizontal: bool,
) -> bool {
    let line_count = if horizontal {
        source_dimensions.1
    } else {
        source_dimensions.0
    };
    let Ok(line_count) = usize::try_from(line_count) else {
        return false;
    };
    (0..line_count).all(|line| {
        gpu_f_resize_integer_sample_bits(
            bytes,
            source_dimensions,
            coeffs,
            output_index,
            horizontal,
            line,
        )
        .is_some()
    })
}

/// Evaluate every word produced by one exact fixed-point resize pass. The
/// resulting words are fed into the second pass proof so a two-axis resize
/// preserves Pillow's f32 intermediate store, rather than proving both axes
/// against the original source independently.
fn gpu_f_resize_integer_pass_bits(
    bytes: &[u8],
    source_dimensions: (u32, u32),
    coeffs: &FilterCoeffs,
    horizontal: bool,
) -> Option<Vec<u32>> {
    let output_count = coeffs.xmin.len();
    let line_count = if horizontal {
        usize::try_from(source_dimensions.1).ok()?
    } else {
        usize::try_from(source_dimensions.0).ok()?
    };
    let word_count = output_count.checked_mul(line_count)?;
    let byte_count = word_count.checked_mul(4)?;
    if byte_count > (GPU_BUFFER_CAPACITY as usize).saturating_mul(4) {
        return None;
    }
    let mut result = Vec::new();
    result.try_reserve(word_count).ok()?;
    if horizontal {
        for line in 0..line_count {
            for output_index in 0..output_count {
                result.push(gpu_f_resize_integer_sample_bits(
                    bytes,
                    source_dimensions,
                    coeffs,
                    output_index,
                    true,
                    line,
                )?);
            }
        }
    } else {
        for output_index in 0..output_count {
            for line in 0..line_count {
                result.push(gpu_f_resize_integer_sample_bits(
                    bytes,
                    source_dimensions,
                    coeffs,
                    output_index,
                    false,
                    line,
                )?);
            }
        }
    }
    Some(result)
}

#[derive(Clone, Copy)]
struct F64IntegerParts {
    negative: bool,
    mantissa: u64,
    exponent: i32,
}

/// Decode a finite normal f64 coefficient into an integer significand and a
/// binary scale.  Pillow's f64 coefficient table is dyadic, so this keeps the
/// device path entirely integer while preserving the coefficient bits that
/// the generic f32 shader previously discarded.  Zero is represented with a
/// zero mantissa; subnormal and non-finite coefficients are rejected because
/// the bounded WGSL conversion below only promises normal values.
fn gpu_f64_integer_parts(value: f64) -> Option<F64IntegerParts> {
    let bits = value.to_bits();
    let exponent_bits = (bits >> 52) & 0x7ff;
    let fraction = bits & ((1u64 << 52) - 1);
    if exponent_bits == 0x7ff || (exponent_bits == 0 && fraction != 0) {
        return None;
    }
    if exponent_bits == 0 {
        return Some(F64IntegerParts {
            negative: false,
            mantissa: 0,
            exponent: 0,
        });
    }
    Some(F64IntegerParts {
        negative: bits & (1u64 << 63) != 0,
        mantissa: fraction | (1u64 << 52),
        exponent: exponent_bits as i32 - 1023 - 52,
    })
}

/// Round a signed integer scaled by `2^scale_exp` directly to an f32 bit
/// pattern.  This is the same round-to-nearest-even operation that a final
/// `f64 as f32` conversion performs, but it avoids an intermediate host f64
/// conversion so the result can be compared with the integer WGSL reducer.
fn gpu_f64_integer_to_f32(sum: i128, scale_exp: i32) -> Option<u32> {
    if sum == 0 {
        return Some(0);
    }
    let negative = sum < 0;
    let magnitude = sum.checked_abs()? as u128;
    let bit_length = 128 - magnitude.leading_zeros();
    let mut exponent = scale_exp.checked_add(bit_length as i32 - 1)?;

    // Values below the normal range are rounded in units of 2^-149, exactly
    // as an f64-to-f32 cast rounds a finite result.  Keeping this conversion
    // integer-only lets marker 9 carry source subnormals without relying on
    // adapter floating-point denormal handling.  A rounded 2^23 fraction is
    // the smallest normal f32 and is represented by exponent field one.
    if exponent < -126 {
        let target_shift = scale_exp.checked_add(149)?;
        let subnormal = if target_shift >= 0 {
            magnitude.checked_shl(u32::try_from(target_shift).ok()?)?
        } else {
            let shift = u32::try_from(target_shift.checked_neg()?).ok()?;
            if shift >= 128 {
                0
            } else if shift == 0 {
                magnitude
            } else {
                let mut rounded = magnitude >> shift;
                let remainder = magnitude & ((1u128 << shift) - 1);
                let halfway = 1u128 << (shift - 1);
                if remainder > halfway || (remainder == halfway && rounded & 1 != 0) {
                    rounded = rounded.checked_add(1)?;
                }
                rounded
            }
        };
        if subnormal >= (1u128 << 23) {
            return Some(if negative { 0x8080_0000 } else { 0x0080_0000 });
        } else {
            let mantissa = u32::try_from(subnormal).ok()?;
            let bits = mantissa;
            return Some(if negative { bits | 0x8000_0000 } else { bits });
        }
    }
    if exponent > 127 {
        return Some(if negative { 0xff80_0000 } else { 0x7f80_0000 });
    }
    if !(-126..=127).contains(&exponent) {
        return None;
    }
    let mut mantissa = if bit_length > 24 {
        let shift = bit_length - 24;
        let mut rounded = magnitude >> shift;
        let remainder = magnitude & ((1u128 << shift) - 1);
        let halfway = 1u128 << (shift - 1);
        if remainder > halfway || (remainder == halfway && rounded & 1 != 0) {
            rounded = rounded.checked_add(1)?;
        }
        rounded
    } else {
        magnitude.checked_shl(24 - bit_length)?
    };
    if mantissa >= (1u128 << 24) {
        mantissa >>= 1;
        exponent = exponent.checked_add(1)?;
        if exponent > 127 {
            return Some(if negative { 0xff80_0000 } else { 0x7f80_0000 });
        }
    }
    let mantissa = u32::try_from(mantissa & 0x7f_ff_ff).ok()?;
    let exponent_bits = u32::try_from(exponent + 127).ok()?;
    let bits = (exponent_bits << 23) | mantissa;
    Some(if negative { bits | 0x8000_0000 } else { bits })
}

/// Round the shader's unsigned 128-bit magnitude plus sign to an f32 word.
///
/// Marker 9 uses four 32-bit limbs on the device, so its exact reducer can
/// retain one more magnitude bit than a signed host `i128`.  The older helper
/// above remains for the typed luma16 proof; this variant mirrors
/// `f64_sum_to_f32`'s U128 conversion and lets the F proof model the same
/// bounded truncation at the device boundary.
fn gpu_f64_u128_integer_to_f32(magnitude: u128, negative: bool, scale_exp: i32) -> Option<u32> {
    if magnitude == 0 {
        return Some(0);
    }
    let bit_length = 128 - magnitude.leading_zeros();
    let mut exponent = scale_exp.checked_add(bit_length as i32 - 1)?;

    if exponent < -126 {
        let target_shift = scale_exp.checked_add(149)?;
        let subnormal = if target_shift >= 0 {
            magnitude.checked_shl(u32::try_from(target_shift).ok()?)?
        } else {
            let shift = u32::try_from(target_shift.checked_neg()?).ok()?;
            if shift >= 128 {
                0
            } else if shift == 0 {
                magnitude
            } else {
                let mut rounded = magnitude >> shift;
                let remainder = magnitude & ((1u128 << shift) - 1);
                let halfway = 1u128 << (shift - 1);
                if remainder > halfway || (remainder == halfway && rounded & 1 != 0) {
                    rounded = rounded.checked_add(1)?;
                }
                rounded
            }
        };
        if subnormal >= (1u128 << 23) {
            return Some(if negative { 0x8080_0000 } else { 0x0080_0000 });
        }
        let mantissa = u32::try_from(subnormal).ok()?;
        return Some(if negative {
            mantissa | 0x8000_0000
        } else {
            mantissa
        });
    }
    if exponent > 127 {
        return Some(if negative { 0xff80_0000 } else { 0x7f80_0000 });
    }
    if !(-126..=127).contains(&exponent) {
        return None;
    }

    let mut mantissa = if bit_length > 24 {
        let shift = bit_length - 24;
        let mut rounded = magnitude >> shift;
        let remainder = magnitude & ((1u128 << shift) - 1);
        let halfway = 1u128 << (shift - 1);
        if remainder > halfway || (remainder == halfway && rounded & 1 != 0) {
            rounded = rounded.checked_add(1)?;
        }
        rounded
    } else {
        magnitude.checked_shl(24 - bit_length)?
    };
    if mantissa >= (1u128 << 24) {
        mantissa >>= 1;
        exponent = exponent.checked_add(1)?;
        if exponent > 127 {
            return Some(if negative { 0xff80_0000 } else { 0x7f80_0000 });
        }
    }
    let mantissa = u32::try_from(mantissa & 0x7f_ff_ff).ok()?;
    let exponent_bits = u32::try_from(exponent + 127).ok()?;
    let bits = (exponent_bits << 23) | mantissa;
    Some(if negative { bits | 0x8000_0000 } else { bits })
}

#[derive(Clone, Copy)]
struct F64SignedMagnitude {
    magnitude: u128,
    negative: bool,
}

// Pillow 12.2.0's arm64 FLOAT32 horizontal resampler uses scalar FMA for
// rows with at most 15 taps, then switches to a vector path that materializes
// each complete 16-tap block before the ordered additions; any tail remains
// on the scalar FMA loop. Its vertical FLOAT32 path stays on the scalar FMA
// loop. Keep this boundary explicit in every host/shader admission model; a
// single arithmetic model falsely admits some wide rows whose final f32 word
// differs by one ULP.
const GPU_F_RESIZE_HORIZONTAL_FMA_MAX_TAPS: usize = 15;
const GPU_F_RESIZE_VECTOR_WIDTH: usize = 16;
const GPU_F_RESIZE_MARKER9_MAX_TAPS: usize = 32;
// The ordered integer reducer keeps only one rounded f64 state at a time, so
// its representability does not depend on the number of taps.  Cap a single
// device invocation at 8388607 taps: the matching shader bound keeps the
// worst-case ordered loop finite while allowing the next bounded direct-resize
// envelope beyond the previously proven 4194304-tap limit.  Keep one tap
// below 8388608: the encoded coefficient range otherwise exceeds the
// 128-MiB adapter binding limit after metadata/alignment overhead.  The
// complete coefficient table is checked separately below because several
// output rows can exceed that binding even when each row fits this cap.
const GPU_F_RESIZE_ORDERED_MAX_TAPS: usize = 8_388_607;
// `GpuInner::new` requests wgpu's default limits, whose storage-buffer
// binding limit is 128 MiB.  Keep f64 coefficient admission within that
// guaranteed limit even when the selected adapter reports a larger maximum;
// this also prevents a future adapter-specific range from turning a valid
// Pillow operation into a device validation error.
const GPU_F_RESIZE_MAX_COEFFICIENT_BINDING_BYTES: usize = 128 << 20;
const GPU_F_RESIZE_COEFFICIENT_ALIGNMENT_BYTES: usize = 256;

fn gpu_f_resize_uses_separate_horizontal_product_add(
    horizontal: bool,
    tap_count: usize,
    tap: usize,
) -> bool {
    if !horizontal || tap_count <= GPU_F_RESIZE_HORIZONTAL_FMA_MAX_TAPS {
        return false;
    }
    // The arm64 vector loop consumes complete 16-tap blocks. Its scalar tail
    // remains an FMA loop, even when the row as a whole is wider than 15.
    tap < (tap_count / GPU_F_RESIZE_VECTOR_WIDTH) * GPU_F_RESIZE_VECTOR_WIDTH
}

fn gpu_f_resize_accumulate_f64(
    accumulator: &mut f64,
    weight: f64,
    sample: f32,
    separate_product_add: bool,
) {
    let sample = f64::from(sample);
    if separate_product_add {
        // Keep the product out of the following add. LLVM may otherwise
        // contract this expression back into an FMA, defeating the arm64
        // wide-row contract that Pillow uses after 15 taps.
        let product = std::hint::black_box(weight * sample);
        *accumulator += product;
    } else {
        *accumulator = weight.mul_add(sample, *accumulator);
    }
}

/// Add one bounded U128 term with the same signed-magnitude ordering used by
/// the marker-9 WGSL reducer.  A same-sign overflow is rejected because the
/// shader wraps its four limbs there; only the representable exact domain is
/// safe to admit.
fn gpu_f64_signed_u128_add(
    sum: F64SignedMagnitude,
    term: u128,
    term_negative: bool,
) -> Option<F64SignedMagnitude> {
    if term == 0 {
        return Some(sum);
    }
    if sum.magnitude == 0 {
        return Some(F64SignedMagnitude {
            magnitude: term,
            negative: term_negative,
        });
    }
    if sum.negative == term_negative {
        return Some(F64SignedMagnitude {
            magnitude: sum.magnitude.checked_add(term)?,
            negative: sum.negative,
        });
    }
    if sum.magnitude < term {
        Some(F64SignedMagnitude {
            magnitude: term - sum.magnitude,
            negative: term_negative,
        })
    } else {
        Some(F64SignedMagnitude {
            magnitude: sum.magnitude - term,
            negative: sum.negative,
        })
    }
}

/// Evaluate one f64-coefficient row as an exact integer sum, then compare its
/// final f32 bits with Pillow's ordered arm64 FLOAT32 accumulation. Horizontal
/// rows use scalar FMA through 15 taps, complete 16-tap vector product/add
/// blocks after that, and scalar FMA for any tail; vertical rows use scalar FMA
/// throughout. The shader uses the same exact-sum representation; rows where
/// an intermediate rounding would change the final f32 value are rejected.
/// Finite rows may end in a signed infinity when the exact result overflows
/// f32.  Special rows use an integer IEEE state machine for NaN/infinity
/// products and are admitted only when their final bits match Pillow's
/// ordered f64 result.  That prepass remains valid for rows wider than the
/// marker-9 finite bound; finite rows above the bound stay with marker 12 or
/// exact host semantic control.
fn gpu_f_resize_f64_sample_bits(
    bytes: &[u8],
    source_dimensions: (u32, u32),
    coeffs: &FilterCoeffsF64,
    output_index: usize,
    horizontal: bool,
    line: usize,
) -> Option<u32> {
    let source_w = usize::try_from(source_dimensions.0).ok()?;
    let source_h = usize::try_from(source_dimensions.1).ok()?;
    let source_start = usize::try_from(*coeffs.xmin.get(output_index)?).ok()?;
    let weights = coeffs.weights.get(output_index)?;
    let expected_count = *coeffs.count.get(output_index)?;
    if expected_count != weights.len() || weights.len() > GPU_F_RESIZE_ORDERED_MAX_TAPS {
        return None;
    }
    if line >= if horizontal { source_h } else { source_w } {
        return None;
    }
    let source_pixel = |tap: usize| -> Option<usize> {
        let coordinate = source_start.checked_add(tap)?;
        if horizontal {
            (coordinate < source_w).then_some(line.checked_mul(source_w)?.checked_add(coordinate)?)
        } else {
            (coordinate < source_h).then_some(coordinate.checked_mul(source_w)?.checked_add(line)?)
        }
    };
    let sample_bits_at = |tap: usize| -> Option<u32> {
        let pixel = source_pixel(tap)?;
        let offset = pixel.checked_mul(4)?;
        let word = bytes.get(offset..offset.checked_add(4)?)?;
        Some(u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
    };
    let sample_at = |tap: usize| -> Option<(u32, F32IntegerParts)> {
        let bits = sample_bits_at(tap)?;
        Some((bits, gpu_f32_f64_integer_parts(bits)?))
    };

    // Non-finite F words do not need floating-point arithmetic on the device:
    // a finite coefficient times NaN is NaN, a zero coefficient times an
    // infinity is the invalid NaN operation, and opposite signed infinities
    // cancel to NaN.  Preserve the first NaN payload in tap order (the same
    // payload Pillow's ordered `mul_add` path exposes). The host-side f64
    // result below still has to agree with this device state machine before
    // the row is admitted.
    let mut ordered_accumulator = 0.0f64;
    let mut first_nan = None;
    let mut positive_infinity = false;
    let mut negative_infinity = false;
    let mut has_special = false;
    for (tap, &weight) in weights.iter().enumerate() {
        let coeff = gpu_f64_integer_parts(weight)?;
        let bits = sample_bits_at(tap)?;
        let sample = f32::from_bits(bits);
        let separate_product_add =
            gpu_f_resize_uses_separate_horizontal_product_add(horizontal, weights.len(), tap);
        gpu_f_resize_accumulate_f64(
            &mut ordered_accumulator,
            weight,
            sample,
            separate_product_add,
        );
        let exponent_bits = (bits >> 23) & 0xff;
        if exponent_bits != 0xff {
            continue;
        }
        has_special = true;
        let fraction = bits & 0x7f_ff_ff;
        if fraction != 0 {
            first_nan.get_or_insert(bits | 0x0040_0000);
        } else if coeff.mantissa == 0 {
            // 0 * infinity is an invalid operation.  Pillow stores the
            // resulting quiet NaN with the canonical f32 payload.
            first_nan.get_or_insert(0x7fc0_0000);
        } else if (bits & 0x8000_0000) != 0 {
            if coeff.negative {
                positive_infinity = true;
            } else {
                negative_infinity = true;
            }
        } else if coeff.negative {
            negative_infinity = true;
        } else {
            positive_infinity = true;
        }
    }
    if has_special {
        let actual = if let Some(bits) = first_nan {
            bits
        } else if positive_infinity && negative_infinity {
            0x7fc0_0000
        } else if positive_infinity {
            0x7f80_0000
        } else if negative_infinity {
            0xff80_0000
        } else {
            // The special scan only reaches this branch for an impossible
            // coefficient/source combination; keep the admission conservative.
            return None;
        };
        let expected = (ordered_accumulator as f32).to_bits();
        return (actual == expected).then_some(actual);
    }

    // Finite rows wider than marker 9's historical domain must use the
    // bounded ordered reducer (marker 12), whose device state models Pillow's
    // arm64 product/add split. Special rows use the same per-row binding cap:
    // their prepass avoids arithmetic, but it still consumes the encoded
    // coefficient range and cannot bypass the adapter's storage limit.
    if weights.len() > GPU_F_RESIZE_MARKER9_MAX_TAPS {
        return None;
    }

    let mut minimum_exponent = None;
    for (tap, &weight) in weights.iter().enumerate() {
        let coeff = gpu_f64_integer_parts(weight)?;
        if coeff.mantissa == 0 {
            continue;
        }
        let (_, sample) = sample_at(tap)?;
        if sample.mantissa == 0 {
            continue;
        }
        let exponent = sample
            .exponent
            .checked_sub(23)?
            .checked_add(coeff.exponent)?;
        minimum_exponent =
            Some(minimum_exponent.map_or(exponent, |minimum: i32| minimum.min(exponent)));
    }
    let Some(minimum_exponent) = minimum_exponent else {
        return Some(0);
    };

    // Keep the host model in the same unsigned-magnitude domain as the
    // shader's four-limb reducer.  Terms shifted beyond 128 bits are dropped
    // by WGSL's bounded `u128_shl`; compare the resulting word with Pillow's
    // f64 result below before admitting such a row.
    let mut sum = F64SignedMagnitude {
        magnitude: 0,
        negative: false,
    };
    let mut f64_accumulator = 0.0f64;
    for (tap, &weight) in weights.iter().enumerate() {
        let coeff = gpu_f64_integer_parts(weight)?;
        let (bits, sample) = sample_at(tap)?;
        let sample_value = f32::from_bits(bits);
        let separate_product_add =
            gpu_f_resize_uses_separate_horizontal_product_add(horizontal, weights.len(), tap);
        gpu_f_resize_accumulate_f64(
            &mut f64_accumulator,
            weight,
            sample_value,
            separate_product_add,
        );
        if coeff.mantissa == 0 || sample.mantissa == 0 {
            continue;
        }
        let exponent = sample
            .exponent
            .checked_sub(23)?
            .checked_add(coeff.exponent)?;
        let shift = u32::try_from(exponent.checked_sub(minimum_exponent)?).ok()?;
        let product = u128::from(sample.mantissa).checked_mul(u128::from(coeff.mantissa))?;
        let term = if shift < 128 {
            product.checked_shl(shift)?
        } else {
            // `u128_shl` in both resize shaders returns an all-zero limb
            // value once the shift reaches the width of the reducer.
            0
        };
        sum = gpu_f64_signed_u128_add(sum, term, sample.negative != coeff.negative)?;
    }

    let expected = (f64_accumulator as f32).to_bits();
    let expected_exponent = expected & 0x7f80_0000;
    if (expected_exponent == 0x7f80_0000 && (expected & 0x007f_ffff) != 0)
        || (expected_exponent != 0x7f80_0000 && !f64_accumulator.is_finite())
    {
        return None;
    }
    let actual = gpu_f64_u128_integer_to_f32(sum.magnitude, sum.negative, minimum_exponent)?;
    (actual == expected).then_some(actual)
}

#[derive(Clone, Copy)]
struct F64OrderedState {
    /// The finite f64 value is `magnitude * 2^exponent`.  A non-zero normal
    /// f64 has at most 53 significant bits in `magnitude`; zero is represented
    /// by a zero magnitude and a positive sign.
    magnitude: u128,
    exponent: i32,
    negative: bool,
}

/// Round an exact signed binary integer to the finite, normal f64 state used
/// by the ordered FMA proof.  The marker-12 shader performs this same
/// round-to-nearest-even step after each product+accumulator operation.  The
/// proof intentionally rejects subnormal/overflowing f64 intermediates: those
/// need a wider state machine and remain on exact host semantic control.
fn gpu_f64_ordered_round(sum: F64SignedMagnitude, scale_exp: i32) -> Option<F64OrderedState> {
    if sum.magnitude == 0 {
        return Some(F64OrderedState {
            magnitude: 0,
            exponent: 0,
            negative: false,
        });
    }
    let bit_length = 128 - sum.magnitude.leading_zeros();
    let mut exponent = scale_exp.checked_add(bit_length as i32 - 1)?;
    if !(-1022..=1023).contains(&exponent) {
        return None;
    }
    let mut mantissa = if bit_length > 53 {
        let shift = bit_length - 53;
        let mut rounded = sum.magnitude >> shift;
        let remainder = sum.magnitude & ((1u128 << shift) - 1);
        let halfway = 1u128 << (shift - 1);
        if remainder > halfway || (remainder == halfway && rounded & 1 != 0) {
            rounded = rounded.checked_add(1)?;
        }
        rounded
    } else {
        sum.magnitude.checked_shl(53 - bit_length)?
    };
    if mantissa >= (1u128 << 53) {
        mantissa >>= 1;
        exponent = exponent.checked_add(1)?;
        if exponent > 1023 {
            return None;
        }
    }
    Some(F64OrderedState {
        magnitude: mantissa,
        exponent: exponent - 52,
        negative: sum.negative,
    })
}

/// Add one exact f64-coefficient/f32-sample product to a rounded f64 state.
/// The product is supplied as an unsigned integer scaled by `product_exp`;
/// aligning it with the previous 53-bit state and rounding once models
/// `weight.mul_add(f64::from(sample), accumulator)` without device floats.
/// The horizontal arm64 vector path instead rounds the product first and then
/// adds it in tap order; `separate_product_add` selects that model.
fn gpu_f64_ordered_add_product(
    state: F64OrderedState,
    product: u128,
    product_exp: i32,
    product_negative: bool,
    separate_product_add: bool,
) -> Option<F64OrderedState> {
    if product == 0 {
        return Some(state);
    }
    let rounded_product = if separate_product_add {
        Some(gpu_f64_ordered_round(
            F64SignedMagnitude {
                magnitude: product,
                negative: product_negative,
            },
            product_exp,
        )?)
    } else {
        None
    };
    if state.magnitude == 0 {
        return rounded_product.or_else(|| {
            gpu_f64_ordered_round(
                F64SignedMagnitude {
                    magnitude: product,
                    negative: product_negative,
                },
                product_exp,
            )
        });
    }
    let (product, product_exp, product_negative) = rounded_product
        .map_or((product, product_exp, product_negative), |rounded| {
            (rounded.magnitude, rounded.exponent, rounded.negative)
        });
    let minimum_exponent = state.exponent.min(product_exp);
    let state_shift = u32::try_from(state.exponent.checked_sub(minimum_exponent)?).ok()?;
    let product_shift = u32::try_from(product_exp.checked_sub(minimum_exponent)?).ok()?;
    if (128 - state.magnitude.leading_zeros()) + state_shift > 128
        || (128 - product.leading_zeros()) + product_shift > 128
    {
        return None;
    }
    let state_term = state.magnitude.checked_shl(state_shift)?;
    let product_term = product.checked_shl(product_shift)?;
    let sum = gpu_f64_signed_u128_add(
        F64SignedMagnitude {
            magnitude: state_term,
            negative: state.negative,
        },
        product_term,
        product_negative,
    )?;
    gpu_f64_ordered_round(sum, minimum_exponent)
}

/// Evaluate a bounded f64 coefficient row with Pillow's ordered arm64
/// semantics. Marker 9 keeps the exact real sum and is necessarily
/// conservative when an intermediate f64 rounding changes the final f32 word;
/// marker 12 handles rows through 8388607 taps by emulating the scalar FMA path and
/// the >15-tap horizontal vector product/add path in integer arithmetic.
fn gpu_f_resize_f64_ordered_sample_bits(
    bytes: &[u8],
    source_dimensions: (u32, u32),
    coeffs: &FilterCoeffsF64,
    output_index: usize,
    horizontal: bool,
    line: usize,
) -> Option<u32> {
    let source_w = usize::try_from(source_dimensions.0).ok()?;
    let source_h = usize::try_from(source_dimensions.1).ok()?;
    let source_start = usize::try_from(*coeffs.xmin.get(output_index)?).ok()?;
    let weights = coeffs.weights.get(output_index)?;
    if *coeffs.count.get(output_index)? != weights.len()
        || weights.len() > GPU_F_RESIZE_ORDERED_MAX_TAPS
    {
        return None;
    }
    if line >= if horizontal { source_h } else { source_w } {
        return None;
    }
    let source_pixel = |tap: usize| -> Option<usize> {
        let coordinate = source_start.checked_add(tap)?;
        if horizontal {
            (coordinate < source_w).then_some(line.checked_mul(source_w)?.checked_add(coordinate)?)
        } else {
            (coordinate < source_h).then_some(coordinate.checked_mul(source_w)?.checked_add(line)?)
        }
    };
    let mut state = F64OrderedState {
        magnitude: 0,
        exponent: 0,
        negative: false,
    };
    let mut ordered_accumulator = 0.0f64;
    for (tap, &weight) in weights.iter().enumerate() {
        let coeff = gpu_f64_integer_parts(weight)?;
        let pixel = source_pixel(tap)?;
        let offset = pixel.checked_mul(4)?;
        let word = bytes.get(offset..offset.checked_add(4)?)?;
        let bits = u32::from_le_bytes([word[0], word[1], word[2], word[3]]);
        let sample = gpu_f32_f64_integer_parts(bits)?;
        let separate_product_add =
            gpu_f_resize_uses_separate_horizontal_product_add(horizontal, weights.len(), tap);
        // f32 subnormal inputs are represented exactly at the fixed 2^-149
        // scale. They remain normal in the f64 accumulator for Pillow's
        // finite resampling coefficients, so marker 12 can model them with
        // the same ordered integer state as normal inputs.
        gpu_f_resize_accumulate_f64(
            &mut ordered_accumulator,
            weight,
            f32::from_bits(bits),
            separate_product_add,
        );
        if coeff.mantissa == 0 || sample.mantissa == 0 {
            continue;
        }
        let product = u128::from(sample.mantissa).checked_mul(u128::from(coeff.mantissa))?;
        let product_exp = sample
            .exponent
            .checked_sub(23)?
            .checked_add(coeff.exponent)?;
        state = gpu_f64_ordered_add_product(
            state,
            product,
            product_exp,
            sample.negative != coeff.negative,
            separate_product_add,
        )?;
    }
    let actual = gpu_f64_u128_integer_to_f32(state.magnitude, state.negative, state.exponent)?;
    let expected = (ordered_accumulator as f32).to_bits();
    (actual == expected).then_some(actual)
}

fn gpu_f_resize_f64_ordered_pass_bits(
    bytes: &[u8],
    source_dimensions: (u32, u32),
    coeffs: &FilterCoeffsF64,
    horizontal: bool,
) -> Option<Vec<u32>> {
    let output_count = coeffs.xmin.len();
    let line_count = if horizontal {
        usize::try_from(source_dimensions.1).ok()?
    } else {
        usize::try_from(source_dimensions.0).ok()?
    };
    let word_count = output_count.checked_mul(line_count)?;
    if word_count > GPU_BUFFER_CAPACITY as usize {
        return None;
    }
    let mut result = Vec::new();
    result.try_reserve(word_count).ok()?;
    if horizontal {
        for line in 0..line_count {
            for output_index in 0..output_count {
                result.push(gpu_f_resize_f64_ordered_sample_bits(
                    bytes,
                    source_dimensions,
                    coeffs,
                    output_index,
                    true,
                    line,
                )?);
            }
        }
    } else {
        for output_index in 0..output_count {
            for line in 0..line_count {
                result.push(gpu_f_resize_f64_ordered_sample_bits(
                    bytes,
                    source_dimensions,
                    coeffs,
                    output_index,
                    false,
                    line,
                )?);
            }
        }
    }
    Some(result)
}

/// Evaluate over-limit Box rows whose normalized coefficient is constant.
///
/// Pillow's `Resample.c::precompute_coeffs` gives an integer-ratio Box
/// downscale exactly `1 / tap_count` for every selected source word.  Such a
/// row does not need one four-word f64 coefficient record per tap: the compact
/// device path transports one record and repeats it in the ordered reducer.
/// Finite rows use the integer state below; IEEE special rows use the same
/// ordered NaN/infinity state machine as marker 9 before being admitted.
fn gpu_f_resize_compact_box_sample_bits(
    bytes: &[u8],
    source_dimensions: (u32, u32),
    source_axis: u32,
    output_axis: u32,
    horizontal: bool,
    output_index: usize,
    line: usize,
) -> Option<u32> {
    if source_axis == 0
        || output_axis == 0
        || source_axis % output_axis != 0
        || output_index >= usize::try_from(output_axis).ok()?
        || line
            >= usize::try_from(if horizontal {
                source_dimensions.1
            } else {
                source_dimensions.0
            })
            .ok()?
    {
        return None;
    }
    let source_w = usize::try_from(source_dimensions.0).ok()?;
    let source_h = usize::try_from(source_dimensions.1).ok()?;
    let source_axis_usize = usize::try_from(source_axis).ok()?;
    let output_axis_usize = usize::try_from(output_axis).ok()?;
    if source_axis_usize != if horizontal { source_w } else { source_h } || output_axis_usize == 0 {
        return None;
    }
    let tap_count = source_axis_usize.checked_div(output_axis_usize)?;
    let source_start = output_index.checked_mul(tap_count)?;
    let coefficient = 1.0 / f64::from(u32::try_from(tap_count).ok()?);
    let coefficient_parts = gpu_f64_integer_parts(coefficient)?;
    let mut state = F64OrderedState {
        magnitude: 0,
        exponent: 0,
        negative: false,
    };
    let mut ordered_accumulator = 0.0f64;
    let mut first_nan = None;
    let mut positive_infinity = false;
    let mut negative_infinity = false;
    let mut has_special = false;
    for tap in 0..tap_count {
        let coordinate = source_start.checked_add(tap)?;
        let pixel = if horizontal {
            line.checked_mul(source_w)?.checked_add(coordinate)?
        } else {
            coordinate.checked_mul(source_w)?.checked_add(line)?
        };
        let offset = pixel.checked_mul(4)?;
        let word = bytes.get(offset..offset.checked_add(4)?)?;
        let bits = u32::from_le_bytes([word[0], word[1], word[2], word[3]]);
        let exponent_bits = (bits >> 23) & 0xff;
        let separate_product_add =
            gpu_f_resize_uses_separate_horizontal_product_add(horizontal, tap_count, tap);
        gpu_f_resize_accumulate_f64(
            &mut ordered_accumulator,
            coefficient,
            f32::from_bits(bits),
            separate_product_add,
        );
        if exponent_bits == 0xff {
            has_special = true;
            let fraction = bits & 0x7f_ff_ff;
            if fraction != 0 {
                first_nan.get_or_insert(bits | 0x0040_0000);
            } else if (bits & 0x8000_0000) != 0 {
                negative_infinity = true;
            } else {
                positive_infinity = true;
            }
            continue;
        }
        let sample = gpu_f32_f64_integer_parts(bits)?;
        if sample.mantissa == 0 {
            continue;
        }
        let product =
            u128::from(sample.mantissa).checked_mul(u128::from(coefficient_parts.mantissa))?;
        let product_exp = sample
            .exponent
            .checked_sub(23)?
            .checked_add(coefficient_parts.exponent)?;
        state = gpu_f64_ordered_add_product(
            state,
            product,
            product_exp,
            sample.negative != coefficient_parts.negative,
            separate_product_add,
        )?;
    }
    if has_special {
        let actual = if let Some(bits) = first_nan {
            bits
        } else if positive_infinity && negative_infinity {
            0x7fc0_0000
        } else if positive_infinity {
            0x7f80_0000
        } else if negative_infinity {
            0xff80_0000
        } else {
            return None;
        };
        let expected = (ordered_accumulator as f32).to_bits();
        return (actual == expected).then_some(actual);
    }
    let actual = gpu_f64_u128_integer_to_f32(state.magnitude, state.negative, state.exponent)?;
    let expected = (ordered_accumulator as f32).to_bits();
    (actual == expected).then_some(actual)
}

fn gpu_f_resize_compact_box_pass_bits(
    bytes: &[u8],
    source_dimensions: (u32, u32),
    source_axis: u32,
    output_axis: u32,
    horizontal: bool,
) -> Option<Vec<u32>> {
    let line_count = usize::try_from(if horizontal {
        source_dimensions.1
    } else {
        source_dimensions.0
    })
    .ok()?;
    let output_count = usize::try_from(output_axis).ok()?;
    let word_count = line_count.checked_mul(output_count)?;
    let mut result = Vec::new();
    result.try_reserve(word_count).ok()?;
    if horizontal {
        for line in 0..line_count {
            for output_index in 0..output_count {
                result.push(gpu_f_resize_compact_box_sample_bits(
                    bytes,
                    source_dimensions,
                    source_axis,
                    output_axis,
                    true,
                    output_index,
                    line,
                )?);
            }
        }
    } else {
        for output_index in 0..output_count {
            for line in 0..line_count {
                result.push(gpu_f_resize_compact_box_sample_bits(
                    bytes,
                    source_dimensions,
                    source_axis,
                    output_axis,
                    false,
                    output_index,
                    line,
                )?);
            }
        }
    }
    Some(result)
}

fn gpu_f_resize_compact_box_axis(source_size: u32, output_size: u32) -> bool {
    output_size != 0
        && source_size % output_size == 0
        && source_size / output_size > GPU_F_RESIZE_ORDERED_MAX_TAPS as u32
}

#[inline]
fn gpu_f_resize_compact_box_any_axis(
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
) -> bool {
    gpu_f_resize_compact_box_axis(source_dimensions.0, output_dimensions.0)
        || gpu_f_resize_compact_box_axis(source_dimensions.1, output_dimensions.1)
}

/// Return whether a direct F Box resize needs only the vertical compact pass.
/// The unchanged horizontal axis has one unit coefficient and must remain an
/// identity copy; skipping that pass is what keeps an over-limit source height
/// below the adapter's workgroup-per-dimension bound.
fn gpu_f_resize_compact_box_vertical_only_geometry(
    op: &PipelineOp,
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    logical_mode: Option<&str>,
) -> bool {
    if logical_mode != Some("F") {
        return false;
    }
    let PipelineOp::Resize { w, h, filter } = op else {
        return false;
    };
    matches!(filter, ResampleFilter::Box)
        && (*w, *h) == output_dimensions
        && output_dimensions.0 == source_dimensions.0
        && !gpu_f_resize_compact_box_axis(source_dimensions.0, output_dimensions.0)
        && gpu_f_resize_compact_box_axis(source_dimensions.1, output_dimensions.1)
}

/// Prove the compact over-limit Box domain without materializing a full f64
/// coefficient table.  An integer-ratio Box downscale has one normalized
/// weight per source tap, so a single repeated coefficient is enough on the
/// device for every output row.  At most one axis can exceed the full-table
/// bound for an image that fits the device buffer budget. The other axis keeps
/// its ordinary coefficient table; when that table is also proven exact, the
/// second axis may change and the host proof below validates the materialized
/// intermediate between passes.
fn gpu_f_resize_compact_box_is_exact(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if logical_mode != Some("F") || ops.len() != 1 || !matches!(image, DynamicImage::ImageRgba8(_))
    {
        return false;
    }
    let DynamicImage::ImageRgba8(pixels) = image else {
        return false;
    };
    let PipelineOp::Resize { w, h, filter } = &ops[0] else {
        return false;
    };
    if !matches!(filter, ResampleFilter::Box) || *w == 0 || *h == 0 {
        return false;
    }
    let source_dimensions = image.dimensions();
    let source_checked = CheckedDims::new(source_dimensions.0, source_dimensions.1, 4).ok();
    if source_dimensions.0 == 0
        || source_dimensions.1 == 0
        || source_checked
            .as_ref()
            .is_none_or(|dims| dims.total_pixels() > GPU_BUFFER_CAPACITY as usize)
        || source_checked.map(|dims| dims.total_bytes()) != Some(pixels.as_raw().len())
    {
        return false;
    }
    let horizontal_compact = gpu_f_resize_compact_box_axis(source_dimensions.0, *w);
    let vertical_compact = gpu_f_resize_compact_box_axis(source_dimensions.1, *h);
    if (horizontal_compact && vertical_compact) || (!horizontal_compact && !vertical_compact) {
        return false;
    }

    // Pillow's `src/libImaging/Resample.c` keeps a wide, non-tall Box resize
    // horizontal-first, so a compact horizontal row may be followed by an
    // ordinary small-axis Box pass. Keep this extension finite-only: the
    // compact shader's special-state machine is exact for a terminal row, but
    // a second pass would require proving NaN payload propagation through the
    // materialized intermediate.
    let second_axis_changes = (!horizontal_compact && *w != source_dimensions.0)
        || (!vertical_compact && *h != source_dimensions.1);
    if vertical_compact && *w != source_dimensions.0 {
        // This orientation is always a Pillow tall-image resize for an
        // image that fits the pixel-capacity budget; its native order is
        // vertical-first, while the GPU plan is horizontal-first.
        return false;
    }
    if second_axis_changes
        && pixels.as_raw().chunks_exact(4).any(|word| {
            let bits = u32::from_le_bytes([word[0], word[1], word[2], word[3]]);
            ((bits >> 23) & 0xff) == 0xff
        })
    {
        return false;
    }

    let ordered_pass =
        |bytes: &[u8], dimensions: (u32, u32), coeffs: &FilterCoeffsF64, horizontal| {
            let safe = coeffs.xmin.len() == coeffs.count.len()
                && coeffs.xmin.len() == coeffs.weights.len()
                && gpu_f_resize_f64_coefficients_fit_binding(coeffs)
                && coeffs
                    .count
                    .iter()
                    .all(|&count| count <= GPU_F_RESIZE_ORDERED_MAX_TAPS);
            if !safe {
                return None;
            }
            gpu_f_resize_f64_ordered_pass_bits(bytes, dimensions, coeffs, horizontal)
        };
    let words_to_bytes = |words: Vec<u32>| -> Option<Vec<u8>> {
        let byte_count = words.len().checked_mul(4)?;
        let mut result = Vec::new();
        result.try_reserve(byte_count).ok()?;
        for word in words {
            result.extend_from_slice(&word.to_le_bytes());
        }
        Some(result)
    };
    let (kernel, support) = filter_from_resample(*filter);
    let mut bytes = pixels.as_raw().to_vec();
    let mut dimensions = source_dimensions;

    if horizontal_compact {
        let Some(words) =
            gpu_f_resize_compact_box_pass_bits(&bytes, dimensions, source_dimensions.0, *w, true)
        else {
            return false;
        };
        let Some(next_bytes) = words_to_bytes(words) else {
            return false;
        };
        bytes = next_bytes;
        dimensions.0 = *w;
    } else if *w != source_dimensions.0 {
        let coeffs = precompute_coeffs_f64(*w, source_dimensions.0, kernel, support);
        let Some(words) = ordered_pass(&bytes, dimensions, &coeffs, true) else {
            return false;
        };
        let Some(next_bytes) = words_to_bytes(words) else {
            return false;
        };
        bytes = next_bytes;
        dimensions.0 = *w;
    }

    if vertical_compact {
        let Some(words) =
            gpu_f_resize_compact_box_pass_bits(&bytes, dimensions, source_dimensions.1, *h, false)
        else {
            return false;
        };
        let Some(next_bytes) = words_to_bytes(words) else {
            return false;
        };
        bytes = next_bytes;
        dimensions.1 = *h;
    } else if *h != source_dimensions.1 {
        let coeffs = precompute_coeffs_f64(*h, source_dimensions.1, kernel, support);
        let Some(words) = ordered_pass(&bytes, dimensions, &coeffs, false) else {
            return false;
        };
        let Some(next_bytes) = words_to_bytes(words) else {
            return false;
        };
        bytes = next_bytes;
        dimensions.1 = *h;
    }

    dimensions == (*w, *h) && !bytes.is_empty()
}

fn gpu_f_resize_compact_box_vertical_only_is_exact(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if ops.len() != 1 {
        return false;
    }
    let Some(output_dimensions) = op_output_dims(&ops[0], image.width(), image.height()) else {
        return false;
    };
    gpu_f_resize_compact_box_vertical_only_geometry(
        &ops[0],
        image.dimensions(),
        output_dimensions,
        logical_mode,
    ) && gpu_f_resize_compact_box_is_exact(ops, image, logical_mode)
}

/// Pillow's high-level Image.resize uses a vertical-first pair of resample
/// passes for very tall inputs before calling Resample.c. The GPU lowering is
/// currently horizontal-first, so keep this geometry on exact host semantic
/// control until a matching vertical-first device plan exists.
fn gpu_f_resize_uses_pillow_tall_order(
    source_dimensions: (u32, u32),
    destination_dimensions: (u32, u32),
) -> bool {
    source_dimensions.1 > source_dimensions.0.saturating_mul(100)
        && destination_dimensions.1 < source_dimensions.1
        && destination_dimensions.0 != source_dimensions.0
}

/// Prove one direct, changed-axis F resize in the bounded ordered-f64 domain.
/// Intermediate horizontal words are materialized before the vertical pass,
/// exactly as Pillow's separable resampler does.  Chained/relocation inputs
/// remain on marker 9 or exact host semantic control until they receive a
/// separate proof.
fn gpu_f_resize_f64_ordered_is_exact(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if gpu_f_resize_compact_box_is_exact(ops, image, logical_mode) {
        return true;
    }
    if logical_mode != Some("F") || ops.len() != 1 || !matches!(image, DynamicImage::ImageRgba8(_))
    {
        return false;
    }
    let DynamicImage::ImageRgba8(pixels) = image else {
        return false;
    };
    let PipelineOp::Resize { w, h, filter } = &ops[0] else {
        return false;
    };
    if matches!(filter, ResampleFilter::Nearest) || *w == 0 || *h == 0 {
        return false;
    }
    let source_dimensions = image.dimensions();
    if source_dimensions.0 == 0 || source_dimensions.1 == 0 {
        return false;
    }
    if CheckedDims::new(*w, *h, 1)
        .ok()
        .is_none_or(|dims| dims.total_pixels() > GPU_BUFFER_CAPACITY as usize)
    {
        return false;
    }
    let source_checked = CheckedDims::new(source_dimensions.0, source_dimensions.1, 4).ok();
    if source_checked
        .as_ref()
        .is_none_or(|dims| dims.total_pixels() > GPU_BUFFER_CAPACITY as usize)
        || source_checked.map(|dims| dims.total_bytes()) != Some(pixels.as_raw().len())
    {
        return false;
    }
    let (kernel, support) = filter_from_resample(*filter);
    let horizontal = precompute_coeffs_f64(*w, source_dimensions.0, kernel, support);
    let vertical = precompute_coeffs_f64(*h, source_dimensions.1, kernel, support);
    if [&horizontal, &vertical].iter().any(|coeffs| {
        coeffs.xmin.len() != coeffs.count.len()
            || coeffs.xmin.len() != coeffs.weights.len()
            || !gpu_f_resize_f64_coefficients_fit_binding(coeffs)
            || coeffs
                .count
                .iter()
                .any(|&count| count > GPU_F_RESIZE_ORDERED_MAX_TAPS)
    }) {
        return false;
    }
    let horizontal_changed = *w != source_dimensions.0;
    let vertical_changed = *h != source_dimensions.1;
    if !horizontal_changed && !vertical_changed {
        return false;
    }
    let words_to_bytes = |words: Vec<u32>| -> Option<Vec<u8>> {
        let byte_count = words.len().checked_mul(4)?;
        let mut result = Vec::new();
        result.try_reserve(byte_count).ok()?;
        for word in words {
            result.extend_from_slice(&word.to_le_bytes());
        }
        Some(result)
    };
    let mut bytes = pixels.as_raw().to_vec();
    let mut dimensions = source_dimensions;
    if horizontal_changed {
        let words = gpu_f_resize_f64_ordered_pass_bits(&bytes, dimensions, &horizontal, true);
        let Some(next_bytes) = words.and_then(words_to_bytes) else {
            return false;
        };
        bytes = next_bytes;
        dimensions.0 = *w;
    }
    if vertical_changed {
        let words = gpu_f_resize_f64_ordered_pass_bits(&bytes, dimensions, &vertical, false);
        let Some(_next_bytes) = words.and_then(words_to_bytes) else {
            return false;
        };
    }
    true
}

/// Round an exact signed binary sum to Pillow's INT32 `ROUND_UP` result.
///
/// `sum * 2^scale_exp` is kept as an integer/rational pair so the admission
/// proof does not rely on a host f64 conversion at a half-integer boundary.
/// The returned value is deliberately restricted to the signed i32 range;
/// overflowing rows stay on exact host semantic control rather than relying
/// on device or Rust cast saturation.
fn gpu_i_f64_integer_to_i32(sum: i128, scale_exp: i32) -> Option<i32> {
    if sum == 0 {
        return Some(0);
    }
    let negative = sum < 0;
    let magnitude = if negative {
        sum.checked_neg()? as u128
    } else {
        sum as u128
    };
    let rounded = if scale_exp >= 0 {
        magnitude.checked_shl(u32::try_from(scale_exp).ok()?)?
    } else {
        let shift = u32::try_from(scale_exp.checked_neg()?).ok()?;
        if shift >= 128 {
            0
        } else if shift == 0 {
            magnitude
        } else {
            let mut quotient = magnitude >> shift;
            let remainder = magnitude & ((1u128 << shift) - 1);
            let halfway = 1u128 << (shift - 1);
            // ROUND_UP is away from zero, so exact half values increment too.
            if remainder >= halfway {
                quotient = quotient.checked_add(1)?;
            }
            quotient
        }
    };
    if negative {
        if rounded > (1u128 << 31) {
            return None;
        }
        if rounded == (1u128 << 31) {
            Some(i32::MIN)
        } else {
            Some(-(i32::try_from(rounded).ok()?))
        }
    } else {
        Some(i32::try_from(rounded).ok()?)
    }
}

/// Evaluate one I-mode f64-coefficient row with an exact integer reducer.
///
/// I samples are exact signed i32 values, while each Pillow coefficient is a
/// finite binary f64.  Multiplying the integer sample by the coefficient's
/// 53-bit mantissa and aligning all terms at the smallest exponent gives an
/// exact rational sum in the same U128 representation used by the WGSL
/// marker-9 reducer.  The host proof compares that sum's away-from-zero i32
/// rounding with Pillow's ordered f64 FMA accumulation; rows where the two
/// boundaries differ are rejected.
fn gpu_i_resize_f64_sample_bits(
    bytes: &[u8],
    source_dimensions: (u32, u32),
    coeffs: &FilterCoeffsF64,
    output_index: usize,
    horizontal: bool,
    line: usize,
) -> Option<u32> {
    let source_w = usize::try_from(source_dimensions.0).ok()?;
    let source_h = usize::try_from(source_dimensions.1).ok()?;
    let source_start = usize::try_from(*coeffs.xmin.get(output_index)?).ok()?;
    let weights = coeffs.weights.get(output_index)?;
    if *coeffs.count.get(output_index)? != weights.len()
        || line >= if horizontal { source_h } else { source_w }
    {
        return None;
    }
    let source_pixel = |tap: usize| -> Option<usize> {
        let coordinate = source_start.checked_add(tap)?;
        if horizontal {
            (coordinate < source_w).then_some(line.checked_mul(source_w)?.checked_add(coordinate)?)
        } else {
            (coordinate < source_h).then_some(coordinate.checked_mul(source_w)?.checked_add(line)?)
        }
    };
    let sample_at = |tap: usize| -> Option<i32> {
        let pixel = source_pixel(tap)?;
        let offset = pixel.checked_mul(4)?;
        let word = bytes.get(offset..offset.checked_add(4)?)?;
        Some(i32::from_le_bytes([word[0], word[1], word[2], word[3]]))
    };

    let mut minimum_exponent = None;
    for (tap, &weight) in weights.iter().enumerate() {
        let coeff = gpu_f64_integer_parts(weight)?;
        let sample = sample_at(tap)?;
        if coeff.mantissa != 0 && sample != 0 {
            minimum_exponent = Some(
                minimum_exponent.map_or(coeff.exponent, |minimum: i32| minimum.min(coeff.exponent)),
            );
        }
    }
    let Some(minimum_exponent) = minimum_exponent else {
        return Some(0);
    };

    let mut exact_sum = 0i128;
    let mut ordered_f64 = 0.0f64;
    for (tap, &weight) in weights.iter().enumerate() {
        let coeff = gpu_f64_integer_parts(weight)?;
        let sample = sample_at(tap)?;
        ordered_f64 = weight.mul_add(f64::from(sample), ordered_f64);
        if coeff.mantissa == 0 || sample == 0 {
            continue;
        }
        let magnitude = if sample < 0 {
            i128::from(sample).checked_neg()?
        } else {
            i128::from(sample)
        };
        let product = magnitude.checked_mul(i128::try_from(coeff.mantissa).ok()?)?;
        let shift = u32::try_from(coeff.exponent.checked_sub(minimum_exponent)?).ok()?;
        let term = product.checked_shl(shift)?;
        let term = if sample < 0 {
            term.checked_neg()?
        } else {
            term
        };
        let term = if coeff.negative {
            term.checked_neg()?
        } else {
            term
        };
        exact_sum = exact_sum.checked_add(term)?;
    }
    if !ordered_f64.is_finite() {
        return None;
    }
    let expected = round_up(ordered_f64);
    if !expected.is_finite() {
        return None;
    }
    let expected = i32::try_from(expected as i64).ok()?;
    let actual = gpu_i_f64_integer_to_i32(exact_sum, minimum_exponent)?;
    (actual == expected).then_some(actual as u32)
}

/// Evaluate every word produced by one exact I-mode resize pass. The result
/// order matches the packed image rows consumed by the next shader pass.
fn gpu_i_resize_f64_pass_bits(
    bytes: &[u8],
    source_dimensions: (u32, u32),
    coeffs: &FilterCoeffsF64,
    horizontal: bool,
) -> Option<Vec<u32>> {
    let output_count = coeffs.xmin.len();
    let source_axis = if horizontal {
        usize::try_from(source_dimensions.0).ok()?
    } else {
        usize::try_from(source_dimensions.1).ok()?
    };
    let line_count = if horizontal {
        usize::try_from(source_dimensions.1).ok()?
    } else {
        usize::try_from(source_dimensions.0).ok()?
    };
    let word_count = output_count.checked_mul(line_count)?;
    if word_count > GPU_BUFFER_CAPACITY as usize {
        return None;
    }
    if output_count == source_axis {
        let expected = word_count.checked_mul(4)?;
        if bytes.len() != expected {
            return None;
        }
        return Some(
            bytes
                .chunks_exact(4)
                .map(|word| u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
                .collect(),
        );
    }
    let mut result = Vec::new();
    result.try_reserve(word_count).ok()?;
    if horizontal {
        for line in 0..line_count {
            for output_index in 0..output_count {
                result.push(gpu_i_resize_f64_sample_bits(
                    bytes,
                    source_dimensions,
                    coeffs,
                    output_index,
                    true,
                    line,
                )?);
            }
        }
    } else {
        for output_index in 0..output_count {
            for line in 0..line_count {
                result.push(gpu_i_resize_f64_sample_bits(
                    bytes,
                    source_dimensions,
                    coeffs,
                    output_index,
                    false,
                    line,
                )?);
            }
        }
    }
    Some(result)
}

/// Prove one pure filtered I resize for the typed GPU reducer. Horizontal
/// results are rounded to INT32 before the vertical pass, matching Pillow's
/// `ImagingResampleHorizontal_32bpc`/`Vertical_32bpc` storage boundary.
fn gpu_i_resize_f64_is_exact(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if logical_mode != Some("I") || ops.len() != 1 || !matches!(image, DynamicImage::ImageRgba8(_))
    {
        return false;
    }
    let PipelineOp::Resize { w, h, filter } = &ops[0] else {
        return false;
    };
    if matches!(filter, ResampleFilter::Nearest) || *w == 0 || *h == 0 {
        return false;
    }
    let source_dimensions = image.dimensions();
    if source_dimensions.0 == 0 || source_dimensions.1 == 0 {
        return false;
    }
    let DynamicImage::ImageRgba8(pixels) = image else {
        return false;
    };
    let expected = CheckedDims::new(source_dimensions.0, source_dimensions.1, 4)
        .ok()
        .map(|dims| dims.total_bytes());
    if expected != Some(pixels.as_raw().len())
        || CheckedDims::new(*w, *h, 1)
            .ok()
            .map(|dims| dims.total_pixels())
            .is_none_or(|count| count > GPU_BUFFER_CAPACITY as usize)
    {
        return false;
    }

    let (kernel, support) = filter_from_resample(*filter);
    let horizontal = precompute_coeffs_f64(*w, source_dimensions.0, kernel, support);
    let vertical = precompute_coeffs_f64(*h, source_dimensions.1, kernel, support);
    for coeffs in [&horizontal, &vertical] {
        if coeffs.xmin.len() != coeffs.count.len()
            || coeffs.xmin.len() != coeffs.weights.len()
            || coeffs.weights.iter().any(|row| {
                row.iter()
                    .any(|&weight| gpu_f64_integer_parts(weight).is_none())
            })
        {
            return false;
        }
    }

    let bytes = pixels.as_raw();
    let horizontal_words = gpu_i_resize_f64_pass_bits(bytes, source_dimensions, &horizontal, true);
    let Some(horizontal_words) = horizontal_words else {
        return false;
    };
    let horizontal_bytes = horizontal_words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<_>>();
    gpu_i_resize_f64_pass_bits(
        &horizontal_bytes,
        (*w, source_dimensions.1),
        &vertical,
        false,
    )
    .is_some()
}

/// Evaluate one I;16 f64-coefficient row with the same f32-normalized integer
/// reducer used by the shader, then compare its independently-clipped u16
/// bytes with Pillow's ordered f64 accumulation.  The source u16 is converted
/// to an f32 word before it reaches WGSL, so the proof must account for that
/// significand/exponent representation (and reject rows whose aligned sum
/// exceeds the device reducer's four-limb envelope).
fn gpu_luma16_f64_sample(
    samples: &[u16],
    source_dimensions: (u32, u32),
    coeffs: &FilterCoeffsF64,
    output_index: usize,
    horizontal: bool,
    line: usize,
    big_endian: bool,
) -> Option<u16> {
    let source_w = usize::try_from(source_dimensions.0).ok()?;
    let source_h = usize::try_from(source_dimensions.1).ok()?;
    let source_start = usize::try_from(*coeffs.xmin.get(output_index)?).ok()?;
    let weights = coeffs.weights.get(output_index)?;
    let expected_count = *coeffs.count.get(output_index)?;
    if expected_count != weights.len() {
        return None;
    }
    if line >= if horizontal { source_h } else { source_w } {
        return None;
    }
    let sample_at = |tap: usize| -> Option<u16> {
        let coordinate = source_start.checked_add(tap)?;
        let index = if horizontal {
            if coordinate >= source_w {
                return None;
            }
            line.checked_mul(source_w)?.checked_add(coordinate)?
        } else {
            if coordinate >= source_h {
                return None;
            }
            coordinate.checked_mul(source_w)?.checked_add(line)?
        };
        let sample = *samples.get(index)?;
        Some(luma16_resample_read(sample, big_endian))
    };

    let mut minimum_exponent = None;
    for (tap, &weight) in weights.iter().enumerate() {
        let coeff = gpu_f64_integer_parts(weight)?;
        if coeff.mantissa == 0 {
            continue;
        }
        let sample = sample_at(tap)?;
        if sample == 0 {
            continue;
        }
        let sample_parts = gpu_f32_f64_integer_parts(f32::from(sample).to_bits())?;
        if sample_parts.mantissa == 0 {
            continue;
        }
        let exponent = sample_parts
            .exponent
            .checked_sub(23)?
            .checked_add(coeff.exponent)?;
        minimum_exponent =
            Some(minimum_exponent.map_or(exponent, |minimum: i32| minimum.min(exponent)));
    }
    let Some(minimum_exponent) = minimum_exponent else {
        return Some(0);
    };

    let mut sum = 0i128;
    let mut f64_accumulator = 0.0f64;
    for (tap, &weight) in weights.iter().enumerate() {
        let coeff = gpu_f64_integer_parts(weight)?;
        let sample = sample_at(tap)?;
        // Keep this in the same ordered multiply/add form as the native
        // luma16 path (`horizontal_pass_luma16`/`vertical_pass_luma16`).
        f64_accumulator += f64::from(sample) * weight;
        if coeff.mantissa == 0 || sample == 0 {
            continue;
        }
        let sample_parts = gpu_f32_f64_integer_parts(f32::from(sample).to_bits())?;
        if sample_parts.mantissa == 0 {
            continue;
        }
        let exponent = sample_parts
            .exponent
            .checked_sub(23)?
            .checked_add(coeff.exponent)?;
        let shift = u32::try_from(exponent.checked_sub(minimum_exponent)?).ok()?;
        let product = u128::from(sample_parts.mantissa).checked_mul(u128::from(coeff.mantissa))?;
        let term = i128::try_from(product.checked_shl(shift)?).ok()?;
        sum = if coeff.negative {
            sum.checked_sub(term)?
        } else {
            sum.checked_add(term)?
        };
    }

    let expected = luma16_resample_write(f64_accumulator, big_endian);
    // The device reducer materializes an f32 before the I;16 byte-level
    // round/clip step.  That conversion can move a half-integer boundary even
    // though the exact integer sum agrees with the f64 accumulator, so prove
    // the same intermediate word rather than admitting on the exact sum
    // alone.
    let shader_bits = gpu_f64_integer_to_f32(sum, minimum_exponent)?;
    let shader_value = f32::from_bits(shader_bits);
    if !shader_value.is_finite() {
        return None;
    }
    let actual = luma16_resample_write(f64::from(shader_value), big_endian);
    (actual == expected).then_some(actual)
}

fn gpu_luma16_f64_pass(
    samples: &[u16],
    source_dimensions: (u32, u32),
    coeffs: &FilterCoeffsF64,
    horizontal: bool,
    big_endian: bool,
) -> Option<Vec<u16>> {
    let output_count = coeffs.xmin.len();
    let source_axis = if horizontal {
        usize::try_from(source_dimensions.0).ok()?
    } else {
        usize::try_from(source_dimensions.1).ok()?
    };
    let line_count = if horizontal {
        usize::try_from(source_dimensions.1).ok()?
    } else {
        usize::try_from(source_dimensions.0).ok()?
    };
    let word_count = output_count.checked_mul(line_count)?;
    if word_count > GPU_BUFFER_CAPACITY as usize {
        return None;
    }
    // Pillow's same-size typed pass is an identity after its byte-level
    // read/round/write boundary.  Mirror the shader's packed-word copy here
    // instead of feeding tiny Lanczos/Bicubic tail coefficients into the
    // integer reducer (those tails are numerically zero at u16 precision but
    // can otherwise consume the reducer's exponent budget).
    if output_count == source_axis {
        return (samples.len() == word_count).then(|| samples.to_vec());
    }
    let mut result = Vec::new();
    result.try_reserve(word_count).ok()?;
    if horizontal {
        for line in 0..line_count {
            for output_index in 0..output_count {
                result.push(gpu_luma16_f64_sample(
                    samples,
                    source_dimensions,
                    coeffs,
                    output_index,
                    true,
                    line,
                    big_endian,
                )?);
            }
        }
    } else {
        for output_index in 0..output_count {
            for line in 0..line_count {
                result.push(gpu_luma16_f64_sample(
                    samples,
                    source_dimensions,
                    coeffs,
                    output_index,
                    false,
                    line,
                    big_endian,
                )?);
            }
        }
    }
    Some(result)
}

/// Prove a single filtered I;16 resize for the typed packed-word shader.
/// Pillow materializes a native u16 intermediate after the horizontal pass,
/// so the proof performs that same byte-order-aware round/clip step before
/// validating the vertical reducer. Chains and mixed operation batches stay
/// on exact host semantic control until their intermediate contracts are
/// separately established.
fn gpu_luma16_resize_f64_is_exact(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if !(logical_mode.is_none()
        || matches!(logical_mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N")))
        || ops.len() != 1
    {
        return false;
    }
    let PipelineOp::Resize { w, h, filter } = &ops[0] else {
        return false;
    };
    if matches!(filter, ResampleFilter::Nearest) || *w == 0 || *h == 0 {
        return false;
    }
    let DynamicImage::ImageLuma16(pixels) = image else {
        return false;
    };
    let source_dimensions = image.dimensions();
    if source_dimensions.0 == 0 || source_dimensions.1 == 0 {
        return false;
    }
    let expected = CheckedDims::new(source_dimensions.0, source_dimensions.1, 1)
        .ok()
        .map(|dims| dims.total_pixels());
    if expected != Some(pixels.as_raw().len()) {
        return false;
    }
    if CheckedDims::new(*w, *h, 1)
        .ok()
        .map(|dims| dims.total_pixels())
        .is_none_or(|count| count > GPU_BUFFER_CAPACITY as usize)
    {
        return false;
    }

    let (kernel, support) = filter_from_resample(*filter);
    let horizontal = precompute_coeffs_f64(*w, source_dimensions.0, kernel, support);
    let vertical = precompute_coeffs_f64(*h, source_dimensions.1, kernel, support);
    for coeffs in [&horizontal, &vertical] {
        if coeffs.xmin.len() != coeffs.count.len()
            || coeffs.xmin.len() != coeffs.weights.len()
            || coeffs.weights.iter().any(|row| {
                row.iter()
                    .any(|&weight| gpu_f64_integer_parts(weight).is_none())
            })
        {
            return false;
        }
    }

    let big_endian = luma16_resample_big_endian(logical_mode);
    let Some(horizontal_samples) = gpu_luma16_f64_pass(
        pixels.as_raw(),
        source_dimensions,
        &horizontal,
        true,
        big_endian,
    ) else {
        return false;
    };
    gpu_luma16_f64_pass(
        &horizontal_samples,
        (*w, source_dimensions.1),
        &vertical,
        false,
        big_endian,
    )
    .is_some()
}

fn gpu_f_resize_f64_pass_bits(
    bytes: &[u8],
    source_dimensions: (u32, u32),
    coeffs: &FilterCoeffsF64,
    horizontal: bool,
) -> Option<Vec<u32>> {
    let output_count = coeffs.xmin.len();
    let line_count = if horizontal {
        usize::try_from(source_dimensions.1).ok()?
    } else {
        usize::try_from(source_dimensions.0).ok()?
    };
    let word_count = output_count.checked_mul(line_count)?;
    if word_count > GPU_BUFFER_CAPACITY as usize {
        return None;
    }
    let mut result = Vec::new();
    result.try_reserve(word_count).ok()?;
    if horizontal {
        for line in 0..line_count {
            for output_index in 0..output_count {
                result.push(gpu_f_resize_f64_sample_bits(
                    bytes,
                    source_dimensions,
                    coeffs,
                    output_index,
                    true,
                    line,
                )?);
            }
        }
    } else {
        for output_index in 0..output_count {
            for line in 0..line_count {
                result.push(gpu_f_resize_f64_sample_bits(
                    bytes,
                    source_dimensions,
                    coeffs,
                    output_index,
                    false,
                    line,
                )?);
            }
        }
    }
    Some(result)
}

/// Relocate complete four-byte F words through an operation that does not
/// interpret the scalar as color or perform arithmetic.  Marker 9 uses this
/// between filtered resize stages so the next f64 reducer sees the exact
/// words Pillow materialized at the preceding geometry boundary.
fn gpu_f_resize_relocate_words(
    bytes: &[u8],
    dimensions: (u32, u32),
    op: &PipelineOp,
) -> Option<(Vec<u8>, (u32, u32))> {
    let (width, height) = dimensions;
    let checked = CheckedDims::new(width, height, 4).ok()?;
    if checked.total_pixels() > GPU_BUFFER_CAPACITY as usize {
        return None;
    }
    let expected = checked.total_bytes();
    if bytes.len() != expected {
        return None;
    }
    let copy_word = |output: &mut [u8], destination: usize, source: usize| -> Option<()> {
        let source_offset = source.checked_mul(4)?;
        let destination_offset = destination.checked_mul(4)?;
        let source_word = bytes.get(source_offset..source_offset.checked_add(4)?)?;
        output
            .get_mut(destination_offset..destination_offset.checked_add(4)?)?
            .copy_from_slice(source_word);
        Some(())
    };

    match op {
        PipelineOp::Duplicate => Some((bytes.to_vec(), dimensions)),
        PipelineOp::Mirror | PipelineOp::Flip => {
            if width == 0 || height == 0 {
                return None;
            }
            let mut output = vec![0u8; expected];
            for y in 0..height as usize {
                for x in 0..width as usize {
                    let source_x = if matches!(op, PipelineOp::Mirror) {
                        width as usize - 1 - x
                    } else {
                        x
                    };
                    let source_y = if matches!(op, PipelineOp::Flip) {
                        height as usize - 1 - y
                    } else {
                        y
                    };
                    let destination = y.checked_mul(width as usize)?.checked_add(x)?;
                    let source = source_y
                        .checked_mul(width as usize)?
                        .checked_add(source_x)?;
                    copy_word(&mut output, destination, source)?;
                }
            }
            Some((output, dimensions))
        }
        PipelineOp::Transpose { method } => {
            if width == 0 || height == 0 {
                return None;
            }
            let output_dimensions = transpose_output_dimensions(method, width, height);
            let output_bytes = CheckedDims::new(output_dimensions.0, output_dimensions.1, 4)
                .ok()?
                .total_bytes();
            let mut output = vec![0u8; output_bytes];
            for y in 0..height as usize {
                for x in 0..width as usize {
                    let (output_x, output_y) =
                        transpose_forward(method, width, height, x as u32, y as u32);
                    let destination = usize::try_from(output_y)
                        .ok()?
                        .checked_mul(usize::try_from(output_dimensions.0).ok()?)?
                        .checked_add(usize::try_from(output_x).ok()?)?;
                    let source = y.checked_mul(width as usize)?.checked_add(x)?;
                    copy_word(&mut output, destination, source)?;
                }
            }
            Some((output, output_dimensions))
        }
        PipelineOp::Crop {
            left,
            top,
            right,
            bottom,
        } => {
            if width == 0
                || height == 0
                || *left >= *right
                || *top >= *bottom
                || *right > width
                || *bottom > height
            {
                return None;
            }
            let output_dimensions = (right - left, bottom - top);
            let output_bytes = CheckedDims::new(output_dimensions.0, output_dimensions.1, 4)
                .ok()?
                .total_bytes();
            let mut output = vec![0u8; output_bytes];
            for y in 0..output_dimensions.1 as usize {
                for x in 0..output_dimensions.0 as usize {
                    let source = usize::try_from(*top)
                        .ok()?
                        .checked_add(y)?
                        .checked_mul(width as usize)?
                        .checked_add(usize::try_from(*left).ok()?.checked_add(x)?)?;
                    let destination = y
                        .checked_mul(output_dimensions.0 as usize)?
                        .checked_add(x)?;
                    copy_word(&mut output, destination, source)?;
                }
            }
            Some((output, output_dimensions))
        }
        PipelineOp::CropBorder { border } => {
            let border = border.checked_mul(2)?;
            if width == 0 || height == 0 || border >= width || border >= height {
                return None;
            }
            let output_dimensions = (width - border, height - border);
            let output_bytes = CheckedDims::new(output_dimensions.0, output_dimensions.1, 4)
                .ok()?
                .total_bytes();
            let mut output = vec![0u8; output_bytes];
            let border = usize::try_from(border).ok()?;
            for y in 0..output_dimensions.1 as usize {
                for x in 0..output_dimensions.0 as usize {
                    let source = (y + border)
                        .checked_mul(width as usize)?
                        .checked_add(x + border)?;
                    let destination = y
                        .checked_mul(output_dimensions.0 as usize)?
                        .checked_add(x)?;
                    copy_word(&mut output, destination, source)?;
                }
            }
            Some((output, output_dimensions))
        }
        PipelineOp::Offset { x, y } => {
            if width == 0 || height == 0 {
                return None;
            }
            let source_x = (-(i64::from(*x))).rem_euclid(i64::from(width)) as usize;
            let source_y = (-(i64::from(*y))).rem_euclid(i64::from(height)) as usize;
            let mut output = vec![0u8; expected];
            for destination_y in 0..height as usize {
                for destination_x in 0..width as usize {
                    let source = ((destination_y + source_y) % height as usize)
                        .checked_mul(width as usize)?
                        .checked_add((destination_x + source_x) % width as usize)?;
                    let destination = destination_y
                        .checked_mul(width as usize)?
                        .checked_add(destination_x)?;
                    copy_word(&mut output, destination, source)?;
                }
            }
            Some((output, dimensions))
        }
        _ => None,
    }
}

/// Materialize one exact nearest F resize pass for marker 9's subsequent
/// filtered stage.  The nearest table contains one unit-weight source word
/// per output coordinate; no scalar arithmetic is performed.
fn gpu_f_resize_nearest_pass_words(
    bytes: &[u8],
    source_dimensions: (u32, u32),
    coeffs: &FilterCoeffs,
    horizontal: bool,
) -> Option<Vec<u32>> {
    let source_width = usize::try_from(source_dimensions.0).ok()?;
    let source_height = usize::try_from(source_dimensions.1).ok()?;
    let output_count = coeffs.xmin.len();
    let line_count = if horizontal {
        source_height
    } else {
        source_width
    };
    let word_count = output_count.checked_mul(line_count)?;
    if word_count > GPU_BUFFER_CAPACITY as usize {
        return None;
    }
    let mut result = Vec::new();
    result.try_reserve(word_count).ok()?;
    for line in 0..line_count {
        for output_index in 0..output_count {
            let source_start = usize::try_from(*coeffs.xmin.get(output_index)?).ok()?;
            if *coeffs.count.get(output_index)? != 1
                || *coeffs.weights.get(*coeffs.offsets.get(output_index)?)? != 1 << 22
            {
                return None;
            }
            let coordinate = source_start;
            let source_pixel = if horizontal {
                if coordinate >= source_width {
                    return None;
                }
                line.checked_mul(source_width)?.checked_add(coordinate)?
            } else {
                if coordinate >= source_height {
                    return None;
                }
                coordinate.checked_mul(source_width)?.checked_add(line)?
            };
            let offset = source_pixel.checked_mul(4)?;
            let word = bytes.get(offset..offset.checked_add(4)?)?;
            result.push(u32::from_le_bytes([word[0], word[1], word[2], word[3]]));
        }
    }
    Some(result)
}

/// Admit one or more filtered F resizes to marker 9 when every f64 coefficient
/// product and final f32 store is reproduced by the exact integer reducer.
/// Each changed axis is materialized as rounded f32 words before the next
/// resize is checked, matching Pillow's observable boundary between chained
/// operations. The encoder places every F resize's horizontal and vertical
/// reducers in separate compute passes so the next stage reads completed
/// intermediates.
fn gpu_f_resize_f64_is_exact(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if logical_mode != Some("F") || ops.is_empty() || !matches!(image, DynamicImage::ImageRgba8(_))
    {
        return false;
    }
    let DynamicImage::ImageRgba8(pixels) = image else {
        return false;
    };
    let mut source_dimensions = image.dimensions();
    let source_checked = CheckedDims::new(source_dimensions.0, source_dimensions.1, 4).ok();
    if source_dimensions.0 == 0
        || source_dimensions.1 == 0
        || source_checked
            .as_ref()
            .is_none_or(|dims| dims.total_pixels() > GPU_BUFFER_CAPACITY as usize)
    {
        return false;
    }
    let expected_bytes = source_checked.map(|dims| dims.total_bytes());
    let mut bytes = pixels.as_raw().to_vec();
    if expected_bytes != Some(bytes.len()) {
        return false;
    }
    let words_to_bytes = |words: Vec<u32>| -> Option<Vec<u8>> {
        let byte_count = words.len().checked_mul(4)?;
        let mut result = Vec::new();
        result.try_reserve(byte_count).ok()?;
        for word in words {
            result.extend_from_slice(&word.to_le_bytes());
        }
        Some(result)
    };
    let mut changed_any = false;

    for op in ops {
        if let PipelineOp::PutData {
            data,
            mode: PixelMode::F,
        } = op
        {
            // PutData(F) is a raw little-endian word replacement in the GPU
            // shader. Replace the proof source with those exact words before
            // checking the following resize, just as Pillow's deferred
            // pipeline does at its materialization boundary.
            let expected_bytes = CheckedDims::new(source_dimensions.0, source_dimensions.1, 4)
                .ok()
                .map(|dims| dims.total_bytes());
            if expected_bytes != Some(data.len()) || !data.chunks_exact(4).remainder().is_empty() {
                return false;
            }
            bytes = data.to_vec();
            continue;
        }
        let PipelineOp::Resize { w, h, filter } = op else {
            // Complete-word relocation operations preserve the scalar F
            // representation exactly. Materialize those words in the proof
            // so a following filtered resize is checked against the same
            // intermediate that the GPU dispatch will consume.
            let Some((next_bytes, next_dimensions)) =
                gpu_f_resize_relocate_words(&bytes, source_dimensions, op)
            else {
                // Arithmetic, mode transitions, and geometry that changes a
                // sample value still need their own typed contract.
                return false;
            };
            bytes = next_bytes;
            source_dimensions = next_dimensions;
            continue;
        };
        if *w == 0 || *h == 0 {
            return false;
        }
        if CheckedDims::new(*w, *h, 1)
            .ok()
            .map(|dims| dims.total_pixels())
            .is_none_or(|pixels| pixels > GPU_BUFFER_CAPACITY as usize)
        {
            return false;
        }

        if matches!(filter, ResampleFilter::Nearest) {
            let horizontal_changed = *w != source_dimensions.0;
            let vertical_changed = *h != source_dimensions.1;
            if horizontal_changed {
                let coefficients =
                    gpu_resize_coefficients(*w, source_dimensions.0, ResampleFilter::Nearest);
                let Some(words) =
                    gpu_f_resize_nearest_pass_words(&bytes, source_dimensions, &coefficients, true)
                else {
                    return false;
                };
                let Some(next_bytes) = words_to_bytes(words) else {
                    return false;
                };
                bytes = next_bytes;
                source_dimensions.0 = *w;
            }
            if vertical_changed {
                let coefficients =
                    gpu_resize_coefficients(*h, source_dimensions.1, ResampleFilter::Nearest);
                let Some(words) = gpu_f_resize_nearest_pass_words(
                    &bytes,
                    source_dimensions,
                    &coefficients,
                    false,
                ) else {
                    return false;
                };
                let Some(next_bytes) = words_to_bytes(words) else {
                    return false;
                };
                bytes = next_bytes;
                source_dimensions = (*w, *h);
            }
            continue;
        }

        let (kernel, support) = filter_from_resample(*filter);
        let horizontal = precompute_coeffs_f64(*w, source_dimensions.0, kernel, support);
        let vertical = precompute_coeffs_f64(*h, source_dimensions.1, kernel, support);
        for coeffs in [&horizontal, &vertical] {
            if coeffs.xmin.len() != coeffs.count.len()
                || coeffs.xmin.len() != coeffs.weights.len()
                || !gpu_f_resize_f64_coefficients_fit_binding(coeffs)
                // Marker 9's exact-real reducer is not a proof of Pillow's
                // arm64 wide-row contract. Finite rows above 32 taps use
                // marker 12's ordered reducer; only rows containing a special
                // value can use marker 9's prepass beyond this bound.
                || coeffs.weights.iter().any(|row| {
                    row.iter()
                        .any(|&weight| gpu_f64_integer_parts(weight).is_none())
                })
            {
                return false;
            }
        }

        let horizontal_changed = *w != source_dimensions.0;
        let vertical_changed = *h != source_dimensions.1;
        changed_any |= horizontal_changed || vertical_changed;
        if horizontal_changed {
            let Some(words) =
                gpu_f_resize_f64_pass_bits(&bytes, source_dimensions, &horizontal, true)
            else {
                return false;
            };
            let Some(next_bytes) = words_to_bytes(words) else {
                return false;
            };
            bytes = next_bytes;
            source_dimensions = (*w, source_dimensions.1);
        }
        if vertical_changed {
            let Some(words) =
                gpu_f_resize_f64_pass_bits(&bytes, source_dimensions, &vertical, false)
            else {
                return false;
            };
            let Some(next_bytes) = words_to_bytes(words) else {
                return false;
            };
            bytes = next_bytes;
            source_dimensions = (*w, *h);
        }
    }
    changed_any
}

/// Admit a heterogeneous F-mode Pad whose contain resize is already covered
/// by marker 9.  Pad adds no sample arithmetic after that resize: its final
/// pass only copies complete four-byte words into the host-chosen canvas and
/// writes the resolved scalar fill word elsewhere.  Keep this proof narrow
/// to one non-nearest Pad with a changed contain axis.  A prefix made only of
/// PutData(F) replacements is allowed because marker 9 already proves those
/// raw word uploads; other prefixes retain their existing host paths.
fn gpu_f_pad_f64_is_exact(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if logical_mode != Some("F") || ops.is_empty() || !matches!(image, DynamicImage::ImageRgba8(_))
    {
        return false;
    }
    let (op, prefix) = ops.split_last().expect("non-empty Pad proof ops");
    let PipelineOp::Pad { w, h, filter, .. } = op else {
        return false;
    };
    if prefix.iter().any(|op| {
        !matches!(
            op,
            PipelineOp::PutData {
                mode: PixelMode::F,
                ..
            }
        )
    }) {
        return false;
    }
    if matches!(filter, ResampleFilter::Nearest) {
        return false;
    }
    let source_dimensions = image.dimensions();
    let Some(((resize_w, resize_h), _)) =
        gpu_pad_geometry(op, source_dimensions.0, source_dimensions.1)
    else {
        return false;
    };
    if (resize_w, resize_h) == source_dimensions
        || CheckedDims::new(resize_w, resize_h, 1)
            .ok()
            .is_none_or(|dims| dims.total_pixels() > GPU_BUFFER_CAPACITY as usize)
        || CheckedDims::new(*w, *h, 1)
            .ok()
            .is_none_or(|dims| dims.total_pixels() > GPU_BUFFER_CAPACITY as usize)
    {
        return false;
    }
    // Reuse the complete marker-9 proof for the contain resize.  The Pad
    // placement shader is word-copy/fill only, so no second arithmetic proof
    // is necessary here.
    let mut resize_ops = prefix.to_vec();
    resize_ops.push(PipelineOp::Resize {
        w: resize_w,
        h: resize_h,
        filter: *filter,
    });
    gpu_f_resize_f64_is_exact(&resize_ops, image, logical_mode)
}

/// Return whether a filtered F resize can use the integer-only marker-6
/// shader. This is intentionally stricter than an empirical GPU comparison:
/// the fixed table must equal Pillow's f64 table, every source word must be a
/// finite normal f32, and every sequential signed row sum must fit a 53-bit
/// aligned integer. For two changed axes, the exact rounded horizontal words
/// are passed through the vertical proof so Pillow's intermediate f32 store
/// remains part of the admission contract.
fn gpu_f_resize_integer_is_exact(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if logical_mode != Some("F") || ops.len() != 1 || !matches!(image, DynamicImage::ImageRgba8(_))
    {
        return false;
    }
    let PipelineOp::Resize { w, h, filter } = &ops[0] else {
        return false;
    };
    if matches!(filter, ResampleFilter::Nearest) || *w == 0 || *h == 0 {
        return false;
    }
    let DynamicImage::ImageRgba8(pixels) = image else {
        return false;
    };
    let source_dimensions = image.dimensions();
    let expected = CheckedDims::new(source_dimensions.0, source_dimensions.1, 4)
        .ok()
        .map(|dims| dims.total_bytes());
    let bytes = pixels.as_raw();
    if expected != Some(bytes.len()) {
        return false;
    }
    for sample in bytes.chunks_exact(4) {
        let bits = u32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]);
        let Some(_) = gpu_f32_integer_parts(bits) else {
            return false;
        };
    }

    let horizontal_changed = *w != source_dimensions.0;
    let vertical_changed = *h != source_dimensions.1;
    if !horizontal_changed && !vertical_changed {
        return false;
    }
    if horizontal_changed
        && vertical_changed
        && *w > source_dimensions.0
        && *h < source_dimensions.1
    {
        return false;
    }

    let (kernel, support) = filter_from_resample(*filter);
    let horizontal = gpu_resize_coefficients(*w, source_dimensions.0, *filter);
    let vertical = gpu_resize_coefficients(*h, source_dimensions.1, *filter);
    let horizontal_f64 = precompute_coeffs_f64(*w, source_dimensions.0, kernel, support);
    let vertical_f64 = precompute_coeffs_f64(*h, source_dimensions.1, kernel, support);
    let rows_match = |fixed: &FilterCoeffs, exact: &FilterCoeffsF64| {
        if fixed.xmin != exact.xmin || fixed.count != exact.count {
            return false;
        }
        for (row, &count) in fixed.count.iter().enumerate() {
            let start = fixed.offsets[row];
            let Some(weights) = fixed.weights.get(start..start.saturating_add(count)) else {
                return false;
            };
            let Some(exact_weights) = exact.weights.get(row) else {
                return false;
            };
            if exact_weights.len() != count
                || weights
                    .iter()
                    .zip(exact_weights)
                    .any(|(&fixed, &exact)| exact != fixed as f64 / 4_194_304.0)
            {
                return false;
            }
        }
        true
    };
    if !rows_match(&horizontal, &horizontal_f64) || !rows_match(&vertical, &vertical_f64) {
        return false;
    }

    if horizontal_changed && vertical_changed {
        // The horizontal shader writes one f32 word per output pixel. Prove
        // that exact integer reduction and feed those rounded words to the
        // vertical proof; independently proving both axes against the raw
        // source would skip this observable Pillow boundary.
        let horizontal_words =
            gpu_f_resize_integer_pass_bits(bytes, source_dimensions, &horizontal, true);
        let Some(horizontal_words) = horizontal_words else {
            return false;
        };
        let mut horizontal_bytes = Vec::new();
        if horizontal_bytes
            .try_reserve(horizontal_words.len().saturating_mul(4))
            .is_err()
        {
            return false;
        }
        for word in horizontal_words {
            horizontal_bytes.extend_from_slice(&word.to_le_bytes());
        }
        return gpu_f_resize_integer_pass_bits(
            &horizontal_bytes,
            (*w, source_dimensions.1),
            &vertical,
            false,
        )
        .is_some();
    }

    if horizontal_changed {
        horizontal.count.iter().enumerate().all(|(row, _)| {
            gpu_f_resize_integer_row_is_exact(bytes, source_dimensions, &horizontal, row, true)
        })
    } else {
        vertical.count.iter().enumerate().all(|(row, _)| {
            gpu_f_resize_integer_row_is_exact(bytes, source_dimensions, &vertical, row, false)
        })
    }
}

/// Return whether a filtered F-mode resize is safe for marker-6's exact
/// device reduction.
///
/// The integer-emulation proof above admits finite normal f32 significands,
/// signed samples, and two changed axes when the fixed table is bit-for-bit
/// equal to Pillow's f64 table. This historical dyadic proof remains for
/// chained and other geometry whose intermediate f32 boundary is not covered
/// by that one-pass integer lane. It requires the fixed table to be bit-for-
/// bit equal to Pillow's f64 table, then proves that every f32 product and
/// sequential reduction is exact on the native adapter.
/// Bilinear rows have at most two taps, while Box downscales use at most 64
/// equal power-of-two taps on each changed axis.  The exponent-span bound
/// keeps every multi-tap Box partial sum representable in f32, making its
/// sequential shader reduction equal to the exact f64 sum followed by the
/// F-mode f32 store.  Multiple all-Box resizes are admitted with the same
/// bound accumulated across their passes; scanned `PutData` words can replace
/// an intermediate without widening the proven value domain.
//
// This mirrors Pillow's `Resample.c::precompute_coeffs` and
// `ImagingResample` f64-accumulate/f32-store boundary.  It intentionally does
// not claim arbitrary arithmetic-filter parity; those inputs remain under
// exact host semantic control.
fn gpu_f_resize_dyadic_is_exact(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if logical_mode != Some("F") || ops.is_empty() || !matches!(image, DynamicImage::ImageRgba8(_))
    {
        return false;
    }

    // Marker 6 also has an exact integer-emulation lane for arbitrary normal
    // f32 significands and signed reductions. Keep the historical dyadic
    // proof below for chained and otherwise unproven geometry.
    if gpu_f_resize_integer_is_exact(ops, image, logical_mode) {
        return true;
    }

    // The native Metal adapter used by the GPU backend flushes some low
    // normal-range intermediate products in relaxed f32 arithmetic.  The
    // 22-bit coefficient table can shift a source word by 22 bits, so keep a
    // 46-bit source margin above the IEEE normal floor (leaving 24 bits after
    // the shift) rather than treating the abstract f32 range as a device
    // guarantee.
    const MIN_NORMAL_EXPONENT: i32 = -80;
    const MAX_EXPONENT_SPAN: i32 = 16;
    const MAX_BOX_TAPS: u32 = MAX_GPU_REDUCE_FACTOR;

    // Keep only positive-zero or normal power-of-two words.  A source sign
    // is returned separately so cancellation and signed zero cannot enter
    // the proof.  Subnormals are excluded because native adapters may flush
    // them during shader arithmetic.
    let scan_words = |bytes: &[u8]| -> Option<(Option<bool>, Option<i32>, Option<i32>)> {
        if !bytes.chunks_exact(4).remainder().is_empty() {
            return None;
        }
        let mut sign = None;
        let mut min_exponent: Option<i32> = None;
        let mut max_exponent: Option<i32> = None;
        for sample in bytes.chunks_exact(4) {
            let bits = u32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]);
            if bits == (-0.0f32).to_bits() {
                return None;
            }
            let value = f32::from_bits(bits);
            if value == 0.0 {
                continue;
            }
            let exponent_bits = ((bits >> 23) & 0xff) as i32;
            let significand = bits & 0x7f_ff_ff;
            if exponent_bits == 0 || exponent_bits == 0xff || significand != 0 {
                return None;
            }
            let positive = bits & (1 << 31) == 0;
            if sign.is_some_and(|expected| expected != positive) {
                return None;
            }
            sign = Some(positive);
            let exponent = exponent_bits - 127;
            min_exponent = Some(min_exponent.map_or(exponent, |min| min.min(exponent)));
            max_exponent = Some(max_exponent.map_or(exponent, |max| max.max(exponent)));
        }
        Some((sign, min_exponent, max_exponent))
    };

    let DynamicImage::ImageRgba8(pixels) = image else {
        return false;
    };
    let expected = CheckedDims::new(image.width(), image.height(), 4)
        .ok()
        .map(|dims| dims.total_bytes());
    if expected != Some(pixels.as_raw().len()) {
        return false;
    }
    let mut summary = match scan_words(pixels.as_raw()) {
        Some(summary) => summary,
        None => return false,
    };

    // Combine source updates with the original words before checking the
    // shared exponent/sign proof.  PutData does not change dimensions.
    let merge_summary = |summary: &mut (Option<bool>, Option<i32>, Option<i32>),
                         next: (Option<bool>, Option<i32>, Option<i32>)|
     -> bool {
        if summary
            .0
            .is_some_and(|expected| next.0.is_some_and(|actual| expected != actual))
        {
            return false;
        }
        summary.0 = summary.0.or(next.0);
        summary.1 = match (summary.1, next.1) {
            (Some(left), Some(right)) => Some(left.min(right)),
            (left, right) => left.or(right),
        };
        summary.2 = match (summary.2, next.2) {
            (Some(left), Some(right)) => Some(left.max(right)),
            (left, right) => left.or(right),
        };
        true
    };

    let mut dimensions = image.dimensions();
    if dimensions.0 == 0 || dimensions.1 == 0 {
        return false;
    }
    let mut resize_count = 0usize;
    let mut changed_axis = false;
    let mut changed_axis_count = 0usize;
    let mut filter_kind = None;
    let mut resize_geometries = Vec::new();
    for op in ops {
        match op {
            PipelineOp::PutData {
                data,
                mode: PixelMode::F,
            } => {
                let expected = CheckedDims::new(dimensions.0, dimensions.1, 4)
                    .ok()
                    .map(|dims| dims.total_bytes());
                let Some(next) = expected
                    .and_then(|expected| (expected == data.len()).then(|| scan_words(data)))
                else {
                    return false;
                };
                let Some(next) = next else {
                    return false;
                };
                if !merge_summary(&mut summary, next) {
                    return false;
                }
            }
            PipelineOp::Resize { w, h, filter } => {
                resize_count = resize_count.saturating_add(1);
                if *w == 0 || *h == 0 {
                    return false;
                }
                if *w > dimensions.0 && *h < dimensions.1 {
                    return false;
                }
                changed_axis_count += usize::from(*w != dimensions.0);
                changed_axis_count += usize::from(*h != dimensions.1);
                changed_axis |= (*w, *h) != dimensions;
                filter_kind = Some(*filter);
                resize_geometries.push((dimensions, (*w, *h), *filter));
                dimensions = (*w, *h);
            }
            _ => return false,
        }
    }
    let Some(filter) = filter_kind else {
        return false;
    };
    if resize_count == 0 || !changed_axis || changed_axis_count == 0 {
        return false;
    }

    let Some(min_exponent) = summary.1 else {
        // An all-zero image is already covered by the constant-bit lowering;
        // keep this proof focused on arithmetic reductions with a known sign.
        return false;
    };
    let max_exponent = summary.2.expect("minimum exponent implies maximum");
    if min_exponent < MIN_NORMAL_EXPONENT
        || max_exponent.saturating_sub(min_exponent) > MAX_EXPONENT_SPAN
    {
        return false;
    }

    let rows_match = |fixed: &FilterCoeffs, exact: &FilterCoeffsF64| {
        if fixed.xmin != exact.xmin || fixed.count != exact.count {
            return false;
        }
        for (row, &count) in fixed.count.iter().enumerate() {
            let start = fixed.offsets[row];
            let Some(weights) = fixed.weights.get(start..start.saturating_add(count)) else {
                return false;
            };
            let Some(exact_weights) = exact.weights.get(row) else {
                return false;
            };
            if exact_weights.len() != count
                || weights
                    .iter()
                    .zip(exact_weights)
                    .any(|(&fixed, &exact)| exact != fixed as f64 / 4_194_304.0)
            {
                return false;
            }
        }
        true
    };
    if resize_count > 1 {
        // A chain has no single arithmetic-filter proof yet. Keep the
        // expanded admission limited to all-Box passes, for which each
        // intermediate is still an exact f32 sum and every following tap is
        // only a power-of-two scaling of that sum.
        let box_axis_shift = |coeffs: &FilterCoeffs| -> Option<i32> {
            let mut max_shift = 0i32;
            for (row, &count) in coeffs.count.iter().enumerate() {
                let count = u32::try_from(count).ok()?;
                if count == 0 || count > MAX_BOX_TAPS || !count.is_power_of_two() {
                    return None;
                }
                let shift = count.trailing_zeros() as i32;
                max_shift = max_shift.max(shift);
                let expected_weight = (1i64 << 22) / i64::from(count);
                let start = coeffs.offsets[row];
                let weights = coeffs
                    .weights
                    .get(start..start.saturating_add(count as usize))?;
                if weights.iter().any(|&weight| weight != expected_weight) {
                    return None;
                }
            }
            Some(max_shift)
        };
        let mut total_shift = 0i32;
        for &(source_dimensions, output_dimensions, filter) in &resize_geometries {
            if !matches!(filter, ResampleFilter::Box) {
                return false;
            }
            let (kernel, support) = filter_from_resample(filter);
            let horizontal =
                gpu_resize_coefficients(output_dimensions.0, source_dimensions.0, filter);
            let vertical =
                gpu_resize_coefficients(output_dimensions.1, source_dimensions.1, filter);
            let horizontal_f64 =
                precompute_coeffs_f64(output_dimensions.0, source_dimensions.0, kernel, support);
            let vertical_f64 =
                precompute_coeffs_f64(output_dimensions.1, source_dimensions.1, kernel, support);
            if !rows_match(&horizontal, &horizontal_f64) || !rows_match(&vertical, &vertical_f64) {
                return false;
            }
            let Some(horizontal_shift) = box_axis_shift(&horizontal) else {
                return false;
            };
            let Some(vertical_shift) = box_axis_shift(&vertical) else {
                return false;
            };
            total_shift = total_shift
                .checked_add(horizontal_shift)
                .and_then(|shift| shift.checked_add(vertical_shift))
                .unwrap_or(i32::MAX);
        }

        return max_exponent
            .saturating_sub(min_exponent)
            .saturating_add(total_shift)
            <= 23;
    }

    let (source_w, source_h) = image.dimensions();
    let (output_w, output_h) = dimensions;
    let (kernel, support) = filter_from_resample(filter);
    let horizontal = gpu_resize_coefficients(output_w, source_w, filter);
    let vertical = gpu_resize_coefficients(output_h, source_h, filter);
    let horizontal_f64 = precompute_coeffs_f64(output_w, source_w, kernel, support);
    let vertical_f64 = precompute_coeffs_f64(output_h, source_h, kernel, support);
    if !rows_match(&horizontal, &horizontal_f64) || !rows_match(&vertical, &vertical_f64) {
        return false;
    }

    match filter {
        ResampleFilter::Bilinear
        | ResampleFilter::Bicubic
        | ResampleFilter::Lanczos
        | ResampleFilter::Hamming => {
            // With power-of-two source words, every dyadic fixed coefficient
            // product is an exactly representable f32 value. Restrict the
            // arithmetic filters to non-negative rows with at most two taps:
            // one f32 addition is then correctly rounded to the same value as
            // Pillow's f64 sum followed by its f32 store. The non-negative
            // condition also excludes ringing-filter cancellation, which can
            // create a signed zero or a low normal value that native adapters
            // do not preserve uniformly.
            let two_tap_nonnegative = |coeffs: &FilterCoeffs| {
                coeffs.count.iter().enumerate().all(|(row, &count)| {
                    if !(1..=2).contains(&count) {
                        return false;
                    }
                    let start = coeffs.offsets[row];
                    coeffs
                        .weights
                        .get(start..start.saturating_add(count))
                        .is_some_and(|weights| {
                            weights
                                .iter()
                                .all(|&weight| (0..=4_194_304).contains(&weight))
                        })
                })
            };
            two_tap_nonnegative(&horizontal) && two_tap_nonnegative(&vertical)
        }
        ResampleFilter::Box => {
            // Box rows are safe for the dyadic shader beyond integral
            // reductions when every row still has an exact power-of-two
            // number of equal taps. Pillow's normalized Box table then has
            // weights `1 / count`; checking the emitted fixed rows (rather
            // than inferring a ratio from dimensions) keeps non-divisor
            // geometry honest. This covers, for example, 5 -> 3 where all
            // rows are two 1/2 taps, while 7 -> 3 remains host-controlled
            // if any row needs a non-dyadic three-tap normalization.
            let box_axis_shift = |coeffs: &FilterCoeffs| -> Option<i32> {
                let mut max_shift = 0i32;
                for (row, &count) in coeffs.count.iter().enumerate() {
                    let count = u32::try_from(count).ok()?;
                    if count == 0 || count > MAX_BOX_TAPS || !count.is_power_of_two() {
                        return None;
                    }
                    let shift = count.trailing_zeros() as i32;
                    max_shift = max_shift.max(shift);
                    let expected_weight = (1i64 << 22) / i64::from(count);
                    let start = coeffs.offsets[row];
                    let weights = coeffs
                        .weights
                        .get(start..start.saturating_add(count as usize))?;
                    if weights.iter().any(|&weight| weight != expected_weight) {
                        return None;
                    }
                }
                Some(max_shift)
            };
            let Some(horizontal_shift) = box_axis_shift(&horizontal) else {
                return false;
            };
            let Some(vertical_shift) = box_axis_shift(&vertical) else {
                return false;
            };
            // Horizontal and vertical averages may each expose one extra
            // significand bit range below the source minimum. Keep the
            // complete two-pass dyadic sum within f32's 24-bit significand;
            // applying the same conservative bound to one-axis rows also
            // covers non-divisor Box geometry without relying on a ratio
            // shortcut.
            if max_exponent.saturating_sub(min_exponent) + horizontal_shift + vertical_shift > 23 {
                return false;
            }
            // A resize must still change at least one axis. The identity path
            // has a separate proof that preserves every bit pattern.
            changed_axis_count > 0
        }
        _ => false,
    }
}

#[derive(Clone, Copy)]
struct BufferRange {
    offset: u64,
    size: u64,
}

/// Immutable auxiliary resources shared by operations in one execution.
///
/// GPU submissions are split at a bounded operation/resource budget, but the
/// source graph may reuse the same secondary image, mask, or LUT on both sides
/// of that boundary. Keep only repeated resources in this execution-wide
/// cache; unique resources remain chunk-local. The aggregate cache is capped
/// so a long graph cannot turn deduplication into unbounded retention.
#[derive(Default)]
struct GpuAuxiliaryCache {
    second_ranges: HashMap<usize, BufferRange>,
    third_ranges: HashMap<usize, BufferRange>,
    lut_ranges: HashMap<[u32; 256], BufferRange>,
    img2_values: Vec<u32>,
    img3_values: Vec<u32>,
    lut_values: Vec<u32>,
}

impl GpuAuxiliaryCache {
    fn from_batch(
        ops: &[PipelineOp],
        auxiliary_images: &[AuxiliaryImages],
        mode: u32,
        capacity: u32,
        storage_alignment: usize,
    ) -> Result<Self, PilError> {
        let mut second_counts = HashMap::<usize, usize>::new();
        let mut third_counts = HashMap::<usize, usize>::new();
        let mut lut_counts = HashMap::<[u32; 256], usize>::new();
        let mut op_modes = Vec::with_capacity(ops.len());
        let mut current_mode = mode;
        for op in ops {
            op_modes.push(current_mode);
            current_mode = gpu_mode_after_op(current_mode, op);
        }
        for ((op, auxiliary), op_mode) in ops.iter().zip(auxiliary_images).zip(&op_modes) {
            if !matches!(op, PipelineOp::PutData { .. }) {
                if let Some(second) = auxiliary.second.as_ref() {
                    *second_counts
                        .entry(Arc::as_ptr(second) as usize)
                        .or_default() += 1;
                }
            }
            if let Some(third) = auxiliary.third.as_ref() {
                *third_counts.entry(Arc::as_ptr(third) as usize).or_default() += 1;
            }
            if let Some(lut) = extract_lut(op, *op_mode) {
                *lut_counts.entry(lut).or_default() += 1;
            }
        }

        let mut cache = Self::default();
        for ((op, auxiliary), op_mode) in ops.iter().zip(auxiliary_images).zip(&op_modes) {
            if !matches!(op, PipelineOp::PutData { .. }) {
                if let Some(second) = auxiliary.second.as_ref() {
                    let key = Arc::as_ptr(second) as usize;
                    if second_counts.get(&key).copied().unwrap_or_default() > 1
                        && !cache.second_ranges.contains_key(&key)
                    {
                        // Typed I;16 Paste sources use a numeric u16 word
                        // rather than the ordinary RGBA byte transport. Do
                        // not place that representation in the shared cache:
                        // the cache key is the image identity and a later
                        // byte-oriented operation could otherwise reuse the
                        // same image with the wrong packing.
                        if !gpu_luma16_paste_source(op, *op_mode, second) {
                            let values = pack_rgba(&second.to_rgba8(), capacity)?;
                            if cache.total_bytes().saturating_add(values.len() * 4)
                                <= MAX_GPU_AUXILIARY_CACHE_BYTES
                            {
                                let range = append_arena_slice(
                                    &mut cache.img2_values,
                                    &values,
                                    storage_alignment,
                                );
                                cache.second_ranges.insert(key, range);
                            }
                        }
                    }
                }
            }
            if let Some(third) = auxiliary.third.as_ref() {
                let key = Arc::as_ptr(third) as usize;
                if third_counts.get(&key).copied().unwrap_or_default() > 1
                    && !cache.third_ranges.contains_key(&key)
                {
                    let values = pack_rgba(&third.to_rgba8(), capacity)?;
                    if cache.total_bytes().saturating_add(values.len() * 4)
                        <= MAX_GPU_AUXILIARY_CACHE_BYTES
                    {
                        let range =
                            append_arena_slice(&mut cache.img3_values, &values, storage_alignment);
                        cache.third_ranges.insert(key, range);
                    }
                }
            }
            if let Some(lut) = extract_lut(op, *op_mode) {
                if lut_counts.get(&lut).copied().unwrap_or_default() > 1
                    && !cache.lut_ranges.contains_key(&lut)
                {
                    if cache.total_bytes().saturating_add(lut.len() * 4)
                        <= MAX_GPU_AUXILIARY_CACHE_BYTES
                    {
                        let range =
                            append_arena_slice(&mut cache.lut_values, &lut, storage_alignment);
                        cache.lut_ranges.insert(lut, range);
                    }
                }
            }
        }
        Ok(cache)
    }

    fn total_bytes(&self) -> usize {
        (self.img2_values.len() + self.img3_values.len() + self.lut_values.len()) * 4
    }
}

/// A batch-owned arena allocation that grows to the largest plan seen by the
/// working-buffer pool and is reused by later plans. The buffer is never
/// shrunk: keeping the capacity with the already-pooled image buffers avoids
/// recreating parameter and auxiliary resources for every materialization.
struct ReusableGpuBuffer {
    buffer: wgpu::Buffer,
    capacity_bytes: u64,
}

impl ReusableGpuBuffer {
    fn new(
        device: &wgpu::Device,
        label: &'static str,
        usage: wgpu::BufferUsages,
        initial_bytes: usize,
        alignment_bytes: usize,
    ) -> Self {
        let capacity_bytes = aligned_bytes(initial_bytes, alignment_bytes) as u64;
        Self {
            buffer: create_sized_buffer(
                device,
                label,
                usage,
                capacity_bytes as usize,
                alignment_bytes,
            ),
            capacity_bytes,
        }
    }

    fn ensure_capacity(
        &mut self,
        device: &wgpu::Device,
        label: &'static str,
        usage: wgpu::BufferUsages,
        required_bytes: usize,
        alignment_bytes: usize,
    ) {
        let required_bytes = aligned_bytes(required_bytes, alignment_bytes) as u64;
        if required_bytes <= self.capacity_bytes {
            return;
        }
        self.buffer = create_sized_buffer(
            device,
            label,
            usage,
            required_bytes as usize,
            alignment_bytes,
        );
        self.capacity_bytes = required_bytes;
    }
}

// ─── BufferPool ────────────────────────────────────────────────────────────

/// GPU storage owned by one in-flight pipeline batch and returned to the
/// bounded working-set pool after readback completes.
///
/// The device, queue, and compiled pipelines are shared, but mutable image
/// storage must not be shared across batches. Keeping these buffers local to
/// an execution removes the need for a process-wide execution mutex. Arena
/// writes remain ordered with their corresponding queue submissions.
struct BufferPool {
    buf_a: wgpu::Buffer,
    buf_b: wgpu::Buffer,
    buf_img2: wgpu::Buffer,      // Second image for dual-input ops
    buf_img3: wgpu::Buffer,      // Third image for 3-input ops (Composite/Paste mask)
    lut_buf: wgpu::Buffer,       // LUT storage buffer for Eval/PointOp (1024 bytes)
    histogram_buf: wgpu::Buffer, // Histogram control storage (1024 u32 words)
    params_arena: ReusableGpuBuffer,
    img2_arena: ReusableGpuBuffer,
    img3_arena: ReusableGpuBuffer,
    lut_arena: ReusableGpuBuffer,
    capacity: u32,
}

/// One reusable map-readable destination for the final device-to-host copy.
///
/// A staging buffer is returned to the pool only after `map_async` completes
/// and the buffer has been unmapped, so no queued command or host mapping can
/// still own it when a later materialization acquires it.
struct StagingBuffer {
    buffer: wgpu::Buffer,
    capacity_bytes: u64,
}

impl StagingBuffer {
    fn new(device: &wgpu::Device, capacity_bytes: u64) -> Self {
        Self {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gpu_readback_staging"),
                size: capacity_bytes.max(4),
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            capacity_bytes: capacity_bytes.max(4),
        }
    }
}

impl BufferPool {
    fn new(device: &wgpu::Device, capacity: u32) -> Self {
        let size = (capacity.max(1) as u64) * 4;
        let buf_a = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_buf_a"),
            size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let buf_b = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_buf_b"),
            size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let buf_img2 = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_buf_img2"),
            // Missing optional auxiliary inputs are never read by the
            // shader. A single storage element is sufficient as the
            // fallback binding and avoids allocating another full image.
            size: 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let buf_img3 = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_buf_img3"),
            size: 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let lut_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_lut"),
            size: 1024, // 256 entries * 4 bytes each
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let histogram_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_histogram"),
            size: GPU_HISTOGRAM_BYTES as u64,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let params_arena = ReusableGpuBuffer::new(
            device,
            "gpu_batch_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            4,
            4,
        );
        let img2_arena = ReusableGpuBuffer::new(
            device,
            "gpu_batch_img2",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            4,
            4,
        );
        let img3_arena = ReusableGpuBuffer::new(
            device,
            "gpu_batch_img3",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            4,
            4,
        );
        let lut_arena = ReusableGpuBuffer::new(
            device,
            "gpu_batch_lut",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            1024,
            4,
        );
        BufferPool {
            buf_a,
            buf_b,
            buf_img2,
            buf_img3,
            lut_buf,
            histogram_buf,
            params_arena,
            img2_arena,
            img3_arena,
            lut_arena,
            capacity,
        }
    }

    fn upload_rgba(&self, queue: &wgpu::Queue, rgba: &RgbaImage) -> Result<(), PilError> {
        let (w, h) = rgba.dimensions();
        let pixel_count = CheckedDims::new(w, h, 1)?.total_pixels();
        if pixel_count > self.capacity as usize {
            return Err(PilError::ValueError(format!(
                "GPU buffer capacity {} < image size {}",
                self.capacity, pixel_count
            )));
        }

        // WGSL packs R, G, B, and A into the low-to-high bytes of one u32.
        // Little-endian RGBA storage already has exactly that byte layout, so
        // upload it directly instead of allocating and filling a second host
        // vector. Big-endian builds retain the explicit portable packer.
        #[cfg(target_endian = "little")]
        queue.write_buffer(&self.buf_a, 0, rgba.as_raw());
        #[cfg(target_endian = "big")]
        {
            let packed = pack_rgba(rgba, self.capacity)?;
            queue.write_buffer(&self.buf_a, 0, bytemuck::cast_slice(&packed));
        }

        // The first ordinary dispatch writes buf_b completely. Uploading the
        // source to both ping-pong buffers doubled host-to-device traffic and
        // did not provide input to any dispatch.
        Ok(())
    }

    #[cfg(target_endian = "little")]
    fn upload_rgb(&self, queue: &wgpu::Queue, rgb: &RgbImage) -> Result<(), PilError> {
        let (w, h) = rgb.dimensions();
        let pixel_count = CheckedDims::new(w, h, 1)?.total_pixels();
        if pixel_count > self.capacity as usize {
            return Err(PilError::ValueError(format!(
                "GPU buffer capacity {} < image size {}",
                self.capacity, pixel_count
            )));
        }
        let byte_count = CheckedDims::new(w, h, 4)?.total_bytes();
        let byte_count = u64::try_from(byte_count)
            .map_err(|_| PilError::InternalError("GPU RGB upload size overflow".into()))?;
        let Some(size) = NonZeroU64::new(byte_count) else {
            return Ok(());
        };
        let mut upload = queue
            .write_buffer_with(&self.buf_a, 0, size)
            .ok_or_else(|| PilError::InternalError("GPU RGB staging allocation failed".into()))?;
        expand_rgb_into_rgba(rgb.as_raw(), &mut upload)?;
        drop(upload);
        Ok(())
    }

    fn upload_standard_image(
        &self,
        queue: &wgpu::Queue,
        image: &DynamicImage,
    ) -> Result<(), PilError> {
        #[cfg(target_endian = "little")]
        if let DynamicImage::ImageRgb8(rgb) = image {
            return self.upload_rgb(queue, rgb);
        }

        let rgba = image.to_rgba8();
        self.upload_rgba(queue, &rgba)
    }

    /// Upload an `I;16*` image as one zero-extended sample per packed storage
    /// word. The admitted GPU operations for this representation only move
    /// complete words, so no shader is allowed to interpret the sample as
    /// four independent RGBA bytes. Keeping the high bytes zero also makes
    /// the readback contract explicit instead of narrowing through `to_rgba8`.
    fn upload_luma16(
        &self,
        queue: &wgpu::Queue,
        image: &ImageBuffer<Luma<u16>, Vec<u16>>,
        mode: Option<&str>,
    ) -> Result<(), PilError> {
        let (w, h) = image.dimensions();
        let pixel_count = CheckedDims::new(w, h, 1)?.total_pixels();
        if pixel_count > self.capacity as usize {
            return Err(PilError::ValueError(format!(
                "GPU buffer capacity {} < image size {}",
                self.capacity, pixel_count
            )));
        }
        let mut packed = Vec::with_capacity(pixel_count * std::mem::size_of::<u32>());
        for &sample in image.as_raw() {
            let sample_bytes = if matches!(mode, Some("I;16B" | "I;16N")) {
                sample.to_be_bytes()
            } else {
                sample.to_ne_bytes()
            };
            packed.extend_from_slice(&[sample_bytes[0], sample_bytes[1], 0, 0]);
        }
        queue.write_buffer(&self.buf_a, 0, &packed);
        Ok(())
    }

    /// Upload `I;16*` samples as numeric unsigned values for a conversion.
    /// Geometry keeps the public byte order opaque, but Convert needs the
    /// decoded sample value to apply Pillow's clamp-to-255 rule. WGSL storage
    /// words are little-endian, so write the host-native `u16` value into the
    /// low two bytes independently of the source mode's declared byte order.
    fn upload_luma16_numeric(
        &self,
        queue: &wgpu::Queue,
        image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    ) -> Result<(), PilError> {
        let (w, h) = image.dimensions();
        let pixel_count = CheckedDims::new(w, h, 1)?.total_pixels();
        if pixel_count > self.capacity as usize {
            return Err(PilError::ValueError(format!(
                "GPU buffer capacity {} < image size {}",
                self.capacity, pixel_count
            )));
        }
        let packed = pack_luma16_numeric(image, self.capacity)?;
        queue.write_buffer(&self.buf_a, 0, bytemuck::cast_slice(&packed));
        Ok(())
    }

    fn retained_bytes(&self) -> u64 {
        gpu_working_set_bytes(self.capacity)
            .saturating_add(self.params_arena.capacity_bytes)
            .saturating_add(self.img2_arena.capacity_bytes)
            .saturating_add(self.img3_arena.capacity_bytes)
            .saturating_add(self.lut_arena.capacity_bytes)
    }
}

fn gpu_working_set_bytes(capacity: u32) -> u64 {
    // The two full-size ping-pong buffers dominate this working set. The
    // optional-input fallbacks contain one word each and the LUT contains 256
    // words.
    u64::from(capacity)
        .saturating_mul(2)
        .saturating_mul(std::mem::size_of::<u32>() as u64)
        .saturating_add(2 * std::mem::size_of::<u32>() as u64)
        .saturating_add(256 * std::mem::size_of::<u32>() as u64)
        .saturating_add(GPU_HISTOGRAM_BYTES as u64)
}

fn gpu_buffer_reuse_allowed(capacity: u32, minimum_capacity: u32) -> bool {
    capacity >= minimum_capacity
        && (minimum_capacity == 0
            || capacity <= minimum_capacity.saturating_mul(MAX_GPU_BUFFER_REUSE_RATIO))
}

#[cfg(target_endian = "little")]
fn expand_rgb_into_rgba(rgb: &[u8], rgba: &mut [u8]) -> Result<(), PilError> {
    if !rgb.len().is_multiple_of(3) {
        return Err(PilError::InternalError(
            "GPU RGB source has a partial pixel".into(),
        ));
    }
    let expected = rgb
        .len()
        .checked_div(3)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| PilError::InternalError("GPU RGB upload size overflow".into()))?;
    if rgba.len() != expected {
        return Err(PilError::InternalError(format!(
            "GPU RGB staging length {} does not match expected {expected}",
            rgba.len()
        )));
    }

    for (source, target) in rgb.chunks_exact(3).zip(rgba.chunks_exact_mut(4)) {
        target.copy_from_slice(&[source[0], source[1], source[2], u8::MAX]);
    }
    Ok(())
}

/// Pack an RGBA image into the storage representation used by the shaders.
fn pack_rgba(rgba: &RgbaImage, capacity: u32) -> Result<Vec<u32>, PilError> {
    let (w, h) = rgba.dimensions();
    let n = CheckedDims::new(w, h, 1)?.total_pixels();
    if n > capacity as usize {
        return Err(PilError::ValueError(format!(
            "GPU buffer capacity {} < image size {}",
            capacity, n
        )));
    }
    Ok(rgba
        .pixels()
        .map(|px| {
            (px[0] as u32) | ((px[1] as u32) << 8) | ((px[2] as u32) << 16) | ((px[3] as u32) << 24)
        })
        .collect())
}

/// Pack typed `I;16*` samples as numeric little-endian u16 values in the low
/// half of one storage word per pixel. This is distinct from the opaque byte
/// packing used by I;16 geometry: Paste blends the decoded unsigned samples,
/// so narrowing through RGBA8 would discard the high byte.
fn pack_luma16_numeric(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    capacity: u32,
) -> Result<Vec<u32>, PilError> {
    let (w, h) = image.dimensions();
    let pixel_count = CheckedDims::new(w, h, 1)?.total_pixels();
    if pixel_count > capacity as usize {
        return Err(PilError::ValueError(format!(
            "GPU buffer capacity {} < image size {}",
            capacity, pixel_count
        )));
    }
    Ok(image
        .as_raw()
        .iter()
        .map(|&sample| u32::from(sample))
        .collect())
}

/// Pack logical `putdata` samples into the auxiliary storage representation.
///
/// LA/PA place alpha in packed byte 3, matching the GPU's RGBA transport;
/// all other modes retain their raw channel order. The shader uses the
/// original byte length to preserve every untouched or partial pixel.
fn pack_put_data(data: &[u8], mode: PixelMode, capacity: u32) -> Result<Vec<u32>, PilError> {
    let channels = mode.channels();
    let pixel_count = data.len().div_ceil(channels);
    if pixel_count > capacity as usize {
        return Err(PilError::ValueError(format!(
            "GPU buffer capacity {} < putdata image size {}",
            capacity, pixel_count
        )));
    }
    let mut packed = Vec::with_capacity(pixel_count);
    for pixel_index in 0..pixel_count {
        let start = pixel_index * channels;
        let samples = &data[start..data.len().min(start + channels)];
        let pixel = match mode {
            PixelMode::L | PixelMode::P | PixelMode::Mode1 => {
                samples.first().copied().unwrap_or(0) as u32
            }
            PixelMode::LA | PixelMode::PA => {
                let luma = samples.first().copied().unwrap_or(0) as u32;
                let alpha = samples.get(1).copied().unwrap_or(0) as u32;
                luma | (alpha << 24)
            }
            PixelMode::RGB | PixelMode::YCbCr | PixelMode::HSV => {
                let r = samples.first().copied().unwrap_or(0) as u32;
                let g = samples.get(1).copied().unwrap_or(0) as u32;
                let b = samples.get(2).copied().unwrap_or(0) as u32;
                r | (g << 8) | (b << 16)
            }
            PixelMode::RGBA | PixelMode::CMYK | PixelMode::I | PixelMode::F => {
                let first = samples.first().copied().unwrap_or(0) as u32;
                let second = samples.get(1).copied().unwrap_or(0) as u32;
                let third = samples.get(2).copied().unwrap_or(0) as u32;
                let fourth = samples.get(3).copied().unwrap_or(0) as u32;
                first | (second << 8) | (third << 16) | (fourth << 24)
            }
        };
        packed.push(pixel);
    }
    Ok(packed)
}

/// Prepare Color3DLut table entries in the same signed 12.4 representation
/// used by the CPU interpolation path. The returned i16 values are serialized
/// as one little-endian u32 word per table component for a read-only shader
/// buffer; keeping this conversion shared prevents f32/table rounding drift.
fn color3dlut_table_words(table: &[f64]) -> Result<Vec<u32>, PilError> {
    const PRECISION_BITS: i32 = 4;
    let mut words = Vec::with_capacity(table.len());
    for &value in table {
        if !value.is_finite() {
            return Err(PilError::ValueError(
                "GPU Color3DLut table contains a non-finite value".into(),
            ));
        }
        let item = value as f32;
        let scaled = item * ((255 << PRECISION_BITS) as f32);
        let prepared = if scaled >= i16::MAX as f32 - 0.5 {
            i16::MAX
        } else if scaled <= i16::MIN as f32 + 0.5 {
            i16::MIN
        } else if item < 0.0 {
            (scaled - 0.5) as i16
        } else {
            (scaled + 0.5) as i16
        };
        words.push(u32::from(u16::from_ne_bytes(prepared.to_ne_bytes())));
    }
    Ok(words)
}

fn align_up(value: usize, alignment: usize) -> usize {
    if alignment <= 1 {
        value
    } else {
        let remainder = value % alignment;
        if remainder == 0 {
            value
        } else {
            value + alignment - remainder
        }
    }
}

fn aligned_bytes(bytes: usize, alignment: usize) -> usize {
    align_up(bytes.max(1), alignment.max(4))
}

fn select_gpu_chunk_end(
    chunk_start: usize,
    resource_bytes: &[usize],
    shader_work_items: &[u64],
) -> Option<usize> {
    if resource_bytes.len() != shader_work_items.len() {
        return None;
    }
    let mut chunk_end = chunk_start;
    let mut total_bytes = 0usize;
    let mut total_work = 0u64;
    while chunk_end < resource_bytes.len() && chunk_end - chunk_start < MAX_GPU_OPS_PER_SUBMISSION {
        let op_bytes = resource_bytes[chunk_end];
        let op_work = shader_work_items[chunk_end];
        if chunk_end > chunk_start
            && (total_bytes.saturating_add(op_bytes) > MAX_GPU_RESOURCE_BYTES_PER_SUBMISSION
                || total_work.saturating_add(op_work) > MAX_GPU_SHADER_WORK_ITEMS)
        {
            break;
        }
        total_bytes = total_bytes.saturating_add(op_bytes);
        total_work = total_work.saturating_add(op_work);
        chunk_end += 1;
    }
    (chunk_end > chunk_start).then_some(chunk_end)
}

/// Append one aligned slice to a u32 arena and return its byte range.
fn append_arena_slice(arena: &mut Vec<u32>, values: &[u32], alignment_bytes: usize) -> BufferRange {
    let alignment_words = alignment_bytes.max(4).div_ceil(4);
    let offset_words = align_up(arena.len(), alignment_words);
    let size_words = align_up(values.len().max(1), alignment_words);
    arena.resize(offset_words + size_words, 0);
    arena[offset_words..offset_words + values.len()].copy_from_slice(values);
    BufferRange {
        offset: (offset_words * std::mem::size_of::<u32>()) as u64,
        size: (size_words * std::mem::size_of::<u32>()) as u64,
    }
}

/// Return whether a nearest resize uses the packed byte convolution plan.
///
/// The convolution plan receives host-generated one-tap tables so its source
/// index follows Pillow's cumulative f64 stepping exactly. I- and F-mode need
/// that plan too: their four-byte samples are copied opaquely by the typed
/// branch in the resize shaders, rather than being interpreted as four byte
/// channels. I;16 retains its established relocation shader until its own
/// nearest-coordinate receipts are expanded with the same proof.
fn gpu_resize_nearest_uses_coefficients(logical_mode: Option<&str>) -> bool {
    !matches!(
        logical_mode,
        Some("1" | "I;16" | "I;16L" | "I;16B" | "I;16N")
    )
}

/// Build a resize coefficient table for the exact GPU byte path.
///
/// Non-nearest filters reuse the shared Pillow fixed-point table. Nearest is
/// different: Pillow's affine implementation advances one f64 accumulator
/// per output sample, so computing `(x + 0.5) * scale` independently in WGSL
/// can select a neighboring source pixel at a representable boundary. A
/// one-tap table preserves that scalar control-plane decision while leaving
/// every pixel read/write on the device.
fn gpu_resize_coefficients(
    out_size: u32,
    in_size: u32,
    filter: ResampleFilter,
) -> Arc<FilterCoeffs> {
    if !matches!(filter, ResampleFilter::Nearest) {
        return precompute_coeffs(out_size, in_size, filter);
    }
    if out_size == 0 || in_size == 0 {
        return Arc::new(FilterCoeffs {
            xmin: Vec::new(),
            count: Vec::new(),
            offsets: Vec::new(),
            weights: Vec::new(),
        });
    }

    let scale = f64::from(in_size) / f64::from(out_size);
    let mut source_position = scale * 0.5;
    let mut xmin = Vec::with_capacity(out_size as usize);
    let mut count = Vec::with_capacity(out_size as usize);
    let mut offsets = Vec::with_capacity(out_size as usize);
    let mut weights = Vec::with_capacity(out_size as usize);
    for _ in 0..out_size {
        let source = if source_position >= f64::from(in_size) {
            in_size - 1
        } else {
            source_position as u32
        };
        xmin.push(i64::from(source));
        count.push(1);
        offsets.push(weights.len());
        weights.push(1i64 << 22);
        source_position += scale;
    }
    Arc::new(FilterCoeffs {
        xmin,
        count,
        offsets,
        weights,
    })
}

/// Validate and size the fixed-point coefficient table consumed by the
/// separable resize shaders.
///
/// `FilterCoeffs` is deliberately shared with the CPU/SIMD implementations so
/// the GPU receives the same Pillow-rounded coefficients.  The shader uses an
/// i32 accumulator; prove the complete byte-domain accumulation before a
/// resize is admitted rather than allowing device arithmetic to wrap.
fn resize_coeff_word_count(coeffs: &FilterCoeffs) -> Result<usize, PilError> {
    if coeffs.xmin.len() != coeffs.count.len()
        || coeffs.xmin.len() != coeffs.offsets.len()
        || coeffs
            .offsets
            .iter()
            .zip(&coeffs.count)
            .any(|(&offset, &count)| {
                offset
                    .checked_add(count)
                    .is_none_or(|end| end > coeffs.weights.len())
            })
    {
        return Err(PilError::InternalError(
            "GPU resize coefficient table shape is invalid".into(),
        ));
    }

    for ((&xmin, &count), &offset) in coeffs.xmin.iter().zip(&coeffs.count).zip(&coeffs.offsets) {
        if xmin < 0
            || u32::try_from(xmin).is_err()
            || u32::try_from(count).is_err()
            || u32::try_from(offset).is_err()
        {
            return Err(PilError::ValueError(
                "GPU resize coefficient metadata exceeds shader limits".into(),
            ));
        }
    }

    for &weight in &coeffs.weights {
        let weight = i32::try_from(weight).map_err(|_| {
            PilError::ValueError("GPU resize coefficient exceeds signed shader limits".into())
        })?;
        let abs_weight = i64::from(weight).checked_abs().ok_or_else(|| {
            PilError::ValueError("GPU resize coefficient magnitude overflow".into())
        })?;
        // Every intermediate partial sum is bounded by the absolute sum of
        // all taps.  The largest byte sample is 255 and the shader adds the
        // fixed-point half-unit before shifting.
        let _ = abs_weight;
    }
    for (&offset, &count) in coeffs.offsets.iter().zip(&coeffs.count) {
        let mut abs_sum = 0i64;
        for &weight in coeffs.weights.get(offset..offset + count).ok_or_else(|| {
            PilError::InternalError("GPU resize coefficient slice is invalid".into())
        })? {
            let abs_weight = weight.checked_abs().ok_or_else(|| {
                PilError::ValueError("GPU resize coefficient magnitude overflow".into())
            })?;
            abs_sum = abs_sum.checked_add(abs_weight).ok_or_else(|| {
                PilError::ValueError("GPU resize coefficient sum overflow".into())
            })?;
        }
        let max_accumulator = abs_sum
            .checked_mul(255)
            .and_then(|value| value.checked_add(1i64 << 21))
            .ok_or_else(|| PilError::ValueError("GPU resize accumulator overflow".into()))?;
        if max_accumulator > i64::from(i32::MAX) {
            return Err(PilError::ValueError(
                "GPU resize accumulator exceeds signed shader limits".into(),
            ));
        }
    }

    coeffs
        .xmin
        .len()
        .checked_mul(3)
        .and_then(|metadata| metadata.checked_add(coeffs.weights.len()))
        .ok_or_else(|| PilError::ValueError("GPU resize coefficient arena size overflow".into()))
}

/// Encode one Pillow-compatible coefficient table as `[xmin,count,offset]`
/// metadata followed by signed fixed-point weights.  The returned words are
/// uploaded as an auxiliary storage range; no image pixels are materialized on
/// the host for this control table.
fn encode_resize_coeffs(coeffs: &FilterCoeffs) -> Result<Vec<u32>, PilError> {
    let word_count = resize_coeff_word_count(coeffs)?;
    let mut words = Vec::with_capacity(word_count);
    for ((&xmin, &count), &offset) in coeffs.xmin.iter().zip(&coeffs.count).zip(&coeffs.offsets) {
        words.push(u32::try_from(xmin).map_err(|_| {
            PilError::ValueError("GPU resize coefficient xmin exceeds shader limits".into())
        })?);
        words.push(u32::try_from(count).map_err(|_| {
            PilError::ValueError("GPU resize coefficient count exceeds shader limits".into())
        })?);
        words.push(u32::try_from(offset).map_err(|_| {
            PilError::ValueError("GPU resize coefficient offset exceeds shader limits".into())
        })?);
    }
    words.extend(coeffs.weights.iter().map(|&weight| {
        // resize_coeff_word_count already checked this conversion.
        i32::try_from(weight).expect("validated GPU resize coefficient") as u32
    }));
    debug_assert_eq!(words.len(), word_count);
    Ok(words)
}

/// Validate and size an f64 coefficient table transported to marker-9's
/// integer reducer.  Each dyadic coefficient occupies four words: a 53-bit
/// mantissa, its binary exponent, and its sign.  Metadata offsets therefore
/// count groups of four rather than individual weights.
fn resize_coeff_word_count_f64(coeffs: &FilterCoeffsF64) -> Result<usize, PilError> {
    if coeffs.xmin.len() != coeffs.count.len()
        || coeffs.xmin.len() != coeffs.weights.len()
        || coeffs
            .count
            .iter()
            .zip(&coeffs.weights)
            .any(|(&count, weights)| count != weights.len())
    {
        return Err(PilError::InternalError(
            "GPU f64 resize coefficient table shape is invalid".into(),
        ));
    }
    for &xmin in &coeffs.xmin {
        if xmin < 0 || u32::try_from(xmin).is_err() {
            return Err(PilError::ValueError(
                "GPU resize coefficient xmin exceeds shader limits".into(),
            ));
        }
    }
    let mut weight_words = 0usize;
    for row in &coeffs.weights {
        for &weight in row {
            if gpu_f64_integer_parts(weight).is_none() {
                return Err(PilError::ValueError(
                    "GPU resize f64 coefficient is not a finite normal value".into(),
                ));
            }
        }
        weight_words = weight_words
            .checked_add(row.len().checked_mul(4).ok_or_else(|| {
                PilError::ValueError("GPU resize coefficient arena size overflow".into())
            })?)
            .ok_or_else(|| {
                PilError::ValueError("GPU resize coefficient arena size overflow".into())
            })?;
    }
    coeffs
        .xmin
        .len()
        .checked_mul(3)
        .and_then(|metadata| metadata.checked_add(weight_words))
        .ok_or_else(|| PilError::ValueError("GPU resize coefficient arena size overflow".into()))
}

/// Return whether one f64 coefficient table can be bound as one storage range.
///
/// The host proof may certify every row's arithmetic while the encoded table
/// still exceeds the device binding limit when multiple output rows are
/// present.  Keep this check next to the encoder's word-count contract so
/// marker 9's special-value prepass and marker 12's ordered reducer make the
/// same adapter-safe admission decision before any bind group is created.
fn gpu_f_resize_f64_coefficients_fit_binding(coeffs: &FilterCoeffsF64) -> bool {
    let Ok(word_count) = resize_coeff_word_count_f64(coeffs) else {
        return false;
    };
    let Some(bytes) = word_count.checked_mul(std::mem::size_of::<u32>()) else {
        return false;
    };
    aligned_bytes(bytes, GPU_F_RESIZE_COEFFICIENT_ALIGNMENT_BYTES)
        <= GPU_F_RESIZE_MAX_COEFFICIENT_BINDING_BYTES
}

fn encode_resize_coeffs_f64(coeffs: &FilterCoeffsF64) -> Result<Vec<u32>, PilError> {
    let word_count = resize_coeff_word_count_f64(coeffs)?;
    let mut words = Vec::with_capacity(word_count);
    let mut weight_offset = 0usize;
    for ((&xmin, &count), row) in coeffs.xmin.iter().zip(&coeffs.count).zip(&coeffs.weights) {
        words.push(u32::try_from(xmin).map_err(|_| {
            PilError::ValueError("GPU resize coefficient xmin exceeds shader limits".into())
        })?);
        words.push(u32::try_from(count).map_err(|_| {
            PilError::ValueError("GPU resize coefficient count exceeds shader limits".into())
        })?);
        words.push(u32::try_from(weight_offset).map_err(|_| {
            PilError::ValueError("GPU resize coefficient offset exceeds shader limits".into())
        })?);
        weight_offset = weight_offset
            .checked_add(row.len().checked_mul(4).ok_or_else(|| {
                PilError::ValueError("GPU resize coefficient arena size overflow".into())
            })?)
            .ok_or_else(|| {
                PilError::ValueError("GPU resize coefficient arena size overflow".into())
            })?;
    }
    for row in &coeffs.weights {
        for &weight in row {
            let parts = gpu_f64_integer_parts(weight).ok_or_else(|| {
                PilError::ValueError(
                    "GPU resize f64 coefficient is not a finite normal value".into(),
                )
            })?;
            words.push(parts.mantissa as u32);
            words.push((parts.mantissa >> 32) as u32);
            words.push(parts.exponent as u32);
            words.push(u32::from(parts.negative && parts.mantissa != 0));
        }
    }
    debug_assert_eq!(words.len(), word_count);
    Ok(words)
}

/// Encode compact Box rows whose normalized coefficient is repeated for every
/// tap.  This is used only for integer-ratio downscales beyond the full f64
/// coefficient binding envelope; ordinary rows continue to use the complete
/// per-tap table above.
fn encode_resize_compact_box_axis(
    source_size: u32,
    output_size: u32,
) -> Result<Vec<u32>, PilError> {
    if output_size == 0
        || source_size % output_size != 0
        || source_size / output_size <= GPU_F_RESIZE_ORDERED_MAX_TAPS as u32
    {
        return Err(PilError::InternalError(
            "GPU compact Box coefficient geometry is invalid".into(),
        ));
    }
    let tap_count = source_size / output_size;
    let coefficient = 1.0 / f64::from(tap_count);
    let parts = gpu_f64_integer_parts(coefficient).ok_or_else(|| {
        PilError::ValueError("GPU compact Box coefficient is not a finite normal value".into())
    })?;
    let output_count = usize::try_from(output_size)
        .map_err(|_| PilError::ValueError("GPU compact Box output size is too large".into()))?;
    let mut words = Vec::with_capacity(
        output_count
            .checked_mul(3)
            .and_then(|count| count.checked_add(4))
            .ok_or_else(|| PilError::ValueError("GPU compact Box arena size overflow".into()))?,
    );
    for output_index in 0..output_count {
        let xmin = u32::try_from(
            output_index
                .checked_mul(usize::try_from(tap_count).map_err(|_| {
                    PilError::ValueError("GPU compact Box tap count is too large".into())
                })?)
                .ok_or_else(|| PilError::ValueError("GPU compact Box offset overflow".into()))?,
        )
        .map_err(|_| PilError::ValueError("GPU compact Box offset exceeds shader limits".into()))?;
        words.extend([xmin, tap_count, 0]);
    }
    words.extend([
        parts.mantissa as u32,
        (parts.mantissa >> 32) as u32,
        parts.exponent as u32,
        u32::from(parts.negative && parts.mantissa != 0),
    ]);
    Ok(words)
}

fn gpu_resize_coefficients_are_safe(
    filter: ResampleFilter,
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
) -> bool {
    let (source_w, source_h) = source_dimensions;
    let (output_w, output_h) = output_dimensions;
    if source_w == 0 || source_h == 0 || output_w == 0 || output_h == 0 {
        return false;
    }
    let horizontal = gpu_resize_coefficients(output_w, source_w, filter);
    let vertical = gpu_resize_coefficients(output_h, source_h, filter);
    resize_coeff_word_count(&horizontal).is_ok() && resize_coeff_word_count(&vertical).is_ok()
}

/// Build the exact two-pass coefficient tables for ImageOps.fit's fractional
/// source box.  The boxed builder performs Pillow's required f32 boundary
/// conversion before calculating f64 centers and 22-bit weights; reusing it
/// here keeps the GPU's control table identical to `pil_resize_boxed` while
/// leaving all sample reads, accumulation, and writes on the device.
/// This follows Pillow's `libImaging/Resample.c::precompute_coeffs` contract:
/// `ImageOps.fit` passes its crop box through the float ABI before those
/// coefficients are calculated, so retaining the fractional box is essential
/// at pixel boundaries.
fn gpu_fit_coefficients(
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    bleed: f64,
    centering: (f64, f64),
    filter: ResampleFilter,
) -> Option<(FilterCoeffs, FilterCoeffs)> {
    let (crop_left, crop_top, crop_w, crop_h) = gpu_fit_box(
        source_dimensions.0,
        source_dimensions.1,
        output_dimensions.0,
        output_dimensions.1,
        bleed,
        centering,
    )?;
    if matches!(filter, ResampleFilter::Nearest) {
        return Some((
            gpu_fit_nearest_coefficients(
                output_dimensions.0,
                source_dimensions.0,
                crop_left,
                crop_left + crop_w,
            ),
            gpu_fit_nearest_coefficients(
                output_dimensions.1,
                source_dimensions.1,
                crop_top,
                crop_top + crop_h,
            ),
        ));
    }
    Some((
        precompute_coeffs_boxed_for_filter(
            output_dimensions.0,
            source_dimensions.0,
            crop_left,
            crop_left + crop_w,
            filter,
        ),
        precompute_coeffs_boxed_for_filter(
            output_dimensions.1,
            source_dimensions.1,
            crop_top,
            crop_top + crop_h,
            filter,
        ),
    ))
}

/// Build one-tap tables for Pillow's boxed nearest affine mapping.
///
/// `Image.resize(box=...)` does not use the convolution coefficients for
/// `NEAREST`: `ImagingScaleAffine` evaluates the source coordinate from the
/// float box bounds for each output pixel.  Keep that mapping in the host
/// control table (after the same f32 boundary conversion) so the exact
/// resize kernels can still own every byte read and write.
fn gpu_fit_nearest_coefficients(
    output_size: u32,
    source_size: u32,
    box_start: f64,
    box_end: f64,
) -> FilterCoeffs {
    if output_size == 0 || source_size == 0 {
        return FilterCoeffs {
            xmin: Vec::new(),
            count: Vec::new(),
            offsets: Vec::new(),
            weights: Vec::new(),
        };
    }
    let box_start_f32 = box_start as f32;
    let box_end_f32 = box_end as f32;
    let box_start = box_start_f32 as f64;
    let scale = f64::from(box_end_f32 - box_start_f32) / f64::from(output_size);
    let last_source = i64::from(source_size - 1);
    let mut xmin = Vec::with_capacity(output_size as usize);
    let mut count = Vec::with_capacity(output_size as usize);
    let mut offsets = Vec::with_capacity(output_size as usize);
    let mut weights = Vec::with_capacity(output_size as usize);
    for output in 0..output_size {
        let coordinate = box_start + (f64::from(output) + 0.5) * scale;
        let source = (coordinate.floor() as i64).clamp(0, last_source);
        xmin.push(source);
        count.push(1);
        offsets.push(weights.len());
        weights.push(1i64 << 22);
    }
    FilterCoeffs {
        xmin,
        count,
        offsets,
        weights,
    }
}

fn gpu_fit_coefficients_are_safe(
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    bleed: f64,
    centering: (f64, f64),
    filter: ResampleFilter,
) -> bool {
    let Some((horizontal, vertical)) = gpu_fit_coefficients(
        source_dimensions,
        output_dimensions,
        bleed,
        centering,
        filter,
    ) else {
        return false;
    };
    resize_coeff_word_count(&horizontal).is_ok() && resize_coeff_word_count(&vertical).is_ok()
}

fn create_sized_buffer(
    device: &wgpu::Device,
    label: &'static str,
    usage: wgpu::BufferUsages,
    size_bytes: usize,
    alignment_bytes: usize,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: aligned_bytes(size_bytes, alignment_bytes) as u64,
        usage,
        mapped_at_creation: false,
    })
}

struct GpuBatchResources<'a> {
    buf_a: &'a wgpu::Buffer,
    buf_b: &'a wgpu::Buffer,
    fallback_img2: &'a wgpu::Buffer,
    fallback_img3: &'a wgpu::Buffer,
    fallback_lut: &'a wgpu::Buffer,
    histogram: &'a wgpu::Buffer,
    params: &'a wgpu::Buffer,
    params_ranges: Vec<BufferRange>,
    img2: Option<&'a wgpu::Buffer>,
    img2_ranges: Vec<Option<BufferRange>>,
    resize_coeff_ranges: Vec<Option<(BufferRange, BufferRange)>>,
    img3: Option<&'a wgpu::Buffer>,
    img3_ranges: Vec<Option<BufferRange>>,
    lut: Option<&'a wgpu::Buffer>,
    lut_ranges: Vec<Option<BufferRange>>,
}

struct PreparedGpuBatch<'a> {
    resources: GpuBatchResources<'a>,
    input_dims: Vec<(u32, u32)>,
    output_dims: Vec<(u32, u32)>,
    /// Contain dimensions used by a public Pad operation before its final
    /// placement pass.  The resize and placement remain in one command
    /// buffer, so this is planner metadata rather than a host-side image
    /// handoff.
    pad_resize_dims: Vec<Option<(u32, u32)>>,
    final_dims: (u32, u32),
    resource_telemetry: PipelineResourceTelemetry,
}

fn ranged_binding(
    buffer: &wgpu::Buffer,
    range: Option<BufferRange>,
) -> Result<wgpu::BindingResource<'_>, PilError> {
    match range {
        Some(range) => {
            let size = NonZeroU64::new(range.size).ok_or_else(|| {
                PilError::InternalError("GPU buffer binding range cannot be empty".into())
            })?;
            Ok(wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer,
                offset: range.offset,
                size: Some(size),
            }))
        }
        None => Ok(buffer.as_entire_binding()),
    }
}

fn auxiliary_binding<'a>(
    arena: Option<&'a wgpu::Buffer>,
    fallback: &'a wgpu::Buffer,
    range: Option<BufferRange>,
) -> Result<wgpu::BindingResource<'a>, PilError> {
    match range {
        Some(range) => {
            let buffer = arena.ok_or_else(|| {
                PilError::InternalError("GPU auxiliary range has no backing buffer".into())
            })?;
            ranged_binding(buffer, Some(range))
        }
        None => ranged_binding(fallback, None),
    }
}

// ─── Count shader bindings ─────────────────────────────────────────────────

/// Count the number of `@binding(N)` entries in WGSL source.
/// Returns max binding index + 1, or 0 if no bindings found.
fn count_shader_bindings(source: &str) -> u32 {
    let mut max_binding: i32 = -1;
    for line in source.lines() {
        if let Some(pos) = line.find("@binding(") {
            let rest = &line[pos + 9..];
            if let Some(end) = rest.find(')') {
                if let Ok(n) = rest[..end].trim().parse::<u32>() {
                    max_binding = max_binding.max(n as i32);
                }
            }
        }
    }
    if max_binding >= 0 {
        max_binding as u32 + 1
    } else {
        0
    }
}

/// Detect if a 4-binding shader uses the LUT layout (Eval/PointOp).
/// In LUT layout, `@binding(1)` is `storage read_write` (output).
/// In dual-input layout, `@binding(1)` is `storage read` (input_b).
fn is_lut_shader(source: &str) -> bool {
    for line in source.lines() {
        if line.contains("@binding(1)") {
            return line.contains("read_write");
        }
    }
    false
}

/// Detect the two-binding generator layout used by kernels that do not read
/// the current pipeline image. Their binding 0 is the writable output and
/// binding 1 is the uniform parameter block, unlike ordinary two-binding
/// kernels whose binding 0 is a read-only input.
fn is_output_only_shader(source: &str) -> bool {
    source
        .lines()
        .any(|line| line.contains("@binding(0)") && line.contains("var<storage, read_write>"))
}

// ─── CachedPipeline ────────────────────────────────────────────────────────

struct CachedPipeline {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    variant_name: &'static str,
    shader_file: &'static str,
    /// Number of bindings in this shader (2-5).
    num_bindings: u32,
    /// True if this is a 4-binding LUT shader (Eval/PointOp).
    is_lut: bool,
    /// True if this is a two-binding generator shader with no input image.
    is_output_only: bool,
    /// True if the shader reads an auxiliary source and updates the current
    /// destination buffer in place (AlphaComposite).
    is_in_place: bool,
}

enum ResolvedPipeline {
    Single(Arc<CachedPipeline>),
    /// The second public operation of a private fused dispatch.  It remains
    /// in the prepared operation/resource arrays so the public operation
    /// count, auxiliary ownership, and parameter indexing stay unchanged.
    Skip,
    Blur {
        horizontal: Arc<CachedPipeline>,
        vertical: Arc<CachedPipeline>,
        pass_count: usize,
    },
    /// Non-nearest resize is the same exact two-pass fixed-point algorithm
    /// used by the CPU/SIMD paths.  Coefficients live in an auxiliary storage
    /// range, so the two dispatches stay on-device with no host materialize.
    Resize {
        horizontal: Arc<CachedPipeline>,
        vertical: Arc<CachedPipeline>,
    },
    /// Pad is an exact contain resize followed by a device-side fill/copy
    /// placement.  Keeping both stages inside one compute pass avoids a host
    /// materialization while preserving Pillow's separable resize contract.
    Pad {
        horizontal: Arc<CachedPipeline>,
        vertical: Arc<CachedPipeline>,
        place: Arc<CachedPipeline>,
    },
    /// Histogram-driven ImageOps keep all control/data passes on the device:
    /// clear the reusable histogram, accumulate the current image, derive a
    /// packed 256-entry LUT, and apply that LUT through the vector remap.
    Histogram {
        clear: Arc<CachedPipeline>,
        histogram: Arc<CachedPipeline>,
        derive: Arc<CachedPipeline>,
        remap: Arc<CachedPipeline>,
    },
}

#[derive(Default)]
struct GpuDeviceState {
    lost: Option<String>,
    uncaptured_error: Option<String>,
}

// ─── GpuInner (lazy-initialized GPU engine) ────────────────────────────────

/// Internal GPU engine. Initialized once and stored in a static OnceLock.
struct GpuInner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// Pipelines are compiled on first use. Values are shared so a resolved
    /// batch can keep every pipeline alive for the full compute-pass lifetime
    /// without holding the cache lock while commands are encoded.
    pipelines: Mutex<HashMap<&'static str, Arc<CachedPipeline>>>,
    device_state: Arc<Mutex<GpuDeviceState>>,
    available_buffers: Mutex<Vec<BufferPool>>,
    available_staging: Mutex<Vec<StagingBuffer>>,
}

#[cfg(not(target_arch = "wasm32"))]
fn format_adapter_inventory(adapters: &[wgpu::Adapter]) -> String {
    let infos = adapters
        .iter()
        .map(wgpu::Adapter::get_info)
        .collect::<Vec<_>>();
    format!("enumerated={} adapters={infos:?}", adapters.len())
}

impl GpuInner {
    fn new() -> Result<Self, PilError> {
        // `Instance::default()` does not apply WGPU_BACKEND. Use the explicit
        // descriptor so backend selection is deterministic and debuggable in
        // the Python extension as well as in standalone Rust binaries.
        let instance_descriptor = wgpu::InstanceDescriptor::from_env_or_default();
        let instance = wgpu::Instance::new(&instance_descriptor);
        let request_options = wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        };

        #[cfg(not(target_arch = "wasm32"))]
        let enumerated_adapters = instance.enumerate_adapters(instance_descriptor.backends);
        #[cfg(not(target_arch = "wasm32"))]
        let adapter_inventory = format_adapter_inventory(&enumerated_adapters);

        // Prefer wgpu's normal selection because it applies power preference
        // and compatibility rules. Some embedded/native-library contexts have
        // been observed to return None here even though direct enumeration
        // succeeds; use that already-enumerated adapter instead of rejecting
        // a usable device in that case.
        let adapter =
            pollster::block_on(instance.request_adapter(&request_options)).or_else(|| {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    enumerated_adapters.into_iter().next()
                }
                #[cfg(target_arch = "wasm32")]
                {
                    None
                }
            });
        let adapter = adapter.ok_or_else(|| {
            #[cfg(not(target_arch = "wasm32"))]
            {
                return PilError::ValueError(format!(
                    "GPU adapter not available: requested={:?}, enabled={:?}, {}",
                    instance_descriptor.backends,
                    wgpu::Instance::enabled_backend_features(),
                    adapter_inventory,
                ));
            }
            #[cfg(target_arch = "wasm32")]
            {
                PilError::ValueError(format!(
                    "GPU adapter not available: requested={:?}",
                    instance_descriptor.backends,
                ))
            }
        })?;
        gpu_log!("[GPU] adapter selected: {:?}", adapter.get_info());
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("pillow-rs-gpu"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .map_err(|error| {
            PilError::ValueError(format!("GPU device initialization failed: {error}"))
        })?;

        let device_state = Arc::new(Mutex::new(GpuDeviceState::default()));
        let lost_state = Arc::clone(&device_state);
        device.set_device_lost_callback(move |reason, message| {
            let detail = format!("{reason:?}: {message}");
            if let Ok(mut state) = lost_state.lock() {
                state.lost = Some(detail.clone());
            }
            log::error!(target: "compute::gpu", "[GPU] device lost: {detail}");
        });
        let error_state = Arc::clone(&device_state);
        device.on_uncaptured_error(Box::new(move |error| {
            let detail = error.to_string();
            if let Ok(mut state) = error_state.lock() {
                state.uncaptured_error = Some(detail.clone());
            }
            log::error!(target: "compute::gpu", "[GPU] uncaptured device error: {detail}");
        }));

        Ok(GpuInner {
            device,
            queue,
            pipelines: Mutex::new(HashMap::new()),
            device_state,
            available_buffers: Mutex::new(Vec::new()),
            available_staging: Mutex::new(Vec::new()),
        })
    }

    fn acquire_buffers(&self, minimum_capacity: u32) -> Result<BufferPool, PilError> {
        let mut available = self.available_buffers.lock().map_err(|_| {
            PilError::InternalError("GPU working-buffer pool lock is poisoned".into())
        })?;
        let candidate = available
            .iter()
            .enumerate()
            .filter(|(_, buffers)| gpu_buffer_reuse_allowed(buffers.capacity, minimum_capacity))
            .min_by_key(|(_, buffers)| buffers.capacity)
            .map(|(index, _)| index);
        Ok(match candidate {
            Some(index) => available.swap_remove(index),
            None => BufferPool::new(&self.device, minimum_capacity),
        })
    }

    fn recycle_buffers(&self, buffers: BufferPool) {
        if self.failure_detail().is_some()
            || buffers.retained_bytes() > MAX_RETAINED_GPU_WORKING_BYTES
        {
            return;
        }
        let Ok(mut available) = self.available_buffers.lock() else {
            return;
        };
        available.push(buffers);
        available.sort_unstable_by_key(|candidate| candidate.capacity);
        while available.len() > MAX_RETAINED_GPU_WORKING_SETS
            || available.iter().fold(0u64, |total, candidate| {
                total.saturating_add(candidate.retained_bytes())
            }) > MAX_RETAINED_GPU_WORKING_BYTES
        {
            let _ = available.pop();
        }
    }

    fn acquire_staging(&self, minimum_bytes: u64) -> Result<StagingBuffer, PilError> {
        let mut available = self.available_staging.lock().map_err(|_| {
            PilError::InternalError("GPU staging-buffer pool lock is poisoned".into())
        })?;
        let candidate = available
            .iter()
            .enumerate()
            .filter(|(_, staging)| staging.capacity_bytes >= minimum_bytes)
            .min_by_key(|(_, staging)| staging.capacity_bytes)
            .map(|(index, _)| index);
        Ok(match candidate {
            Some(index) => available.swap_remove(index),
            None => StagingBuffer::new(&self.device, minimum_bytes),
        })
    }

    fn recycle_staging(&self, staging: StagingBuffer) {
        if self.failure_detail().is_some()
            || staging.capacity_bytes > MAX_RETAINED_GPU_STAGING_BYTES
        {
            return;
        }
        let Ok(mut available) = self.available_staging.lock() else {
            return;
        };
        available.push(staging);
        available.sort_unstable_by_key(|candidate| candidate.capacity_bytes);
        while available.len() > MAX_RETAINED_GPU_STAGING_BUFFERS
            || available.iter().fold(0u64, |total, candidate| {
                total.saturating_add(candidate.capacity_bytes)
            }) > MAX_RETAINED_GPU_STAGING_BYTES
        {
            let _ = available.pop();
        }
    }

    fn invalidate_resource_pools(&self) {
        if let Ok(mut pipelines) = self.pipelines.lock() {
            pipelines.clear();
        }
        if let Ok(mut available) = self.available_buffers.lock() {
            available.clear();
        }
        if let Ok(mut available) = self.available_staging.lock() {
            available.clear();
        }
    }

    fn resolve_pipeline(
        &self,
        key: &'static str,
        shader_file: &'static str,
        source: &'static str,
    ) -> Result<Arc<CachedPipeline>, PilError> {
        let mut pipelines = self
            .pipelines
            .lock()
            .map_err(|_| PilError::InternalError("GPU pipeline-cache lock is poisoned".into()))?;
        if let Some(pipeline) = pipelines.get(key) {
            return Ok(Arc::clone(pipeline));
        }

        gpu_log!("[GPU] compiling shader on first use: {key}");
        let pipeline =
            Self::build_pipeline(&self.device, key, shader_file, source).ok_or_else(|| {
                PilError::ValueError(format!(
                    "GPU operation '{key}' has no validated executable pipeline"
                ))
            })?;
        gpu_log!(
            "[GPU] compiled shader on first use: {key} ({} bindings)",
            pipeline.num_bindings
        );
        let pipeline = Arc::new(pipeline);
        pipelines.insert(key, Arc::clone(&pipeline));
        Ok(pipeline)
    }

    fn resolve_batch_pipelines(
        &self,
        ops: &[PipelineOp],
        logical_mode: Option<&str>,
    ) -> Result<Vec<ResolvedPipeline>, PilError> {
        let mut resolved = Vec::with_capacity(ops.len());
        let mut index = 0usize;
        while index < ops.len() {
            if index + 1 < ops.len() && can_fuse_gpu_multiply_screen(ops, index) {
                let fused = self.resolve_pipeline(
                    "__internal_multiply_screen",
                    "multiply_screen.wgsl",
                    include_str!("shaders/multiply_screen.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(fused));
                resolved.push(ResolvedPipeline::Skip);
                index += 2;
                continue;
            }

            let op = &ops[index];
            if matches!(op, PipelineOp::Autocontrast { .. } | PipelineOp::Equalize) {
                let clear = self.resolve_pipeline(
                    "__internal_histogram_clear",
                    "histogram_clear.wgsl",
                    include_str!("shaders/histogram_clear.wgsl"),
                )?;
                let histogram = match op {
                    PipelineOp::Autocontrast { .. } => self.resolve_pipeline(
                        "__internal_autocontrast_histogram",
                        "autocontrast_histogram.wgsl",
                        include_str!("shaders/autocontrast_histogram.wgsl"),
                    )?,
                    PipelineOp::Equalize => self.resolve_pipeline(
                        "__internal_equalize_histogram",
                        "equalize_histogram.wgsl",
                        include_str!("shaders/equalize_histogram.wgsl"),
                    )?,
                    _ => unreachable!("histogram pipeline branch changed"),
                };
                let derive = match op {
                    PipelineOp::Autocontrast { .. } => self.resolve_pipeline(
                        "__internal_autocontrast_lut",
                        "autocontrast_cutoff.wgsl",
                        include_str!("shaders/autocontrast_cutoff.wgsl"),
                    )?,
                    PipelineOp::Equalize => self.resolve_pipeline(
                        "__internal_equalize_lut",
                        "equalize_cdf.wgsl",
                        include_str!("shaders/equalize_cdf.wgsl"),
                    )?,
                    _ => unreachable!("histogram pipeline branch changed"),
                };
                let remap = self.resolve_pipeline(
                    "__internal_histogram_remap",
                    "point_op.wgsl",
                    include_str!("shaders/point_op.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Histogram {
                    clear,
                    histogram,
                    derive,
                    remap,
                });
            } else if let Some(pass_count) = Self::blur_pass_count(op) {
                // These internal variants expand one public blur operation
                // into horizontal and vertical dispatches without a host
                // materialization between them.
                let horizontal = self.resolve_pipeline(
                    "__internal_blur_h",
                    "box_blur_h.wgsl",
                    include_str!("shaders/box_blur_h.wgsl"),
                )?;
                let vertical = self.resolve_pipeline(
                    "__internal_blur_v",
                    "box_blur_v.wgsl",
                    include_str!("shaders/box_blur_v.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Blur {
                    horizontal,
                    vertical,
                    pass_count,
                });
            } else if matches!(op, PipelineOp::Pad { .. }) {
                let horizontal = self.resolve_pipeline(
                    "__internal_resize_h",
                    "resize_convolution_h.wgsl",
                    include_str!("shaders/resize_convolution_h.wgsl"),
                )?;
                let vertical = self.resolve_pipeline(
                    "__internal_resize_v",
                    "resize_convolution_v.wgsl",
                    include_str!("shaders/resize_convolution_v.wgsl"),
                )?;
                let place = self.resolve_pipeline(
                    "__internal_pad_place",
                    "pad.wgsl",
                    include_str!("shaders/pad.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Pad {
                    horizontal,
                    vertical,
                    place,
                });
            } else if matches!(op, PipelineOp::Fit { .. }) {
                let horizontal = self.resolve_pipeline(
                    "__internal_resize_h",
                    "resize_convolution_h.wgsl",
                    include_str!("shaders/resize_convolution_h.wgsl"),
                )?;
                let vertical = self.resolve_pipeline(
                    "__internal_resize_v",
                    "resize_convolution_v.wgsl",
                    include_str!("shaders/resize_convolution_v.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Resize {
                    horizontal,
                    vertical,
                });
            } else if matches!(op, PipelineOp::Resize { filter, .. }
                if matches!(filter, ResampleFilter::Nearest)
                    && !gpu_resize_nearest_uses_coefficients(logical_mode))
            {
                let key = registry::variant_key(op);
                let source = registry::registry()?
                    .get(key)
                    .and_then(|entry| entry.gpu_source)
                    .ok_or_else(|| {
                        PilError::ValueError(format!(
                            "GPU operation '{key}' has no registered shader source"
                        ))
                    })?;
                let shader_file = registry::registry()?
                    .get(key)
                    .and_then(|entry| entry.gpu_shader)
                    .ok_or_else(|| {
                        PilError::ValueError(format!(
                            "GPU operation '{key}' has no registered shader name"
                        ))
                    })?;
                resolved.push(
                    self.resolve_pipeline(key, shader_file, source)
                        .map(ResolvedPipeline::Single)?,
                );
            } else if matches!(op, PipelineOp::Resize { .. }) {
                let horizontal = self.resolve_pipeline(
                    "__internal_resize_h",
                    "resize_convolution_h.wgsl",
                    include_str!("shaders/resize_convolution_h.wgsl"),
                )?;
                let vertical = self.resolve_pipeline(
                    "__internal_resize_v",
                    "resize_convolution_v.wgsl",
                    include_str!("shaders/resize_convolution_v.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Resize {
                    horizontal,
                    vertical,
                });
            } else {
                let key = registry::variant_key(op);
                let source = registry::registry()?
                    .get(key)
                    .and_then(|entry| entry.gpu_source)
                    .ok_or_else(|| {
                        PilError::ValueError(format!(
                            "GPU operation '{key}' has no registered shader source"
                        ))
                    })?;
                let shader_file = registry::registry()?
                    .get(key)
                    .and_then(|entry| entry.gpu_shader)
                    .ok_or_else(|| {
                        PilError::ValueError(format!(
                            "GPU operation '{key}' has no registered shader name"
                        ))
                    })?;
                resolved.push(
                    self.resolve_pipeline(key, shader_file, source)
                        .map(ResolvedPipeline::Single)?,
                );
            }
            index += 1;
        }
        Ok(resolved)
    }

    fn failure_detail(&self) -> Option<String> {
        self.device_state.lock().ok().and_then(|state| {
            state
                .lost
                .clone()
                .or_else(|| state.uncaptured_error.clone())
        })
    }

    fn ensure_healthy(&self, stage: &str) -> Result<(), PilError> {
        if let Some(detail) = self.failure_detail() {
            self.invalidate_resource_pools();
            return Err(PilError::ValueError(format!(
                "GPU device unavailable during {stage}: {detail}"
            )));
        }
        Ok(())
    }

    fn mark_failed(&self, detail: String) {
        if let Ok(mut state) = self.device_state.lock() {
            if state.lost.is_none() {
                state.lost = Some(detail);
            }
        }
        self.invalidate_resource_pools();
    }

    fn poll_device(&self, stage: &str) -> Result<(), PilError> {
        self.ensure_healthy(stage)?;
        self.device.poll(wgpu::Maintain::Poll);
        self.ensure_healthy(stage)
    }

    fn build_pipeline(
        device: &wgpu::Device,
        variant_name: &'static str,
        shader_file: &'static str,
        shader_source: &str,
    ) -> Option<CachedPipeline> {
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let cs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(variant_name),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });
        if let Some(error) = pollster::block_on(device.pop_error_scope()) {
            gpu_log!(
                "[GPU] shader module validation failed: variant={variant_name} file={shader_file}: {error}"
            );
            return None;
        }

        let num_bindings = count_shader_bindings(shader_source);

        // Supported: 2-5 binding shaders. 0/1/>5 are invalid.
        if !(2..=5).contains(&num_bindings) {
            gpu_log!(
                "[GPU] shader rejected: variant={variant_name} file={shader_file}: bindings={num_bindings}"
            );
            return None;
        }

        // Detect if this is a LUT shader (Eval/PointOp) with 4 bindings.
        let is_lut = num_bindings == 4 && is_lut_shader(shader_source);
        let is_output_only = num_bindings == 2 && is_output_only_shader(shader_source);
        let is_in_place = variant_name == "AlphaComposite";

        // Build bind group layout matching shader declarations.
        // Layout depends on binding count and LUT variant:
        //   2: [input(read), output(read_write)]
        //   2 (generator): [output(read_write), params(uniform)]
        //   3: [input(read), output(read_write), params(uniform)]
        //   4 (dual-input): [input_a(read), input_b(read), output(read_write), params(uniform)]
        //   4 (LUT):        [input(read), output(read_write), params(uniform), lut(read)]
        //   5: [input_a(read), input_b(read), input_c(read), output(read_write), params(uniform)]
        let mut bindings = Vec::with_capacity(num_bindings as usize);

        if num_bindings == 5 {
            // 5-binding: 3 inputs + output + params (Composite/CompositeModule/Paste)
            for i in 0..3 {
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: i,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
            }
            bindings.push(wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
            bindings.push(wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
        } else if num_bindings == 4 {
            if is_lut {
                // LUT layout: [input(read), output(rw), params(uniform), lut(read)]
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
            } else {
                // Dual-input layout
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
            }
        } else if is_output_only {
            // Generator layout: [output(rw), params(uniform)].
            bindings.push(wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
            bindings.push(wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
        } else {
            // 2 or 3 binding (single-input) layout
            bindings.push(wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
            bindings.push(wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });

            if num_bindings > 2 {
                bindings.push(wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
            }
        }

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(variant_name),
            entries: &bindings,
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(variant_name),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Use error scope to catch shader validation errors without panicking.
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(variant_name),
            layout: Some(&pipeline_layout),
            module: &cs_module,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // If validation failed, skip this shader — it won't be available on GPU.
        if let Some(error) = pollster::block_on(device.pop_error_scope()) {
            gpu_log!(
                "[GPU] shader validation failed: variant={variant_name} file={shader_file}: {error}"
            );
            return None;
        }

        Some(CachedPipeline {
            pipeline,
            bind_group_layout,
            variant_name,
            shader_file,
            num_bindings,
            is_lut,
            is_output_only,
            is_in_place,
        })
    }

    fn make_bind_group(
        &self,
        cached: &CachedPipeline,
        input_buf: &wgpu::Buffer,
        output_buf: &wgpu::Buffer,
        resources: &GpuBatchResources,
        op_index: usize,
    ) -> Result<wgpu::BindGroup, PilError> {
        let params = ranged_binding(resources.params, Some(resources.params_ranges[op_index]))?;
        let second = auxiliary_binding(
            resources.img2,
            resources.fallback_img2,
            resources.img2_ranges[op_index],
        )?;
        let third = auxiliary_binding(
            resources.img3,
            resources.fallback_img3,
            resources.img3_ranges[op_index],
        )?;
        let lut = auxiliary_binding(
            resources.lut,
            resources.fallback_lut,
            resources.lut_ranges[op_index],
        )?;
        let mut entries = Vec::with_capacity(cached.num_bindings as usize);
        // Histogram control passes reuse the ordinary binding counts but
        // reinterpret the storage slots:
        //   clear:  [unused input, histogram rw, params]
        //   gather:  [image, optional mask, histogram rw, params]
        //   derive:  [histogram, generated LUT rw, params]
        // The final remap is a normal LUT shader, except that it must read the
        // generated LUT buffer rather than a host-uploaded LUT arena.
        if cached.variant_name == "__internal_histogram_clear" {
            entries.push(wgpu::BindGroupEntry {
                binding: 0,
                resource: input_buf.as_entire_binding(),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 1,
                resource: resources.histogram.as_entire_binding(),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 2,
                resource: params,
            });
        } else if cached.variant_name == "__internal_autocontrast_histogram"
            || cached.variant_name == "__internal_equalize_histogram"
        {
            entries.push(wgpu::BindGroupEntry {
                binding: 0,
                resource: input_buf.as_entire_binding(),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 1,
                resource: auxiliary_binding(
                    resources.img3,
                    resources.fallback_img3,
                    resources.img3_ranges[op_index],
                )?,
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 2,
                resource: resources.histogram.as_entire_binding(),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 3,
                resource: params,
            });
        } else if cached.variant_name == "__internal_autocontrast_lut"
            || cached.variant_name == "__internal_equalize_lut"
        {
            entries.push(wgpu::BindGroupEntry {
                binding: 0,
                resource: resources.histogram.as_entire_binding(),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 1,
                resource: resources.fallback_lut.as_entire_binding(),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 2,
                resource: params,
            });
        } else if cached.variant_name == "__internal_histogram_remap" {
            entries.push(wgpu::BindGroupEntry {
                binding: 0,
                resource: input_buf.as_entire_binding(),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 1,
                resource: output_buf.as_entire_binding(),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 2,
                resource: params,
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 3,
                resource: resources.fallback_lut.as_entire_binding(),
            });
        } else if matches!(
            cached.variant_name,
            "__internal_resize_h" | "__internal_resize_v"
        ) {
            let (horizontal, vertical) = resources
                .resize_coeff_ranges
                .get(op_index)
                .and_then(Option::as_ref)
                .ok_or_else(|| {
                    PilError::InternalError(
                        "GPU resize pipeline is missing its coefficient ranges".into(),
                    )
                })?;
            let range = if cached.variant_name == "__internal_resize_h" {
                *horizontal
            } else {
                *vertical
            };
            entries.push(wgpu::BindGroupEntry {
                binding: 0,
                resource: input_buf.as_entire_binding(),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 1,
                resource: output_buf.as_entire_binding(),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 2,
                resource: params,
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 3,
                resource: auxiliary_binding(resources.img2, resources.fallback_img2, Some(range))?,
            });
        } else {
            match (cached.num_bindings, cached.is_lut, cached.is_output_only) {
                (2, _, true) => {
                    // Generator layout: [output(rw), params(uniform)].
                    entries.push(wgpu::BindGroupEntry {
                        binding: 0,
                        resource: output_buf.as_entire_binding(),
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 1,
                        resource: params,
                    });
                }
                (5, _, _) => {
                    // 5-binding: [in_a(read), in_b(read), in_c(read), out(rw), params(uniform)]
                    entries.push(wgpu::BindGroupEntry {
                        binding: 0,
                        resource: input_buf.as_entire_binding(),
                    });
                    // Storage read and read_write bindings may not alias within one
                    // compute dispatch. Keep absent optional inputs on their
                    // dedicated auxiliary buffers; shaders guard those reads with
                    // their corresponding presence parameter.
                    entries.push(wgpu::BindGroupEntry {
                        binding: 1,
                        resource: second,
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 2,
                        resource: third,
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 3,
                        resource: output_buf.as_entire_binding(),
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 4,
                        resource: params,
                    });
                }
                (4, true, _) => {
                    // LUT layout: [input(read), output(rw), params(uniform), lut(read)]
                    entries.push(wgpu::BindGroupEntry {
                        binding: 0,
                        resource: input_buf.as_entire_binding(),
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 1,
                        resource: output_buf.as_entire_binding(),
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 2,
                        resource: params,
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 3,
                        resource: lut,
                    });
                }
                (4, false, _) => {
                    // Dual-input layout: [input_a(read), input_b(read), output(rw), params(uniform)]
                    entries.push(wgpu::BindGroupEntry {
                        binding: 0,
                        resource: input_buf.as_entire_binding(),
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 1,
                        resource: second,
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 2,
                        resource: output_buf.as_entire_binding(),
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 3,
                        resource: params,
                    });
                }
                (3, _, _) if cached.is_in_place => {
                    // AlphaComposite: [source(read), destination(read_write), params].
                    // The destination is the current ping-pong buffer, so the
                    // dispatch updates it in place and encode_batch keeps the
                    // current buffer selection unchanged.
                    entries.push(wgpu::BindGroupEntry {
                        binding: 0,
                        resource: second,
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 1,
                        resource: input_buf.as_entire_binding(),
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 2,
                        resource: params,
                    });
                }
                _ => {
                    // 2 or 3 binding: [input(read), output(rw), ...params(uniform)]
                    entries.push(wgpu::BindGroupEntry {
                        binding: 0,
                        resource: input_buf.as_entire_binding(),
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 1,
                        resource: output_buf.as_entire_binding(),
                    });
                    if cached.num_bindings > 2 {
                        entries.push(wgpu::BindGroupEntry {
                            binding: 2,
                            resource: params,
                        });
                    }
                }
            }
        }
        Ok(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(cached.variant_name),
            layout: &cached.bind_group_layout,
            entries: &entries,
        }))
    }

    fn estimate_resource_bytes(
        &self,
        op: &PipelineOp,
        auxiliary: &AuxiliaryImages,
        buffers: &BufferPool,
        uniform_alignment: usize,
        storage_alignment: usize,
        source_dimensions: (u32, u32),
        mode: u32,
        f_resize_f64_is_exact: bool,
        f_resize_f64_ordered_is_exact: bool,
    ) -> Result<usize, PilError> {
        let op_param_words = if matches!(op, PipelineOp::Pad { .. }) {
            // Header + resize controls + placement controls. The final
            // output dimensions appended below are included separately.
            9
        } else if matches!(op, PipelineOp::Fit { .. }) {
            // Fit is lowered to the exact separable resize kernels. Its
            // fractional crop is carried by the coefficient ranges, leaving
            // the same four resize control words as a public Resize.
            4
        } else if let PipelineOp::Transform { method, .. } = op {
            // extract_params contributes ten public words; the executor then
            // adds premultiply, method, two projective values, and (for one
            // mesh record) four remaining source-corner values.
            if matches!(method, TransformMethod::Mesh) {
                18
            } else {
                14
            }
        } else if matches!(op, PipelineOp::Color3DLut { .. }) {
            7
        } else {
            registry::extract_params(op).len()
        };
        let param_words = 4usize
            .checked_add(op_param_words)
            .and_then(|words| {
                words.checked_add(usize::from(matches!(op, PipelineOp::Contrast { .. })))
            })
            .and_then(|words| words.checked_add(2))
            .ok_or_else(|| PilError::ValueError("GPU parameter arena size overflow".into()))?;
        let mut total = aligned_bytes(
            param_words
                .checked_mul(std::mem::size_of::<u32>())
                .ok_or_else(|| PilError::ValueError("GPU parameter arena size overflow".into()))?,
            uniform_alignment,
        );

        let image_bytes = |image: &DynamicImage| -> Result<usize, PilError> {
            let (w, h) = image.dimensions();
            Ok(aligned_bytes(
                CheckedDims::new(w, h, 4)?.total_bytes(),
                storage_alignment,
            ))
        };

        if let PipelineOp::PutData { data, mode } = op {
            let pixels = data.len().div_ceil(mode.channels());
            if pixels > buffers.capacity as usize {
                return Err(PilError::ValueError(format!(
                    "GPU buffer capacity {} < putdata image size {}",
                    buffers.capacity, pixels
                )));
            }
            total = total
                .checked_add(aligned_bytes(
                    pixels
                        .checked_mul(std::mem::size_of::<u32>())
                        .ok_or_else(|| {
                            PilError::ValueError("GPU putdata arena size overflow".into())
                        })?,
                    storage_alignment,
                ))
                .ok_or_else(|| PilError::ValueError("GPU auxiliary arena size overflow".into()))?;
        } else {
            if let Some(second) = auxiliary.second.as_ref() {
                total = total.checked_add(image_bytes(second)?).ok_or_else(|| {
                    PilError::ValueError("GPU auxiliary arena size overflow".into())
                })?;
            }
        }
        if let Some(third) = auxiliary.third.as_ref() {
            total = total
                .checked_add(image_bytes(third)?)
                .ok_or_else(|| PilError::ValueError("GPU auxiliary arena size overflow".into()))?;
        }

        if matches!(op, PipelineOp::Autocontrast { .. } | PipelineOp::Equalize) {
            total = total
                .checked_add(GPU_HISTOGRAM_BYTES)
                .ok_or_else(|| PilError::ValueError("GPU histogram arena size overflow".into()))?;
        }

        let resize_coefficients = match op {
            PipelineOp::Resize { w, h, filter } => Some((*w, *h, *filter)),
            PipelineOp::Pad { filter, .. } => {
                gpu_pad_geometry(op, source_dimensions.0, source_dimensions.1)
                    .map(|((resize_w, resize_h), _)| (resize_w, resize_h, *filter))
            }
            _ => None,
        };
        if let Some((resize_w, resize_h, filter)) = resize_coefficients {
            let (source_w, source_h) = source_dimensions;
            let (horizontal_bytes, vertical_bytes) = if (f_resize_f64_is_exact
                || f_resize_f64_ordered_is_exact)
                && matches!(mode, 5 | 7 | 8)
                && !matches!(filter, ResampleFilter::Nearest)
                && matches!(op, PipelineOp::Resize { .. } | PipelineOp::Pad { .. })
            {
                let compact_box = f_resize_f64_ordered_is_exact
                    && mode == 8
                    && matches!(op, PipelineOp::Resize { .. })
                    && matches!(filter, ResampleFilter::Box)
                    && gpu_f_resize_compact_box_any_axis(
                        (source_w, source_h),
                        (resize_w, resize_h),
                    );
                let (kernel, support) = filter_from_resample(filter);
                let axis_words = |source_size: u32, output_size: u32| {
                    if gpu_f_resize_compact_box_axis(source_size, output_size) {
                        usize::try_from(output_size)
                            .ok()
                            .and_then(|count| count.checked_mul(3))
                            .and_then(|metadata| metadata.checked_add(4))
                            .ok_or_else(|| {
                                PilError::ValueError(
                                    "GPU compact Box coefficient arena size overflow".into(),
                                )
                            })
                    } else {
                        let coeffs =
                            precompute_coeffs_f64(output_size, source_size, kernel, support);
                        resize_coeff_word_count_f64(&coeffs)
                    }
                };
                let (horizontal, vertical) = if compact_box {
                    (
                        axis_words(source_w, resize_w)?,
                        axis_words(source_h, resize_h)?,
                    )
                } else {
                    let horizontal = precompute_coeffs_f64(resize_w, source_w, kernel, support);
                    let vertical = precompute_coeffs_f64(resize_h, source_h, kernel, support);
                    (
                        resize_coeff_word_count_f64(&horizontal)?,
                        resize_coeff_word_count_f64(&vertical)?,
                    )
                };
                (horizontal, vertical)
            } else {
                let horizontal = gpu_resize_coefficients(resize_w, source_w, filter);
                let vertical = gpu_resize_coefficients(resize_h, source_h, filter);
                (
                    resize_coeff_word_count(&horizontal)?,
                    resize_coeff_word_count(&vertical)?,
                )
            };
            let horizontal_bytes = horizontal_bytes
                .checked_mul(std::mem::size_of::<u32>())
                .ok_or_else(|| {
                    PilError::ValueError("GPU resize coefficient arena size overflow".into())
                })?;
            let vertical_bytes = vertical_bytes
                .checked_mul(std::mem::size_of::<u32>())
                .ok_or_else(|| {
                    PilError::ValueError("GPU resize coefficient arena size overflow".into())
                })?;
            total = total
                .checked_add(aligned_bytes(horizontal_bytes, storage_alignment))
                .and_then(|value| {
                    value.checked_add(aligned_bytes(vertical_bytes, storage_alignment))
                })
                .ok_or_else(|| {
                    PilError::ValueError("GPU resize coefficient arena size overflow".into())
                })?;
        }

        if let PipelineOp::Fit {
            w,
            h,
            bleed,
            centering,
            filter,
            ..
        } = op
        {
            let (horizontal, vertical) =
                gpu_fit_coefficients(source_dimensions, (*w, *h), *bleed, *centering, *filter)
                    .ok_or_else(|| {
                        PilError::ValueError("GPU Fit has no safe crop geometry".into())
                    })?;
            let horizontal_bytes = resize_coeff_word_count(&horizontal)?
                .checked_mul(std::mem::size_of::<u32>())
                .ok_or_else(|| {
                    PilError::ValueError("GPU Fit coefficient arena size overflow".into())
                })?;
            let vertical_bytes = resize_coeff_word_count(&vertical)?
                .checked_mul(std::mem::size_of::<u32>())
                .ok_or_else(|| {
                    PilError::ValueError("GPU Fit coefficient arena size overflow".into())
                })?;
            total = total
                .checked_add(aligned_bytes(horizontal_bytes, storage_alignment))
                .and_then(|value| {
                    value.checked_add(aligned_bytes(vertical_bytes, storage_alignment))
                })
                .ok_or_else(|| {
                    PilError::ValueError("GPU Fit coefficient arena size overflow".into())
                })?;
        }

        if extract_lut(op, mode).is_some() {
            total = total
                .checked_add(aligned_bytes(
                    256 * std::mem::size_of::<u32>(),
                    storage_alignment,
                ))
                .ok_or_else(|| PilError::ValueError("GPU LUT arena size overflow".into()))?;
        }
        Ok(total)
    }

    fn validate_output_dims(&self, buffers: &BufferPool, w: u32, h: u32) -> Result<(), PilError> {
        let pixels = CheckedDims::new(w, h, 1)?.total_pixels();
        if pixels > buffers.capacity as usize {
            return Err(PilError::ValueError(format!(
                "GPU buffer capacity {} < output image size {}",
                buffers.capacity, pixels
            )));
        }
        Ok(())
    }

    fn upload_auxiliary_cache(
        &self,
        cache: &GpuAuxiliaryCache,
        buffers: &mut BufferPool,
        storage_alignment: usize,
    ) {
        if !cache.img2_values.is_empty() {
            buffers.img2_arena.ensure_capacity(
                &self.device,
                "gpu_batch_img2",
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                cache.img2_values.len() * std::mem::size_of::<u32>(),
                storage_alignment,
            );
            self.queue.write_buffer(
                &buffers.img2_arena.buffer,
                0,
                bytemuck::cast_slice(&cache.img2_values),
            );
        }
        if !cache.img3_values.is_empty() {
            buffers.img3_arena.ensure_capacity(
                &self.device,
                "gpu_batch_img3",
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                cache.img3_values.len() * std::mem::size_of::<u32>(),
                storage_alignment,
            );
            self.queue.write_buffer(
                &buffers.img3_arena.buffer,
                0,
                bytemuck::cast_slice(&cache.img3_values),
            );
        }
        if !cache.lut_values.is_empty() {
            buffers.lut_arena.ensure_capacity(
                &self.device,
                "gpu_batch_lut",
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                cache.lut_values.len() * std::mem::size_of::<u32>(),
                storage_alignment,
            );
            self.queue.write_buffer(
                &buffers.lut_arena.buffer,
                0,
                bytemuck::cast_slice(&cache.lut_values),
            );
        }
    }

    fn append_resize_coeff_ranges(
        &self,
        img2_arena: &mut Vec<u32>,
        auxiliary_cache: &GpuAuxiliaryCache,
        horizontal: &FilterCoeffs,
        vertical: &FilterCoeffs,
        storage_alignment: usize,
    ) -> Result<(BufferRange, BufferRange), PilError> {
        let horizontal_words = encode_resize_coeffs(horizontal)?;
        let vertical_words = encode_resize_coeffs(vertical)?;
        let base_offset = (auxiliary_cache.img2_values.len() * 4) as u64;
        let mut horizontal_range =
            append_arena_slice(img2_arena, &horizontal_words, storage_alignment);
        horizontal_range.offset += base_offset;
        let mut vertical_range = append_arena_slice(img2_arena, &vertical_words, storage_alignment);
        vertical_range.offset += base_offset;
        Ok((horizontal_range, vertical_range))
    }

    fn append_resize_f64_coeff_ranges(
        &self,
        img2_arena: &mut Vec<u32>,
        auxiliary_cache: &GpuAuxiliaryCache,
        horizontal: &FilterCoeffsF64,
        vertical: &FilterCoeffsF64,
        storage_alignment: usize,
    ) -> Result<(BufferRange, BufferRange), PilError> {
        let horizontal_words = encode_resize_coeffs_f64(horizontal)?;
        let vertical_words = encode_resize_coeffs_f64(vertical)?;
        self.append_resize_coeff_word_ranges(
            img2_arena,
            auxiliary_cache,
            &horizontal_words,
            &vertical_words,
            storage_alignment,
        )
    }

    fn append_resize_coeff_word_ranges(
        &self,
        img2_arena: &mut Vec<u32>,
        auxiliary_cache: &GpuAuxiliaryCache,
        horizontal_words: &[u32],
        vertical_words: &[u32],
        storage_alignment: usize,
    ) -> Result<(BufferRange, BufferRange), PilError> {
        let base_offset = (auxiliary_cache.img2_values.len() * 4) as u64;
        let mut horizontal_range =
            append_arena_slice(img2_arena, horizontal_words, storage_alignment);
        horizontal_range.offset += base_offset;
        let mut vertical_range = append_arena_slice(img2_arena, vertical_words, storage_alignment);
        vertical_range.offset += base_offset;
        Ok((horizontal_range, vertical_range))
    }

    fn prepare_batch<'a>(
        &self,
        ops: &[PipelineOp],
        auxiliary_images: &[AuxiliaryImages],
        w: u32,
        h: u32,
        mode: u32,
        logical_mode: Option<&str>,
        contrast_mean: Option<u8>,
        f_resize_constant_bits: Option<u32>,
        f_resize_box_copy_is_exact: bool,
        f_resize_identity_is_exact: bool,
        f_resize_box_average_is_exact: bool,
        f_resize_dyadic_is_exact: bool,
        f_resize_f64_is_exact: bool,
        f_resize_f64_ordered_is_exact: bool,
        buffers: &'a mut BufferPool,
        auxiliary_cache: &GpuAuxiliaryCache,
    ) -> Result<PreparedGpuBatch<'a>, PilError> {
        if ops.len() != auxiliary_images.len() {
            return Err(PilError::InternalError(
                "GPU operation and auxiliary-image counts differ".into(),
            ));
        }

        let limits = self.device.limits();
        let uniform_alignment = limits.min_uniform_buffer_offset_alignment as usize;
        let storage_alignment = limits.min_storage_buffer_offset_alignment as usize;
        let mut params_arena = Vec::new();
        let mut params_ranges = Vec::with_capacity(ops.len());
        let mut img2_arena = Vec::new();
        let mut img2_ranges = Vec::with_capacity(ops.len());
        let mut img3_arena = Vec::new();
        let mut img3_ranges = Vec::with_capacity(ops.len());
        let mut lut_arena = Vec::new();
        let mut lut_ranges = Vec::with_capacity(ops.len());
        let mut second_cache: HashMap<usize, BufferRange> = HashMap::new();
        let mut third_cache: HashMap<usize, BufferRange> = HashMap::new();
        let mut lut_cache: HashMap<[u32; 256], BufferRange> = HashMap::new();
        let mut resize_coeff_ranges = Vec::with_capacity(ops.len());
        let mut input_dims = Vec::with_capacity(ops.len());
        let mut output_dims = Vec::with_capacity(ops.len());
        let mut pad_resize_dims = Vec::with_capacity(ops.len());
        let mut cur_w = w;
        let mut cur_h = h;
        let mut current_mode = mode;

        for (index, op) in ops.iter().enumerate() {
            let cached = if can_fuse_gpu_multiply_screen(ops, index) {
                self.resolve_pipeline(
                    "__internal_multiply_screen",
                    "multiply_screen.wgsl",
                    include_str!("shaders/multiply_screen.wgsl"),
                )?
            } else if matches!(
                &ops[index],
                PipelineOp::Autocontrast { .. } | PipelineOp::Equalize
            ) {
                // Histogram-driven operations resolve to a multi-pass plan
                // during encoding. Use the clear pass here because it has
                // the same image-sized output contract as the public op,
                // while the operation-specific cutoff/CDF parameters are
                // still appended below.
                self.resolve_pipeline(
                    "__internal_histogram_clear",
                    "histogram_clear.wgsl",
                    include_str!("shaders/histogram_clear.wgsl"),
                )?
            } else if Self::blur_pass_count(op).is_some() {
                self.resolve_pipeline(
                    "__internal_blur_h",
                    "box_blur_h.wgsl",
                    include_str!("shaders/box_blur_h.wgsl"),
                )?
            } else if matches!(op, PipelineOp::Pad { .. }) {
                // Pad's first device pass is the exact separable resize.  The
                // final placement shader is resolved alongside it during
                // encode_batch; using the resize variant here makes the
                // coefficient range and uniform preparation follow the same
                // binding contract as public Resize.
                self.resolve_pipeline(
                    "__internal_resize_h",
                    "resize_convolution_h.wgsl",
                    include_str!("shaders/resize_convolution_h.wgsl"),
                )?
            } else if matches!(op, PipelineOp::Fit { .. }) {
                // Fit's public crop box is fractional, but its exact byte
                // implementation is the same boxed two-pass convolution as
                // Resize once the host has supplied the coefficient tables.
                self.resolve_pipeline(
                    "__internal_resize_h",
                    "resize_convolution_h.wgsl",
                    include_str!("shaders/resize_convolution_h.wgsl"),
                )?
            } else if matches!(op, PipelineOp::Resize { filter, .. }
                if !matches!(filter, ResampleFilter::Nearest)
                    || gpu_resize_nearest_uses_coefficients(logical_mode))
            {
                self.resolve_pipeline(
                    "__internal_resize_h",
                    "resize_convolution_h.wgsl",
                    include_str!("shaders/resize_convolution_h.wgsl"),
                )?
            } else {
                let base_key = registry::variant_key(op);
                let entry = registry::registry()?.get(base_key).ok_or_else(|| {
                    PilError::ValueError(format!(
                        "GPU operation '{base_key}' has no registered shader entry"
                    ))
                })?;
                let source = entry.gpu_source.ok_or_else(|| {
                    PilError::ValueError(format!(
                        "GPU operation '{base_key}' has no registered shader source"
                    ))
                })?;
                let shader_file = entry.gpu_shader.ok_or_else(|| {
                    PilError::ValueError(format!(
                        "GPU operation '{base_key}' has no registered shader name"
                    ))
                })?;
                self.resolve_pipeline(base_key, shader_file, source)?
            };
            let (out_w, out_h) = op_output_dims(op, cur_w, cur_h).unwrap_or((cur_w, cur_h));
            self.validate_output_dims(buffers, out_w, out_h)?;
            input_dims.push((cur_w, cur_h));
            output_dims.push((out_w, out_h));

            // Transpose's swap variants and output-only generators describe
            // their dispatch dimensions in the first uniform words. Ordinary
            // kernels use the current source dimensions there and consume the
            // appended output dimensions only when their shader declares them.
            let (shader_w, shader_h) = if cached.is_output_only
                || matches!(
                    op,
                    PipelineOp::Transpose {
                        method: TransposeMethod::Rotate90
                            | TransposeMethod::Rotate270
                            | TransposeMethod::Transpose
                            | TransposeMethod::Transverse
                    }
                ) {
                (out_w, out_h)
            } else {
                (cur_w, cur_h)
            };
            // Reduce's mode word distinguishes ordinary four-byte samples
            // from RGBA/LA alpha layouts. RGBa and RGBX share the physical
            // RGBA transport, but Pillow averages all four stored channels
            // directly; use the raw-four-channel Reduce encoding only for
            // that operation while retaining the source mode code for the
            // rest of the batch.
            let op_mode = if matches!(op, PipelineOp::Reduce { .. })
                && matches!(logical_mode, Some("RGBa" | "RGBX"))
            {
                4
            } else {
                current_mode
            };
            let mut params = vec![shader_w, shader_h, op_mode, 0u32];
            if matches!(
                op,
                PipelineOp::Resize {
                    filter: ResampleFilter::Nearest,
                    ..
                }
            ) {
                // F-mode nearest samples are opaque f32 words. Use the
                // coefficient path's host-generated one-tap table so its
                // source row/column follows Pillow's cumulative f64 affine
                // walk; the marker selects an exact word copy in WGSL and
                // preserves NaN, infinity, and signed zero.
                let nearest_mode = if logical_mode == Some("F") {
                    7
                } else {
                    u32::from(gpu_resize_should_premultiply(
                        op_mode,
                        logical_mode,
                        ResampleFilter::Nearest,
                    ))
                };
                params.extend([
                    out_w,
                    out_h,
                    gpu_resize_channel_count(op_mode),
                    nearest_mode,
                ]);
            } else if let PipelineOp::Resize { filter, .. } = op {
                debug_assert!(!matches!(filter, ResampleFilter::Nearest));
                // F-mode constant images are lowered by the mode-8 shader as
                // an exact bit-pattern fill.  `channels` is unused by that
                // branch and `premultiply` is false for F, so carry the
                // scalar without changing the shared uniform ABI.
                let constant_bits = f_resize_constant_bits.filter(|_| {
                    logical_mode == Some("F") && !matches!(filter, ResampleFilter::Nearest)
                });
                let box_copy_is_exact = f_resize_box_copy_is_exact
                    && logical_mode == Some("F")
                    && matches!(filter, ResampleFilter::Box);
                let identity_is_exact = f_resize_identity_is_exact && logical_mode == Some("F");
                let box_average_is_exact = f_resize_box_average_is_exact
                    && logical_mode == Some("F")
                    && matches!(filter, ResampleFilter::Box);
                let dyadic_is_exact = f_resize_dyadic_is_exact
                    && logical_mode == Some("F")
                    && matches!(
                        filter,
                        ResampleFilter::Bilinear
                            | ResampleFilter::Bicubic
                            | ResampleFilter::Lanczos
                            | ResampleFilter::Hamming
                            | ResampleFilter::Box
                    );
                let f64_is_exact = f_resize_f64_is_exact
                    && logical_mode == Some("F")
                    && !matches!(filter, ResampleFilter::Nearest);
                let f64_ordered_is_exact = f_resize_f64_ordered_is_exact
                    && logical_mode == Some("F")
                    && !matches!(filter, ResampleFilter::Nearest);
                let compact_box_is_exact = f64_ordered_is_exact
                    && matches!(filter, ResampleFilter::Box)
                    && gpu_f_resize_compact_box_any_axis((cur_w, cur_h), (out_w, out_h));
                let i_f64_is_exact = f_resize_f64_is_exact
                    && logical_mode == Some("I")
                    && !matches!(filter, ResampleFilter::Nearest);
                let luma16_f64_is_exact = f_resize_f64_is_exact
                    && op_mode == 5
                    && logical_mode
                        .is_none_or(|mode| matches!(mode, "I;16" | "I;16L" | "I;16B" | "I;16N"))
                    && !matches!(filter, ResampleFilter::Nearest);
                let coefficient_channels =
                    constant_bits.unwrap_or_else(|| gpu_resize_channel_count(op_mode));
                params.extend([
                    out_w,
                    out_h,
                    coefficient_channels,
                    if constant_bits.is_some() {
                        2
                    } else if box_copy_is_exact {
                        3
                    } else if identity_is_exact {
                        5
                    } else if box_average_is_exact {
                        4
                    } else if compact_box_is_exact {
                        13
                    } else if f64_ordered_is_exact {
                        12
                    } else if f64_is_exact {
                        9
                    } else if i_f64_is_exact {
                        11
                    } else if luma16_f64_is_exact {
                        10
                    } else if dyadic_is_exact {
                        6
                    } else {
                        u32::from(gpu_resize_should_premultiply(
                            op_mode,
                            logical_mode,
                            *filter,
                        ))
                    },
                ]);
            } else if let PipelineOp::Pad { filter, .. } = op {
                let ((resize_w, resize_h), (offset_x, offset_y)) =
                    gpu_pad_geometry(op, cur_w, cur_h).ok_or_else(|| {
                        PilError::ValueError("GPU Pad has no safe geometry".into())
                    })?;
                let resize_filter = if logical_mode == Some("P") {
                    ResampleFilter::Nearest
                } else {
                    *filter
                };
                let fill = gpu_pad_fill(op, logical_mode, op_mode);
                let constant_bits = f_resize_constant_bits.filter(|_| {
                    logical_mode == Some("F") && !matches!(resize_filter, ResampleFilter::Nearest)
                });
                let f64_is_exact = f_resize_f64_is_exact
                    && logical_mode == Some("F")
                    && !matches!(resize_filter, ResampleFilter::Nearest);
                params.extend([
                    resize_w,
                    resize_h,
                    constant_bits.unwrap_or_else(|| gpu_resize_channel_count(op_mode)),
                    if constant_bits.is_some() {
                        2
                    } else if f64_is_exact {
                        9
                    } else {
                        u32::from(gpu_resize_should_premultiply(
                            op_mode,
                            logical_mode,
                            resize_filter,
                        ))
                    },
                    out_w,
                    out_h,
                    fill,
                    offset_x,
                    offset_y,
                ]);
            } else if let PipelineOp::Autocontrast { cutoff, .. } = op {
                let selected_pixels = gpu_autocontrast_selected_pixels(
                    cur_w,
                    cur_h,
                    auxiliary_images[index].third.as_deref(),
                )?;
                let (cutoff_low, cutoff_high) =
                    gpu_autocontrast_cutoff_indices(selected_pixels, *cutoff);
                params[3] = u32::from(auxiliary_images[index].third.is_some());
                // Keep the original cutoff bits for receipt/debugging
                // compatibility; the exact integer thresholds below avoid
                // f32 cutoff rounding inside the device control pass.
                params.extend([(*cutoff as f32).to_bits(), cutoff_low, cutoff_high]);
                params.push(selected_pixels.min(u32::MAX as usize) as u32);
            } else if let PipelineOp::Transform {
                filter,
                method,
                data,
                ..
            } = op
            {
                let mut transform_params = registry::extract_params(op);
                // Pillow's affine nearest kernel quantizes its coefficients
                // to signed 16.16 once, then advances integer coordinates
                // across each output row.  Carry those fixed-point values
                // in the six affine slots for the native GPU path instead
                // of reconstructing them from f32 values in the shader.  The
                // slots are still consumed as f32 for bilinear/projective
                // transforms, so this is an ABI-preserving mode-specific
                // encoding.
                if matches!(method, TransformMethod::Affine)
                    && gpu_transform_uses_nearest(logical_mode, *filter)
                {
                    let fixed = |value: f64| (value.mul_add(65_536.0, 0.5).floor() as i64) as u32;
                    let a = data.first().copied().unwrap_or(0.0);
                    let b = data.get(1).copied().unwrap_or(0.0);
                    let c = data.get(2).copied().unwrap_or(0.0);
                    let d = data.get(3).copied().unwrap_or(0.0);
                    let e = data.get(4).copied().unwrap_or(0.0);
                    let f = data.get(5).copied().unwrap_or(0.0);
                    let (origin_x, origin_y) = if logical_mode
                        .is_some_and(|mode| matches!(mode, "I;16" | "I;16L" | "I;16B" | "I;16N"))
                    {
                        // Geometry.c's native I;16 affine path evaluates the
                        // source coordinate at the integer destination pixel
                        // and then applies floor(value + 0.5).  Unlike the
                        // byte/I/F affine path it does not add half of each
                        // affine step before the fixed-point walk.
                        (fixed(c + 0.5), fixed(f + 0.5))
                    } else {
                        (fixed(c + a * 0.5 + b * 0.5), fixed(f + d * 0.5 + e * 0.5))
                    };
                    transform_params[2..8].copy_from_slice(&[
                        fixed(a),
                        fixed(b),
                        origin_x,
                        fixed(d),
                        fixed(e),
                        origin_y,
                    ]);
                }
                // `extract_params` deliberately contains only the public
                // operation values.  The GPU transport additionally needs
                // the scalar decision that mirrors Image.transform's
                // premultiplied-alpha round trip; keep that decision in the
                // control plane and let the shader own every pixel sample.
                transform_params[8] = gpu_transform_fill(op, logical_mode, op_mode);
                if gpu_transform_uses_nearest(logical_mode, *filter) {
                    transform_params[9] = 0;
                }
                params.extend(transform_params);
                params.push(u32::from(gpu_transform_should_premultiply(
                    op_mode,
                    logical_mode,
                    *filter,
                    method.clone(),
                )));
                // The affine shader historically used this final word as
                // padding. Reuse it as the method selector and append the
                // two projective coefficients/mesh tail values below so one
                // packed uniform contract covers every bounded transform
                // family without a second dispatch or auxiliary image.
                let method_code = match method {
                    TransformMethod::Affine => 0,
                    TransformMethod::Perspective => 1,
                    TransformMethod::Quad => 2,
                    TransformMethod::Mesh => 3,
                };
                params.push(method_code);
                params.push((data.get(6).copied().unwrap_or(0.0) as f32).to_bits());
                params.push((data.get(7).copied().unwrap_or(0.0) as f32).to_bits());
                if matches!(method, TransformMethod::Mesh) {
                    for value in data.get(8..12).unwrap_or(&[]) {
                        params.push((*value as f32).to_bits());
                    }
                }
            } else if let PipelineOp::Color3DLut {
                size,
                channels,
                source_mode,
                ..
            } = op
            {
                let scale = |extent: u32| {
                    ((f64::from(extent.saturating_sub(1)) / 255.0) * f64::from(1u32 << 18)) as u32
                };
                params.extend([
                    size.0,
                    size.1,
                    size.2,
                    *channels,
                    scale(size.0),
                    scale(size.1),
                    scale(size.2),
                ]);
                params[2] = gpu_pixel_mode_code(*source_mode).unwrap_or(op_mode);
            } else if let PipelineOp::Fit { filter, .. } = op {
                // Fit's fractional crop is represented exactly by its boxed
                // coefficient ranges.  The convolution shaders therefore
                // consume the same control words as Resize; no source pixels
                // are materialized on the host and no f32 crop mapping is
                // performed by a shader.
                let nearest_mode =
                    if logical_mode == Some("F") && matches!(filter, ResampleFilter::Nearest) {
                        // F-mode nearest Fit uses the host-generated one-tap
                        // table for the boxed affine mapping. Copy complete
                        // words so NaN, infinity, and signed zero survive.
                        7
                    } else {
                        u32::from(gpu_resize_should_premultiply(
                            op_mode,
                            logical_mode,
                            *filter,
                        ))
                    };
                params.extend([
                    out_w,
                    out_h,
                    gpu_resize_channel_count(op_mode),
                    nearest_mode,
                ]);
            } else {
                params.extend(registry::extract_params(op));
            }
            if matches!(op, PipelineOp::Contrast { .. }) {
                params.push(u32::from(contrast_mean.ok_or_else(|| {
                    PilError::ValueError("GPU Contrast is missing its scalar midpoint".into())
                })?));
            }
            // Shaders that declare dst_w/dst_h at the end of Params read these
            // words; shaders without them ignore the trailing words.
            params.extend([out_w, out_h]);
            params_ranges.push(append_arena_slice(
                &mut params_arena,
                &params,
                uniform_alignment,
            ));

            let resize_coeff_range = match op {
                PipelineOp::Resize { filter, .. } => {
                    let uses_coefficients = !matches!(filter, ResampleFilter::Nearest)
                        || gpu_resize_nearest_uses_coefficients(logical_mode);
                    if !uses_coefficients {
                        None
                    } else {
                        let horizontal = gpu_resize_coefficients(out_w, cur_w, *filter);
                        let vertical = gpu_resize_coefficients(out_h, cur_h, *filter);
                        if (f_resize_f64_is_exact || f_resize_f64_ordered_is_exact)
                            && !matches!(filter, ResampleFilter::Nearest)
                            && (logical_mode == Some("F")
                                || (op_mode == 5
                                    && logical_mode.is_none_or(|mode| {
                                        matches!(mode, "I;16" | "I;16L" | "I;16B" | "I;16N")
                                    }))
                                || (op_mode == 7 && logical_mode == Some("I")))
                        {
                            let (kernel, support) = filter_from_resample(*filter);
                            let compact_box = f_resize_f64_ordered_is_exact
                                && logical_mode == Some("F")
                                && matches!(filter, ResampleFilter::Box)
                                && gpu_f_resize_compact_box_any_axis(
                                    (cur_w, cur_h),
                                    (out_w, out_h),
                                );
                            if compact_box {
                                let horizontal_words =
                                    if gpu_f_resize_compact_box_axis(cur_w, out_w) {
                                        encode_resize_compact_box_axis(cur_w, out_w)?
                                    } else {
                                        let coeffs =
                                            precompute_coeffs_f64(out_w, cur_w, kernel, support);
                                        encode_resize_coeffs_f64(&coeffs)?
                                    };
                                let vertical_words = if gpu_f_resize_compact_box_axis(cur_h, out_h)
                                {
                                    encode_resize_compact_box_axis(cur_h, out_h)?
                                } else {
                                    let coeffs =
                                        precompute_coeffs_f64(out_h, cur_h, kernel, support);
                                    encode_resize_coeffs_f64(&coeffs)?
                                };
                                Some(self.append_resize_coeff_word_ranges(
                                    &mut img2_arena,
                                    auxiliary_cache,
                                    &horizontal_words,
                                    &vertical_words,
                                    storage_alignment,
                                )?)
                            } else {
                                let horizontal_f64 =
                                    precompute_coeffs_f64(out_w, cur_w, kernel, support);
                                let vertical_f64 =
                                    precompute_coeffs_f64(out_h, cur_h, kernel, support);
                                Some(self.append_resize_f64_coeff_ranges(
                                    &mut img2_arena,
                                    auxiliary_cache,
                                    &horizontal_f64,
                                    &vertical_f64,
                                    storage_alignment,
                                )?)
                            }
                        } else {
                            Some(self.append_resize_coeff_ranges(
                                &mut img2_arena,
                                auxiliary_cache,
                                &horizontal,
                                &vertical,
                                storage_alignment,
                            )?)
                        }
                    }
                }
                PipelineOp::Pad { filter, .. } => {
                    let ((resize_w, resize_h), _) =
                        gpu_pad_geometry(op, cur_w, cur_h).ok_or_else(|| {
                            PilError::ValueError("GPU Pad has no safe geometry".into())
                        })?;
                    let resize_filter = if logical_mode == Some("P") {
                        ResampleFilter::Nearest
                    } else {
                        *filter
                    };
                    if f_resize_f64_is_exact
                        && logical_mode == Some("F")
                        && !matches!(resize_filter, ResampleFilter::Nearest)
                    {
                        let (kernel, support) = filter_from_resample(resize_filter);
                        let horizontal = precompute_coeffs_f64(resize_w, cur_w, kernel, support);
                        let vertical = precompute_coeffs_f64(resize_h, cur_h, kernel, support);
                        Some(self.append_resize_f64_coeff_ranges(
                            &mut img2_arena,
                            auxiliary_cache,
                            &horizontal,
                            &vertical,
                            storage_alignment,
                        )?)
                    } else {
                        let horizontal = gpu_resize_coefficients(resize_w, cur_w, resize_filter);
                        let vertical = gpu_resize_coefficients(resize_h, cur_h, resize_filter);
                        Some(self.append_resize_coeff_ranges(
                            &mut img2_arena,
                            auxiliary_cache,
                            &horizontal,
                            &vertical,
                            storage_alignment,
                        )?)
                    }
                }
                PipelineOp::Fit {
                    bleed,
                    centering,
                    filter,
                    ..
                } => {
                    let (horizontal, vertical) = gpu_fit_coefficients(
                        (cur_w, cur_h),
                        (out_w, out_h),
                        *bleed,
                        *centering,
                        *filter,
                    )
                    .ok_or_else(|| {
                        PilError::ValueError("GPU Fit has no safe crop geometry".into())
                    })?;
                    let ranges = self.append_resize_coeff_ranges(
                        &mut img2_arena,
                        auxiliary_cache,
                        &horizontal,
                        &vertical,
                        storage_alignment,
                    )?;
                    Some(ranges)
                }
                _ => None,
            };
            resize_coeff_ranges.push(resize_coeff_range);
            pad_resize_dims.push(
                matches!(op, PipelineOp::Pad { .. })
                    .then(|| gpu_pad_geometry(op, cur_w, cur_h).map(|(dims, _)| dims))
                    .flatten(),
            );

            let second_range = if let PipelineOp::PutData { data, mode } = op {
                let second_values = pack_put_data(data, *mode, buffers.capacity)?;
                if second_values.is_empty() {
                    None
                } else {
                    let mut range =
                        append_arena_slice(&mut img2_arena, &second_values, storage_alignment);
                    range.offset += (auxiliary_cache.img2_values.len() * 4) as u64;
                    Some(range)
                }
            } else if let Some(second) = auxiliary_images[index].second.as_ref() {
                if gpu_luma16_paste_source(op, op_mode, second) {
                    let DynamicImage::ImageLuma16(source) = second.as_ref() else {
                        return Err(PilError::InternalError(
                            "GPU typed Paste source was admitted without an ImageLuma16 buffer"
                                .into(),
                        ));
                    };
                    let values = pack_luma16_numeric(source, buffers.capacity)?;
                    let mut range = append_arena_slice(&mut img2_arena, &values, storage_alignment);
                    range.offset += (auxiliary_cache.img2_values.len() * 4) as u64;
                    Some(range)
                } else {
                    let key = Arc::as_ptr(second) as usize;
                    if let Some(range) = auxiliary_cache.second_ranges.get(&key).copied() {
                        Some(range)
                    } else if let Some(range) = second_cache.get(&key).copied() {
                        Some(range)
                    } else {
                        let values = pack_rgba(&second.to_rgba8(), buffers.capacity)?;
                        let mut range =
                            append_arena_slice(&mut img2_arena, &values, storage_alignment);
                        range.offset += (auxiliary_cache.img2_values.len() * 4) as u64;
                        second_cache.insert(key, range);
                        Some(range)
                    }
                }
            } else {
                None
            };
            img2_ranges.push(second_range);

            let third_range = if let Some(third) = auxiliary_images[index].third.as_ref() {
                let key = Arc::as_ptr(third) as usize;
                if let Some(range) = auxiliary_cache.third_ranges.get(&key).copied() {
                    Some(range)
                } else if let Some(range) = third_cache.get(&key).copied() {
                    Some(range)
                } else {
                    let values = pack_rgba(&third.to_rgba8(), buffers.capacity)?;
                    let mut range = append_arena_slice(&mut img3_arena, &values, storage_alignment);
                    range.offset += (auxiliary_cache.img3_values.len() * 4) as u64;
                    third_cache.insert(key, range);
                    Some(range)
                }
            } else {
                None
            };
            img3_ranges.push(third_range);

            let lut_values = if cached.is_lut
                && !matches!(
                    cached.variant_name,
                    "__internal_resize_h" | "__internal_resize_v"
                ) {
                Some(extract_lut(op, op_mode).ok_or_else(|| {
                    PilError::ValueError(format!(
                        "GPU LUT length does not match source mode for '{}'",
                        registry::variant_key(op)
                    ))
                })?)
            } else {
                None
            };
            lut_ranges.push(lut_values.map(|lut| {
                if let Some(range) = auxiliary_cache.lut_ranges.get(&lut).copied() {
                    range
                } else if let Some(range) = lut_cache.get(&lut).copied() {
                    range
                } else {
                    let mut range = append_arena_slice(&mut lut_arena, &lut, storage_alignment);
                    range.offset += (auxiliary_cache.lut_values.len() * 4) as u64;
                    lut_cache.insert(lut, range);
                    range
                }
            }));

            cur_w = out_w;
            cur_h = out_h;
            current_mode = gpu_mode_after_op(current_mode, op);
        }

        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_batch_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            params_arena
                .len()
                .saturating_mul(std::mem::size_of::<u32>()),
            uniform_alignment,
        );
        self.queue.write_buffer(
            &buffers.params_arena.buffer,
            0,
            bytemuck::cast_slice(&params_arena),
        );

        let img2 = if auxiliary_cache.img2_values.is_empty() && img2_arena.is_empty() {
            None
        } else {
            let previous_capacity = buffers.img2_arena.capacity_bytes;
            buffers.img2_arena.ensure_capacity(
                &self.device,
                "gpu_batch_img2",
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                (auxiliary_cache.img2_values.len() + img2_arena.len())
                    .saturating_mul(std::mem::size_of::<u32>()),
                storage_alignment,
            );
            if buffers.img2_arena.capacity_bytes != previous_capacity
                && !auxiliary_cache.img2_values.is_empty()
            {
                self.queue.write_buffer(
                    &buffers.img2_arena.buffer,
                    0,
                    bytemuck::cast_slice(&auxiliary_cache.img2_values),
                );
            }
            if !img2_arena.is_empty() {
                self.queue.write_buffer(
                    &buffers.img2_arena.buffer,
                    (auxiliary_cache.img2_values.len() * 4) as u64,
                    bytemuck::cast_slice(&img2_arena),
                );
            }
            Some(&buffers.img2_arena.buffer)
        };
        let img3 = if auxiliary_cache.img3_values.is_empty() && img3_arena.is_empty() {
            None
        } else {
            let previous_capacity = buffers.img3_arena.capacity_bytes;
            buffers.img3_arena.ensure_capacity(
                &self.device,
                "gpu_batch_img3",
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                (auxiliary_cache.img3_values.len() + img3_arena.len())
                    .saturating_mul(std::mem::size_of::<u32>()),
                storage_alignment,
            );
            if buffers.img3_arena.capacity_bytes != previous_capacity
                && !auxiliary_cache.img3_values.is_empty()
            {
                self.queue.write_buffer(
                    &buffers.img3_arena.buffer,
                    0,
                    bytemuck::cast_slice(&auxiliary_cache.img3_values),
                );
            }
            if !img3_arena.is_empty() {
                self.queue.write_buffer(
                    &buffers.img3_arena.buffer,
                    (auxiliary_cache.img3_values.len() * 4) as u64,
                    bytemuck::cast_slice(&img3_arena),
                );
            }
            Some(&buffers.img3_arena.buffer)
        };
        let lut = if auxiliary_cache.lut_values.is_empty() && lut_arena.is_empty() {
            None
        } else {
            let previous_capacity = buffers.lut_arena.capacity_bytes;
            buffers.lut_arena.ensure_capacity(
                &self.device,
                "gpu_batch_lut",
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                (auxiliary_cache.lut_values.len() + lut_arena.len())
                    .saturating_mul(std::mem::size_of::<u32>()),
                storage_alignment,
            );
            if buffers.lut_arena.capacity_bytes != previous_capacity
                && !auxiliary_cache.lut_values.is_empty()
            {
                self.queue.write_buffer(
                    &buffers.lut_arena.buffer,
                    0,
                    bytemuck::cast_slice(&auxiliary_cache.lut_values),
                );
            }
            if !lut_arena.is_empty() {
                self.queue.write_buffer(
                    &buffers.lut_arena.buffer,
                    (auxiliary_cache.lut_values.len() * 4) as u64,
                    bytemuck::cast_slice(&lut_arena),
                );
            }
            Some(&buffers.lut_arena.buffer)
        };

        let resource_telemetry = PipelineResourceTelemetry {
            parameter_bytes: (params_arena.len() * std::mem::size_of::<u32>()) as u64,
            auxiliary_bytes: ((img2_arena.len() + img3_arena.len() + lut_arena.len())
                * std::mem::size_of::<u32>()) as u64,
            ..PipelineResourceTelemetry::default()
        };

        Ok(PreparedGpuBatch {
            resources: GpuBatchResources {
                buf_a: &buffers.buf_a,
                buf_b: &buffers.buf_b,
                fallback_img2: &buffers.buf_img2,
                fallback_img3: &buffers.buf_img3,
                fallback_lut: &buffers.lut_buf,
                histogram: &buffers.histogram_buf,
                params: &buffers.params_arena.buffer,
                params_ranges,
                img2,
                img2_ranges,
                resize_coeff_ranges,
                img3,
                img3_ranges,
                lut,
                lut_ranges,
            },
            input_dims,
            output_dims,
            pad_resize_dims,
            final_dims: (cur_w, cur_h),
            resource_telemetry,
        })
    }

    fn encode_dispatch(
        &self,
        cpass: &mut wgpu::ComputePass<'_>,
        cached: &CachedPipeline,
        index: usize,
        current_is_a: bool,
        resources: &GpuBatchResources,
        input_dims: (u32, u32),
        output_dims: (u32, u32),
    ) -> Result<bool, PilError> {
        let (input_buf, output_buf) = if current_is_a {
            (resources.buf_a, resources.buf_b)
        } else {
            (resources.buf_b, resources.buf_a)
        };
        let bind_group = self.make_bind_group(cached, input_buf, output_buf, resources, index)?;
        cpass.set_pipeline(&cached.pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        let (dispatch_w, dispatch_h) = match cached.variant_name {
            "__internal_blur_h" => (1, output_dims.1),
            "__internal_blur_v" => (output_dims.0, 1),
            "__internal_resize_h" => (output_dims.0.div_ceil(16), input_dims.1.div_ceil(16)),
            "__internal_resize_v" => (output_dims.0.div_ceil(16), output_dims.1.div_ceil(16)),
            "__internal_histogram_clear"
            | "__internal_autocontrast_histogram"
            | "__internal_equalize_histogram"
            | "__internal_autocontrast_lut"
            | "__internal_equalize_lut" => (1, 1),
            _ => (output_dims.0.div_ceil(16), output_dims.1.div_ceil(16)),
        };
        crate::compute::record_gpu_shader_dispatch(
            cached.variant_name,
            cached.shader_file,
            u64::from(dispatch_w).saturating_mul(u64::from(dispatch_h)),
        );
        cpass.dispatch_workgroups(dispatch_w, dispatch_h, 1);
        let keeps_image_buffer = matches!(
            cached.variant_name,
            "__internal_histogram_clear"
                | "__internal_autocontrast_histogram"
                | "__internal_equalize_histogram"
                | "__internal_autocontrast_lut"
                | "__internal_equalize_lut"
        );
        Ok(if cached.is_in_place || keeps_image_buffer {
            current_is_a
        } else {
            !current_is_a
        })
    }

    fn blur_pass_count(op: &PipelineOp) -> Option<usize> {
        match op {
            PipelineOp::BoxBlur { .. } => Some(1),
            PipelineOp::BoxBlurXY { passes, .. } => Some((*passes).max(1) as usize),
            PipelineOp::GaussianBlur { .. } => Some(3),
            _ => None,
        }
    }

    fn encode_batch(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        ops: &[PipelineOp],
        prepared: &PreparedGpuBatch,
        start_is_a: bool,
        logical_mode: Option<&str>,
    ) -> Result<bool, PilError> {
        let resolved = self.resolve_batch_pipelines(ops, logical_mode)?;
        let mut current_is_a = start_is_a;
        for (index, pipeline) in resolved.iter().enumerate() {
            if matches!(pipeline, ResolvedPipeline::Skip) {
                continue;
            }
            // Metal requires an explicit compute-pass boundary between the
            // horizontal and vertical typed resize passes.  Both passes
            // ping-pong the same two storage buffers; keeping them in one
            // pass leaves the second dispatch racing the first on some
            // drivers, producing a stale row while the host coefficients
            // themselves remain exact.  Keep the boundary for the F-mode
            // exact-arithmetic path as well as the historical I-mode nearest
            // path; the ordinary byte path retains its grouped pass for the
            // existing throughput contract.
            if matches!(pipeline, ResolvedPipeline::Resize { .. })
                && (logical_mode == Some("F")
                    && matches!(
                        ops.get(index),
                        Some(
                            PipelineOp::Resize { .. }
                                | PipelineOp::Fit {
                                    filter: ResampleFilter::Nearest,
                                    ..
                                }
                        )
                    )
                    || logical_mode == Some("I")
                        && matches!(ops.get(index), Some(PipelineOp::Resize { .. }))
                    || matches!(logical_mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
                        && matches!(ops.get(index), Some(PipelineOp::Resize { .. })))
            {
                let ResolvedPipeline::Resize {
                    horizontal,
                    vertical,
                } = pipeline
                else {
                    unreachable!("typed resize pipeline shape changed")
                };
                let compact_vertical = ops.len() == 1
                    && gpu_f_resize_compact_box_vertical_only_geometry(
                        &ops[index],
                        prepared.input_dims[index],
                        prepared.output_dims[index],
                        logical_mode,
                    );
                if !compact_vertical {
                    let mut resize_pass =
                        encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                            label: Some("gpu_batch_compute_resize_h"),
                            timestamp_writes: None,
                        });
                    current_is_a = self.encode_dispatch(
                        &mut resize_pass,
                        horizontal,
                        index,
                        current_is_a,
                        &prepared.resources,
                        prepared.input_dims[index],
                        prepared.output_dims[index],
                    )?;
                }
                {
                    let mut resize_pass =
                        encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                            label: Some("gpu_batch_compute_resize_v"),
                            timestamp_writes: None,
                        });
                    current_is_a = self.encode_dispatch(
                        &mut resize_pass,
                        vertical,
                        index,
                        current_is_a,
                        &prepared.resources,
                        prepared.input_dims[index],
                        prepared.output_dims[index],
                    )?;
                }
                continue;
            }
            // Typed F/I Pad has the same intermediate dependency as a
            // Resize, but also appends a placement dispatch. Keep each stage
            // in its own compute pass so Metal cannot expose a stale
            // horizontal/vertical intermediate to the next stage.
            if matches!(logical_mode, Some("F" | "I"))
                && matches!(pipeline, ResolvedPipeline::Pad { .. })
                && matches!(
                    ops.get(index),
                    Some(PipelineOp::Pad {
                        filter,
                        ..
                    }) if (logical_mode == Some("I")
                        && matches!(filter, ResampleFilter::Nearest))
                        || (logical_mode == Some("F")
                            && !matches!(filter, ResampleFilter::Nearest))
                )
            {
                let ResolvedPipeline::Pad {
                    horizontal,
                    vertical,
                    place,
                } = pipeline
                else {
                    unreachable!("typed I Pad pipeline shape changed")
                };
                let resize_dims = prepared.pad_resize_dims[index].ok_or_else(|| {
                    PilError::InternalError("GPU I Pad is missing its contain dimensions".into())
                })?;
                if resize_dims != prepared.input_dims[index] {
                    {
                        let mut resize_pass =
                            encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                                label: Some("gpu_batch_compute_typed_pad_resize_h"),
                                timestamp_writes: None,
                            });
                        current_is_a = self.encode_dispatch(
                            &mut resize_pass,
                            horizontal,
                            index,
                            current_is_a,
                            &prepared.resources,
                            prepared.input_dims[index],
                            resize_dims,
                        )?;
                    }
                    {
                        let mut resize_pass =
                            encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                                label: Some("gpu_batch_compute_typed_pad_resize_v"),
                                timestamp_writes: None,
                            });
                        current_is_a = self.encode_dispatch(
                            &mut resize_pass,
                            vertical,
                            index,
                            current_is_a,
                            &prepared.resources,
                            prepared.input_dims[index],
                            resize_dims,
                        )?;
                    }
                }
                {
                    let mut place_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: Some("gpu_batch_compute_typed_pad_place"),
                        timestamp_writes: None,
                    });
                    current_is_a = self.encode_dispatch(
                        &mut place_pass,
                        place,
                        index,
                        current_is_a,
                        &prepared.resources,
                        resize_dims,
                        prepared.output_dims[index],
                    )?;
                }
                continue;
            }
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_batch_compute"),
                timestamp_writes: None,
            });
            if let ResolvedPipeline::Blur {
                horizontal,
                vertical,
                pass_count,
            } = pipeline
            {
                // Pillow's GaussianBlur is three horizontal box passes
                // followed by three vertical box passes. BoxBlur is one of
                // each. Keep all passes in this compute pass and ping-pong
                // between the two image buffers; no intermediate readback or
                // CPU serialization is needed.
                for _ in 0..*pass_count {
                    current_is_a = self.encode_dispatch(
                        &mut cpass,
                        horizontal,
                        index,
                        current_is_a,
                        &prepared.resources,
                        prepared.input_dims[index],
                        prepared.output_dims[index],
                    )?;
                }
                for _ in 0..*pass_count {
                    current_is_a = self.encode_dispatch(
                        &mut cpass,
                        vertical,
                        index,
                        current_is_a,
                        &prepared.resources,
                        prepared.input_dims[index],
                        prepared.output_dims[index],
                    )?;
                }
            } else if let ResolvedPipeline::Resize {
                horizontal,
                vertical,
            } = pipeline
            {
                current_is_a = self.encode_dispatch(
                    &mut cpass,
                    horizontal,
                    index,
                    current_is_a,
                    &prepared.resources,
                    prepared.input_dims[index],
                    prepared.output_dims[index],
                )?;
                current_is_a = self.encode_dispatch(
                    &mut cpass,
                    vertical,
                    index,
                    current_is_a,
                    &prepared.resources,
                    prepared.input_dims[index],
                    prepared.output_dims[index],
                )?;
            } else if let ResolvedPipeline::Pad {
                horizontal,
                vertical,
                place,
            } = pipeline
            {
                let resize_dims = prepared.pad_resize_dims[index].ok_or_else(|| {
                    PilError::InternalError("GPU Pad is missing its contain dimensions".into())
                })?;
                if resize_dims != prepared.input_dims[index] {
                    current_is_a = self.encode_dispatch(
                        &mut cpass,
                        horizontal,
                        index,
                        current_is_a,
                        &prepared.resources,
                        prepared.input_dims[index],
                        resize_dims,
                    )?;
                    current_is_a = self.encode_dispatch(
                        &mut cpass,
                        vertical,
                        index,
                        current_is_a,
                        &prepared.resources,
                        prepared.input_dims[index],
                        resize_dims,
                    )?;
                }
                current_is_a = self.encode_dispatch(
                    &mut cpass,
                    place,
                    index,
                    current_is_a,
                    &prepared.resources,
                    resize_dims,
                    prepared.output_dims[index],
                )?;
            } else if let ResolvedPipeline::Histogram {
                clear,
                histogram,
                derive,
                remap,
            } = pipeline
            {
                current_is_a = self.encode_dispatch(
                    &mut cpass,
                    clear,
                    index,
                    current_is_a,
                    &prepared.resources,
                    prepared.input_dims[index],
                    prepared.output_dims[index],
                )?;
                current_is_a = self.encode_dispatch(
                    &mut cpass,
                    histogram,
                    index,
                    current_is_a,
                    &prepared.resources,
                    prepared.input_dims[index],
                    prepared.output_dims[index],
                )?;
                current_is_a = self.encode_dispatch(
                    &mut cpass,
                    derive,
                    index,
                    current_is_a,
                    &prepared.resources,
                    prepared.input_dims[index],
                    prepared.output_dims[index],
                )?;
                current_is_a = self.encode_dispatch(
                    &mut cpass,
                    remap,
                    index,
                    current_is_a,
                    &prepared.resources,
                    prepared.input_dims[index],
                    prepared.output_dims[index],
                )?;
            } else {
                let ResolvedPipeline::Single(cached) = pipeline else {
                    unreachable!("resolved GPU pipeline variant changed during encoding")
                };
                current_is_a = self.encode_dispatch(
                    &mut cpass,
                    cached,
                    index,
                    current_is_a,
                    &prepared.resources,
                    prepared.input_dims[index],
                    prepared.output_dims[index],
                )?;
            }
            drop(cpass);
        }
        Ok(current_is_a)
    }

    fn readback_bytes(&self, size: u64, staging: &wgpu::Buffer) -> Result<Vec<u8>, PilError> {
        let slice = staging.slice(..size);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let deadline = Instant::now() + GPU_READBACK_TIMEOUT;
        let fast_polling = size <= GPU_FAST_POLL_MAX_READBACK_BYTES;
        let mut empty_polls = 0usize;
        loop {
            self.poll_device("GPU readback")?;
            match rx.try_recv() {
                Ok(Ok(())) => break,
                Ok(Err(error)) => {
                    return Err(PilError::ValueError(format!(
                        "GPU readback map_async failed: {error:?}"
                    )));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    return Err(PilError::ValueError(
                        "GPU readback channel closed before completion".into(),
                    ));
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    empty_polls = empty_polls.saturating_add(1);
                    if let Some(backoff) =
                        readback_poll_backoff(fast_polling, empty_polls, Instant::now(), deadline)
                    {
                        std::thread::sleep(backoff);
                        continue;
                    }
                    let detail = self
                        .failure_detail()
                        .unwrap_or_else(|| "device did not complete the submission".into());
                    let message = format!(
                        "GPU readback timed out after {}s: {detail}",
                        GPU_READBACK_TIMEOUT.as_secs()
                    );
                    self.mark_failed(message.clone());
                    // Do not leave a wedged native queue available to a later
                    // call in this process. The process-level parity runner
                    // also terminates the isolated child on its own deadline.
                    self.device.destroy();
                    return Err(PilError::ValueError(message));
                }
            }
        }

        let data = slice.get_mapped_range().to_vec();
        let _ = slice;
        staging.unmap();
        Ok(data)
    }

    fn readback_to_image(
        &self,
        w: u32,
        h: u32,
        staging: &wgpu::Buffer,
    ) -> Result<DynamicImage, PilError> {
        let size = CheckedDims::new(w, h, 4)?.total_bytes() as u64;
        let data = self.readback_bytes(size, staging)?;

        let n = CheckedDims::new(w, h, 1)?.total_pixels();
        #[cfg(target_endian = "little")]
        {
            let expected = n * std::mem::size_of::<u32>();
            if data.len() != expected {
                return Err(PilError::ValueError(format!(
                    "GPU readback byte length {} does not match image size {expected}",
                    data.len()
                )));
            }
            return RgbaImage::from_raw(w, h, data)
                .map(DynamicImage::ImageRgba8)
                .ok_or_else(|| PilError::ValueError("bad readback buffer".into()));
        }

        #[cfg(target_endian = "big")]
        let mut rgba_bytes = Vec::with_capacity(n * 4);
        #[cfg(target_endian = "big")]
        let mut pixels = data.chunks_exact(std::mem::size_of::<u32>());
        #[cfg(target_endian = "big")]
        for _ in 0..n {
            let bytes = pixels.next().ok_or_else(|| {
                PilError::ValueError("GPU readback buffer ended before image pixels".into())
            })?;
            let pixel = u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            rgba_bytes.push((pixel & 0xff) as u8);
            rgba_bytes.push(((pixel >> 8) & 0xff) as u8);
            rgba_bytes.push(((pixel >> 16) & 0xff) as u8);
            rgba_bytes.push(((pixel >> 24) & 0xff) as u8);
        }
        #[cfg(target_endian = "big")]
        if !pixels.remainder().is_empty() {
            return Err(PilError::ValueError(
                "GPU readback buffer has a partial pixel".into(),
            ));
        }

        #[cfg(target_endian = "big")]
        let img = RgbaImage::from_raw(w, h, rgba_bytes)
            .ok_or_else(|| PilError::ValueError("bad readback buffer".into()))?;
        #[cfg(target_endian = "big")]
        Ok(DynamicImage::ImageRgba8(img))
    }

    fn readback_to_luma16(
        &self,
        w: u32,
        h: u32,
        staging: &wgpu::Buffer,
        mode: Option<&str>,
    ) -> Result<DynamicImage, PilError> {
        let size = CheckedDims::new(w, h, 4)?.total_bytes() as u64;
        let data = self.readback_bytes(size, staging)?;
        let n = CheckedDims::new(w, h, 1)?.total_pixels();
        let expected = n * std::mem::size_of::<u32>();
        if data.len() != expected {
            return Err(PilError::ValueError(format!(
                "GPU readback byte length {} does not match image size {expected}",
                data.len()
            )));
        }
        let mut pixels = Vec::with_capacity(n);
        for sample in data.chunks_exact(std::mem::size_of::<u32>()) {
            let bytes = [sample[0], sample[1]];
            pixels.push(if matches!(mode, Some("I;16B" | "I;16N")) {
                u16::from_be_bytes(bytes)
            } else {
                u16::from_ne_bytes(bytes)
            });
        }
        if !data
            .chunks_exact(std::mem::size_of::<u32>())
            .remainder()
            .is_empty()
        {
            return Err(PilError::ValueError(
                "GPU readback buffer has a partial typed sample".into(),
            ));
        }
        let image = ImageBuffer::<Luma<u16>, Vec<u16>>::from_raw(w, h, pixels)
            .ok_or_else(|| PilError::ValueError("bad typed readback buffer".into()))?;
        Ok(DynamicImage::ImageLuma16(image))
    }

    /// Read back the numeric-u16 representation used by typed Paste. Unlike
    /// I;16 geometry, this path deliberately ignores the public byte-order
    /// label: the shader blended decoded unsigned samples and wrote the
    /// result as a little-endian numeric word. The returned host image keeps
    /// those samples ready for the existing mode-preserving boundary.
    fn readback_to_luma16_numeric(
        &self,
        w: u32,
        h: u32,
        staging: &wgpu::Buffer,
    ) -> Result<DynamicImage, PilError> {
        let size = CheckedDims::new(w, h, 4)?.total_bytes() as u64;
        let data = self.readback_bytes(size, staging)?;
        let n = CheckedDims::new(w, h, 1)?.total_pixels();
        let expected = n * std::mem::size_of::<u32>();
        if data.len() != expected {
            return Err(PilError::ValueError(format!(
                "GPU readback byte length {} does not match image size {expected}",
                data.len()
            )));
        }
        let pixels = data
            .chunks_exact(std::mem::size_of::<u32>())
            .map(|sample| u16::from_le_bytes([sample[0], sample[1]]))
            .collect::<Vec<_>>();
        if !data
            .chunks_exact(std::mem::size_of::<u32>())
            .remainder()
            .is_empty()
        {
            return Err(PilError::ValueError(
                "GPU readback buffer has a partial typed sample".into(),
            ));
        }
        let image = ImageBuffer::<Luma<u16>, Vec<u16>>::from_raw(w, h, pixels)
            .ok_or_else(|| PilError::ValueError("bad typed readback buffer".into()))?;
        Ok(DynamicImage::ImageLuma16(image))
    }

    fn execute_batch_impl(
        &self,
        ops: &[PipelineOp],
        auxiliary_images: &[AuxiliaryImages],
        w: u32,
        h: u32,
        mode: u32,
        logical_mode: Option<&str>,
        contrast_mean: Option<u8>,
        f_resize_constant_bits: Option<u32>,
        f_resize_box_copy_is_exact: bool,
        f_resize_identity_is_exact: bool,
        f_resize_box_average_is_exact: bool,
        f_resize_dyadic_is_exact: bool,
        f_resize_f64_is_exact: bool,
        f_resize_f64_ordered_is_exact: bool,
        buffers: &mut BufferPool,
    ) -> Result<
        (
            bool,
            u32,
            u32,
            StagingBuffer,
            PipelineResourceTelemetry,
            u64,
        ),
        PilError,
    > {
        let mut current_is_a = true;
        let mut cur_w = w;
        let mut cur_h = h;
        let dispatch_count = gpu_dispatch_count(ops, logical_mode, (w, h));
        gpu_log!(
            "[GPU] batch_impl: {} ops, start dims {}x{}",
            ops.len(),
            cur_w,
            cur_h
        );
        if ops.len() != auxiliary_images.len() {
            return Err(PilError::InternalError(
                "GPU operation and auxiliary-image counts differ".into(),
            ));
        }

        let limits = self.device.limits();
        let uniform_alignment = limits.min_uniform_buffer_offset_alignment as usize;
        let storage_alignment = limits.min_storage_buffer_offset_alignment as usize;
        let mut op_modes = Vec::with_capacity(ops.len());
        let mut current_mode = mode;
        for op in ops {
            op_modes.push(current_mode);
            current_mode = gpu_mode_after_op(current_mode, op);
        }
        let mut resource_source_dims = Vec::with_capacity(ops.len());
        let mut resource_w = w;
        let mut resource_h = h;
        for op in ops {
            resource_source_dims.push((resource_w, resource_h));
            let next = match op_output_dims(op, resource_w, resource_h) {
                Some(dimensions) => dimensions,
                None if op_has_explicit_output_dimensions(op) => {
                    return Err(PilError::ValueError(format!(
                        "GPU operation '{}' has no safe source dimensions",
                        registry::variant_key(op)
                    )));
                }
                None => (resource_w, resource_h),
            };
            (resource_w, resource_h) = next;
        }
        let resource_bytes = ops
            .iter()
            .zip(auxiliary_images.iter())
            .enumerate()
            .map(|(index, (op, auxiliary))| {
                self.estimate_resource_bytes(
                    op,
                    auxiliary,
                    buffers,
                    uniform_alignment,
                    storage_alignment,
                    resource_source_dims[index],
                    op_modes[index],
                    f_resize_f64_is_exact,
                    f_resize_f64_ordered_is_exact,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut work_w = w;
        let mut work_h = h;
        let shader_work_items = ops
            .iter()
            .map(|op| {
                let next = match op_output_dims(op, work_w, work_h) {
                    Some(dimensions) => dimensions,
                    None if op_has_explicit_output_dimensions(op) => {
                        return Err(PilError::ValueError(format!(
                            "GPU operation '{}' has no safe work dimensions",
                            registry::variant_key(op)
                        )));
                    }
                    None => (work_w, work_h),
                };
                let work = gpu_shader_work_items(op, (work_w, work_h), next, logical_mode)
                    .ok_or_else(|| {
                        PilError::ValueError(format!(
                            "GPU operation '{}' has no bounded shader work estimate",
                            registry::variant_key(op)
                        ))
                    })?;
                (work_w, work_h) = next;
                Ok(work)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let auxiliary_cache = GpuAuxiliaryCache::from_batch(
            ops,
            auxiliary_images,
            mode,
            buffers.capacity,
            storage_alignment,
        )?;
        self.upload_auxiliary_cache(&auxiliary_cache, buffers, storage_alignment);
        let mut chunk_start = 0usize;
        let mut submission_index = 0usize;
        let mut staging = None;
        let mut resource_telemetry = PipelineResourceTelemetry {
            auxiliary_bytes: auxiliary_cache.total_bytes() as u64,
            ..PipelineResourceTelemetry::default()
        };
        current_mode = mode;
        while chunk_start < ops.len() {
            self.ensure_healthy("GPU batch submission")?;
            let chunk_end = select_gpu_chunk_end(chunk_start, &resource_bytes, &shader_work_items)
                .ok_or_else(|| {
                    PilError::InternalError(
                    "GPU chunk scheduling made no progress because its estimates are inconsistent"
                        .into(),
                )
                })?;
            let estimated_bytes = resource_bytes[chunk_start..chunk_end]
                .iter()
                .fold(0usize, |total, bytes| total.saturating_add(*bytes));
            let estimated_work = shader_work_items[chunk_start..chunk_end]
                .iter()
                .fold(0u64, |total, work| total.saturating_add(*work));

            let prepared = self.prepare_batch(
                &ops[chunk_start..chunk_end],
                &auxiliary_images[chunk_start..chunk_end],
                cur_w,
                cur_h,
                current_mode,
                logical_mode,
                contrast_mean,
                f_resize_constant_bits,
                f_resize_box_copy_is_exact,
                f_resize_identity_is_exact,
                f_resize_box_average_is_exact,
                f_resize_dyadic_is_exact,
                f_resize_f64_is_exact,
                f_resize_f64_ordered_is_exact,
                buffers,
                &auxiliary_cache,
            )?;
            resource_telemetry.parameter_bytes = resource_telemetry
                .parameter_bytes
                .saturating_add(prepared.resource_telemetry.parameter_bytes);
            resource_telemetry.auxiliary_bytes = resource_telemetry
                .auxiliary_bytes
                .saturating_add(prepared.resource_telemetry.auxiliary_bytes);
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("gpu_batch_encoder"),
                });
            current_is_a = self.encode_batch(
                &mut encoder,
                &ops[chunk_start..chunk_end],
                &prepared,
                current_is_a,
                logical_mode,
            )?;

            let final_dims = prepared.final_dims;
            if chunk_end == ops.len() {
                let size = CheckedDims::new(final_dims.0, final_dims.1, 4)?.total_bytes() as u64;
                let readback = self.acquire_staging(size)?;
                let src = if current_is_a {
                    prepared.resources.buf_a
                } else {
                    prepared.resources.buf_b
                };
                // Record the copy after the compute pass in the same command
                // buffer. Queue ordering therefore covers compute and
                // readback together, avoiding a second command-buffer/submit
                // lifecycle at the point where the native driver previously
                // wedged during command recording.
                encoder.copy_buffer_to_buffer(src, 0, &readback.buffer, 0, size);
                staging = Some(readback);
            }
            self.queue.submit(Some(encoder.finish()));
            // The queue preserves submission order: the next chunk's writes
            // to these arenas are ordered after this chunk's reads. Release
            // the borrow only after the command buffer has captured it.
            drop(prepared);
            self.poll_device("GPU batch submission")?;
            submission_index += 1;
            gpu_log!(
                "[GPU] submitted chunk={} ops={}..{} resources={} bytes work={}",
                submission_index,
                chunk_start,
                chunk_end,
                estimated_bytes,
                estimated_work
            );
            (cur_w, cur_h) = final_dims;
            current_mode = gpu_mode_after_ops(current_mode, &ops[chunk_start..chunk_end]);
            chunk_start = chunk_end;
        }
        // After all chunks, current_is_a tracks where the latest result lives:
        //   true → buf_a has the final result, false → buf_b. Queue ordering
        // keeps chunk submissions dependent without a blocking poll between them.
        let staging = staging.ok_or_else(|| {
            PilError::InternalError("GPU batch produced no readback staging buffer".into())
        })?;
        Ok((
            current_is_a,
            cur_w,
            cur_h,
            staging,
            resource_telemetry,
            dispatch_count,
        ))
    }
}

static GPU: std::sync::OnceLock<Result<GpuInner, PilError>> = std::sync::OnceLock::new();

// ─── Mode helpers ───────────────────────────────────────────────────────────

/// Map a DynamicImage variant to its mode code for the GPU uniform buffer.
/// 0 = L, 1 = LA, 2 = RGB, 3 = RGBA, 4 = CMYK, 5 = native I;16* geometry,
/// 6 = RGBX, and 7 = native I-mode convolution. The non-byte modes use the
/// packed word only where the selected shader explicitly owns that contract.
fn mode_code(img: &DynamicImage) -> u32 {
    match img {
        DynamicImage::ImageLuma8(_) => 0,
        DynamicImage::ImageLumaA8(_) => 1,
        DynamicImage::ImageRgb8(_) => 2,
        DynamicImage::ImageRgba8(_) => 3,
        DynamicImage::ImageLuma16(_) => 5,
        _ => 3, // fallback: treat as RGBA
    }
}

fn execution_mode_code(img: &DynamicImage, logical_mode: Option<&str>) -> u32 {
    match logical_mode {
        Some("CMYK") => 4,
        Some("I;16" | "I;16L" | "I;16B" | "I;16N") => 5,
        Some("I") => 7,
        // F stores one little-endian f32 sample in the four-byte transport.
        // The order-statistic shaders use this distinct code instead of
        // treating the sample's representation as four independent bytes.
        Some("F") => 8,
        // RGBX shares RGBA storage, but byte three is padding rather than an
        // alpha sample. The Convert shader uses this distinct source code to
        // force opaque output alpha instead of preserving that padding byte.
        Some("RGBX") => 6,
        // P/PA use the L/LA transport respectively. Preserve the physical
        // two-band code for PA so ExtractBand(1) selects byte three (alpha),
        // and so later raw PA writes do not mistake the alpha byte for green.
        Some("P" | "1") => 0,
        Some("PA") => 1,
        Some("HSV" | "YCbCr") => 2,
        _ => mode_code(img),
    }
}

fn gpu_resize_channel_count(mode: u32) -> u32 {
    match mode {
        0 => 1,
        1 => 2,
        2 => 3,
        _ => 4,
    }
}

fn gpu_resize_should_premultiply(
    mode: u32,
    logical_mode: Option<&str>,
    filter: ResampleFilter,
) -> bool {
    // Pillow's nearest affine path copies samples directly. Alpha is
    // premultiplied only by the separable convolution filters.
    if matches!(filter, ResampleFilter::Nearest) {
        return false;
    }
    match logical_mode {
        Some("PA" | "RGBa" | "CMYK" | "RGBX" | "P" | "1" | "I" | "F") => false,
        Some("LA" | "RGBA") => true,
        Some("L" | "RGB" | "HSV" | "YCbCr") => false,
        Some("I;16" | "I;16L" | "I;16B" | "I;16N") => false,
        None => matches!(mode, 1 | 3),
        Some(_) => matches!(mode, 1 | 3),
    }
}

fn gpu_transform_uses_nearest(logical_mode: Option<&str>, filter: ResampleFilter) -> bool {
    matches!(filter, ResampleFilter::Nearest)
        || matches!(
            logical_mode,
            Some("P" | "1" | "I" | "F" | "I;16" | "I;16L" | "I;16B" | "I;16N")
        )
}

fn gpu_transform_should_premultiply(
    mode: u32,
    logical_mode: Option<&str>,
    filter: ResampleFilter,
    method: TransformMethod,
) -> bool {
    // Perspective/Quad use the direct ImagingTransformProjective byte path.
    // Its filtered LA/RGBA relocation envelope still performs Pillow's
    // premultiplied round trip in the shader, while raw L/RGB/PA paths remain
    // unpremultiplied. The bounded Mesh relocation path follows the same
    // projective contract.
    if gpu_transform_uses_nearest(logical_mode, filter) {
        return false;
    }
    if !matches!(method, TransformMethod::Affine) {
        return matches!(logical_mode, Some("LA" | "RGBA"))
            || (logical_mode.is_none() && matches!(mode, 1 | 3));
    }
    match logical_mode {
        Some("LA" | "RGBA") => true,
        Some("PA" | "RGBa" | "CMYK" | "I" | "F" | "P" | "1") => false,
        Some("I;16" | "I;16L" | "I;16B" | "I;16N") => false,
        None => matches!(mode, 1 | 3),
        Some(_) => matches!(mode, 1 | 3),
    }
}

fn gpu_transform_fill(op: &PipelineOp, logical_mode: Option<&str>, mode: u32) -> u32 {
    let PipelineOp::Transform {
        fill, palette_fill, ..
    } = op
    else {
        return 0;
    };
    let has_alpha =
        matches!(logical_mode, Some("LA" | "PA" | "RGBA" | "RGBa")) || matches!(mode, 1 | 3);
    // Pillow's default rotate fill for CMYK is the complete zero C/M/Y/K
    // sample. It is not the opaque-alpha default used by RGB-like modes.
    let default_fill = if matches!(logical_mode, Some("CMYK" | "F")) {
        // F uses a zero floating-point word, even though its packed
        // transport has four bytes and no logical alpha channel. CMYK uses
        // the same all-zero default for its four raw channels.
        (0, 0, 0, 0)
    } else {
        (0, 0, 0, if has_alpha { 0 } else { 255 })
    };
    let resolved = palette_fill
        .map(|index| (index, 0, 0, 255))
        .or(*fill)
        .unwrap_or(default_fill);
    // Core normalizes LA/PA colors as `(luma, alpha, 0, 0)` because that is
    // the native two-band representation.  The GPU transport is packed
    // RGBA, so move the logical alpha into byte three and replicate luma in
    // the unused color lanes just as `to_rgba8()` does for the source image.
    let packed = if matches!(logical_mode, Some("LA" | "PA")) {
        (resolved.0, resolved.0, resolved.0, resolved.1)
    } else {
        resolved
    };
    u32::from(packed.0)
        | (u32::from(packed.1) << 8)
        | (u32::from(packed.2) << 16)
        | (u32::from(packed.3) << 24)
}

/// Return the packed transport mode used by the exact four-mode Convert
/// shader.  Non-byte targets are deliberately excluded: their public sample
/// representation needs a different buffer layout and therefore remains an
/// explicit preflight failure.
fn gpu_standard_color_mode_code(mode: &ColorMode) -> Option<u32> {
    match mode {
        ColorMode::L => Some(0),
        ColorMode::LA => Some(1),
        ColorMode::RGB => Some(2),
        ColorMode::RGBA => Some(3),
        _ => None,
    }
}

/// Return the packed target-mode code understood by `convert.wgsl`.
/// Non-standard Pillow modes remain packed byte buffers at this executor
/// boundary, but their output representation is restored by the final
/// color-type conversion and the public explicit mode on the lazy image.
fn gpu_convert_target_mode_code(mode: &ColorMode) -> Option<u32> {
    match mode {
        ColorMode::L => Some(0),
        ColorMode::LA => Some(1),
        ColorMode::RGB => Some(2),
        ColorMode::RGBA => Some(3),
        ColorMode::CMYK => Some(4),
        ColorMode::YCbCr => Some(5),
        ColorMode::HSV => Some(6),
        ColorMode::I => Some(7),
        ColorMode::F => Some(8),
        ColorMode::P | ColorMode::Mode1 => None,
    }
}

/// Update the packed mode word after a mode-changing operation that the GPU
/// batch can represent.  Keeping this scalar state beside the uniform builder
/// lets a Convert feed a later shader without a host readback: the first
/// dispatch writes the new packed layout and the next dispatch receives the
/// corresponding mode code.
fn gpu_mode_after_op(mode: u32, op: &PipelineOp) -> u32 {
    match op {
        PipelineOp::Convert { mode: target, .. } => {
            gpu_convert_target_mode_code(target).unwrap_or(mode)
        }
        PipelineOp::Grayscale | PipelineOp::ExtractBand { .. } | PipelineOp::Constant { .. } => 0,
        PipelineOp::Colorize { .. } => 2,
        PipelineOp::EffectNoise { .. } => 0,
        PipelineOp::Color3DLut { target_mode, .. } => {
            gpu_pixel_mode_code(*target_mode).unwrap_or(mode)
        }
        _ => mode,
    }
}

fn gpu_pixel_mode_code(mode: PixelMode) -> Option<u32> {
    match mode {
        PixelMode::RGB => Some(2),
        PixelMode::RGBA => Some(3),
        PixelMode::CMYK => Some(4),
        _ => None,
    }
}

fn gpu_mode_after_ops(mut mode: u32, ops: &[PipelineOp]) -> u32 {
    for op in ops {
        mode = gpu_mode_after_op(mode, op);
    }
    mode
}

/// Compute Pillow's rounded Contrast midpoint without materializing a
/// grayscale image. This is scalar control-plane work: the original native
/// byte layout is scanned once, then the complete per-pixel blend is executed
/// by `contrast.wgsl`. CMYK keeps its C/M/Y/K interpretation instead of
/// treating the fourth packed byte as alpha.
fn gpu_contrast_mean(img: &DynamicImage, logical_mode: Option<&str>) -> Option<u8> {
    let (channels, cmyk) = match (img, logical_mode) {
        (DynamicImage::ImageLuma8(_), None | Some("L")) => (1usize, false),
        (DynamicImage::ImageLumaA8(_), None | Some("LA")) => (2, false),
        (DynamicImage::ImageRgb8(_), None | Some("RGB")) => (3, false),
        (DynamicImage::ImageRgba8(_), None | Some("RGBA")) => (4, false),
        (DynamicImage::ImageRgba8(_), Some("CMYK")) => (4, true),
        _ => return None,
    };
    let pixels = usize::try_from(img.width())
        .ok()?
        .checked_mul(usize::try_from(img.height()).ok()?)?;
    if pixels == 0 {
        return None;
    }
    let expected = pixels.checked_mul(channels)?;
    let source = img.as_bytes();
    if source.len() != expected {
        return None;
    }
    let sum = if cmyk {
        source
            .chunks_exact(4)
            .map(|pixel| {
                let c = u32::from(pixel[0]);
                let m = u32::from(pixel[1]);
                let y = u32::from(pixel[2]);
                let k = u32::from(pixel[3]);
                let nk = 255u32.saturating_sub(k);
                let r = (nk as i32 - crate::color::muldiv255(c, nk) as i32).clamp(0, 255) as u8;
                let g = (nk as i32 - crate::color::muldiv255(m, nk) as i32).clamp(0, 255) as u8;
                let b = (nk as i32 - crate::color::muldiv255(y, nk) as i32).clamp(0, 255) as u8;
                u64::from(crate::color::rgb_to_luma_u8(r, g, b))
            })
            .sum::<u64>()
    } else {
        source
            .chunks_exact(channels)
            .map(|pixel| match channels {
                1 | 2 => u64::from(pixel[0]),
                3 | 4 => u64::from(crate::color::rgb_to_luma_u8(pixel[0], pixel[1], pixel[2])),
                _ => 0,
            })
            .sum::<u64>()
    };
    Some(((sum as f64 / pixels as f64) + 0.5) as u8)
}

/// Compute Contrast's midpoint after a narrowly provable host prefix.
///
/// `Image.putpixel` is an exact byte-layout update, while Contrast's midpoint
/// belongs to the image produced by that update rather than to the original
/// source.  The GPU batch already executes both operations in order; this
/// helper mirrors only the one exact prefix needed to supply the scalar
/// midpoint.  Keep the proof deliberately small: palette writes, additional
/// operations, and a later Contrast still require the ordinary host-control
/// path because their current-image semantics are not represented by one
/// static uniform.
fn gpu_contrast_mean_after_exact_prefix(
    img: &DynamicImage,
    ops: &[PipelineOp],
    logical_mode: Option<&str>,
) -> Option<u8> {
    let first_contrast = ops
        .iter()
        .position(|op| matches!(op, PipelineOp::Contrast { .. }))?;
    if ops
        .iter()
        .skip(first_contrast.saturating_add(1))
        .any(|op| matches!(op, PipelineOp::Contrast { .. }))
    {
        return None;
    }

    if first_contrast == 0 {
        return gpu_contrast_mean(img, logical_mode);
    }

    // A single PutPixel is the only prefix whose exact result can be formed
    // without invoking the CPU batch executor (which would publish a second
    // backend/operation receipt).  Palette-index writes need palette state and
    // therefore remain on the existing exact host-control path.
    if first_contrast != 1 {
        return None;
    }
    let PipelineOp::PutPixel {
        x,
        y,
        color,
        palette_index: false,
    } = &ops[0]
    else {
        return None;
    };
    let prefixed =
        crate::compute::pool_cpu::ops::effects::op_put_pixel(img, *x, *y, *color).ok()?;
    gpu_contrast_mean(&prefixed, logical_mode)
}

/// Return the concrete byte-buffer color type produced by a supported GPU
/// mode-changing operation. The shader always writes packed RGBA8, but the
/// public result must expose the requested number of bands rather than the
/// source image's type.
fn gpu_output_color_type(mode: &ColorMode) -> Option<crate::raster::ColorType> {
    match mode {
        ColorMode::L => Some(crate::raster::ColorType::L8),
        ColorMode::LA => Some(crate::raster::ColorType::La8),
        ColorMode::RGB => Some(crate::raster::ColorType::Rgb8),
        ColorMode::RGBA => Some(crate::raster::ColorType::Rgba8),
        ColorMode::CMYK | ColorMode::I | ColorMode::F => Some(crate::raster::ColorType::Rgba8),
        ColorMode::YCbCr | ColorMode::HSV => Some(crate::raster::ColorType::Rgb8),
        _ => None,
    }
}

fn gpu_pixel_mode_color_type(mode: PixelMode) -> Option<crate::raster::ColorType> {
    match mode {
        PixelMode::RGB => Some(crate::raster::ColorType::Rgb8),
        PixelMode::RGBA | PixelMode::CMYK => Some(crate::raster::ColorType::Rgba8),
        _ => None,
    }
}

/// Convert the packed GPU result to the requested standard byte mode without
/// applying a second luma conversion. GPU L/LA shaders intentionally keep the
/// luma sample in byte 0; calling `to_luma8()` on that RGBA transport would
/// weight the zeroed G/B bytes again and change every sample.
fn gpu_result_as_color_type(
    result: DynamicImage,
    color_type: crate::raster::ColorType,
) -> Result<DynamicImage, PilError> {
    let rgba = result.to_rgba8();
    let (w, h) = rgba.dimensions();
    match color_type {
        crate::raster::ColorType::L8 => {
            let luma = rgba.pixels().map(|pixel| pixel[0]).collect();
            crate::raster::GrayImage::from_raw(w, h, luma)
                .map(DynamicImage::ImageLuma8)
                .ok_or_else(|| PilError::InternalError("GPU L output shape mismatch".into()))
        }
        crate::raster::ColorType::La8 => {
            let samples = rgba
                .pixels()
                .flat_map(|pixel| [pixel[0], pixel[3]])
                .collect();
            crate::raster::GrayAlphaImage::from_raw(w, h, samples)
                .map(DynamicImage::ImageLumaA8)
                .ok_or_else(|| PilError::InternalError("GPU LA output shape mismatch".into()))
        }
        crate::raster::ColorType::Rgb8 => Ok(DynamicImage::ImageRgb8(
            DynamicImage::ImageRgba8(rgba).to_rgb8(),
        )),
        crate::raster::ColorType::Rgba8 => Ok(DynamicImage::ImageRgba8(rgba)),
        _ => Err(PilError::InternalError(
            "unsupported GPU output color type".into(),
        )),
    }
}

/// Return whether a batch contains a mode-changing operation that cannot feed
/// a later shader in-place. Even byte-to-byte Convert changes the source
/// layout used by subsequent image-aware checks, so it terminates a segment
/// just like the other mode transitions.
fn gpu_batch_has_nonterminal_mode_change(ops: &[PipelineOp]) -> bool {
    ops.iter().enumerate().any(|(index, op)| {
        index + 1 < ops.len()
            && matches!(
                op,
                PipelineOp::Grayscale
                    | PipelineOp::ExtractBand { .. }
                    | PipelineOp::Constant { .. }
                    | PipelineOp::Colorize { .. }
                    | PipelineOp::PutAlpha { .. }
                    | PipelineOp::PutAlphaData { .. }
                    | PipelineOp::Convert { .. }
                    | PipelineOp::EffectNoise { .. }
            )
    })
}

/// Find the first operation whose public result changes the packed logical
/// layout while another operation still follows it. A host-visible native
/// result keeps the following operation's mode checks tied to the converted
/// image rather than the original source layout.
fn gpu_first_nonterminal_mode_change(ops: &[PipelineOp]) -> Option<usize> {
    ops.iter().enumerate().find_map(|(index, op)| {
        (index + 1 < ops.len()
            && matches!(
                op,
                PipelineOp::Grayscale
                    | PipelineOp::ExtractBand { .. }
                    | PipelineOp::Constant { .. }
                    | PipelineOp::Colorize { .. }
                    | PipelineOp::PutAlpha { .. }
                    | PipelineOp::PutAlphaData { .. }
                    | PipelineOp::Convert { .. }
                    | PipelineOp::EffectNoise { .. }
            ))
        .then_some(index)
    })
}

/// Return the logical mode that follows a mode-changing GPU segment. The
/// image result is used as a conservative fallback for direct backend callers
/// that do not provide an explicit logical-mode tag.
fn gpu_logical_mode_after_op(
    previous: Option<&str>,
    op: &PipelineOp,
    result: &DynamicImage,
) -> Option<String> {
    let known = match op {
        PipelineOp::Grayscale | PipelineOp::ExtractBand { .. } | PipelineOp::Constant { .. } => {
            Some("L")
        }
        PipelineOp::Colorize { .. } => Some("RGB"),
        PipelineOp::Convert { mode, .. } => match mode {
            ColorMode::L => Some("L"),
            ColorMode::LA => Some("LA"),
            ColorMode::RGB => Some("RGB"),
            ColorMode::RGBA => Some("RGBA"),
            ColorMode::CMYK => Some("CMYK"),
            ColorMode::YCbCr => Some("YCbCr"),
            ColorMode::HSV => Some("HSV"),
            ColorMode::I => Some("I"),
            ColorMode::F => Some("F"),
            ColorMode::P => Some("P"),
            ColorMode::Mode1 => Some("1"),
        },
        PipelineOp::PutAlpha { mode, .. } | PipelineOp::PutAlphaData { mode, .. } => match mode {
            PixelMode::L | PixelMode::LA => Some("LA"),
            PixelMode::RGB | PixelMode::RGBA | PixelMode::YCbCr | PixelMode::HSV => Some("RGBA"),
            PixelMode::P | PixelMode::PA => Some("PA"),
            PixelMode::CMYK => Some("CMYK"),
            PixelMode::Mode1 | PixelMode::I | PixelMode::F => None,
        },
        _ => None,
    };
    known
        .map(str::to_owned)
        .or_else(|| previous.map(str::to_owned))
        .or_else(|| match result {
            DynamicImage::ImageLuma8(_) => Some("L".to_owned()),
            DynamicImage::ImageLumaA8(_) => Some("LA".to_owned()),
            DynamicImage::ImageRgb8(_) => Some("RGB".to_owned()),
            DynamicImage::ImageRgba8(_) => Some("RGBA".to_owned()),
            DynamicImage::ImageLuma16(_) => None,
            _ => None,
        })
}

fn put_alpha_output(result: DynamicImage, mode: PixelMode) -> Result<DynamicImage, PilError> {
    if matches!(
        mode,
        PixelMode::L | PixelMode::LA | PixelMode::P | PixelMode::PA
    ) {
        let rgba = result.to_rgba8();
        let (w, h) = rgba.dimensions();
        let samples = rgba
            .pixels()
            .flat_map(|pixel| [pixel[0], pixel[3]])
            .collect();
        crate::raster::GrayAlphaImage::from_raw(w, h, samples)
            .map(DynamicImage::ImageLumaA8)
            .ok_or_else(|| {
                PilError::InternalError("GPU putalpha buffer shape mismatch".to_string())
            })
    } else {
        Ok(DynamicImage::ImageRgba8(result.to_rgba8()))
    }
}

/// Extract the second (right-hand) image from a dual-input PipelineOp, if present.
/// Returns shared materialized pixels ready for GPU upload.
fn extract_second_image(
    op: &PipelineOp,
    primary_dimensions: Option<(u32, u32)>,
    draw_source: Option<&DynamicImage>,
) -> Result<Option<Arc<DynamicImage>>, PilError> {
    if crate::compute::pool_cpu::ops::draw::is_draw_op(op) {
        let rendered = if let Some(rendered) = draw_source {
            rendered.clone()
        } else {
            return Err(PilError::InternalError(
                "GPU draw operation is missing its control canvas".into(),
            ));
        };
        return Ok(Some(Arc::new(DynamicImage::ImageRgba8(
            rendered.to_rgba8(),
        ))));
    }
    if let PipelineOp::EffectSpread { distance } = op {
        let (width, height) = primary_dimensions.ok_or_else(|| {
            PilError::InternalError("GPU EffectSpread is missing primary dimensions".into())
        })?;
        let mapping = crate::compute::pool_cpu::ops::effects::effect_spread_mapping(
            width, height, *distance,
        )?;
        let mut bytes = Vec::with_capacity(mapping.len().saturating_mul(4));
        for value in mapping {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        let synthetic = RgbaImage::from_raw(width.saturating_mul(height), 1, bytes)
            .ok_or_else(|| PilError::InternalError("GPU EffectSpread map shape mismatch".into()))?;
        return Ok(Some(Arc::new(DynamicImage::ImageRgba8(synthetic))));
    }
    if let PipelineOp::EffectNoise { sigma } = op {
        let (width, height) = primary_dimensions.ok_or_else(|| {
            PilError::InternalError("GPU EffectNoise is missing primary dimensions".into())
        })?;
        let values =
            crate::compute::pool_cpu::ops::effects::effect_noise_values(width, height, *sigma)?;
        let mut bytes = Vec::with_capacity(values.len().saturating_mul(4));
        for value in values {
            bytes.extend_from_slice(&[value, 0, 0, 255]);
        }
        let synthetic = RgbaImage::from_raw(width, height, bytes).ok_or_else(|| {
            PilError::InternalError("GPU EffectNoise buffer shape mismatch".into())
        })?;
        return Ok(Some(Arc::new(DynamicImage::ImageRgba8(synthetic))));
    }
    if let PipelineOp::Merge { bands, .. } = op {
        // Merge's first band is the current pipeline image. Pack the remaining
        // single-band values consecutively into a synthetic Luma image so the
        // ordinary dual-input binding can carry all bands without a new ABI.
        let first = bands
            .first()
            .ok_or_else(|| PilError::ValueError("GPU Merge requires at least one band".into()))?;
        let (w, h) = first.size()?;
        let pixels = CheckedDims::new(w, h, 1)?.total_pixels();
        let extras = bands.len().saturating_sub(1);
        if extras == 0 {
            return Ok(None);
        }
        let packed_width = pixels.checked_mul(extras).ok_or_else(|| {
            PilError::ValueError("GPU Merge auxiliary band dimensions overflow".into())
        })?;
        let packed_width = u32::try_from(packed_width).map_err(|_| {
            PilError::ValueError("GPU Merge auxiliary band dimensions exceed u32".into())
        })?;
        let mut values = Vec::with_capacity(packed_width as usize);
        for band in bands.iter().skip(1) {
            let image = band.materialize_for_ops()?;
            if image.dimensions() != (w, h) {
                return Err(PilError::ValueError(
                    "GPU Merge band dimensions do not match".into(),
                ));
            }
            values.extend(image.to_luma8().into_raw());
        }
        let synthetic = crate::raster::GrayImage::from_raw(packed_width, 1, values)
            .ok_or_else(|| PilError::InternalError("GPU Merge auxiliary shape mismatch".into()))?;
        return Ok(Some(Arc::new(DynamicImage::ImageLuma8(synthetic))));
    }
    if let PipelineOp::Color3DLut { table, .. } = op {
        // Color3DLut table entries are prepared once on the host using the
        // exact signed 12.4 conversion from the CPU implementation. Store one
        // signed i16 value per synthetic pixel; the shader reads the low 16
        // bits through the normal packed RGBA upload path.
        let values = color3dlut_table_words(table)?;
        let width = u32::try_from(values.len()).map_err(|_| {
            PilError::ValueError("GPU Color3DLut table exceeds u32 dimensions".into())
        })?;
        let mut bytes = Vec::with_capacity(values.len().saturating_mul(4));
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        let synthetic = RgbaImage::from_raw(width, 1, bytes).ok_or_else(|| {
            PilError::InternalError("GPU Color3DLut auxiliary table shape mismatch".into())
        })?;
        return Ok(Some(Arc::new(DynamicImage::ImageRgba8(synthetic))));
    }
    if let PipelineOp::Paste { source, .. } = op {
        // Paste has already converted its source to the destination mode. Keep
        // P-mode sources as their one-byte indices here: expanding them through
        // the palette before packing the GPU upload would make the GPU lane
        // disagree with the CPU and SIMD paste implementations.
        return source.materialized_shared().map(Some);
    }
    if let PipelineOp::CompositeModule { other, .. } = op {
        if matches!(other.mode()?.as_str(), "P" | "PA") {
            // Image.composite blends indexed samples and gives the result
            // image2's palette. Upload image2's raw index/(index, alpha)
            // bytes, not its visible RGB(A) expansion.
            return other.materialized_shared().map(Some);
        }
    }

    let arc_img: Option<&std::sync::Arc<crate::image::Image>> = match op {
        PipelineOp::Add { other, .. }
        | PipelineOp::Subtract { other, .. }
        | PipelineOp::Multiply { other }
        | PipelineOp::Screen { other }
        | PipelineOp::Darker { other }
        | PipelineOp::Lighter { other }
        | PipelineOp::Difference { other }
        | PipelineOp::Overlay { other }
        | PipelineOp::HardLight { other }
        | PipelineOp::SoftLight { other }
        | PipelineOp::AddModulo { other }
        | PipelineOp::SubtractModulo { other }
        | PipelineOp::LogicalAnd { other }
        | PipelineOp::LogicalOr { other }
        | PipelineOp::LogicalXor { other }
        | PipelineOp::BlendModule { other, .. }
        | PipelineOp::CompositeModule { other, .. } => Some(other),
        PipelineOp::AlphaComposite { source, .. } => Some(source),
        _ => None,
    };
    arc_img
        .map(|image| image.materialized_shared().map(Some))
        .unwrap_or(Ok(None))
}

/// Extract the third image (mask) from a 3-input PipelineOp, if present.
/// Returns shared materialized pixels ready for GPU upload.
fn extract_third_image(op: &PipelineOp) -> Result<Option<Arc<DynamicImage>>, PilError> {
    match op {
        PipelineOp::CompositeModule { mask, .. } => mask.materialized_shared().map(Some),
        PipelineOp::Paste { mask, .. } => mask
            .as_ref()
            .map(|image| image.materialized_shared())
            .transpose(),
        PipelineOp::Autocontrast { mask, .. } => mask
            .as_ref()
            .map(|image| image.materialized_shared())
            .transpose(),
        // PutAlphaData owns an operation-ready L mask already. Reuse its Arc
        // directly so adding the GPU mask binding does not create a second
        // host image or trigger another materialization.
        PipelineOp::PutAlphaData { mask, .. } => Ok(Some(Arc::clone(mask))),
        _ => Ok(None),
    }
}

/// Count the samples participating in an Autocontrast histogram. This is
/// scalar control-plane work only; the pixel histogram itself is accumulated
/// by the GPU gather pass. Keeping the count on the host lets the cutoff pass
/// use exact integer thresholds instead of depending on device f32 rounding.
fn gpu_autocontrast_selected_pixels(
    width: u32,
    height: u32,
    mask: Option<&DynamicImage>,
) -> Result<usize, PilError> {
    let image_pixels = CheckedDims::new(width, height, 1)?.total_pixels();
    let Some(mask) = mask else {
        return Ok(image_pixels);
    };
    if mask.dimensions() != (width, height) {
        return Err(PilError::ValueError(
            "Autocontrast mask dimensions do not match image".into(),
        ));
    }
    Ok(mask
        .to_luma8()
        .pixels()
        .filter(|pixel| pixel[0] != 0)
        .count())
}

fn gpu_autocontrast_cutoff_indices(selected_pixels: usize, cutoff: f64) -> (u32, u32) {
    if selected_pixels == 0 {
        return (0, 0);
    }
    let total = selected_pixels as f64;
    let low = (total * cutoff / 100.0) as usize;
    let high = (total * (100.0 - cutoff) / 100.0) as usize;
    (
        low.min(u32::MAX as usize) as u32,
        high.min(selected_pixels.saturating_sub(1))
            .min(u32::MAX as usize) as u32,
    )
}

struct AuxiliaryImages {
    second: Option<Arc<DynamicImage>>,
    third: Option<Arc<DynamicImage>>,
}

fn extract_auxiliary_images(
    op: &PipelineOp,
    primary_dimensions: (u32, u32),
    draw_source: Option<&DynamicImage>,
) -> Result<AuxiliaryImages, PilError> {
    // Pillow resolves each operation's source before its mask, then advances
    // to the next operation. Preserve that observable error order instead of
    // collecting one auxiliary slot across the whole batch at a time.
    Ok(AuxiliaryImages {
        second: extract_second_image(op, Some(primary_dimensions), draw_source)?,
        third: extract_third_image(op)?,
    })
}

/// Extract and pack LUT data from a PipelineOp into [u32; 256] for GPU upload.
/// Each u32 packs RGBA channels for one LUT entry (R in byte 0, G byte 1, B byte 2, A byte 3).
fn extract_lut(op: &PipelineOp, mode: u32) -> Option<[u32; 256]> {
    if let PipelineOp::RemapPalette { dest_map } = op {
        let mut inverse = [0u8; 256];
        for (new_index, &old_index) in dest_map.iter().take(256).enumerate() {
            inverse[usize::from(old_index)] = new_index as u8;
        }
        let mut packed = [0u32; 256];
        for (entry, &value) in packed.iter_mut().zip(inverse.iter()) {
            let value = u32::from(value);
            *entry = value | (value << 8) | (value << 16) | (value << 24);
        }
        return Some(packed);
    }
    let lut_bytes: &[u8] = match op {
        PipelineOp::Eval { lut } | PipelineOp::PointOp { lut } => lut.as_ref(),
        _ => return None,
    };
    let channels = match mode {
        0 => 1usize,
        1 => 2,
        2 => 3,
        3 | 4 => 4,
        _ => return None,
    };
    if lut_bytes.len() != channels * 256 {
        return None;
    }
    let mut packed = [0u32; 256];
    // CPU Eval/PointOp stores one complete 256-entry table per logical band.
    // Pack those band-major tables into the per-index RGBA transport used by
    // the shader. Alpha is looked up for LA/RGBA; opaque modes use 255.
    for (i, p) in packed.iter_mut().enumerate() {
        let r = u32::from(lut_bytes[i]);
        let g = if channels >= 3 {
            u32::from(lut_bytes[256 + i])
        } else {
            r
        };
        let b = if channels >= 3 {
            u32::from(lut_bytes[512 + i])
        } else {
            r
        };
        let a = if channels == 2 {
            u32::from(lut_bytes[256 + i])
        } else if channels == 4 {
            u32::from(lut_bytes[768 + i])
        } else {
            255
        };
        *p = r | (g << 8) | (b << 16) | (a << 24);
    }
    Some(packed)
}

/// Build the byte LUT represented by a GPU-compatible point operation.
///
/// `Invert`, `InvertChops`, `Solarize`, and `Posterize` normally have their
/// own shaders.  For a contiguous run of point operations their per-channel
/// byte semantics are exactly representable by the generic LUT shader.  The
/// mode restrictions mirror the public ImageOps constructors: alpha-bearing
/// Solarize/Posterize operations stay on their existing shader path, while
/// explicit Eval/PointOp tables remain valid for every packed byte mode.
fn gpu_point_lut(op: &PipelineOp, mode: u32) -> Option<Vec<u8>> {
    let channels = match mode {
        0 => 1usize,
        1 => 2,
        2 => 3,
        3 | 4 => 4,
        _ => return None,
    };
    if let PipelineOp::Eval { lut } | PipelineOp::PointOp { lut } = op {
        return (lut.len() == channels * 256).then(|| lut.to_vec());
    }
    if matches!(
        op,
        PipelineOp::Solarize { .. } | PipelineOp::Posterize { .. }
    ) && matches!(mode, 1 | 3)
    {
        return None;
    }

    let map = |value: u8| -> Option<u8> {
        match op {
            PipelineOp::Invert | PipelineOp::InvertChops => Some(255 - value),
            PipelineOp::Solarize { threshold } => Some(if value >= *threshold {
                255 - value
            } else {
                value
            }),
            PipelineOp::Posterize { bits } if (1..=8).contains(bits) => {
                let mask = !((1u8 << (8 - bits)) - 1);
                Some(value & mask)
            }
            _ => None,
        }
    };
    let mut lut = Vec::with_capacity(channels * 256);
    for _ in 0..channels {
        for value in 0..=u8::MAX {
            lut.push(map(value)?);
        }
    }
    Some(lut)
}

/// Return whether a logical mode has the same byte-band contract as the
/// concrete image storage.  The point LUT composes complete byte-band maps,
/// so this is exact for ordinary L/LA/RGB/RGBA images even when the public
/// lazy image retains its explicit mode tag.  Palette and typed modes remain
/// excluded because their public point operations have additional sample
/// semantics beyond a packed byte lookup.
fn gpu_byte_point_mode_allowed(image: &DynamicImage, mode: Option<&str>) -> bool {
    match image {
        DynamicImage::ImageLuma8(_) => matches!(mode, None | Some("L")),
        DynamicImage::ImageLumaA8(_) => matches!(mode, None | Some("LA")),
        DynamicImage::ImageRgb8(_) => matches!(mode, None | Some("RGB")),
        DynamicImage::ImageRgba8(_) => matches!(mode, None | Some("RGBA")),
        _ => false,
    }
}

/// Collapse adjacent exact point operations into one generic LUT dispatch.
///
/// The public operation count remains the original count in the outer
/// telemetry receipt; only the GPU execution plan is rewritten.  Non-point
/// operations terminate a run, so geometry, neighborhood, and multi-image
/// ordering are unchanged.
fn fuse_gpu_point_ops(ops: &[PipelineOp], mode: u32) -> Vec<PipelineOp> {
    let mut fused = Vec::with_capacity(ops.len());
    let mut index = 0usize;
    while index < ops.len() {
        let Some(first) = gpu_point_lut(&ops[index], mode) else {
            fused.push(ops[index].clone());
            index += 1;
            continue;
        };
        let mut composed = first;
        let mut consumed = 1usize;
        while index + consumed < ops.len() {
            let Some(next) = gpu_point_lut(&ops[index + consumed], mode) else {
                break;
            };
            for band in 0..(composed.len() / 256) {
                let offset = band * 256;
                for value in 0..256usize {
                    composed[offset + value] = next[offset + composed[offset + value] as usize];
                }
            }
            consumed += 1;
        }
        if consumed >= 2 {
            fused.push(PipelineOp::PointOp {
                lut: composed.into(),
            });
        } else {
            fused.push(ops[index].clone());
        }
        index += consumed;
    }
    fused
}

fn transpose_output_dimensions(method: &TransposeMethod, width: u32, height: u32) -> (u32, u32) {
    match method {
        TransposeMethod::Rotate90
        | TransposeMethod::Rotate270
        | TransposeMethod::Transpose
        | TransposeMethod::Transverse => (height, width),
        _ => (width, height),
    }
}

fn transpose_forward(
    method: &TransposeMethod,
    width: u32,
    height: u32,
    x: u32,
    y: u32,
) -> (u32, u32) {
    match method {
        TransposeMethod::FlipLeftRight => (width - 1 - x, y),
        TransposeMethod::FlipTopBottom => (x, height - 1 - y),
        TransposeMethod::Rotate90 => (y, width - 1 - x),
        TransposeMethod::Rotate180 => (width - 1 - x, height - 1 - y),
        TransposeMethod::Rotate270 => (height - 1 - y, x),
        TransposeMethod::Transpose => (y, x),
        TransposeMethod::Transverse => (height - 1 - y, width - 1 - x),
    }
}

/// Compose adjacent GPU transpose operations before resources and dispatches
/// are planned. The seven Pillow methods form a closed dihedral transform
/// set, so corner mapping is an exact composition check rather than a pixel
/// approximation.
fn compose_transpose_methods(
    first: &TransposeMethod,
    second: &TransposeMethod,
    width: u32,
    height: u32,
) -> Option<TransposeMethod> {
    if width == 0 || height == 0 {
        return None;
    }
    let middle_dimensions = transpose_output_dimensions(first, width, height);
    let output_dimensions =
        transpose_output_dimensions(second, middle_dimensions.0, middle_dimensions.1);
    let corners = [
        (0, 0),
        (width - 1, 0),
        (0, height - 1),
        (width - 1, height - 1),
    ];
    let candidates = [
        TransposeMethod::FlipLeftRight,
        TransposeMethod::FlipTopBottom,
        TransposeMethod::Rotate90,
        TransposeMethod::Rotate180,
        TransposeMethod::Rotate270,
        TransposeMethod::Transpose,
        TransposeMethod::Transverse,
    ];
    candidates.into_iter().find(|candidate| {
        if transpose_output_dimensions(candidate, width, height) != output_dimensions {
            return false;
        }
        corners.iter().all(|&(x, y)| {
            let middle = transpose_forward(first, width, height, x, y);
            let expected = transpose_forward(
                second,
                middle_dimensions.0,
                middle_dimensions.1,
                middle.0,
                middle.1,
            );
            transpose_forward(candidate, width, height, x, y) == expected
        })
    })
}

fn fuse_gpu_transpose_ops(ops: &[PipelineOp], width: u32, height: u32) -> Vec<PipelineOp> {
    let mut fused = Vec::with_capacity(ops.len());
    let mut index = 0usize;
    while index < ops.len() {
        let PipelineOp::Transpose { method } = &ops[index] else {
            fused.push(ops[index].clone());
            index += 1;
            continue;
        };
        let mut combined = method.clone();
        let mut consumed = 1usize;
        while index + consumed < ops.len() {
            let PipelineOp::Transpose { method: next } = &ops[index + consumed] else {
                break;
            };
            let Some(composed) = compose_transpose_methods(&combined, next, width, height) else {
                break;
            };
            combined = composed;
            consumed += 1;
        }
        fused.push(PipelineOp::Transpose { method: combined });
        index += consumed;
    }
    fused
}

/// Return whether two adjacent public Chops operations can share one exact
/// dual-input GPU traversal.  The source identity guard is important: the
/// fused shader consumes the same secondary image for both formulas, so two
/// equal-looking but independently constructed images must retain the normal
/// two-dispatch path.
fn can_fuse_gpu_multiply_screen(ops: &[PipelineOp], index: usize) -> bool {
    if index + 1 >= ops.len() {
        return false;
    }
    match (&ops[index], &ops[index + 1]) {
        (PipelineOp::Multiply { other: first }, PipelineOp::Screen { other: second }) => {
            first.shares_execution_source(second)
        }
        _ => false,
    }
}

fn gpu_dispatch_count(
    ops: &[PipelineOp],
    logical_mode: Option<&str>,
    image_dimensions: (u32, u32),
) -> u64 {
    let mut count = 0u64;
    let mut index = 0usize;
    let (mut cur_w, mut cur_h) = image_dimensions;
    while index < ops.len() {
        if can_fuse_gpu_multiply_screen(ops, index) {
            count += 1;
            if let Some(next) = op_output_dims(&ops[index + 1], cur_w, cur_h) {
                (cur_w, cur_h) = next;
            }
            index += 2;
            continue;
        }
        let next = op_output_dims(&ops[index], cur_w, cur_h).unwrap_or((cur_w, cur_h));
        count += if ops.len() == 1
            && gpu_f_resize_compact_box_vertical_only_geometry(
                &ops[index],
                (cur_w, cur_h),
                next,
                logical_mode,
            ) {
            // The unchanged horizontal axis is an identity copy.  The
            // vertical compact reducer can consume the original source
            // directly, so this proof emits one native dispatch.
            1
        } else if matches!(
            &ops[index],
            PipelineOp::Autocontrast { .. } | PipelineOp::Equalize
        ) {
            // Histogram operations are one public step but four device
            // dispatches: clear, gather, LUT derivation, and remap.
            4
        } else if matches!(&ops[index], PipelineOp::Fit { .. })
            || matches!(&ops[index], PipelineOp::Resize { filter, .. }
            if !matches!(filter, ResampleFilter::Nearest)
                || gpu_resize_nearest_uses_coefficients(logical_mode))
        {
            2
        } else if matches!(&ops[index], PipelineOp::Pad { .. }) {
            // Pad is an exact resize followed by a fill/copy placement pass.
            3
        } else {
            GpuInner::blur_pass_count(&ops[index]).map_or(1usize, |passes| passes.saturating_mul(2))
                as u64
        };
        (cur_w, cur_h) = next;
        index += 1;
    }
    count
}

/// Compute output dimensions for a size-changing op given current input dimensions.
/// Returns `None` if the op does not change the image dimensions.
fn round_positive_ties_even(value: f64) -> f64 {
    if !value.is_finite() || value <= 0.0 {
        return 0.0;
    }
    let lower = value.floor();
    let fraction = value - lower;
    if fraction < 0.5 {
        lower
    } else if fraction > 0.5 || (lower as u64) % 2 == 1 {
        lower + 1.0
    } else {
        lower
    }
}

/// Reproduce Pillow's reverse affine planner for `Image.rotate`.
///
/// The native implementation rounds the trigonometric coefficients to
/// fifteen decimal places, computes the expanded bounds from the transformed
/// image edges, and then applies the center-preserving expansion shift.  Keep
/// this control-plane calculation identical to the CPU/SIMD geometry helpers;
/// the resulting coefficients are consumed by the real GPU Transform shader.
fn gpu_rotate_affine(
    angle: f64,
    expand: bool,
    _fill: Option<(u8, u8, u8, u8)>,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    width: u32,
    height: u32,
) -> Option<([f64; 6], (u32, u32))> {
    let sw = f64::from(width);
    let sh = f64::from(height);
    let rad = -angle.to_radians();
    let a = crate::ops::rotate::round_rotate_coefficient(rad.cos());
    let b = crate::ops::rotate::round_rotate_coefficient(rad.sin());
    let d = crate::ops::rotate::round_rotate_coefficient(-rad.sin());
    let e = a;
    let (center_x, center_y) = center.unwrap_or((sw / 2.0, sh / 2.0));
    let (translate_x, translate_y) = translate.unwrap_or((0.0, 0.0));
    let mut c = a * (-center_x - translate_x) + b * (-center_y - translate_y) + center_x;
    let mut f = d * (-center_x - translate_x) + e * (-center_y - translate_y) + center_y;
    let transform = |x: f64, y: f64, c: f64, f: f64| (a * x + b * y + c, d * x + e * y + f);
    let corners = [(0.0, 0.0), (sw, 0.0), (sw, sh), (0.0, sh)];
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(x, y) in &corners {
        let (rx, ry) = transform(x, y, c, f);
        min_x = min_x.min(rx);
        max_x = max_x.max(rx);
        min_y = min_y.min(ry);
        max_y = max_y.max(ry);
    }
    let (out_w, out_h) = if expand {
        let w = max_x.ceil() - min_x.floor();
        let h = max_y.ceil() - min_y.floor();
        if !w.is_finite()
            || !h.is_finite()
            || w < 0.0
            || h < 0.0
            || w > f64::from(u32::MAX)
            || h > f64::from(u32::MAX)
        {
            return None;
        }
        (w as u32, h as u32)
    } else {
        (width, height)
    };
    if expand {
        let shift_x = -(f64::from(out_w) - sw) / 2.0;
        let shift_y = -(f64::from(out_h) - sh) / 2.0;
        (c, f) = transform(shift_x, shift_y, c, f);
    }
    Some(([a, b, c, d, e, f], (out_w, out_h)))
}

/// Return the floating-point source crop box used by ImageOps.fit.
///
/// Fit's public contract resamples this box directly; rounding it to integer
/// crop coordinates before resizing changes edge pixels for non-centred or
/// bleed-enabled calls. The exact device path uses these values to build the
/// same boxed coefficient tables as the scalar implementation.
fn gpu_fit_box(
    source_w: u32,
    source_h: u32,
    output_w: u32,
    output_h: u32,
    bleed: f64,
    centering: (f64, f64),
) -> Option<(f64, f64, f64, f64)> {
    if source_w == 0 || source_h == 0 || output_w == 0 || output_h == 0 {
        return None;
    }
    let b = if (0.0..0.5).contains(&bleed) {
        bleed
    } else {
        0.0
    };
    let source_w = f64::from(source_w);
    let source_h = f64::from(source_h);
    let output_w = f64::from(output_w);
    let output_h = f64::from(output_h);
    let bleed_w = b * source_w;
    let bleed_h = b * source_h;
    let live_w = (source_w - 2.0 * bleed_w).max(1.0);
    let live_h = (source_h - 2.0 * bleed_h).max(1.0);
    let live_ratio = live_w / live_h;
    let output_ratio = output_w / output_h;
    let (crop_w, crop_h) = if (live_ratio - output_ratio).abs() < 1e-10 {
        (live_w, live_h)
    } else if live_ratio >= output_ratio {
        (output_ratio * live_h, live_h)
    } else {
        (live_w, live_w / output_ratio)
    };
    let cx = centering.0.clamp(0.0, 1.0);
    let cy = centering.1.clamp(0.0, 1.0);
    let left = bleed_w + (live_w - crop_w) * cx;
    let top = bleed_h + (live_h - crop_h) * cy;
    let values = (left, top, crop_w, crop_h);
    [values.0, values.1, values.2, values.3]
        .into_iter()
        .all(|value| value.is_finite() && (value as f32).is_finite())
        .then_some(values)
}

fn op_output_dims(op: &PipelineOp, cur_w: u32, cur_h: u32) -> Option<(u32, u32)> {
    match op {
        PipelineOp::Resize { w, h, .. } => Some((*w, *h)),
        PipelineOp::Contain { .. } | PipelineOp::Cover { .. } => {
            gpu_contain_cover_output_dims(op, cur_w, cur_h)
        }
        PipelineOp::Pad { w, h, .. } => Some((*w, *h)),
        PipelineOp::Crop {
            left,
            top,
            right,
            bottom,
        } => Some((right.checked_sub(*left)?, bottom.checked_sub(*top)?)),
        PipelineOp::Expand { border, .. } => {
            let border = border.checked_mul(2)?;
            let new_w = cur_w.checked_add(border)?;
            let new_h = cur_h.checked_add(border)?;
            Some((new_w, new_h))
        }
        PipelineOp::CropBorder { border } => {
            let border = border.checked_mul(2)?;
            let new_w = cur_w.checked_sub(border)?;
            let new_h = cur_h.checked_sub(border)?;
            Some((new_w, new_h))
        }
        PipelineOp::Rotate {
            angle,
            expand,
            fill,
            center,
            translate,
            ..
        } => gpu_rotate_affine(*angle, *expand, *fill, *center, *translate, cur_w, cur_h)
            .map(|(_, dimensions)| dimensions),
        PipelineOp::Thumbnail { w, h, .. } => gpu_thumbnail_output_dims(cur_w, cur_h, *w, *h),
        PipelineOp::Fit { w, h, .. } => Some((*w, *h)),
        PipelineOp::Transpose { method } => {
            if matches!(
                method,
                TransposeMethod::Rotate90
                    | TransposeMethod::Rotate270
                    | TransposeMethod::Transpose
                    | TransposeMethod::Transverse
            ) {
                Some((cur_h, cur_w))
            } else {
                Some((cur_w, cur_h))
            }
        }
        PipelineOp::Reduce { x_factor, y_factor } => {
            let fx = (*x_factor).max(1);
            let fy = (*y_factor).max(1);
            Some((cur_w.div_ceil(fx), cur_h.div_ceil(fy)))
        }
        PipelineOp::Scale { factor, .. } => {
            // ImageOps.scale uses Python's round(width * factor), including
            // ties-to-even at half-pixel products.
            let new_w = round_positive_ties_even(f64::from(cur_w) * factor);
            let new_h = round_positive_ties_even(f64::from(cur_h) * factor);
            if !new_w.is_finite()
                || !new_h.is_finite()
                || new_w < 0.0
                || new_h < 0.0
                || new_w > f64::from(u32::MAX)
                || new_h > f64::from(u32::MAX)
            {
                return None;
            }
            let new_w = new_w.max(1.0) as u32;
            let new_h = new_h.max(1.0) as u32;
            Some((new_w, new_h))
        }
        PipelineOp::Transform { w, h, .. } => Some((*w, *h)),
        PipelineOp::LinearGradient { .. } | PipelineOp::RadialGradient { .. } => Some((256, 256)),
        PipelineOp::EffectMandelbrot { w, h, .. } => Some((*w, *h)),
        PipelineOp::CompositeModule { other, .. } => other.size().ok(),
        _ => None,
    }
}

/// Compute the scalar aspect-ratio result used by ImageOps.contain/cover.
///
/// These public operations do not have a distinct pixel algorithm: after
/// Pillow chooses the rounded output size they call the ordinary resize path.
/// Keep the calculation in the GPU planner identical to the CPU ImageOps
/// implementation, including ties-to-even rounding and Cover's minimum size
/// clamp. Returning `None` for zero/invalid geometry leaves the original public
/// operation on its normal CPU/error path.
fn gpu_contain_cover_output_dims(
    op: &PipelineOp,
    source_w: u32,
    source_h: u32,
) -> Option<(u32, u32)> {
    let (target_w, target_h, is_cover) = match op {
        PipelineOp::Contain { w, h, .. } => (*w, *h, false),
        PipelineOp::Cover { w, h, .. } => (*w, *h, true),
        _ => return None,
    };
    if source_w == 0 || source_h == 0 || target_w == 0 || target_h == 0 {
        return None;
    }

    let image_ratio = f64::from(source_w) / f64::from(source_h);
    let destination_ratio = f64::from(target_w) / f64::from(target_h);
    if (image_ratio - destination_ratio).abs() < 1e-10 {
        return Some((target_w, target_h));
    }

    let adjust_height = (image_ratio > destination_ratio) != is_cover;
    if adjust_height {
        let height = round_positive_ties_even(
            f64::from(source_h) / f64::from(source_w) * f64::from(target_w),
        ) as u32;
        Some((target_w, if is_cover { height.max(1) } else { height }))
    } else {
        let width = round_positive_ties_even(
            f64::from(source_w) / f64::from(source_h) * f64::from(target_h),
        ) as u32;
        Some((if is_cover { width.max(1) } else { width }, target_h))
    }
}

/// Return the exact intermediate resize size and integer placement used by
/// ImageOps.pad.  Pillow performs contain sizing with ties-to-even rounding,
/// then chooses the axis-dependent rounded paste offset.  The values are
/// scalar control-plane decisions; the resize and final fill/copy stay on the
/// device.
fn gpu_pad_geometry(
    op: &PipelineOp,
    source_w: u32,
    source_h: u32,
) -> Option<((u32, u32), (u32, u32))> {
    let PipelineOp::Pad {
        w, h, centering, ..
    } = op
    else {
        return None;
    };
    if source_w == 0 || source_h == 0 || *w == 0 || *h == 0 {
        return None;
    }

    let image_ratio = f64::from(source_w) / f64::from(source_h);
    let destination_ratio = f64::from(*w) / f64::from(*h);
    let (raw_w, raw_h) = if (image_ratio - destination_ratio).abs() < 1e-10 {
        (f64::from(*w), f64::from(*h))
    } else if image_ratio > destination_ratio {
        (
            f64::from(*w),
            round_positive_ties_even(f64::from(source_h) / f64::from(source_w) * f64::from(*w)),
        )
    } else {
        (
            round_positive_ties_even(f64::from(source_w) / f64::from(source_h) * f64::from(*h)),
            f64::from(*h),
        )
    };
    if !raw_w.is_finite()
        || !raw_h.is_finite()
        || raw_w < 0.0
        || raw_h < 0.0
        || raw_w > f64::from(u32::MAX)
        || raw_h > f64::from(u32::MAX)
    {
        return None;
    }
    let raw_w_u32 = raw_w as u32;
    let raw_h_u32 = raw_h as u32;
    let resize_w = raw_w_u32.max(1);
    let resize_h = raw_h_u32.max(1);
    let cx = centering.0.clamp(0.0, 1.0);
    let cy = centering.1.clamp(0.0, 1.0);
    let (offset_x, offset_y) = if raw_w_u32 != *w {
        (
            round_positive_ties_even(f64::from(*w - raw_w_u32) * cx) as u32,
            0,
        )
    } else {
        (
            0,
            round_positive_ties_even(f64::from(*h - raw_h_u32) * cy) as u32,
        )
    };
    Some(((resize_w, resize_h), (offset_x, offset_y)))
}

fn gpu_pad_fill(op: &PipelineOp, logical_mode: Option<&str>, mode: u32) -> u32 {
    let PipelineOp::Pad { color, .. } = op else {
        return 0;
    };
    // I and F store one scalar sample in the packed four-byte transport. The
    // public color resolver has already converted scalar/named colors to
    // that representation; preserve the complete word instead of
    // interpreting the bytes as RGB channels. An omitted fill is scalar
    // +0.0/+0, not an opaque RGBA black.
    if matches!(logical_mode, Some("I" | "F")) {
        let (a, b, c, d) = color.unwrap_or((0, 0, 0, 0));
        return u32::from(a) | (u32::from(b) << 8) | (u32::from(c) << 16) | (u32::from(d) << 24);
    }
    let has_alpha =
        matches!(logical_mode, Some("LA" | "PA" | "RGBA" | "RGBa")) || matches!(mode, 1 | 3);
    let (r, g, b, a) = color.unwrap_or((0, 0, 0, if has_alpha { 0 } else { 255 }));
    u32::from(r) | (u32::from(g) << 8) | (u32::from(b) << 16) | (u32::from(a) << 24)
}

/// Return true for operations whose output dimensions are not simply the
/// current input dimensions. `op_output_dims` returns `None` for an overflow or
/// an unavailable nested-image size, so callers must distinguish that from a
/// genuinely dimension-preserving operation before dispatching.
fn op_has_explicit_output_dimensions(op: &PipelineOp) -> bool {
    matches!(
        op,
        PipelineOp::Resize { .. }
            | PipelineOp::Pad { .. }
            | PipelineOp::Crop { .. }
            | PipelineOp::Expand { .. }
            | PipelineOp::CropBorder { .. }
            | PipelineOp::Rotate { .. }
            | PipelineOp::Transpose { .. }
            | PipelineOp::Thumbnail { .. }
            | PipelineOp::Contain { .. }
            | PipelineOp::Cover { .. }
            | PipelineOp::Fit { .. }
            | PipelineOp::Reduce { .. }
            | PipelineOp::Scale { .. }
            | PipelineOp::Transform { .. }
            | PipelineOp::CompositeModule { .. }
            | PipelineOp::LinearGradient { .. }
            | PipelineOp::RadialGradient { .. }
            | PipelineOp::EffectMandelbrot { .. }
    )
}

fn auxiliary_dimensions(image: Option<&DynamicImage>) -> Option<(u32, u32)> {
    image.map(DynamicImage::dimensions)
}

/// The ordinary GPU transport is packed RGBA8. Do not let
/// `DynamicImage::to_rgba8` silently narrow a 16-bit or floating-point source
/// before a shader runs: those formats have different Pillow sample semantics
/// and must stay on the CPU path until a native GPU representation exists.
fn gpu_image_layout_is_supported(image: &DynamicImage) -> bool {
    matches!(
        image,
        DynamicImage::ImageLuma8(_)
            | DynamicImage::ImageLumaA8(_)
            | DynamicImage::ImageRgb8(_)
            | DynamicImage::ImageRgba8(_)
    )
}

/// `I;16*` geometry can use the packed storage buffers without narrowing a
/// sample to the ordinary byte layout. Relocation and nearest paths copy the
/// packed word; a single filtered resize is admitted only when its typed
/// f64-coefficient proof covers both device passes. Other arithmetic, fills,
/// and mixed-input operations remain on exact host semantic control until
/// their native typed-sample contracts are proven.
fn gpu_luma16_geometry_is_supported(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if !matches!(image, DynamicImage::ImageLuma16(_)) || ops.is_empty() {
        return false;
    }
    // A filtered resize is admitted only as a pure, single operation whose
    // f64 coefficient products and both native u16 pass boundaries were
    // proven by the host reducer.  Keep nearest/relocation geometry on the
    // established opaque-word path, but do not mix it with the new arithmetic
    // marker until a chained typed contract exists.
    if ops.iter().any(|op| {
        matches!(
            op,
            PipelineOp::Resize {
                filter,
                ..
            } if !matches!(filter, crate::pipeline::ResampleFilter::Nearest)
        )
    }) {
        return gpu_luma16_resize_f64_is_exact(ops, image, logical_mode);
    }
    ops.iter().all(|op| {
        matches!(
            op,
            PipelineOp::Offset { .. }
                | PipelineOp::Flip
                | PipelineOp::Mirror
                | PipelineOp::Transpose { .. }
                | PipelineOp::Crop { .. }
                | PipelineOp::CropBorder { .. }
                | PipelineOp::Resize {
                    filter: crate::pipeline::ResampleFilter::Nearest,
                    ..
                }
                | PipelineOp::Contain {
                    filter: crate::pipeline::ResampleFilter::Nearest,
                    ..
                }
                | PipelineOp::Cover {
                    filter: crate::pipeline::ResampleFilter::Nearest,
                    ..
                }
                | PipelineOp::Transform { .. }
                | PipelineOp::Duplicate
        )
    })
}

/// `I;16*` conversion uses the same one-word-per-sample transport as native
/// geometry, but the words are uploaded in numeric little-endian form so the
/// Convert shader can clamp the unsigned sample before producing byte output.
/// Keep the conversion terminal: once it emits L/LA/RGB/RGBA bytes, the batch
/// mode changes and the typed source contract no longer applies.
fn gpu_luma16_convert_is_supported(ops: &[PipelineOp], image: &DynamicImage) -> bool {
    matches!(image, DynamicImage::ImageLuma16(_))
        && !ops.is_empty()
        && ops
            .iter()
            .filter(|op| matches!(op, PipelineOp::Convert { .. }))
            .count()
            == 1
        && ops.iter().all(|op| match op {
            PipelineOp::Offset { .. }
            | PipelineOp::Flip
            | PipelineOp::Mirror
            | PipelineOp::Transpose { .. }
            | PipelineOp::Crop { .. }
            | PipelineOp::CropBorder { .. }
            | PipelineOp::Duplicate => true,
            PipelineOp::Convert {
                mode: target,
                matrix: None,
                dither: None,
            } => gpu_standard_color_mode_code(target).is_some(),
            _ => false,
        })
}

/// The GPU Paste shader has a separate numeric-u16 branch for one typed
/// `I;16*` destination/source pair. Keep the first contract deliberately
/// narrow: one Paste operation, with no preceding byte-oriented operation
/// changing the representation. Mask validation is handled after auxiliary
/// images are materialized, where the concrete L/RGBA mask layout is known.
fn gpu_luma16_paste_is_supported(ops: &[PipelineOp], image: &DynamicImage) -> bool {
    matches!(image, DynamicImage::ImageLuma16(_)) && matches!(ops, [PipelineOp::Paste { .. }])
}

fn gpu_luma16_paste_source(op: &PipelineOp, mode: u32, image: &DynamicImage) -> bool {
    mode == 5
        && matches!(op, PipelineOp::Paste { .. })
        && matches!(image, DynamicImage::ImageLuma16(_))
}

/// `F` stores one little-endian `f32` sample in each four-byte word. The
/// order-statistic shaders have a separate mode-8 path that compares those
/// samples as floats instead of independently comparing their four bytes.
/// Require finite source and raw `PutData(F)` words for now: this gives WGSL
/// ordering the same total behavior as Pillow's finite-value path while NaN
/// ordering remains an explicit future contract rather than an accidental
/// driver-dependent result.
fn gpu_float_filter_is_supported(ops: &[PipelineOp], image: &DynamicImage) -> bool {
    let DynamicImage::ImageRgba8(pixels) = image else {
        return false;
    };
    let expected = (image.width() as usize)
        .checked_mul(image.height() as usize)
        .and_then(|count| count.checked_mul(4));
    let finite_words = |bytes: &[u8]| {
        expected == Some(bytes.len())
            && bytes.chunks_exact(4).remainder().is_empty()
            && bytes
                .chunks_exact(4)
                .all(|word| f32::from_le_bytes([word[0], word[1], word[2], word[3]]).is_finite())
    };
    expected == Some(pixels.as_raw().len())
        && !ops.is_empty()
        && finite_words(pixels.as_raw())
        && ops.iter().all(|op| match op {
            PipelineOp::Mirror => true,
            PipelineOp::PutData {
                data,
                mode: PixelMode::F,
            } => finite_words(data),
            PipelineOp::MaxFilter { .. }
            | PipelineOp::MinFilter { .. }
            | PipelineOp::MedianFilter { .. }
            | PipelineOp::RankFilter { .. } => gpu_operation_is_safe(op),
            _ => false,
        })
}

/// `I` stores one signed little-endian i32 sample in each four-byte word.
/// Its convolution contract is different from the byte filter: coefficients
/// are applied to the decoded sample, negative results clamp to zero, and
/// positive results retain the full signed-integer range. The WGSL path uses
/// the same f32 accumulation as Pillow's I-mode implementation, including
/// the reversed rows and +0.5 bias.
fn gpu_int_filter_is_supported(ops: &[PipelineOp], image: &DynamicImage) -> bool {
    let DynamicImage::ImageRgba8(pixels) = image else {
        return false;
    };
    let expected = (image.width() as usize)
        .checked_mul(image.height() as usize)
        .and_then(|count| count.checked_mul(4));
    if expected != Some(pixels.as_raw().len()) || ops.is_empty() {
        return false;
    }
    let kernel_is_safe = |kernel: &[f32], scale: f32, offset: f32| {
        let Some(offset) = registry::filter_offset_i32(offset) else {
            return false;
        };
        if !scale.is_finite() || scale == 0.0 {
            return false;
        }
        let normalized_scale = scale;
        if !normalized_scale.is_finite() || normalized_scale == 0.0 {
            return false;
        }
        let coefficient_bound = kernel.iter().try_fold(0.0f64, |total, value| {
            let normalized = f64::from(*value) / f64::from(normalized_scale);
            normalized.is_finite().then_some(total + normalized.abs())
        });
        let Some(coefficient_bound) = coefficient_bound else {
            return false;
        };
        // Bound the f32 accumulation for every possible i32 source sample.
        // The output helper also clamps the positive conversion explicitly,
        // matching Rust's saturating float-to-i32 cast at the core boundary.
        let worst_case = f64::from(i32::MAX) * coefficient_bound + f64::from(offset).abs() + 1.0;
        worst_case.is_finite() && worst_case < f64::from(f32::MAX)
    };
    let mut has_filter = false;
    for op in ops {
        match op {
            PipelineOp::Filter3x3 {
                kernel,
                scale,
                offset,
            } => {
                has_filter = true;
                if !kernel_is_safe(kernel, *scale, *offset) {
                    return false;
                }
            }
            PipelineOp::Filter5x5 {
                kernel,
                scale,
                offset,
            } => {
                has_filter = true;
                if !kernel_is_safe(kernel, *scale, *offset) {
                    return false;
                }
            }
            PipelineOp::PutData {
                mode: PixelMode::I, ..
            } => continue,
            _ => return false,
        }
    }
    has_filter
}

/// Return whether an I-mode convolution followed by a nearest resize can
/// remain in one device batch.
///
/// The 3x3 filter shader is the typed I32 path described by
/// `pool_cpu/ops/filter.rs`; the following nearest resize only relocates the
/// resulting four-byte signed words through host-generated one-tap tables.
/// Keep the composition deliberately narrow: a 5x5 filter, filtered resize,
/// another arithmetic operation, or a mode-changing node would consume the
/// words with a different contract and must retain exact host semantic
/// control until its own composition proof is recorded.
fn gpu_int_filter_resize_chain_is_supported(ops: &[PipelineOp], image: &DynamicImage) -> bool {
    let [
        filter,
        PipelineOp::Resize {
            w,
            h,
            filter: resize_filter,
        },
    ] = ops
    else {
        return false;
    };
    if !matches!(resize_filter, ResampleFilter::Nearest)
        || *w == 0
        || *h == 0
        || !matches!(filter, PipelineOp::Filter3x3 { .. })
        || !gpu_int_filter_is_supported(std::slice::from_ref(filter), image)
    {
        return false;
    }
    op_output_dims(
        &PipelineOp::Resize {
            w: *w,
            h: *h,
            filter: *resize_filter,
        },
        image.width(),
        image.height(),
    )
    .is_some_and(|(out_w, out_h)| out_w == *w && out_h == *h)
}

/// Return whether the native merge shader can consume Pillow's indexed first
/// band without expanding its palette.
///
/// `ImagingMerge` accepts a `P` image only in the first position of a
/// multi-band byte merge and treats that image as raw index bytes.  The
/// ordinary merge shader has the same byte-interleave contract for canonical
/// RGB, but it does not implement LAB's signed-channel bias or any typed
/// scalar destination.  Keep this admission to one validated RGB merge so a
/// palette lookup can never be silently replaced by an unrelated channel
/// conversion.
fn gpu_palette_first_rgb_merge_is_supported(ops: &[PipelineOp], mode: Option<&str>) -> bool {
    if mode != Some("P") {
        return false;
    }
    let [
        PipelineOp::Merge {
            mode: ColorMode::RGB,
            logical_mode,
            bands,
        },
    ] = ops
    else {
        return false;
    };
    logical_mode == "RGB"
        && bands.len() == 3
        && bands
            .first()
            .is_some_and(|band| band.mode().ok().as_deref() == Some("P"))
        && bands
            .iter()
            .skip(1)
            .all(|band| band.mode().ok().as_deref() == Some("L"))
}

/// Validate the index space assumptions made by multi-input shaders. A
/// storage-array read is not bounds-checked by WGSL, so an image that is
/// smaller than the coordinates used by a shader must never be uploaded for a
/// GPU dispatch. Operations with intentionally different canvases (Paste and
/// CompositeModule) are checked against the dimensions encoded in their
/// parameter contract instead of being forced to match the destination.
fn gpu_auxiliary_shapes_are_safe(
    op: &PipelineOp,
    auxiliary: &AuxiliaryImages,
    cur_w: u32,
    cur_h: u32,
) -> bool {
    let second = auxiliary_dimensions(auxiliary.second.as_deref());
    let third = auxiliary_dimensions(auxiliary.third.as_deref());
    let current = (cur_w, cur_h);

    match op {
        PipelineOp::EffectSpread { .. } => {
            let expected = CheckedDims::new(cur_w, cur_h, 1)
                .ok()
                .and_then(|dims| u32::try_from(dims.total_pixels()).ok());
            auxiliary.third.is_none() && second == expected.map(|width| (width, 1))
        }
        PipelineOp::EffectNoise { .. } => auxiliary.third.is_none() && second == Some(current),
        PipelineOp::Merge { bands, .. } => {
            let expected = CheckedDims::new(cur_w, cur_h, 1)
                .ok()
                .and_then(|dims| {
                    dims.total_pixels()
                        .checked_mul(bands.len().saturating_sub(1))
                })
                .and_then(|pixels| u32::try_from(pixels).ok());
            auxiliary.third.is_none()
                && match (second, expected) {
                    (None, Some(0)) => true,
                    (Some((w, h)), Some(pixels)) => h == 1 && w == pixels,
                    _ => false,
                }
        }
        PipelineOp::Color3DLut { table, .. } => {
            auxiliary.third.is_none()
                && second
                    == color3dlut_table_words(table)
                        .ok()
                        .and_then(|words| u32::try_from(words.len()).ok())
                        .map(|width| (width, 1))
        }
        PipelineOp::Paste { w, h, mask, .. } => {
            if *w <= 0 || *h <= 0 {
                return false;
            }
            let source = (*w as u32, *h as u32);
            second == Some(source)
                && match mask {
                    Some(_) => third == Some(source),
                    None => third.is_none(),
                }
        }
        PipelineOp::CompositeModule { .. } => third == Some(current),
        PipelineOp::PutAlphaData { .. } => third == Some(current),
        PipelineOp::AlphaComposite { .. } => second == Some(current),
        _ => match (second, third) {
            (None, None) => true,
            (second, third) => {
                second
                    .map(|dimensions| dimensions == current)
                    .unwrap_or(true)
                    && third
                        .map(|dimensions| dimensions == current)
                        .unwrap_or(true)
            }
        },
    }
}

/// A zero-area two-input operation has no pixel work to dispatch. Treat it as a
/// successful GPU no-op after validating its auxiliary image contract; this
/// keeps empty Pillow images out of the CPU fallback without pretending that a
/// zero-work storage-buffer dispatch occurred.
fn gpu_empty_two_input_batch_is_noop(ops: &[PipelineOp], image: &DynamicImage) -> bool {
    (image.width() == 0 || image.height() == 0)
        && !ops.is_empty()
        && ops.iter().all(|op| {
            matches!(
                op,
                PipelineOp::AlphaComposite { .. }
                    | PipelineOp::Multiply { .. }
                    | PipelineOp::Screen { .. }
                    | PipelineOp::Darker { .. }
                    | PipelineOp::Lighter { .. }
                    | PipelineOp::Difference { .. }
                    | PipelineOp::Overlay { .. }
                    | PipelineOp::HardLight { .. }
                    | PipelineOp::SoftLight { .. }
                    | PipelineOp::AddModulo { .. }
                    | PipelineOp::SubtractModulo { .. }
                    | PipelineOp::Add { .. }
                    | PipelineOp::Subtract { .. }
                    | PipelineOp::BlendModule { .. }
                    | PipelineOp::LogicalAnd { .. }
                    | PipelineOp::LogicalOr { .. }
                    | PipelineOp::LogicalXor { .. }
            )
        })
}

fn gpu_empty_two_input_inputs_are_safe(
    ops: &[PipelineOp],
    image: &DynamicImage,
    auxiliary_images: &[AuxiliaryImages],
) -> bool {
    ops.len() == auxiliary_images.len()
        && auxiliary_images
            .iter()
            .enumerate()
            .all(|(index, auxiliary)| {
                auxiliary.second.as_ref().is_some_and(|second| {
                    gpu_image_layout_is_supported(second)
                        && second.dimensions() == image.dimensions()
                }) && auxiliary.third.is_none()
                    && gpu_auxiliary_shapes_are_safe(
                        &ops[index],
                        auxiliary,
                        image.width(),
                        image.height(),
                    )
                    && gpu_auxiliary_modes_are_safe(&ops[index], image, auxiliary)
            })
}

/// A Reduce by `(1, 1)` is Pillow's identity operation. It must return an
/// independent image, but it has no reduction work for a GPU to perform. Such
/// operations can be removed from a GPU batch after validation, allowing the
/// remaining real pixel operations to stay on the GPU as well.
fn gpu_reduce_is_identity(op: &PipelineOp) -> bool {
    matches!(
        op,
        PipelineOp::Reduce {
            x_factor: 1,
            y_factor: 1
        }
    )
}

/// Return whether `ImageOps.fit` is the exact identity for this GPU segment.
///
/// With no bleed and equal source/target dimensions, the fit planner's crop
/// box is the complete source image.  Pillow's boxed resize is then an exact
/// sample-preserving copy for every byte resampling filter; use the native
/// Duplicate shader instead of building redundant fractional-box coefficient
/// tables for this case.
/// Keep the lowering limited to ordinary packed byte modes so a logical mode
/// whose sample contract is not represented by the RGBA transport cannot be
/// silently narrowed by the optimization.
fn gpu_fit_is_exact_identity(
    op: &PipelineOp,
    source_dimensions: (u32, u32),
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    let PipelineOp::Fit { w, h, bleed, .. } = op else {
        return false;
    };
    source_dimensions.0 > 0
        && source_dimensions.1 > 0
        && (*w, *h) == source_dimensions
        && *bleed == 0.0
        && gpu_image_layout_is_supported(image)
        && logical_mode.is_none_or(|mode| matches!(mode, "L" | "LA" | "RGB" | "RGBA"))
}

/// Replace valid aspect-ratio/Scale nodes with the equivalent exact Resize
/// operation.
///
/// Pillow computes each Scale output axis with ties-to-even rounding before
/// calling its resize implementation. The GPU executor already carries that
/// dimension contract and has exact nearest/fixed-point convolution kernels
/// for Resize, so retaining a separate floating-point Scale shader would
/// duplicate the mapping and risk a one-pixel boundary difference. Invalid
/// or unrepresentable factors are deliberately left unchanged; the normal
/// public-operation preflight then reports the same error or CPU fallback.
fn expand_gpu_geometry_ops(
    ops: &[PipelineOp],
    image: &DynamicImage,
    dimensions: (u32, u32),
    logical_mode: Option<&str>,
) -> Vec<PipelineOp> {
    let mut expanded = Vec::with_capacity(ops.len());
    let (mut cur_w, mut cur_h) = dimensions;
    let i_resize_identity_is_exact = dimensions == image.dimensions()
        && gpu_i_resize_identity_is_exact(ops, image, logical_mode);
    for op in ops {
        if i_resize_identity_is_exact {
            // The proof is restricted to the one pure same-size resize, so a
            // raw-word Duplicate is equivalent to Pillow's typed identity
            // result and avoids entering the filtered-I host-control branch.
            expanded.push(PipelineOp::Duplicate);
            continue;
        }
        if gpu_fit_is_exact_identity(op, (cur_w, cur_h), image, logical_mode) {
            // The operation still needs an independent result image and a
            // terminal GPU receipt, so lower to a real copy rather than
            // dropping Fit as a no-op.
            expanded.push(PipelineOp::Duplicate);
            continue;
        }
        // Pillow's default Thumbnail reducing_gap=2.0 performs an integer
        // box reduction before the final resize.  When both source axes are
        // divisible by the chosen factors, the exact two-step contract is
        // representable by the native Reduce and Resize kernels.  Preserve
        // the original node for partial edge blocks; those use a fractional
        // resize box and remain on the exact host path until that geometry is
        // carried by the device plan.
        if let PipelineOp::Thumbnail { filter, .. } = op {
            let effective_filter = if matches!(logical_mode, Some("1" | "P")) {
                ResampleFilter::Nearest
            } else {
                *filter
            };
            let has_alpha =
                matches!(
                    image,
                    DynamicImage::ImageLumaA8(_) | DynamicImage::ImageRgba8(_)
                ) && !matches!(logical_mode, Some("F" | "I" | "CMYK" | "RGBa" | "RGBX"));
            if !gpu_thumbnail_requires_exact_host_control(op, (cur_w, cur_h), image, logical_mode)
                && !matches!(effective_filter, ResampleFilter::Nearest)
                && !has_alpha
                && !matches!(logical_mode, Some("F" | "I"))
                && let Some((out_w, out_h)) = op_output_dims(op, cur_w, cur_h)
            {
                let factor_x = ((f64::from(cur_w) / f64::from(out_w) / 2.0) as u32).max(1);
                let factor_y = ((f64::from(cur_h) / f64::from(out_h) / 2.0) as u32).max(1);
                if (factor_x > 1 || factor_y > 1) && cur_w % factor_x == 0 && cur_h % factor_y == 0
                {
                    expanded.push(PipelineOp::Reduce {
                        x_factor: factor_x,
                        y_factor: factor_y,
                    });
                    expanded.push(PipelineOp::Resize {
                        w: out_w,
                        h: out_h,
                        filter: effective_filter,
                    });
                    cur_w = out_w;
                    cur_h = out_h;
                    continue;
                }
            }
        }
        let replacement = match op {
            PipelineOp::Scale { filter, .. }
            | PipelineOp::Contain { filter, .. }
            | PipelineOp::Cover { filter, .. } => match op_output_dims(op, cur_w, cur_h) {
                Some((w, h)) => PipelineOp::Resize {
                    w,
                    h,
                    filter: *filter,
                },
                None => op.clone(),
            },
            // Thumbnail has already computed its aspect-preserving output
            // dimensions at the public boundary.  Lower it to the exact
            // separable Resize implementation so every resampling filter
            // (including bicubic/lanczos) uses the same fixed-point kernels
            // as Image.resize instead of the older bilinear-only shader.
            PipelineOp::Thumbnail { filter, .. }
                if gpu_thumbnail_requires_exact_host_control(
                    op,
                    (cur_w, cur_h),
                    image,
                    logical_mode,
                ) =>
            {
                op.clone()
            }
            PipelineOp::Thumbnail { filter, .. } => match op_output_dims(op, cur_w, cur_h) {
                Some((w, h)) => PipelineOp::Resize {
                    w,
                    h,
                    filter: if matches!(logical_mode, Some("1" | "P")) {
                        ResampleFilter::Nearest
                    } else {
                        *filter
                    },
                },
                None => op.clone(),
            },
            // Rotation is an affine mapping after Pillow's scalar geometry
            // planner has selected the expanded canvas.  Feed that mapping
            // into the reviewed Transform shader; exact right-angle
            // rotations use the byte-relocation transpose kernel instead.
            PipelineOp::Rotate { .. }
                if gpu_rotate_requires_exact_host_control(image, logical_mode)
                    && !matches!(logical_mode, Some("1" | "P"))
                    && !gpu_rotate_has_exact_transpose_lowering(
                        op,
                        logical_mode,
                        (cur_w, cur_h),
                    )
                    && !gpu_rotate_nearest_affine_is_exact(
                        op,
                        image,
                        logical_mode,
                        (cur_w, cur_h),
                    ) =>
            {
                op.clone()
            }
            PipelineOp::Rotate {
                angle,
                expand,
                fill,
                center,
                translate,
                filter,
                nearest,
            } => {
                let normalized_angle = angle.rem_euclid(360.0);
                if center.is_none() && translate.is_none() && normalized_angle.abs() <= f64::EPSILON
                {
                    PipelineOp::Duplicate
                } else {
                    let right_angle = if gpu_rotate_has_exact_transpose_lowering(
                        op,
                        logical_mode,
                        (cur_w, cur_h),
                    ) {
                        match angle.rem_euclid(360.0) {
                            value if (value - 90.0).abs() < f64::EPSILON => {
                                Some(TransposeMethod::Rotate90)
                            }
                            value if (value - 180.0).abs() < f64::EPSILON => {
                                Some(TransposeMethod::Rotate180)
                            }
                            value if (value - 270.0).abs() < f64::EPSILON => {
                                Some(TransposeMethod::Rotate270)
                            }
                            _ => None,
                        }
                    } else {
                        None
                    };
                    if let Some(method) = right_angle {
                        PipelineOp::Transpose { method }
                    } else if !*nearest && !matches!(filter, ResampleFilter::Bilinear) {
                        // The reviewed Transform lowering implements Pillow's
                        // bilinear byte kernel only. Keep bicubic and any
                        // future filtered rotate requests on the exact CPU
                        // path until their arithmetic is separately proven.
                        op.clone()
                    } else if let Some((affine, (w, h))) =
                        gpu_rotate_affine(*angle, *expand, *fill, *center, *translate, cur_w, cur_h)
                    {
                        // `resolve_imageops_color` keeps LA/PA rotate colors
                        // in the public RGBA-shaped tuple `(gray, gray,
                        // gray, alpha)`, while Transform's native two-band
                        // fill contract is `(gray, alpha, 0, 0)`. Normalize
                        // only this lowered rotate node so the shader packs
                        // the logical alpha into byte three; direct
                        // Transform fills already use the latter layout.
                        let transform_fill = if matches!(logical_mode, Some("LA" | "PA")) {
                            (*fill).map(|(gray, _, _, alpha)| (gray, alpha, 0, 0))
                        } else {
                            *fill
                        };
                        PipelineOp::Transform {
                            w,
                            h,
                            method: TransformMethod::Affine,
                            data: Arc::from(affine.to_vec()),
                            filter: if *nearest || matches!(logical_mode, Some("1" | "P")) {
                                ResampleFilter::Nearest
                            } else {
                                *filter
                            },
                            fill: transform_fill,
                            fill_is_none: fill.is_none(),
                            palette_fill: None,
                        }
                    } else {
                        op.clone()
                    }
                }
            }
            _ => op.clone(),
        };
        if let Some(next) = op_output_dims(&replacement, cur_w, cur_h) {
            (cur_w, cur_h) = next;
        }
        expanded.push(replacement);
    }
    expanded
}

/// Check dimensions before creating a device or uploading any image data.
/// Empty images and zero-sized GPU outputs are valid CPU/Pillow states, but
/// they are not valid storage-buffer dispatches: sampling kernels subtract one
/// from source dimensions and readback needs a non-empty copy range. Operations
/// whose host/shader output-size contract is incomplete are deliberately kept
/// on the CPU until that contract is explicit.
fn gpu_dimensions_require_cpu(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    let dimensions_fit = |w: u32, h: u32| {
        CheckedDims::new(w, h, 1)
            .map(|dims| dims.total_pixels() <= GPU_BUFFER_CAPACITY as usize)
            .unwrap_or(false)
    };

    if !gpu_image_layout_is_supported(image)
        && !gpu_luma16_geometry_is_supported(ops, image, None)
        && !gpu_luma16_convert_is_supported(ops, image)
        && !gpu_luma16_paste_is_supported(ops, image)
    {
        return true;
    }
    let source_mode = mode_code(image);
    if ops.iter().any(|op| {
        matches!(op, PipelineOp::Eval { .. } | PipelineOp::PointOp { .. })
            && extract_lut(op, source_mode).is_none()
    }) {
        return true;
    }
    let (mut cur_w, mut cur_h) = image.dimensions();
    if cur_w == 0 || cur_h == 0 || !dimensions_fit(cur_w, cur_h) {
        return true;
    }
    for op in ops {
        if gpu_operation_mode_requires_cpu(op, image) {
            return true;
        }
        // CPU PutPixel reports IndexError for an out-of-range coordinate;
        // the shader would silently perform no write. Route that case to the
        // CPU before device initialization so error semantics are preserved.
        if let PipelineOp::PutPixel { x, y, .. } = op {
            if *x >= cur_w || *y >= cur_h {
                return true;
            }
        }
        let next = match op_output_dims(op, cur_w, cur_h) {
            Some(dimensions) => dimensions,
            None if op_has_explicit_output_dimensions(op) => return true,
            None => (cur_w, cur_h),
        };
        if let PipelineOp::Resize { filter, .. }
        | PipelineOp::Scale { filter, .. }
        | PipelineOp::Contain { filter, .. }
        | PipelineOp::Cover { filter, .. } = op
        {
            // A direct F Box reduction to one column can use the compact
            // repeated-coefficient vertical shader. Check that proof before
            // asking for the full vertical table, which could otherwise
            // allocate hundreds of MiB only to reject an over-limit row.
            let compact_vertical = logical_mode == Some("F")
                && gpu_f_resize_compact_box_vertical_only_is_exact(ops, image, logical_mode);
            let compact_box = logical_mode == Some("F")
                && ops.len() == 1
                && matches!(ops.first(), Some(PipelineOp::Resize { filter, .. }) if matches!(filter, ResampleFilter::Box))
                && gpu_f_resize_compact_box_is_exact(ops, image, logical_mode);
            if compact_vertical {
                let horizontal = gpu_resize_coefficients(next.0, cur_w, *filter);
                if resize_coeff_word_count(&horizontal).is_err() {
                    return true;
                }
            } else if compact_box {
                // Marker 13 supplies the compact table for the over-limit
                // axis.  The other axis may still change when its ordinary
                // f64 table fits the binding and every row stays within the
                // ordered reducer's tap cap; the compact proof has already
                // validated both pass results and finite-only chaining.
                let (kernel, support) = filter_from_resample(*filter);
                for (source_size, output_size, compact) in [
                    (cur_w, next.0, gpu_f_resize_compact_box_axis(cur_w, next.0)),
                    (cur_h, next.1, gpu_f_resize_compact_box_axis(cur_h, next.1)),
                ] {
                    if compact {
                        continue;
                    }
                    let coeffs = precompute_coeffs_f64(output_size, source_size, kernel, support);
                    if !gpu_f_resize_f64_coefficients_fit_binding(&coeffs)
                        || coeffs
                            .count
                            .iter()
                            .any(|&count| count > GPU_F_RESIZE_ORDERED_MAX_TAPS)
                    {
                        return true;
                    }
                }
            } else if !gpu_resize_coefficients_are_safe(*filter, (cur_w, cur_h), next) {
                return true;
            }
        }
        if let PipelineOp::Fit {
            bleed,
            centering,
            filter,
            ..
        } = op
        {
            if !gpu_fit_coefficients_are_safe((cur_w, cur_h), next, *bleed, *centering, *filter) {
                return true;
            }
        }
        if let PipelineOp::Pad { filter, .. } = op {
            let Some(((resize_w, resize_h), _)) = gpu_pad_geometry(op, cur_w, cur_h) else {
                return true;
            };
            if !gpu_resize_coefficients_are_safe(*filter, (cur_w, cur_h), (resize_w, resize_h)) {
                return true;
            }
        }
        if next.0 == 0 || next.1 == 0 || !dimensions_fit(next.0, next.1) {
            return true;
        }
        if gpu_shader_work_requires_cpu(op, (cur_w, cur_h), next, None) {
            return true;
        }
        (cur_w, cur_h) = next;
    }
    false
}

/// Some shader math is layout-safe but still differs from Pillow for a
/// particular native mode. Keep this check beside the dimension preflight so
/// those cases fall back before any device or upload work begins.
fn gpu_operation_mode_requires_cpu(op: &PipelineOp, image: &DynamicImage) -> bool {
    match op {
        // AlphaComposite's public implementation is defined for LA/RGBA
        // canvases. On L/RGB, the CPU operation promotes to RGBA while the
        // packed GPU result would otherwise be fed through preserve_mode.
        PipelineOp::AlphaComposite { .. } => !matches!(
            image,
            DynamicImage::ImageLumaA8(_) | DynamicImage::ImageRgba8(_)
        ),
        // ImageChops.blend intentionally converts through RGB and restores
        // an opaque alpha for an alpha-bearing source. Image.blend (the
        // module operation) blends every stored channel, including alpha, so
        // all packed byte layouts use the same shader path.
        PipelineOp::BlendModule { .. } => !matches!(
            image,
            DynamicImage::ImageLuma8(_)
                | DynamicImage::ImageLumaA8(_)
                | DynamicImage::ImageRgb8(_)
                | DynamicImage::ImageRgba8(_)
        ),
        // PutData and alpha promotion carry the logical source/target layout
        // in the operation. A direct core caller can construct a mismatched
        // pair; CPU converts according to that mode, while the packed shader
        // would otherwise reinterpret the existing storage in place.
        PipelineOp::PutData { mode, .. }
        | PipelineOp::PutAlpha { mode, .. }
        | PipelineOp::PutAlphaData { mode, .. } => !pixel_mode_matches_image(*mode, image),
        // The CPU ImageOps implementation runs Posterize/Solarize through
        // an RGB temporary and preserve_mode, which makes alpha opaque for
        // LA/RGBA. The packed shaders currently retain alpha, so keep those
        // native alpha cases on CPU until the public operation is normalized.
        PipelineOp::Posterize { .. } | PipelineOp::Solarize { .. } => matches!(
            image,
            DynamicImage::ImageLumaA8(_) | DynamicImage::ImageRgba8(_)
        ),
        // ImageOps.colorize accepts only an L image and always produces RGB.
        // The shader reads the luma byte directly; running it on a packed RGB
        // source would silently colorize the wrong sample contract.
        PipelineOp::Colorize { .. } => !matches!(image, DynamicImage::ImageLuma8(_)),
        // Autocontrast and Equalize use the packed byte representation only
        // for native L/RGB images.  Their scalar histogram control plane is
        // exact for those layouts; alpha and typed images have different
        // Pillow contracts and must be rejected before any GPU work starts.
        PipelineOp::Autocontrast { .. } | PipelineOp::Equalize => !matches!(
            image,
            DynamicImage::ImageLuma8(_) | DynamicImage::ImageRgb8(_)
        ),
        // getchannel raises IndexError for a band that the source mode does
        // not have; a shader would read byte 3 or an unused packed byte.
        PipelineOp::ExtractBand { index } => {
            usize::from(*index) >= usize::from(image.color().channel_count())
        }
        _ => false,
    }
}

fn pixel_mode_matches_image(mode: PixelMode, image: &DynamicImage) -> bool {
    matches!(
        (mode, image),
        (PixelMode::L, DynamicImage::ImageLuma8(_))
            | (PixelMode::LA, DynamicImage::ImageLumaA8(_))
            | (PixelMode::RGB, DynamicImage::ImageRgb8(_))
            | (PixelMode::RGBA, DynamicImage::ImageRgba8(_))
            // P/PA retain raw index samples in the same one-/two-byte
            // buffers as L/LA until the binding restores palette metadata.
            | (PixelMode::P, DynamicImage::ImageLuma8(_))
            | (PixelMode::PA, DynamicImage::ImageLumaA8(_))
            | (PixelMode::Mode1, DynamicImage::ImageLuma8(_))
            | (PixelMode::YCbCr, DynamicImage::ImageRgb8(_))
            | (PixelMode::HSV, DynamicImage::ImageRgb8(_))
            // CMYK is represented as C/M/Y/K in the four bytes of Rgba8.
            | (PixelMode::CMYK, DynamicImage::ImageRgba8(_))
            // I/F are four-byte logical planes stored without conversion in
            // the RGBA transport; putdata replaces their raw little-endian
            // samples rather than interpreting them as color channels.
            | (PixelMode::I, DynamicImage::ImageRgba8(_))
            | (PixelMode::F, DynamicImage::ImageRgba8(_))
    )
}

fn gpu_auxiliary_layout_is_supported_for_op(
    op: &PipelineOp,
    current_color: crate::raster::ColorType,
    is_second: bool,
    image: &DynamicImage,
) -> bool {
    // A typed I;16 Paste is the one multi-input GPU contract whose source is
    // intentionally not representable by the ordinary RGBA uploader. Keep
    // this exception attached to the exact operation and slot; masks still
    // use the byte-oriented transport below.
    if is_second
        && current_color == crate::raster::ColorType::L16
        && matches!(op, PipelineOp::Paste { .. })
    {
        matches!(image, DynamicImage::ImageLuma16(_))
    } else {
        gpu_image_layout_is_supported(image)
    }
}

fn gpu_pipeline_requires_cpu(
    ops: &[PipelineOp],
    image: &DynamicImage,
    auxiliary_images: &[AuxiliaryImages],
    logical_mode: Option<&str>,
) -> bool {
    if ops.len() != auxiliary_images.len() {
        return true;
    }
    if gpu_dimensions_require_cpu(ops, image, logical_mode) {
        return true;
    }
    let dimensions_fit = |w: u32, h: u32| {
        CheckedDims::new(w, h, 1)
            .map(|dims| dims.total_pixels() <= GPU_BUFFER_CAPACITY as usize)
            .unwrap_or(false)
    };
    let (mut cur_w, mut cur_h) = image.dimensions();
    let mut current_color = image.color();
    for (index, op) in ops.iter().enumerate() {
        let auxiliary = &auxiliary_images[index];
        let auxiliary_is_safe = |is_second: bool, image: &DynamicImage| {
            let (w, h) = image.dimensions();
            gpu_auxiliary_layout_is_supported_for_op(op, current_color, is_second, image)
                && w != 0
                && h != 0
                && dimensions_fit(w, h)
        };
        if auxiliary
            .second
            .as_deref()
            .is_some_and(|second| !auxiliary_is_safe(true, second))
            || auxiliary
                .third
                .as_deref()
                .is_some_and(|third| !auxiliary_is_safe(false, third))
        {
            return true;
        }
        if !gpu_auxiliary_shapes_are_safe(op, auxiliary, cur_w, cur_h) {
            return true;
        }
        if !gpu_auxiliary_modes_are_safe_for_color(op, current_color, auxiliary) {
            return true;
        }
        if let Some(next) = op_output_dims(op, cur_w, cur_h) {
            (cur_w, cur_h) = next;
        }
        current_color = gpu_color_after_op(current_color, op);
    }
    false
}

fn gpu_color_after_op(
    current: crate::raster::ColorType,
    op: &PipelineOp,
) -> crate::raster::ColorType {
    match op {
        PipelineOp::Convert { mode, .. } => match gpu_convert_target_mode_code(mode) {
            Some(0) => crate::raster::ColorType::L8,
            Some(1) => crate::raster::ColorType::La8,
            Some(2) => crate::raster::ColorType::Rgb8,
            Some(3) => crate::raster::ColorType::Rgba8,
            Some(4) | Some(7) | Some(8) => crate::raster::ColorType::Rgba8,
            Some(5) | Some(6) => crate::raster::ColorType::Rgb8,
            _ => current,
        },
        PipelineOp::Grayscale | PipelineOp::ExtractBand { .. } | PipelineOp::Constant { .. } => {
            crate::raster::ColorType::L8
        }
        PipelineOp::Colorize { .. } => crate::raster::ColorType::Rgb8,
        PipelineOp::Color3DLut { target_mode, .. } => match target_mode {
            PixelMode::RGB => crate::raster::ColorType::Rgb8,
            PixelMode::RGBA | PixelMode::CMYK => crate::raster::ColorType::Rgba8,
            _ => current,
        },
        _ => current,
    }
}

/// Multi-input shaders use the primary mode word for every packed sample.
/// Require an auxiliary source to have the same native byte layout; otherwise
/// a legal CPU operation such as RGB-vs-L would be interpreted as unrelated
/// channels on GPU. Paste masks have a separate luma/alpha contract and are
/// checked below against the channel the shader actually samples.
fn gpu_auxiliary_modes_are_safe(
    op: &PipelineOp,
    image: &DynamicImage,
    auxiliary: &AuxiliaryImages,
) -> bool {
    gpu_auxiliary_modes_are_safe_for_color(op, image.color(), auxiliary)
}

fn gpu_auxiliary_modes_are_safe_for_color(
    op: &PipelineOp,
    current_color: crate::raster::ColorType,
    auxiliary: &AuxiliaryImages,
) -> bool {
    if matches!(op, PipelineOp::Autocontrast { mask: Some(_), .. }) {
        // ImageOps.autocontrast validates masks as mode 1/L. Both are
        // represented by the Luma8 transport, whose first byte is exactly the
        // nonzero selector consumed by the histogram gather shader.
        return matches!(
            auxiliary.third.as_deref(),
            Some(DynamicImage::ImageLuma8(_))
        );
    }
    if let PipelineOp::Paste { mask_alpha, .. } = op {
        if current_color == crate::raster::ColorType::L16 {
            let source_is_luma16 = auxiliary
                .second
                .as_deref()
                .is_some_and(|source| matches!(source, DynamicImage::ImageLuma16(_)));
            let mask_is_supported = match (*mask_alpha, auxiliary.third.as_deref()) {
                (false, None) | (false, Some(DynamicImage::ImageLuma8(_))) => true,
                (true, Some(DynamicImage::ImageRgba8(_))) => true,
                _ => false,
            };
            return source_is_luma16 && mask_is_supported;
        }
    }
    if let PipelineOp::CompositeModule { mask_alpha, .. } = op {
        let Some(destination) = auxiliary.second.as_ref() else {
            return false;
        };
        let Some(mask) = auxiliary.third.as_ref() else {
            return false;
        };
        // Image.composite converts image1 to image2's mode before the
        // operation is queued. The shader can therefore use one mode word
        // only when both packed sources have the same native layout.
        if destination.color() != current_color {
            return false;
        }
        // Pillow's mask contract selects byte 0 for 1/L and byte 3 for
        // LA/RGBA/RGBa. Reject RGB luma masks here: their CPU path computes
        // weighted luma while the packed shader would read only R.
        return if *mask_alpha {
            matches!(
                mask.as_ref(),
                DynamicImage::ImageLumaA8(_) | DynamicImage::ImageRgba8(_)
            )
        } else {
            matches!(mask.as_ref(), DynamicImage::ImageLuma8(_))
        };
    }
    let requires_matching_source = matches!(
        op,
        PipelineOp::Add { .. }
            | PipelineOp::Subtract { .. }
            | PipelineOp::Multiply { .. }
            | PipelineOp::Screen { .. }
            | PipelineOp::Darker { .. }
            | PipelineOp::Lighter { .. }
            | PipelineOp::Difference { .. }
            | PipelineOp::AddModulo { .. }
            | PipelineOp::SubtractModulo { .. }
            | PipelineOp::LogicalAnd { .. }
            | PipelineOp::LogicalOr { .. }
            | PipelineOp::LogicalXor { .. }
            | PipelineOp::BlendModule { .. }
            | PipelineOp::Paste { .. }
            | PipelineOp::AlphaComposite { .. }
    );
    if requires_matching_source
        && auxiliary
            .second
            .as_ref()
            .is_some_and(|second| second.color() != current_color)
    {
        return false;
    }
    if let PipelineOp::Paste {
        mask_alpha: false, ..
    } = op
    {
        // Paste's luma-mask path calls DynamicImage::to_luma8(), which uses
        // weighted RGB conversion for RGB/RGBA masks. The shader samples byte
        // 0 directly, so only native L/LA masks are exact.
        return !auxiliary.third.as_ref().is_some_and(|mask| {
            !matches!(
                mask.as_ref(),
                DynamicImage::ImageLuma8(_) | DynamicImage::ImageLumaA8(_)
            )
        });
    }
    true
}

/// Return the largest packed RGBA image that a batch needs. Image storage is
/// batch-owned, so allocating to the batch's actual high-water mark avoids
/// reserving the global maximum for every concurrent lazy pipeline.
fn gpu_batch_capacity(
    ops: &[PipelineOp],
    image: &DynamicImage,
    auxiliary_images: &[AuxiliaryImages],
    logical_mode: Option<&str>,
) -> Result<u32, PilError> {
    if ops.len() != auxiliary_images.len() {
        return Err(PilError::InternalError(
            "GPU operation and auxiliary-image counts differ".into(),
        ));
    }

    let pixels = |(w, h): (u32, u32)| -> Result<usize, PilError> {
        Ok(CheckedDims::new(w, h, 1)?.total_pixels())
    };
    let mut high_water = pixels(image.dimensions())?;
    for auxiliary in auxiliary_images {
        for nested in auxiliary
            .second
            .as_ref()
            .into_iter()
            .chain(auxiliary.third.as_ref())
        {
            high_water = high_water.max(pixels(nested.dimensions())?);
        }
    }

    let (mut cur_w, mut cur_h) = image.dimensions();
    for op in ops {
        let (out_w, out_h) = match op_output_dims(op, cur_w, cur_h) {
            Some(dimensions) => dimensions,
            None if op_has_explicit_output_dimensions(op) => {
                return Err(PilError::ValueError(format!(
                    "GPU operation '{}' has no safe output dimensions",
                    registry::variant_key(op)
                )));
            }
            None => (cur_w, cur_h),
        };
        high_water = high_water.max(pixels((out_w, out_h))?);
        let uses_separable_resize = matches!(
            op,
            PipelineOp::Resize { filter, .. }
                if !matches!(filter, ResampleFilter::Nearest)
                    || gpu_resize_nearest_uses_coefficients(logical_mode)
        );
        if uses_separable_resize {
            // The separable resize plan materializes an intermediate frame
            // with the destination width and source height before its
            // vertical pass.  A wide, short resize can therefore need more
            // storage than either endpoint; account for that frame before
            // the device buffers are allocated.  This applies to byte,
            // I/F, and typed I;16 paths alike.
            high_water = high_water.max(pixels((out_w, cur_h))?);
        }
        if let PipelineOp::PutData { data, mode } = op {
            high_water = high_water.max(data.len().div_ceil(mode.channels()));
        }
        (cur_w, cur_h) = (out_w, out_h);
    }

    if high_water == 0 || high_water > GPU_BUFFER_CAPACITY as usize {
        return Err(PilError::ValueError(format!(
            "GPU batch requires {} pixels; supported capacity is {}",
            high_water, GPU_BUFFER_CAPACITY
        )));
    }
    Ok(high_water as u32)
}

/// Check the size of each batch-owned image buffer against the selected
/// adapter's actual limits before calling `Device::create_buffer`.
///
/// The static pixel cap is intentionally conservative, but keeping this
/// check dynamic prevents a future cap change or an adapter with narrower
/// limits from turning a valid public image into a device validation error.
fn gpu_buffer_capacity_exceeds_limits(
    capacity: u32,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
) -> bool {
    let Some(bytes) = u64::from(capacity).checked_mul(4) else {
        return true;
    };
    bytes > u64::from(max_storage_buffer_binding_size) || bytes > max_buffer_size
}

/// Validate the dispatch grid against the adapter limit. Pixel-count limits
/// alone are insufficient: a very wide, short image can fit in storage while
/// still requiring more workgroups in one dimension than the device accepts.
fn gpu_dispatch_dimensions_require_cpu(
    ops: &[PipelineOp],
    image_dimensions: (u32, u32),
    max_workgroups_per_dimension: u32,
    logical_mode: Option<&str>,
) -> bool {
    if max_workgroups_per_dimension == 0 {
        return true;
    }
    let (mut cur_w, mut cur_h) = image_dimensions;
    if cur_w == 0 || cur_h == 0 {
        return true;
    }
    for op in ops {
        let next = match op_output_dims(op, cur_w, cur_h) {
            Some(dimensions) => dimensions,
            None if op_has_explicit_output_dimensions(op) => return true,
            None => (cur_w, cur_h),
        };
        // The rolling blur shaders are deliberately 1x1 workgroups: one
        // invocation owns a complete row or column. Their dispatch grid is
        // therefore (1, height) for the horizontal pass and (width, 1) for
        // the vertical pass, unlike the ordinary 16x16 kernels. Checking
        // only ceil(dim / 16) would admit a tall, narrow image and let the
        // later blur dispatch exceed the adapter's per-dimension limit.
        let dispatch_exceeds_limit = if matches!(op, PipelineOp::Pad { .. }) {
            let Some(((resize_w, resize_h), _)) = gpu_pad_geometry(op, cur_w, cur_h) else {
                return true;
            };
            resize_w.div_ceil(16) > max_workgroups_per_dimension
                || cur_h.div_ceil(16) > max_workgroups_per_dimension
                || resize_w.div_ceil(16) > max_workgroups_per_dimension
                || resize_h.div_ceil(16) > max_workgroups_per_dimension
                || next.0.div_ceil(16) > max_workgroups_per_dimension
                || next.1.div_ceil(16) > max_workgroups_per_dimension
        } else if matches!(
            op,
            PipelineOp::BoxBlur { .. }
                | PipelineOp::BoxBlurXY { .. }
                | PipelineOp::GaussianBlur { .. }
        ) {
            next.1 > max_workgroups_per_dimension || next.0 > max_workgroups_per_dimension
        } else if matches!(op, PipelineOp::Fit { .. })
            || matches!(op, PipelineOp::Resize { filter, .. }
            if !matches!(filter, ResampleFilter::Nearest)
                || gpu_resize_nearest_uses_coefficients(logical_mode))
        {
            // The horizontal resize pass is indexed by output x and source
            // y; the vertical pass is indexed by output x and output y.
            let compact_vertical = ops.len() == 1
                && gpu_f_resize_compact_box_vertical_only_geometry(
                    op,
                    (cur_w, cur_h),
                    next,
                    logical_mode,
                );
            next.0.div_ceil(16) > max_workgroups_per_dimension
                || (!compact_vertical && cur_h.div_ceil(16) > max_workgroups_per_dimension)
                || next.1.div_ceil(16) > max_workgroups_per_dimension
        } else {
            next.0.div_ceil(16) > max_workgroups_per_dimension
                || next.1.div_ceil(16) > max_workgroups_per_dimension
        };
        if next.0 == 0 || next.1 == 0 || dispatch_exceeds_limit {
            return true;
        }
        (cur_w, cur_h) = next;
    }
    false
}

/// Keep finite-but-expensive kernels below a conservative watchdog budget.
/// The estimate counts the inner loop body per output pixel; kernels without
/// dynamic inner work return false and remain eligible for GPU dispatch.
fn gpu_shader_work_items(
    op: &PipelineOp,
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    logical_mode: Option<&str>,
) -> Option<u64> {
    // The convolution shaders guard their neighborhood loads and copy every
    // pixel when no interior exists. Small non-empty images are therefore
    // valid zero-interior GPU workloads; only the shader's invocation count,
    // not the kernel radius, determines their safety.
    let output_pixels = u64::from(output_dimensions.0) * u64::from(output_dimensions.1);
    let (source_w, source_h) = source_dimensions;
    if matches!(op, PipelineOp::Fit { .. })
        || matches!(op, PipelineOp::Resize { filter, .. } | PipelineOp::Scale { filter, .. }
        if !matches!(filter, ResampleFilter::Nearest)
            || gpu_resize_nearest_uses_coefficients(logical_mode))
    {
        // Unlike the single-pass kernels below, the separable resize has one
        // pass over source rows for every output column and a second pass over
        // the final output. Return the total directly; it must not be
        // multiplied by output_pixels a second time.
        return Some(
            u64::from(source_h)
                .saturating_mul(u64::from(output_dimensions.0))
                .saturating_add(output_pixels),
        );
    }
    if matches!(op, PipelineOp::Pad { .. }) {
        let Some(((resize_w, resize_h), _)) =
            gpu_pad_geometry(op, source_dimensions.0, source_dimensions.1)
        else {
            return None;
        };
        // Horizontal resize visits source rows for every intermediate
        // column, vertical resize visits every intermediate pixel, and the
        // final placement visits the requested canvas once.
        return Some(
            u64::from(source_dimensions.1)
                .saturating_mul(u64::from(resize_w))
                .saturating_add(u64::from(resize_w).saturating_mul(u64::from(resize_h)))
                .saturating_add(output_pixels),
        );
    }
    let inner_work = match op {
        PipelineOp::BoxBlur { radius } => {
            let _ = radius;
            // One remove/add update per pixel in each of the horizontal and
            // vertical rolling passes. Count four channels plus edge reads;
            // the radius-sized initialization is paid once per row/column.
            24
        }
        PipelineOp::BoxBlurXY {
            radius_x,
            radius_y,
            passes,
        } => {
            let _ = (radius_x, radius_y);
            24 * u64::from((*passes).max(1))
        }
        PipelineOp::GaussianBlur { sigma } => {
            let _ = sigma;
            // GaussianBlur expands to three horizontal and three vertical
            // rolling passes. The estimate is radius-independent; each
            // pass advances one window per output pixel and pays its
            // radius-sized initialization once per row/column.
            72
        }
        PipelineOp::MedianFilter { size } | PipelineOp::RankFilter { size, .. } => {
            // These shaders insertion-sort four channel arrays. The sort is
            // quadratic in the window area, so size² alone understates the
            // work by orders of magnitude and can admit watchdog-triggering
            // dispatches for the supported 9x9 maximum.
            let area = u64::from(*size).saturating_mul(u64::from(*size));
            let channels = match logical_mode {
                Some("F" | "I" | "L") => 1,
                Some("LA") => 2,
                Some("RGB" | "YCbCr" | "HSV") => 3,
                _ => 4,
            };
            area.saturating_mul(area).saturating_mul(channels)
        }
        PipelineOp::MaxFilter { size } | PipelineOp::MinFilter { size } => u64::from(*size)
            .saturating_mul(u64::from(*size))
            .saturating_mul(4),
        PipelineOp::Filter3x3 { .. } => 9,
        PipelineOp::Filter5x5 { .. } => 25,
        PipelineOp::Reduce { x_factor, y_factor } => {
            let block_w = u64::from((*x_factor).max(1).min(source_w.max(1)));
            let block_h = u64::from((*y_factor).max(1).min(source_h.max(1)));
            block_w.saturating_mul(block_h)
        }
        PipelineOp::EffectMandelbrot { quality, .. } => u64::from(*quality),
        PipelineOp::PutData { mode, .. } => mode.channels() as u64,
        PipelineOp::Pad { .. } => 4,
        // Even a constant-work shader consumes one invocation per output
        // pixel. Count that work so a long point-operation chain is split
        // before its cumulative dispatch cost can monopolize one submission.
        _ => 1,
    };
    Some(output_pixels.saturating_mul(inner_work))
}

fn gpu_shader_work_requires_cpu(
    op: &PipelineOp,
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    logical_mode: Option<&str>,
) -> bool {
    gpu_shader_work_items(op, source_dimensions, output_dimensions, logical_mode)
        .is_none_or(|work| work > MAX_GPU_SHADER_WORK_ITEMS)
}

// ─── GpuPool ───────────────────────────────────────────────────────────────

/// GPU compute pool — wgpu-based compute shader dispatch.
///
/// Uses packed u32 RGBA and 16x16 workgroups. GPU is lazily initialized on the
/// first GPU capability query or execution. If wgpu cannot provide an adapter,
/// the capability query reports no support so automatic routing can select the
/// next host backend before any device buffer is created.
pub struct GpuPool;

fn gpu_operation_is_safe(op: &PipelineOp) -> bool {
    let finite_f32 = |value: f64| value.is_finite() && (value as f32).is_finite();
    match op {
        PipelineOp::Filter3x3 {
            kernel,
            scale,
            offset,
        } => {
            registry::gpu_filter_kernel_is_exact(kernel, *scale, *offset)
                || registry::gpu_filter_rational_denominator(kernel, *scale, *offset).is_some()
        }
        PipelineOp::Filter5x5 {
            kernel,
            scale,
            offset,
        } => {
            registry::gpu_filter_kernel_is_exact(kernel, *scale, *offset)
                || registry::gpu_filter_rational_denominator(kernel, *scale, *offset).is_some()
        }
        PipelineOp::GaussianBlur { sigma } => {
            registry::separable_gaussian_blur_radius(*sigma).is_some()
        }
        PipelineOp::BoxBlur { radius } => *radius <= MAX_GPU_BLUR_RADIUS,
        PipelineOp::BoxBlurXY {
            radius_x,
            radius_y,
            passes,
        } => {
            *passes > 0
                && *passes <= 3
                && registry::separable_box_blur_params_f32(*radius_x).is_some()
                && registry::separable_box_blur_params_f32(*radius_y).is_some()
        }
        PipelineOp::MedianFilter { size }
        | PipelineOp::MaxFilter { size }
        | PipelineOp::MinFilter { size } => {
            *size >= 1 && *size <= MAX_GPU_FILTER_SIZE && *size % 2 == 1
        }
        PipelineOp::RankFilter { size, .. } => {
            *size >= 1 && *size <= MAX_GPU_FILTER_SIZE && *size % 2 == 1
        }
        PipelineOp::Reduce { x_factor, y_factor } => {
            *x_factor <= MAX_GPU_REDUCE_FACTOR
                && *y_factor <= MAX_GPU_REDUCE_FACTOR
                && *x_factor >= 1
                && *y_factor >= 1
        }
        // The CPU implementation shifts by `8 - bits`; zero is not a valid
        // direct PipelineOp value and would otherwise underflow that shift.
        // Public constructors already clamp to 1..=8, but keep the GPU
        // safety gate correct for callers that build the pipeline directly.
        PipelineOp::Posterize { bits } => (1..=8).contains(bits),
        PipelineOp::ExtractBand { index } => *index < 4,
        // Keep the scalar preflight in lockstep with the WGSL fixed-point
        // parameterization. The helper checks every byte result against
        // Pillow's f64 contract before a dispatch is admitted.
        PipelineOp::Brightness { factor } => registry::gpu_brightness_factor_int(*factor).is_some(),
        PipelineOp::Contrast { factor } => registry::gpu_contrast_factor_int(*factor).is_some(),
        PipelineOp::ColorSaturation { factor } => {
            registry::gpu_color_saturation_factor_int(*factor).is_some()
        }
        PipelineOp::Sharpness { factor } => registry::gpu_sharpness_factor_int(*factor).is_some(),
        PipelineOp::Add { scale, offset, .. } => {
            // The scalar preflight exhaustively compares all byte pairs with
            // the f32 values consumed by the real WGSL kernel.
            registry::gpu_chops_affine_params(*scale, *offset, false).is_some()
        }
        PipelineOp::Subtract { scale, offset, .. } => {
            registry::gpu_chops_affine_params(*scale, *offset, true).is_some()
        }
        PipelineOp::BlendModule { alpha, .. } => registry::gpu_blend_alpha_params(*alpha).is_some(),
        PipelineOp::Scale { factor, .. } => {
            factor.is_finite()
                && *factor > 0.0
                && (*factor * 65536.0).is_finite()
                && *factor * 65536.0 >= 1.0
                && *factor * 65536.0 <= MAX_GPU_SCALE_FIXED_POINT
        }
        PipelineOp::Contain { w, h, .. }
        | PipelineOp::Cover { w, h, .. }
        | PipelineOp::Pad { w, h, .. } => *w > 0 && *h > 0,
        PipelineOp::EffectSpread { distance } => *distance <= i32::MAX as u32,
        PipelineOp::EffectNoise { sigma } => finite_f32(*sigma),
        PipelineOp::Rotate {
            angle,
            center,
            translate,
            ..
        } => {
            angle.is_finite()
                && center
                    .map(|(x, y)| x.is_finite() && y.is_finite())
                    .unwrap_or(true)
                && translate
                    .map(|(x, y)| x.is_finite() && y.is_finite())
                    .unwrap_or(true)
        }
        PipelineOp::Transform {
            w,
            h,
            method,
            data,
            filter,
            ..
        } => {
            let shape_valid = match method {
                TransformMethod::Affine | TransformMethod::Perspective | TransformMethod::Quad => {
                    data.len()
                        >= if matches!(method, TransformMethod::Affine) {
                            6
                        } else {
                            8
                        }
                }
                // The GPU mesh lowering carries one complete 12-value mesh
                // record in its uniform block. Larger meshes remain on the
                // CPU until a bounded auxiliary mesh buffer is available.
                TransformMethod::Mesh => data.len() == 12,
            };
            *w > 0
                && *h > 0
                && shape_valid
                && data.iter().copied().all(finite_f32)
                // The image-aware admission proof owns the narrow
                // direct/axis unit-relocation cases for filtered Quad and
                // one-record Mesh transforms. Keep arbitrary filtered maps
                // on the exact host semantic path before this coarse check
                // reaches execution.
                && (!matches!(method, TransformMethod::Mesh)
                    || matches!(
                        filter,
                        ResampleFilter::Nearest
                            | ResampleFilter::Bilinear
                            | ResampleFilter::Bicubic
                    ))
        }
        PipelineOp::AlphaComposite { dest, src, .. } => *dest == (0, 0) && *src == (0, 0),
        PipelineOp::Autocontrast { cutoff, .. } => finite_f32(*cutoff),
        PipelineOp::EffectMandelbrot {
            w,
            h,
            x0,
            y0,
            x1,
            y1,
            quality,
        } => {
            *w > 0
                && *h > 0
                && *quality >= 1
                && *quality <= MAX_GPU_MANDELBROT_ITERS
                && [*x0, *y0, *x1, *y1].into_iter().all(finite_f32)
        }
        _ => true,
    }
}

/// Return whether an operation's GPU capability depends on the source image.
///
/// Convolution filters and geometry transforms are valid for more than one
/// concrete image contract. The byte filter path is admitted only for kernels
/// proven exact by the operation validator, while the I-mode path uses the
/// same shader with signed samples and a different accumulation contract.
/// Likewise, Transform admission depends on the materialized source layout
/// and method-specific map proof; a non-finite projective map is still a
/// valid Pillow operation whose exact result must be produced by host control.
/// The operation-only routing pass runs before lazy source materialization, so
/// it must defer these operations to image-aware preflight instead of
/// manufacturing a backend capability error merely because it cannot inspect
/// the concrete image yet.
fn gpu_operation_requires_image_context(op: &PipelineOp) -> bool {
    matches!(
        op,
        PipelineOp::Filter3x3 { .. } | PipelineOp::Filter5x5 { .. } | PipelineOp::Transform { .. }
    )
}

/// Return whether the current packed geometry path still needs the exact
/// host implementation for this concrete logical sample contract. The GPU
/// geometry kernels are intentionally retained for ordinary byte-mode work,
/// while Pillow's Thumbnail/fit reducing-gap and typed F/I convolution paths
/// have additional rounding/storage rules. Proven typed I;16 filtered
/// resizes are admitted by their two-pass reducer proof; every other typed
/// geometry remains on exact host semantic control until its device plan is
/// proven.
/// Return whether a rotate node can use the exact byte-relocation lowering.
///
/// Pillow's right-angle fast paths are precisely the byte-relocation
/// transpose contract, regardless of the requested resampling filter. A
/// 180-degree rotation is a fast path with either expansion setting; 90/270
/// are fast paths when expansion is requested or the source is square.
/// Keeping this predicate deliberately narrow is important: the general
/// affine shader still has different pixel-center and fill semantics for
/// fractional angles, rectangular non-expanded rotations, and custom
/// geometry.
fn gpu_rotate_has_exact_transpose_lowering(
    op: &PipelineOp,
    mode: Option<&str>,
    source_dimensions: (u32, u32),
) -> bool {
    let PipelineOp::Rotate {
        angle,
        expand,
        center,
        translate,
        ..
    } = op
    else {
        return false;
    };
    if center.is_some() || translate.is_some() {
        return false;
    }
    if mode.is_some_and(|value| {
        !matches!(
            value,
            "1" | "P"
                | "PA"
                | "L"
                | "LA"
                | "RGB"
                | "RGBA"
                | "RGBX"
                | "RGBa"
                | "CMYK"
                | "HSV"
                | "YCbCr"
                | "I"
                | "F"
                | "I;16"
                | "I;16L"
                | "I;16B"
                | "I;16N"
        )
    }) {
        return false;
    }
    // Transpose is a complete-word relocation.  The shader's mode-7/8
    // branches preserve all four bytes of I/F samples, while mode 5 keeps the
    // native two-byte I;16 payload in the low word (the typed readback drops
    // the transport padding).  These modes therefore have the same exact
    // right-angle contract as packed byte layouts; filtered/fractional
    // rotations remain on their typed semantic paths.
    let normalized = angle.rem_euclid(360.0);
    if (normalized - 180.0).abs() <= f64::EPSILON {
        return true;
    }
    ((normalized - 90.0).abs() <= f64::EPSILON || (normalized - 270.0).abs() <= f64::EPSILON)
        && (*expand || source_dimensions.0 == source_dimensions.1)
}

fn gpu_thumbnail_output_dims(
    source_width: u32,
    source_height: u32,
    target_width: u32,
    target_height: u32,
) -> Option<(u32, u32)> {
    if source_width == 0 || source_height == 0 || target_width == 0 || target_height == 0 {
        return None;
    }
    // `Image::thumbnail` computes the aspect-preserving final dimensions
    // before queuing its lazy operation. Recomputing the ratio here would
    // apply the adjustment twice and make backend shape planning disagree
    // with the public image size.
    Some((
        target_width.min(source_width).max(1),
        target_height.min(source_height).max(1),
    ))
}

fn gpu_thumbnail_requires_exact_host_control(
    op: &PipelineOp,
    source_dimensions: (u32, u32),
    image: &DynamicImage,
    mode: Option<&str>,
) -> bool {
    let PipelineOp::Thumbnail { w, h, filter } = op else {
        return false;
    };
    let Some((output_width, output_height)) =
        gpu_thumbnail_output_dims(source_dimensions.0, source_dimensions.1, *w, *h)
    else {
        return true;
    };
    let effective_filter = if matches!(mode, Some("1" | "P")) {
        ResampleFilter::Nearest
    } else {
        *filter
    };
    if matches!(effective_filter, ResampleFilter::Nearest) {
        return false;
    }
    // Thumbnail's reducing_gap=2.0 pass is skipped for alpha images and for
    // scalar F/I contracts.  A plain Resize is exact in those cases.  For
    // ordinary byte images the pool expands the divisible integer-reduce
    // case into native Reduce + Resize below; only partial edge blocks still
    // require the host's fractional resize box.
    let has_alpha = matches!(
        image,
        DynamicImage::ImageLumaA8(_) | DynamicImage::ImageRgba8(_)
    ) && !matches!(mode, Some("F" | "I" | "CMYK" | "RGBa" | "RGBX"));
    let factor_x = ((f64::from(source_dimensions.0) / f64::from(output_width) / 2.0) as u32).max(1);
    let factor_y =
        ((f64::from(source_dimensions.1) / f64::from(output_height) / 2.0) as u32).max(1);
    // When both reducing-gap factors are one, Pillow's F thumbnail is exactly
    // the ordinary filtered Resize. Let the geometry expander lower it to
    // that path so marker-9 can prove heterogeneous/non-dyadic words instead
    // of needlessly publishing an exact-host receipt.
    if mode == Some("F") && factor_x == 1 && factor_y == 1 {
        return false;
    }
    // A finite constant F source is invariant under both Thumbnail's
    // reducing-gap pass and its final resize. Lower this narrow case to the
    // exact constant Resize marker instead of materializing the scalar
    // reducing pass on the host. Typed I still needs its integer rounding
    // contract and remains on exact host semantic control.
    if mode == Some("F") && gpu_f_thumbnail_constant_is_exact(op, source_dimensions, image, mode) {
        return false;
    }
    if matches!(mode, Some("F" | "I")) {
        return true;
    }
    if has_alpha {
        return false;
    }
    (factor_x > 1 || factor_y > 1)
        && (source_dimensions.0 % factor_x != 0 || source_dimensions.1 % factor_y != 0)
}

/// Return whether a constant F Thumbnail can skip the scalar reducing-gap
/// pass without changing Pillow's observable word.
///
/// A zero source remains zero for every finite block average. Nonzero
/// constants are admitted only when both reducing factors are one; Pillow's
/// f32 reduction can otherwise overflow or round the repeated sum before the
/// final resize (even though a direct normalized resize would preserve the
/// constant word).
fn gpu_f_thumbnail_constant_is_exact(
    op: &PipelineOp,
    source_dimensions: (u32, u32),
    image: &DynamicImage,
    mode: Option<&str>,
) -> bool {
    let Some(bits) = gpu_f_source_constant_bits(image, mode) else {
        return false;
    };
    if bits == 0 {
        return true;
    }
    let PipelineOp::Thumbnail { w, h, .. } = op else {
        return false;
    };
    let Some((output_width, output_height)) =
        gpu_thumbnail_output_dims(source_dimensions.0, source_dimensions.1, *w, *h)
    else {
        return false;
    };
    let factor_x = ((f64::from(source_dimensions.0) / f64::from(output_width) / 2.0) as u32).max(1);
    let factor_y =
        ((f64::from(source_dimensions.1) / f64::from(output_height) / 2.0) as u32).max(1);
    factor_x == 1 && factor_y == 1
}

fn gpu_rotate_requires_exact_host_control(image: &DynamicImage, mode: Option<&str>) -> bool {
    // The affine shader is byte-exact for the ordinary packed L/LA/RGB/RGBA
    // layouts. Pillow's typed, indexed, and palette-alpha modes use a
    // different sample contract (or a palette lookup) even when their
    // temporary storage happens to be four bytes per pixel.
    mode.is_some_and(|value| !matches!(value, "L" | "LA" | "RGB" | "RGBA"))
        || !gpu_image_layout_is_supported(image)
}

/// Return whether a typed, indexed, palette-alpha, or raw packed nearest
/// affine transform can use the fixed-point relocation shader without changing
/// Pillow's source selection.
///
/// `Geometry.c` computes the six affine values as signed 16.16 integers and
/// then walks each output row with integer additions.  The GPU shader carries
/// those same integers, but its arithmetic is i32 while the scalar path uses
/// i64.  Keep the admission proof deliberately explicit: every coefficient,
/// row origin, and absolute output-coordinate sum must fit i32, and the
/// source dimensions must fit the shader's signed bounds checks.  The typed
/// sample itself remains an opaque word (or an index/alpha pair for `PA`), so
/// no integer/float conversion or approximation is introduced.  I;16* uses
/// Pillow's separate nearest coordinate contract (rounding
/// `a*dx+b*dy+c` with `floor(+0.5)`), so its coefficients must be exactly
/// representable in the uploaded 16.16 plan. F uses the ordinary affine
/// center/truncation contract; it is admitted only after every f64 source
/// selection agrees with that uploaded fixed-point walk.
fn gpu_nearest_affine_is_exact(
    op: &PipelineOp,
    image: &DynamicImage,
    mode: Option<&str>,
    source_dimensions: (u32, u32),
) -> bool {
    let PipelineOp::Transform {
        w,
        h,
        method,
        data,
        filter,
        ..
    } = op
    else {
        return false;
    };
    let typed_mode = mode == Some("I");
    let float_mode = mode == Some("F");
    let luma16_mode = matches!(mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N"));
    let indexed_mode = matches!(mode, Some("1" | "P"));
    let palette_alpha_mode = mode == Some("PA");
    // CMYK is four raw channel bytes in the same Rgba8 transport. Unlike
    // RGBA, its fourth byte is K rather than alpha, and the mode-4 nearest
    // shader branch copies all four bytes without color conversion.
    let raw_packed_mode = mode == Some("CMYK");
    if (!typed_mode
        && !float_mode
        && !luma16_mode
        && !indexed_mode
        && !palette_alpha_mode
        && !raw_packed_mode)
        || (typed_mode && !matches!(image, DynamicImage::ImageRgba8(_)))
        || (float_mode && !matches!(image, DynamicImage::ImageRgba8(_)))
        // The mode-8 shader's affine branch copies complete float words only
        // for an explicit nearest request.  Pillow's filtered F transform
        // contract interpolates scalar values; retaining those rows on the
        // exact host path avoids silently treating a bilinear request as a
        // nearest relocation.
        || (float_mode && !matches!(filter, ResampleFilter::Nearest))
        || (luma16_mode && !matches!(image, DynamicImage::ImageLuma16(_)))
        || (indexed_mode && !matches!(image, DynamicImage::ImageLuma8(_)))
        || (palette_alpha_mode && !matches!(image, DynamicImage::ImageLumaA8(_)))
        || (raw_packed_mode && !matches!(image, DynamicImage::ImageRgba8(_)))
        || !matches!(method, TransformMethod::Affine)
        || (!gpu_transform_uses_nearest(mode, *filter))
        || source_dimensions.0 == 0
        || source_dimensions.1 == 0
        || source_dimensions.0 > i32::MAX as u32
        || source_dimensions.1 > i32::MAX as u32
        || *w > i32::MAX as u32
        || *h > i32::MAX as u32
        || *w == 0
        || *h == 0
        || (float_mode
            && u64::from(*w).saturating_mul(u64::from(*h))
                > GPU_F_AFFINE_PROOF_MAX_PIXELS as u64)
        || data.len() < 6
        || data[..6]
            .iter()
            .any(|value| !value.is_finite() || (*value as f32).is_infinite())
    {
        return false;
    }

    // Keep this expression byte-for-byte equivalent to the fixed-point
    // encoding in `prepare_batch` and `affine_nearest_fixed`.
    let fixed = |value: f64| {
        let rounded = value.mul_add(65_536.0, 0.5).floor();
        rounded.is_finite().then_some(rounded as i64)
    };
    let [a, b, c, d, e, f] = [data[0], data[1], data[2], data[3], data[4], data[5]];
    let Some(step_x_x) = fixed(a) else {
        return false;
    };
    let Some(step_y_x) = fixed(b) else {
        return false;
    };
    let origin_x_value = if luma16_mode {
        c + 0.5
    } else {
        c + a * 0.5 + b * 0.5
    };
    let Some(origin_x) = fixed(origin_x_value) else {
        return false;
    };
    let Some(step_x_y) = fixed(d) else {
        return false;
    };
    let Some(step_y_y) = fixed(e) else {
        return false;
    };
    let origin_y_value = if luma16_mode {
        f + 0.5
    } else {
        f + d * 0.5 + e * 0.5
    };
    let Some(origin_y) = fixed(origin_y_value) else {
        return false;
    };
    if luma16_mode {
        // The I;16 CPU path evaluates its f64 affine expression directly and
        // rounds the resulting source coordinate with `floor(value + 0.5)`.
        // Admit only coefficients already exact in the shader's 16.16 plan;
        // otherwise quantizing a fractional coefficient can change the
        // selected source word at a boundary even when no i32 overflow is
        // possible.
        let exactly_fixed = |value: f64| {
            let scaled = value * 65_536.0;
            scaled.is_finite()
                && scaled.fract() == 0.0
                && scaled >= i32::MIN as f64
                && scaled <= i32::MAX as f64
        };
        if ![a, b, c, d, e, f].into_iter().all(exactly_fixed) {
            return false;
        }
    }
    if [step_x_x, step_y_x, origin_x, step_x_y, step_y_y, origin_y]
        .into_iter()
        .any(|value| i32::try_from(value).is_err())
    {
        return false;
    }

    // Bound both the final coordinate and the intermediate addends.  The
    // absolute-sum form is conservative, but lets the shader use ordinary
    // i32 arithmetic with no wraparound while preserving the scalar order.
    let max_x = i128::from(w.saturating_sub(1));
    let max_y = i128::from(h.saturating_sub(1));
    let fits_i32 = |origin: i64, step_x: i64, step_y: i64| {
        i128::from(origin).abs()
            + i128::from(step_x).abs() * max_x
            + i128::from(step_y).abs() * max_y
            <= i128::from(i32::MAX)
    };
    if !fits_i32(origin_x, step_x_x, step_y_x) || !fits_i32(origin_y, step_x_y, step_y_y) {
        return false;
    }

    if !float_mode {
        return true;
    }

    // F affine nearest uses the same opaque-word sampler as I, but its CPU
    // reference evaluates the original f64 matrix at each pixel before
    // truncating non-negative coordinates. The GPU path instead walks the
    // quantized signed-16.16 plan above. Compare the resulting in-bounds
    // source selection for every output pixel; equal fill classifications are
    // sufficient when both coordinates are outside the source rectangle.
    let host_index = |value: f64| -> Option<i64> {
        if !value.is_finite() || value < 0.0 {
            return Some(-1);
        }
        (value <= i64::MAX as f64).then_some(value as i64)
    };
    let fixed_index = |value: i64| -> i64 { if value < 0 { -1 } else { value >> 16 } };
    let source_width = i64::from(source_dimensions.0);
    let source_height = i64::from(source_dimensions.1);
    for dy in 0..*h {
        for dx in 0..*w {
            let dx_f = f64::from(dx);
            let dy_f = f64::from(dy);
            let host_x = a * (dx_f + 0.5) + b * (dy_f + 0.5) + c;
            let host_y = d * (dx_f + 0.5) + e * (dy_f + 0.5) + f;
            let Some(host_x) = host_index(host_x) else {
                return false;
            };
            let Some(host_y) = host_index(host_y) else {
                return false;
            };
            let fixed_x = i128::from(origin_x)
                + i128::from(step_x_x) * i128::from(dx)
                + i128::from(step_y_x) * i128::from(dy);
            let fixed_y = i128::from(origin_y)
                + i128::from(step_x_y) * i128::from(dx)
                + i128::from(step_y_y) * i128::from(dy);
            let fixed_x = i64::try_from(fixed_x).ok();
            let fixed_y = i64::try_from(fixed_y).ok();
            let (Some(fixed_x), Some(fixed_y)) = (fixed_x, fixed_y) else {
                return false;
            };
            let device_x = fixed_index(fixed_x);
            let device_y = fixed_index(fixed_y);
            let host_sample =
                (host_x >= 0 && host_x < source_width && host_y >= 0 && host_y < source_height)
                    .then_some((host_x, host_y));
            let device_sample = (device_x >= 0
                && device_x < source_width
                && device_y >= 0
                && device_y < source_height)
                .then_some((device_x, device_y));
            if host_sample != device_sample {
                return false;
            }
        }
    }
    true
}

/// Prove a nearest projective/quad/mesh transform for raw byte samples.
///
/// Pillow's `src/libImaging/Geometry.c` `quad_transform` and mesh path evaluate
/// the inverse map in `f64`, while the projective shader evaluates destination
/// coordinates in `f32` and then rounds its selected source coordinate. Admit
/// only a bounded proof where every coefficient, destination coordinate,
/// intermediate arithmetic result, and final source coordinate select the
/// same source pixel in both domains.
/// This covers proof-certified f32-representable maps for ordinary packed byte
/// modes and raw indexed samples without claiming parity for arbitrary
/// homographies or mesh records. Nearest Quad and Mesh maps use the same
/// exhaustive source-selection proof, with explicit finite-intermediate
/// guards for their generic f32 bilinear arithmetic. Filtered projective
/// transforms remain host controlled unless an L/LA/RGB/RGBA/PA Perspective,
/// Quad, or complete one-record Mesh map is a unit-scale integer relocation or
/// a constant half-pixel source coordinate, or an interior integer coordinate
/// with the dedicated Bicubic tap proof. The Bilinear/relocation envelopes
/// land exactly on one source pixel, while the Bicubic envelope uses the
/// integer `[-1, 5, 5, -1] / 8` tap reduction; LA/RGBA also mirror Pillow's
/// premultiplied round trip.
// The comparison is intentional: the current shader receives raw destination
// indices and uses `floor(source + 0.5)`, whereas Pillow adds the destination
// pixel center before evaluating the map and then applies `COORD` truncation.
fn gpu_mesh_unit_relocation_is_admitted(data: &[f64], w: u32, h: u32) -> bool {
    if data.len() != 12 {
        return false;
    }

    // ImagingGenericTransform clips each record's destination box before it
    // calls Geometry.c's local quad mapper.  Admit a partial record only when
    // the box is wholly inside the output and all four bounds are integers;
    // otherwise the Rust CPU path's truncating/clipping conversion would not
    // be represented by the shader's f32 comparisons.  A full-output record
    // is the existing special case of this same shape.
    let integral = |value: f64| value.is_finite() && value.fract() == 0.0;
    let [bx0, by0, bx1, by1] = [data[0], data[1], data[2], data[3]];
    if ![bx0, by0, bx1, by1].into_iter().all(integral)
        || bx0 < 0.0
        || by0 < 0.0
        || bx1 > f64::from(w)
        || by1 > f64::from(h)
        || bx1 <= bx0
        || by1 <= by0
    {
        return false;
    }
    let box_width = bx1 - bx0;
    let box_height = by1 - by0;
    if !data[4..12].iter().all(|value| value.is_finite()) {
        return false;
    }
    if !data[4..12].iter().copied().all(integral) {
        // A fractional translation is not a raw relocation: nearest uses
        // Geometry.c's truncation while the projective sampler rounds, and
        // filtered requests would retain nonzero interpolation weights.
        return false;
    }

    // Mesh records use Pillow's corner order: top-left, bottom-left,
    // bottom-right, top-right.  These two forms are the only non-identity
    // records admitted for ordinary bytes: unit-scale relocation over either
    // a complete output or an in-output partial box, and the corresponding
    // axis swap.  The exhaustive proof below still checks every source
    // selection and fill boundary; this shape guard prevents arbitrary
    // bilinear corner arithmetic from entering the shader merely because one
    // small image happened to compare equal.
    let tx = data[4];
    let ty = data[5];
    let direct = [
        tx,
        ty,
        tx,
        ty + box_height,
        tx + box_width,
        ty + box_height,
        tx + box_width,
        ty,
    ];
    let swapped = [
        tx,
        ty,
        tx + box_height,
        ty,
        tx + box_height,
        ty + box_width,
        tx,
        ty + box_width,
    ];
    data[4..12] == direct || data[4..12] == swapped
}

/// Return whether a complete one-record Mesh maps every destination sample to
/// one constant f32 source coordinate. The generic shader may still do
/// different f32 arithmetic for the bilinear weights, so the image-aware
/// proof below must compare every source selection before admitting it.
fn gpu_mesh_constant_map_is_admitted(data: &[f64], w: u32, h: u32) -> bool {
    if data.len() != 12 || data[..4] != [0.0, 0.0, f64::from(w), f64::from(h)] {
        return false;
    }
    let x = data[4];
    let y = data[5];
    x.is_finite()
        && y.is_finite()
        && (x as f32).is_finite()
        && (y as f32).is_finite()
        && f64::from(x as f32) == x
        && f64::from(y as f32) == y
        && data[6..12].chunks_exact(2).all(|pair| pair == [x, y])
}

/// Return whether a Perspective map is a signed unit-axis relocation.
///
/// Pillow's `src/libImaging/Geometry.c` evaluates the inverse map at each
/// destination pixel center and then applies `COORD` truncation. For a
/// reflected unit axis, the shader's raw destination index plus
/// `floor(source + 0.5)` is one pixel off unless the center offset is reduced
/// explicitly. Keep the shape narrow; arbitrary affine arithmetic remains
/// subject to the exhaustive source-selection proof below.
fn gpu_perspective_signed_unit_relocation_is_admitted(data: &[f64]) -> bool {
    if data.len() < 8 || data[6] != 0.0 || data[7] != 0.0 {
        return false;
    }
    let integer_f32 = |value: f64| {
        value.is_finite()
            && value.fract() == 0.0
            && (value as f32).is_finite()
            && f64::from(value as f32) == value
    };
    if !integer_f32(data[2]) || !integer_f32(data[5]) {
        return false;
    }
    matches!(
        data[..6],
        [1.0, 0.0, _, 0.0, 1.0, _]
            | [-1.0, 0.0, _, 0.0, 1.0, _]
            | [1.0, 0.0, _, 0.0, -1.0, _]
            | [-1.0, 0.0, _, 0.0, -1.0, _]
            | [0.0, 1.0, _, 1.0, 0.0, _]
            | [0.0, -1.0, _, 1.0, 0.0, _]
            | [0.0, 1.0, _, -1.0, 0.0, _]
            | [0.0, -1.0, _, -1.0, 0.0, _]
    )
}

/// Return whether a Quad is a complete unit-scale relocation in Pillow's
/// NW/SW/SE/NE corner order. Direct identity and axis-swapped forms keep
/// every destination sample on an existing source pixel; arbitrary corner
/// arithmetic remains behind the exhaustive host/device source-selection
/// proof.
fn gpu_quad_unit_relocation_is_admitted(data: &[f64], w: u32, h: u32) -> bool {
    if data.len() < 8 {
        return false;
    }
    let width = f64::from(w);
    let height = f64::from(h);
    // A translated Quad is still a raw relocation when both source origins
    // are integral and f32-exact.  Pillow's Geometry.c evaluates the map at
    // the destination center; the filtered callback then subtracts 0.5, so
    // an integer translation leaves zero interpolation weights.  Fractional
    // origins remain behind the centered f64/host proof because the shader's
    // raw-gid sampler rounds them differently.
    let integer_f32 = |value: f64| {
        value.is_finite()
            && value.fract() == 0.0
            && (value as f32).is_finite()
            && f64::from(value as f32) == value
    };
    let tx = data[0];
    let ty = data[1];
    if !integer_f32(tx) || !integer_f32(ty) {
        return false;
    }
    let direct = [
        tx,
        ty,
        tx,
        ty + height,
        tx + width,
        ty + height,
        tx + width,
        ty,
    ];
    // The swapped map exchanges destination axes, so its source extents are
    // height on X and width on Y.  Using `(width, height)` here only works
    // for square outputs and admits a scaled map for every other shape.
    let swapped = [
        tx,
        ty,
        tx + height,
        ty,
        tx + height,
        ty + width,
        tx,
        ty + width,
    ];
    data[..8] == direct || data[..8] == swapped
}

/// Return whether a Quad maps every destination sample to one constant f32
/// source coordinate. Nearest sampling needs only the resulting
/// source selection; the exhaustive proof still rejects any f32 arithmetic
/// that could move that selection across a fill or pixel boundary.
fn gpu_quad_constant_map_is_admitted(data: &[f64]) -> bool {
    if data.len() != 8 {
        return false;
    }
    let x = data[0];
    let y = data[1];
    x.is_finite()
        && y.is_finite()
        && (x as f32).is_finite()
        && (y as f32).is_finite()
        && f64::from(x as f32) == x
        && f64::from(y as f32) == y
        && data[2..8].chunks_exact(2).all(|pair| pair == [x, y])
}

/// Check that the generic Quad WGSL bilinear expression has no non-finite
/// f32 intermediate for one output pixel.  A finite f64 map can still
/// overflow in the device arithmetic; a resulting NaN would bypass the
/// shader sampler's unordered bounds comparisons.  The exhaustive source
/// proof calls this only for nonconstant maps, while relocation/constant
/// branches avoid the generic expression entirely.
fn gpu_quad_shader_arithmetic_is_finite(data: &[f64], w: u32, h: u32, dx: f32, dy: f32) -> bool {
    if data.len() < 8 {
        return false;
    }
    let f = |index: usize| data[index] as f32;
    let x0 = f(0);
    let y0 = f(1);
    let x1 = f(2);
    let y1 = f(3);
    let x2 = f(4);
    let y2 = f(5);
    let x3 = f(6);
    let y3 = f(7);
    let width = w as f32;
    let height = h as f32;
    let u = dx / width;
    let v = dy / height;
    let one_minus_u = 1.0 - u;
    let one_minus_v = 1.0 - v;
    let x3_minus_x0 = x3 - x0;
    let x1_minus_x0 = x1 - x0;
    let x2_minus_x1 = x2 - x1;
    let x2_minus_x1_minus_x3 = x2_minus_x1 - x3;
    let x_cross = x2_minus_x1_minus_x3 + x0;
    let y3_minus_y0 = y3 - y0;
    let y1_minus_y0 = y1 - y0;
    let y2_minus_y1 = y2 - y1;
    let y2_minus_y1_minus_y3 = y2_minus_y1 - y3;
    let y_cross = y2_minus_y1_minus_y3 + y0;
    let x_term0 = x3_minus_x0 * u;
    let x_term1 = x1_minus_x0 * v;
    let x_term2 = x_cross * u * v;
    let y_term0 = y3_minus_y0 * u;
    let y_term1 = y1_minus_y0 * v;
    let y_term2 = y_cross * u * v;
    let sx = x0 + x_term0 + x_term1 + x_term2;
    let sy = y0 + y_term0 + y_term1 + y_term2;
    [
        width,
        height,
        u,
        v,
        one_minus_u,
        one_minus_v,
        x3_minus_x0,
        x1_minus_x0,
        x2_minus_x1,
        x2_minus_x1_minus_x3,
        x_cross,
        y3_minus_y0,
        y1_minus_y0,
        y2_minus_y1,
        y2_minus_y1_minus_y3,
        y_cross,
        x_term0,
        x_term1,
        x_term2,
        y_term0,
        y_term1,
        y_term2,
        sx,
        sy,
    ]
    .into_iter()
    .all(f32::is_finite)
}

/// Check that the generic one-record Mesh WGSL bilinear expression has no
/// non-finite f32 intermediate for one output pixel.  The bbox is complete
/// before this helper is called, but its arithmetic is retained here so the
/// check mirrors the shader even if callers are later reused for another
/// bounded record shape.
fn gpu_mesh_shader_arithmetic_is_finite(data: &[f64], w: u32, h: u32, dx: f32, dy: f32) -> bool {
    if data.len() < 12 {
        return false;
    }
    let f = |index: usize| data[index] as f32;
    let bx0 = f(0);
    let by0 = f(1);
    let bx1 = f(2);
    let by1 = f(3);
    let width = w as f32;
    let height = h as f32;
    let bw = (bx1 - bx0).max(1.0);
    let bh = (by1 - by0).max(1.0);
    let u = (dx - bx0) / bw;
    let v = (dy - by0) / bh;
    let one_minus_u = 1.0 - u;
    let one_minus_v = 1.0 - v;
    let x0 = f(4);
    let y0 = f(5);
    let x1 = f(6);
    let y1 = f(7);
    let x2 = f(8);
    let y2 = f(9);
    let x3 = f(10);
    let y3 = f(11);
    let x_term0 = one_minus_u * one_minus_v * x0;
    let x_term1 = u * one_minus_v * x3;
    let x_term2 = u * v * x2;
    let x_term3 = one_minus_u * v * x1;
    let y_term0 = one_minus_u * one_minus_v * y0;
    let y_term1 = u * one_minus_v * y3;
    let y_term2 = u * v * y2;
    let y_term3 = one_minus_u * v * y1;
    let sx = x_term0 + x_term1 + x_term2 + x_term3;
    let sy = y_term0 + y_term1 + y_term2 + y_term3;
    [
        width,
        height,
        bw,
        bh,
        u,
        v,
        one_minus_u,
        one_minus_v,
        x_term0,
        x_term1,
        x_term2,
        x_term3,
        y_term0,
        y_term1,
        y_term2,
        y_term3,
        sx,
        sy,
    ]
    .into_iter()
    .all(f32::is_finite)
}

/// Return whether a finite projective coordinate can be safely rounded and
/// converted by the WGSL nearest sampler.  Negative values are checked before
/// the shader's `u32` conversion and are therefore valid fill coordinates;
/// positive values must remain below the first unrepresentable `u32` value.
fn gpu_projective_shader_coordinate_is_safe(value: f32) -> bool {
    if !value.is_finite() {
        return false;
    }
    let rounded = value + 0.5;
    rounded.is_finite() && (value < 0.0 || rounded.floor() < 4_294_967_296.0)
}

/// Return whether a constant integer map is safe for filtered Bilinear or
/// Bicubic sampling in the generic projective shader.
///
/// Geometry.c validates the original source coordinate before subtracting
/// 0.5 for its filter window.  An interior integer coordinate therefore
/// becomes an exact half-pixel `(n - 0.5)` in the shader, with all four
/// neighbors in bounds and nonzero 0.5 weights.  Coordinates on the source
/// edge are intentionally excluded: Pillow clamps their filter window while
/// the shader's shifted bounds check would classify `0` or `width`
/// differently. LA/RGBA are also excluded because their transform pipeline
/// has a premultiplied-alpha round trip that this raw-channel path does not
/// reproduce for nonzero weights.
/// CMYK, HSV, YCbCr, RGBX, and RGBa remain raw byte channels in their native
/// projective paths, so they can share this proof once their physical packed
/// layout is checked.
/// Bicubic additionally requires two valid source pixels on each side: at an
/// integer source coordinate `n`, Geometry.c's four taps are `n-2..n+1` with
/// the exact half-pixel weights `[-1, 5, 5, -1] / 8`.
fn gpu_projective_filtered_integer_constant_is_admitted(
    method: TransformMethod,
    data: &[f64],
    filter: ResampleFilter,
    mode: Option<&str>,
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
) -> bool {
    if !matches!(filter, ResampleFilter::Bilinear | ResampleFilter::Bicubic)
        || !matches!(
            mode,
            Some("L" | "PA" | "RGB" | "CMYK" | "HSV" | "YCbCr" | "RGBX" | "RGBa")
        )
    {
        return false;
    }
    let bicubic = matches!(filter, ResampleFilter::Bicubic);
    let interior_integer_f32 = |value: f64, extent: u32| {
        let lower_bound = if bicubic { 2.0 } else { 1.0 };
        let upper_margin = if bicubic { 1.0 } else { 0.0 };
        value.is_finite()
            && value.fract() == 0.0
            && value >= lower_bound
            && value + upper_margin < f64::from(extent)
            && (value as f32).is_finite()
            && f64::from(value as f32) == value
    };
    match method {
        TransformMethod::Perspective => {
            data.len() >= 8
                && data[0] == 0.0
                && data[1] == 0.0
                && data[3] == 0.0
                && data[4] == 0.0
                && data[6] == 0.0
                && data[7] == 0.0
                && interior_integer_f32(data[2], source_dimensions.0)
                && interior_integer_f32(data[5], source_dimensions.1)
        }
        TransformMethod::Quad => {
            gpu_quad_constant_map_is_admitted(data)
                && interior_integer_f32(data[0], source_dimensions.0)
                && interior_integer_f32(data[1], source_dimensions.1)
        }
        TransformMethod::Mesh => {
            gpu_mesh_constant_map_is_admitted(data, output_dimensions.0, output_dimensions.1)
                && interior_integer_f32(data[4], source_dimensions.0)
                && interior_integer_f32(data[5], source_dimensions.1)
        }
        TransformMethod::Affine => false,
    }
}

fn gpu_projective_filtered_relocation_is_admitted(
    method: TransformMethod,
    data: &[f64],
    filter: ResampleFilter,
    mode: Option<&str>,
    output_dimensions: (u32, u32),
) -> bool {
    // Pillow's Geometry.c filtered projective path subtracts the destination
    // center offset before bilinear sampling. For integer unit maps and
    // constant half-pixel source coordinates, that produces an integral
    // source coordinate and zero filter weights; the shader's raw-gid
    // projective sampler therefore selects the same source sample. LA/RGBA
    // use Pillow's premultiplied round trip; the projective shader mirrors
    // that round trip for the zero-weight source word. PA is different: its
    // projective path preserves raw index/alpha pairs without premultiplied
    // conversion, so the zero-weight argument applies to PA as well.
    if !matches!(mode, Some("L" | "LA" | "PA" | "RGB" | "RGBA"))
        || !matches!(filter, ResampleFilter::Bilinear | ResampleFilter::Bicubic)
        || data.len() < 8
    {
        return false;
    }

    let integer_translation = |value: f64| {
        value.is_finite()
            && value.fract() == 0.0
            && (value as f32).is_finite()
            && f64::from(value as f32) == value
    };
    let filter_center = |value: f64| {
        let narrowed = value as f32;
        let centered = value - 0.5;
        value.is_finite()
            && narrowed.is_finite()
            && f64::from(narrowed) == value
            && centered.fract() == 0.0
            && (centered as f32).is_finite()
            && f64::from(centered as f32) == centered
    };
    match method {
        TransformMethod::Perspective => {
            let direct_perspective =
                |tx: f64, ty: f64| data[..8] == [1.0, 0.0, tx, 0.0, 1.0, ty, 0.0, 0.0];
            let swapped_perspective =
                |tx: f64, ty: f64| data[..8] == [0.0, 1.0, tx, 1.0, 0.0, ty, 0.0, 0.0];
            let tx = data[2];
            let ty = data[5];
            (integer_translation(tx)
                && integer_translation(ty)
                && (direct_perspective(tx, ty) || swapped_perspective(tx, ty)))
                || (data[..6] == [0.0, 0.0, tx, 0.0, 0.0, ty]
                    && data[6] == 0.0
                    && data[7] == 0.0
                    && filter_center(tx)
                    && filter_center(ty))
        }
        TransformMethod::Quad => {
            // QUAD's direct and axis-swapped unit maps evaluate to integral
            // source coordinates at every raw destination gid. A constant
            // half-pixel map reaches the same integral coordinate after the
            // Geometry.c filter window subtracts 0.5.
            gpu_quad_unit_relocation_is_admitted(data, output_dimensions.0, output_dimensions.1)
                || (gpu_quad_constant_map_is_admitted(data)
                    && filter_center(data[0])
                    && filter_center(data[1]))
        }
        TransformMethod::Mesh => {
            // A complete one-record mesh has the same zero-weight property
            // for unit relocations and constant half-pixel source maps.
            gpu_mesh_unit_relocation_is_admitted(data, output_dimensions.0, output_dimensions.1)
                || (gpu_mesh_constant_map_is_admitted(
                    data,
                    output_dimensions.0,
                    output_dimensions.1,
                ) && filter_center(data[4])
                    && filter_center(data[5]))
        }
        TransformMethod::Affine => false,
    }
}

/// Return whether a palette-alpha projective transform is a raw pair
/// relocation that the projective shader can reproduce exactly.
///
/// PA stores one palette index and one per-pixel alpha byte in the native
/// two-band image.  A nearest signed unit-axis map with an integer translation
/// only selects an existing `(index, alpha)` pair; it
/// never expands the palette or performs alpha arithmetic.  Keep this
/// separate from ordinary byte modes: PA's palette metadata and two-band fill
/// contract need their own proof. Pillow does not force PA's non-nearest
/// transform requests to nearest. The filtered Perspective relocation below
/// is admitted only when its weights are provably zero; other PA filtered
/// transforms remain on the exact host path rather than being silently
/// changed to pair-copy sampling. Quad and complete one-record Mesh also
/// admit a constant f32 source coordinate for nearest sampling, while the
/// filtered proof admits only a half-pixel coordinate: Geometry.c's 0.5
/// center shift then makes its filter weights exactly zero. The image-aware
/// proof still checks every output boundary.
fn gpu_palette_alpha_projective_relocation_is_admitted(
    method: TransformMethod,
    data: &[f64],
    filter: ResampleFilter,
    output_dimensions: (u32, u32),
) -> bool {
    if !matches!(filter, ResampleFilter::Nearest) || data.len() < 8 {
        return false;
    }
    let integer_f32 = |value: f64| {
        value.is_finite()
            && value.fract() == 0.0
            && (value as f32).is_finite()
            && f64::from(value as f32) == value
    };
    match method {
        TransformMethod::Perspective => {
            let direct = data[..8] == [1.0, 0.0, data[2], 0.0, 1.0, data[5], 0.0, 0.0];
            let swapped = data[..8] == [0.0, 1.0, data[2], 1.0, 0.0, data[5], 0.0, 0.0];
            integer_f32(data[2])
                && integer_f32(data[5])
                && (direct || swapped || gpu_perspective_signed_unit_relocation_is_admitted(data))
        }
        TransformMethod::Quad => {
            gpu_quad_unit_relocation_is_admitted(data, output_dimensions.0, output_dimensions.1)
                || gpu_quad_constant_map_is_admitted(data)
        }
        TransformMethod::Mesh => {
            gpu_mesh_unit_relocation_is_admitted(data, output_dimensions.0, output_dimensions.1)
                || gpu_mesh_constant_map_is_admitted(data, output_dimensions.0, output_dimensions.1)
        }
        TransformMethod::Affine => false,
    }
}

fn gpu_projective_nearest_is_exact(
    op: &PipelineOp,
    image: &DynamicImage,
    mode: Option<&str>,
    source_dimensions: (u32, u32),
) -> bool {
    let PipelineOp::Transform {
        w,
        h,
        method,
        data,
        filter,
        fill_is_none,
        ..
    } = op
    else {
        return false;
    };
    let image_layout_is_valid = match mode {
        Some("P" | "1" | "L") => matches!(image, DynamicImage::ImageLuma8(_)),
        Some("LA" | "PA") => matches!(image, DynamicImage::ImageLumaA8(_)),
        Some("RGB" | "HSV" | "YCbCr") => matches!(image, DynamicImage::ImageRgb8(_)),
        Some("RGBA" | "RGBX" | "RGBa" | "CMYK") => {
            matches!(image, DynamicImage::ImageRgba8(_))
        }
        _ => false,
    };
    let ordinary_byte_mode = matches!(
        mode,
        Some("L" | "LA" | "RGB" | "RGBA" | "RGBX" | "RGBa" | "CMYK" | "HSV" | "YCbCr")
    );
    let filtered_integer_constant = gpu_projective_filtered_integer_constant_is_admitted(
        method.clone(),
        data,
        *filter,
        mode,
        source_dimensions,
        (*w, *h),
    );
    let filtered_relocation = filtered_integer_constant
        || gpu_projective_filtered_relocation_is_admitted(
            method.clone(),
            data,
            *filter,
            mode,
            (*w, *h),
        );
    let palette_alpha_relocation = mode == Some("PA")
        && (gpu_palette_alpha_projective_relocation_is_admitted(
            method.clone(),
            data,
            *filter,
            (*w, *h),
        ) || filtered_relocation);
    // Perspective, Quad, and Mesh maps use the shader's raw-gid f32
    // arithmetic, while the exhaustive source-selection proof below compares
    // it with Pillow's centered f64/COORD result for every output pixel.
    // Finite-intermediate guards in the proof keep generic Quad/Mesh
    // bilinear overflow from bypassing the shader sampler's bounds checks.
    // Filtered transforms intentionally remain limited to zero-weight
    // relocations, constant half-pixel maps, and the interior integer
    // Bilinear/Bicubic envelopes proven above.
    let ordinary_projective_geometry_is_admitted = match method {
        TransformMethod::Perspective => true,
        TransformMethod::Quad | TransformMethod::Mesh => true,
        TransformMethod::Affine => false,
    };
    if !image_layout_is_valid
        || (ordinary_byte_mode && !ordinary_projective_geometry_is_admitted)
        || (mode == Some("PA") && !palette_alpha_relocation)
        || !matches!(
            method,
            TransformMethod::Perspective | TransformMethod::Quad | TransformMethod::Mesh
        )
        || (!gpu_transform_uses_nearest(mode, *filter) && !filtered_relocation)
        || source_dimensions.0 == 0
        || source_dimensions.1 == 0
        || *w == 0
        || *h == 0
        || source_dimensions.0 > (1 << 24)
        || source_dimensions.1 > (1 << 24)
        || *w > (1 << 24)
        || *h > (1 << 24)
        || u64::from(*w).saturating_mul(u64::from(*h)) > 1_048_576
    {
        return false;
    }

    let required_data = match method {
        TransformMethod::Perspective | TransformMethod::Quad => 8,
        TransformMethod::Mesh => 12,
        TransformMethod::Affine => return false,
    };
    if data.len() < required_data || (matches!(method, TransformMethod::Mesh) && data.len() != 12) {
        return false;
    }
    // The transform uniform stores all coefficients as f32 bit patterns. Do
    // not admit a value that changes when it crosses that ABI boundary.
    let f32_exact = |value: f64| {
        let narrowed = value as f32;
        narrowed.is_finite() && f64::from(narrowed) == value
    };
    if data[..required_data]
        .iter()
        .copied()
        .any(|value| !f32_exact(value))
    {
        return false;
    }

    // The shader carries one Mesh record and can only represent a partial
    // record's outside-bbox result with an explicit fill word.  Its direct
    // and axis-swapped unit relocation lowering is exact for an in-output
    // integer bbox; keep fractional/scaled/partially clipped records and
    // no-fill records on the host path rather than letting the generic
    // shader's clipping or default fill semantics stand in for Geometry.c.
    let mesh_is_full_output = !matches!(method, TransformMethod::Mesh)
        || data[..4] == [0.0, 0.0, f64::from(*w), f64::from(*h)];
    let mesh_is_partial_relocation = matches!(method, TransformMethod::Mesh)
        && gpu_mesh_unit_relocation_is_admitted(data, *w, *h);
    if matches!(method, TransformMethod::Mesh)
        && !mesh_is_full_output
        && !mesh_is_partial_relocation
    {
        return false;
    }
    if matches!(method, TransformMethod::Mesh) && !mesh_is_full_output && *fill_is_none {
        return false;
    }

    let source_at = |dx: f64, dy: f64| -> Option<(f64, f64)> {
        match method {
            TransformMethod::Perspective => {
                let dx = dx + 0.5;
                let dy = dy + 0.5;
                let denominator = data[6] * dx + data[7] * dy + 1.0;
                if denominator == 0.0 || !denominator.is_finite() {
                    return None;
                }
                let sx = (data[0] * dx + data[1] * dy + data[2]) / denominator;
                let sy = (data[3] * dx + data[4] * dy + data[5]) / denominator;
                (sx.is_finite() && sy.is_finite()).then_some((sx, sy))
            }
            TransformMethod::Quad => {
                if gpu_quad_constant_map_is_admitted(data) {
                    return Some((data[0], data[1]));
                }
                let dx = dx + 0.5;
                let dy = dy + 0.5;
                let sw = f64::from(*w);
                let sh = f64::from(*h);
                let u = dx / sw;
                let v = dy / sh;
                let x0 = data[0];
                let y0 = data[1];
                let sx = x0
                    + (data[6] - x0) * u
                    + (data[2] - x0) * v
                    + (data[4] - data[2] - data[6] + x0) * u * v;
                let sy = y0
                    + (data[7] - y0) * u
                    + (data[3] - y0) * v
                    + (data[5] - data[3] - data[7] + y0) * u * v;
                (sx.is_finite() && sy.is_finite()).then_some((sx, sy))
            }
            TransformMethod::Mesh => {
                // The shader emits the fill word outside a record's bbox.
                // Compare source selection only for pixels inside the
                // record, just as Geometry.c's clipped destination loop
                // does; a full-output record naturally passes this check.
                if dx < data[0] || dx >= data[2] || dy < data[1] || dy >= data[3] {
                    return None;
                }
                if gpu_mesh_constant_map_is_admitted(data, *w, *h) {
                    return Some((data[4], data[5]));
                }
                if ordinary_byte_mode && gpu_mesh_unit_relocation_is_admitted(data, *w, *h) {
                    // For the admitted unit relocations, mirror the CPU
                    // quad_transform FMA order directly.  Reconstructing the
                    // same affine map from four bilinear weights would add a
                    // second rounding shape to the proof at large integer
                    // translations, even though Pillow's coefficients have
                    // already reduced to a unit step.
                    let local_x = dx + 0.5 - data[0];
                    let local_y = dy + 0.5 - data[1];
                    let box_width = data[2] - data[0];
                    // Mesh source corners are NW/SW/SE/NE.  The direct
                    // relocation therefore has its NE corner one width to
                    // the right of TL and at the same Y; checking the
                    // opposite diagonal admits a scaled/axis map instead.
                    let direct = data[10] == data[4] + box_width && data[11] == data[5];
                    let (sx, sy) = if direct {
                        (data[4] + local_x, data[5] + local_y)
                    } else {
                        (data[4] + local_y, data[5] + local_x)
                    };
                    return (sx.is_finite() && sy.is_finite()).then_some((sx, sy));
                }
                let bbox_width = data[2] - data[0];
                let bbox_height = data[3] - data[1];
                if bbox_width <= 0.0 || bbox_height <= 0.0 {
                    return None;
                }
                let u = (dx + 0.5 - data[0]) / bbox_width;
                let v = (dy + 0.5 - data[1]) / bbox_height;
                let x0 = data[4];
                let y0 = data[5];
                let sx = (1.0 - u) * (1.0 - v) * x0
                    + u * (1.0 - v) * data[10]
                    + u * v * data[8]
                    + (1.0 - u) * v * data[6];
                let sy = (1.0 - u) * (1.0 - v) * y0
                    + u * (1.0 - v) * data[11]
                    + u * v * data[9]
                    + (1.0 - u) * v * data[7];
                (sx.is_finite() && sy.is_finite()).then_some((sx, sy))
            }
            TransformMethod::Affine => None,
        }
    };
    let shader_source_at = |dx: f32, dy: f32| -> Option<(f32, f32)> {
        let f = |index: usize| data[index] as f32;
        match method {
            TransformMethod::Perspective => {
                if matches!(filter, ResampleFilter::Bilinear | ResampleFilter::Bicubic)
                    && f(6) == 0.0
                    && f(7) == 0.0
                    && f(0) == 0.0
                    && f(1) == 0.0
                    && f(3) == 0.0
                    && f(4) == 0.0
                {
                    return Some((f(2) - 0.5, f(5) - 0.5));
                }
                if gpu_perspective_signed_unit_relocation_is_admitted(data) {
                    let a = f(0);
                    let b = f(1);
                    let d = f(3);
                    let e = f(4);
                    let sx = if a == 1.0 {
                        f(2) + dx
                    } else if a == -1.0 {
                        f(2) - dx - 1.0
                    } else if b == 1.0 {
                        f(2) + dy
                    } else {
                        f(2) - dy - 1.0
                    };
                    let sy = if d == 1.0 {
                        f(5) + if b.abs() == 1.0 { dx } else { dy }
                    } else if d == -1.0 {
                        f(5) - if b.abs() == 1.0 { dx } else { dy } - 1.0
                    } else if e == 1.0 {
                        f(5) + dy
                    } else {
                        f(5) - dy - 1.0
                    };
                    return (sx.is_finite() && sy.is_finite()).then_some((sx, sy));
                }
                if f(6) == 0.0 && f(7) == 0.0 && matches!(filter, ResampleFilter::Nearest) {
                    let center_x = dx + 0.5;
                    let center_y = dy + 0.5;
                    let sx = (f(0) * center_x + f(1) * center_y + f(2)).floor();
                    let sy = (f(3) * center_x + f(4) * center_y + f(5)).floor();
                    return (sx.is_finite() && sy.is_finite()).then_some((sx, sy));
                }
                let denominator = f(6) * dx + f(7) * dy + 1.0;
                if denominator == 0.0 || !denominator.is_finite() {
                    return None;
                }
                let sx = (f(0) * dx + f(1) * dy + f(2)) / denominator;
                let sy = (f(3) * dx + f(4) * dy + f(5)) / denominator;
                (sx.is_finite() && sy.is_finite()).then_some((sx, sy))
            }
            TransformMethod::Quad => {
                let x0 = f(0);
                let y0 = f(1);
                if x0 == f(2) && x0 == f(4) && x0 == f(6) && y0 == f(3) && y0 == f(5) && y0 == f(7)
                {
                    // Geometry.c filters after subtracting 0.5 from the
                    // mapped source coordinate.  A constant half-pixel map
                    // therefore lands exactly on one source pixel, while
                    // the projective shader's bilinear sampler expects the
                    // already-shifted coordinate.
                    if !matches!(filter, ResampleFilter::Nearest) {
                        return Some((x0 - 0.5, y0 - 0.5));
                    }
                    return Some((x0, y0));
                }
                let width = *w as f32;
                let height = *h as f32;
                let u = dx / width;
                let v = dy / height;
                let sx = x0 + (f(6) - x0) * u + (f(2) - x0) * v + (f(4) - f(2) - f(6) + x0) * u * v;
                let sy = y0 + (f(7) - y0) * u + (f(3) - y0) * v + (f(5) - f(3) - f(7) + y0) * u * v;
                (sx.is_finite() && sy.is_finite()).then_some((sx, sy))
            }
            TransformMethod::Mesh => {
                let bx0 = f(0);
                let by0 = f(1);
                let bx1 = f(2);
                let by1 = f(3);
                if dx < bx0 || dx >= bx1 || dy < by0 || dy >= by1 {
                    return None;
                }
                let width = *w as f32;
                let height = *h as f32;
                let x0 = f(4);
                let y0 = f(5);
                if x0 == f(6)
                    && x0 == f(8)
                    && x0 == f(10)
                    && y0 == f(7)
                    && y0 == f(9)
                    && y0 == f(11)
                {
                    if !matches!(filter, ResampleFilter::Nearest) {
                        return Some((x0 - 0.5, y0 - 0.5));
                    }
                    return Some((x0, y0));
                }
                let direct_relocation = bx0 == 0.0
                    && by0 == 0.0
                    && bx1 == width
                    && by1 == height
                    && f(6) == x0
                    && f(7) == y0 + height
                    && f(8) == x0 + width
                    && f(9) == y0 + height
                    && f(10) == x0 + width
                    && f(11) == y0;
                let swapped_relocation = bx0 == 0.0
                    && by0 == 0.0
                    && bx1 == width
                    && by1 == height
                    && f(6) == x0 + height
                    && f(7) == y0
                    && f(8) == x0 + height
                    && f(9) == y0 + width
                    && f(10) == x0
                    && f(11) == y0 + width;
                if direct_relocation {
                    return Some((x0 + dx, y0 + dy));
                }
                if swapped_relocation {
                    return Some((x0 + dy, y0 + dx));
                }
                let bw = (bx1 - bx0).max(1.0);
                let bh = (by1 - by0).max(1.0);
                let u = (dx - bx0) / bw;
                let v = (dy - by0) / bh;
                let sx = (1.0 - u) * (1.0 - v) * x0
                    + u * (1.0 - v) * f(10)
                    + u * v * f(8)
                    + (1.0 - u) * v * f(6);
                let sy = (1.0 - u) * (1.0 - v) * y0
                    + u * (1.0 - v) * f(11)
                    + u * v * f(9)
                    + (1.0 - u) * v * f(7);
                (sx.is_finite() && sy.is_finite()).then_some((sx, sy))
            }
            TransformMethod::Affine => None,
        }
    };

    let host_coordinate = |value: f64| -> Option<i64> {
        if !value.is_finite() {
            return None;
        }
        Some(
            if matches!(
                method,
                TransformMethod::Perspective | TransformMethod::Quad | TransformMethod::Mesh
            ) {
                if value < 0.0 { -1 } else { value as i64 }
            } else {
                (value + 0.5).floor() as i64
            },
        )
    };
    let device_coordinate =
        |value: f32| -> Option<i64> { value.is_finite().then(|| (value + 0.5).floor() as i64) };

    for dy in 0..*h {
        for dx in 0..*w {
            // The generic projective shader checks only a zero denominator
            // before dividing.  Reject any non-finite f32 intermediate here:
            // a NaN source coordinate would bypass the sampler's bounds
            // checks, whereas Pillow's f64 path still resolves a fill or a
            // source pixel.  Constant-denominator maps use a different
            // centered shader branch; this guard is for the nonconstant
            // arithmetic admitted by the proof above.
            if matches!(method, TransformMethod::Perspective) && (data[6] != 0.0 || data[7] != 0.0)
            {
                let f = |index: usize| data[index] as f32;
                let dx = dx as f32;
                let dy = dy as f32;
                let denominator = f(6) * dx + f(7) * dy + 1.0;
                if denominator != 0.0
                    && (!denominator.is_finite()
                        || !(f(0) * dx + f(1) * dy + f(2)).is_finite()
                        || !(f(3) * dx + f(4) * dy + f(5)).is_finite())
                {
                    return false;
                }
            }
            if matches!(method, TransformMethod::Quad)
                && !gpu_quad_constant_map_is_admitted(data)
                && !gpu_quad_shader_arithmetic_is_finite(data, *w, *h, dx as f32, dy as f32)
            {
                return false;
            }
            if matches!(method, TransformMethod::Mesh)
                && !gpu_mesh_constant_map_is_admitted(data, *w, *h)
                && !gpu_mesh_unit_relocation_is_admitted(data, *w, *h)
                && !gpu_mesh_shader_arithmetic_is_finite(data, *w, *h, dx as f32, dy as f32)
            {
                return false;
            }
            let host = source_at(f64::from(dx), f64::from(dy));
            let device = shader_source_at(dx as f32, dy as f32);
            if matches!(method, TransformMethod::Quad | TransformMethod::Mesh)
                && device.is_some_and(|(x, y)| {
                    !gpu_projective_shader_coordinate_is_safe(x)
                        || !gpu_projective_shader_coordinate_is_safe(y)
                })
            {
                return false;
            }
            let host_sample = host.and_then(|(x, y)| {
                let x = host_coordinate(x)?;
                let y = host_coordinate(y)?;
                (x >= 0
                    && x < i64::from(source_dimensions.0)
                    && y >= 0
                    && y < i64::from(source_dimensions.1))
                .then_some((x, y))
            });
            let device_sample = device.and_then(|(x, y)| {
                let x = device_coordinate(x)?;
                let y = device_coordinate(y)?;
                (x >= 0
                    && x < i64::from(source_dimensions.0)
                    && y >= 0
                    && y < i64::from(source_dimensions.1))
                .then_some((x, y))
            });
            if host_sample != device_sample {
                return false;
            }
        }
    }
    true
}

/// Return whether every destination pixel of an affine transform is outside
/// the source rectangle, so the device can emit the already-resolved fill word
/// without sampling or interpolating any source value.
///
/// This is a deliberately narrow identity proof for typed/raw packed modes.
/// For ordinary floating-point affine transforms, prove both the host f64
/// coordinates and the shader's f32 coordinates are outside by at least one
/// source pixel. Typed nearest modes use the signed-16.16 coordinate walk, so
/// their integer bounds are checked against the exact values uploaded to the
/// shader instead. No source bytes are inspected because the result is wholly
/// determined by Pillow's validated fill contract.
fn gpu_transform_all_fill_is_exact(
    op: &PipelineOp,
    image: &DynamicImage,
    mode: Option<&str>,
    source_dimensions: (u32, u32),
) -> bool {
    let PipelineOp::Transform {
        w,
        h,
        method,
        data,
        filter,
        palette_fill,
        ..
    } = op
    else {
        return false;
    };
    if !matches!(method, TransformMethod::Affine)
        || *w == 0
        || *h == 0
        || source_dimensions.0 == 0
        || source_dimensions.1 == 0
        || data.len() < 6
        || palette_fill.is_some()
        || !matches!(
            mode,
            Some(
                "L" | "LA"
                    | "RGB"
                    | "RGBA"
                    | "RGBX"
                    | "RGBa"
                    | "CMYK"
                    | "HSV"
                    | "YCbCr"
                    | "I"
                    | "F"
                    | "I;16"
                    | "I;16L"
                    | "I;16B"
                    | "I;16N"
            )
        )
    {
        return false;
    }

    let source_layout_is_valid = match mode {
        Some("I" | "F") => matches!(image, DynamicImage::ImageRgba8(_)),
        Some("I;16" | "I;16L" | "I;16B" | "I;16N") => {
            matches!(image, DynamicImage::ImageLuma16(_))
        }
        _ => gpu_image_layout_is_supported(image),
    };
    if !source_layout_is_valid {
        return false;
    }

    let coefficients = &data[..6];
    if coefficients
        .iter()
        .any(|value| !value.is_finite() || (*value as f32).is_infinite())
    {
        return false;
    }
    // Keep destination and source extents exactly representable in the
    // shader's f32 coordinate and dimension expressions. The actual buffer
    // limits are much smaller, but making the mathematical bound explicit
    // prevents a rounded endpoint from weakening the interval proof.
    const MAX_F32_EXACT_INTEGER: u32 = 1 << 24;
    if *w > MAX_F32_EXACT_INTEGER
        || *h > MAX_F32_EXACT_INTEGER
        || source_dimensions.0 > MAX_F32_EXACT_INTEGER
        || source_dimensions.1 > MAX_F32_EXACT_INTEGER
    {
        return false;
    }

    let outside = |minimum: f64, maximum: f64, extent: u32| {
        maximum <= -1.0 || minimum >= f64::from(extent) + 1.0
    };
    let bounds_f64 = |a: f64, b: f64, c: f64, center: f64| -> Option<(f64, f64)> {
        let mut minimum = f64::INFINITY;
        let mut maximum = f64::NEG_INFINITY;
        for dx in [0.0, f64::from(w.saturating_sub(1))] {
            for dy in [0.0, f64::from(h.saturating_sub(1))] {
                let value = a * (dx + center) + b * (dy + center) + c;
                if !value.is_finite() {
                    return None;
                }
                minimum = minimum.min(value);
                maximum = maximum.max(value);
            }
        }
        Some((minimum, maximum))
    };

    let [a, b, c, d, e, f] = coefficients else {
        return false;
    };
    let center = if matches!(mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N")) {
        0.0
    } else {
        0.5
    };

    if gpu_transform_uses_nearest(mode, *filter) {
        // Keep this encoding byte-for-byte identical to `prepare_batch` and
        // `sample_nearest_fixed`; the integer bounds cover all four corners
        // and therefore every affine destination coordinate.
        let fixed = |value: f64| {
            let rounded = value.mul_add(65_536.0, 0.5);
            (rounded.is_finite() && rounded >= i64::MIN as f64 && rounded <= i64::MAX as f64)
                .then_some(rounded as i64)
        };
        let Some(step_x_x) = fixed(*a) else {
            return false;
        };
        let Some(step_y_x) = fixed(*b) else {
            return false;
        };
        let luma16_mode = matches!(mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N"));
        if luma16_mode {
            // Keep the fill-only proof on the same exact coordinate plan as
            // the in-bounds mode-5 admission.  A fractional coefficient that
            // rounds differently in 16.16 could move one edge sample across
            // the nearest fill boundary even though the bulk of the canvas
            // is outside the source.
            let exactly_fixed = |value: f64| {
                let scaled = value * 65_536.0;
                scaled.is_finite()
                    && scaled.fract() == 0.0
                    && scaled >= i32::MIN as f64
                    && scaled <= i32::MAX as f64
            };
            if ![*a, *b, *c, *d, *e, *f].into_iter().all(exactly_fixed) {
                return false;
            }
        }
        let origin_x_value = if luma16_mode {
            *c + 0.5
        } else {
            *c + *a * 0.5 + *b * 0.5
        };
        let Some(origin_x) = fixed(origin_x_value) else {
            return false;
        };
        let Some(step_x_y) = fixed(*d) else {
            return false;
        };
        let Some(step_y_y) = fixed(*e) else {
            return false;
        };
        let origin_y_value = if luma16_mode {
            *f + 0.5
        } else {
            *f + *d * 0.5 + *e * 0.5
        };
        let Some(origin_y) = fixed(origin_y_value) else {
            return false;
        };
        if [step_x_x, step_y_x, origin_x, step_x_y, step_y_y, origin_y]
            .into_iter()
            .any(|value| i32::try_from(value).is_err())
        {
            return false;
        }
        let max_x = i128::from(w.saturating_sub(1));
        let max_y = i128::from(h.saturating_sub(1));
        let fits_i32 = |origin: i64, step_x: i64, step_y: i64| {
            i128::from(origin).abs()
                + i128::from(step_x).abs() * max_x
                + i128::from(step_y).abs() * max_y
                <= i128::from(i32::MAX)
        };
        if !fits_i32(origin_x, step_x_x, step_y_x) || !fits_i32(origin_y, step_x_y, step_y_y) {
            return false;
        }
        let bounds_i128 = |origin: i64, step_x: i64, step_y: i64| {
            let x = i128::from(step_x) * i128::from(w.saturating_sub(1));
            let y = i128::from(step_y) * i128::from(h.saturating_sub(1));
            let values = [
                i128::from(origin),
                i128::from(origin) + x,
                i128::from(origin) + y,
                i128::from(origin) + x + y,
            ];
            let minimum = *values.iter().min()?;
            let maximum = *values.iter().max()?;
            Some((minimum, maximum))
        };
        let Some((min_x, max_x)) = bounds_i128(origin_x, step_x_x, step_y_x) else {
            return false;
        };
        let Some((min_y, max_y)) = bounds_i128(origin_y, step_x_y, step_y_y) else {
            return false;
        };
        let source_w = i128::from(source_dimensions.0) * 65_536;
        let source_h = i128::from(source_dimensions.1) * 65_536;
        if luma16_mode {
            // The native I;16 nearest sampler maps a fixed coordinate to
            // floor(source + 0.5), and the shader rejects negative fixed
            // coordinates before shifting.  These are the exact fill-only
            // boundaries for the mode-5 word path.
            return max_x < 0 || min_x >= source_w || max_y < 0 || min_y >= source_h;
        }
        return max_x <= -65_536
            || min_x >= source_w + 65_536
            || max_y <= -65_536
            || min_y >= source_h + 65_536;
    }

    let Some((min_x, max_x)) = bounds_f64(*a, *b, *c, center) else {
        return false;
    };
    let Some((min_y, max_y)) = bounds_f64(*d, *e, *f, center) else {
        return false;
    };
    let f32_coefficients = [
        *a as f32, *b as f32, *c as f32, *d as f32, *e as f32, *f as f32,
    ];
    // Evaluate the shader's actual f32 operation order at each affine
    // corner. Relying on the exact f64 polynomial here would miss a rounded
    // multiply/add that moves a coordinate back across the source edge.
    // Every individual term and the final sum must remain finite; otherwise
    // an intermediate infinity/NaN could defeat the shader's bounds check.
    let bounds_f32_shader = |a: f32, b: f32, c: f32, center: f32| -> Option<(f64, f64)> {
        let mut minimum = f32::INFINITY;
        let mut maximum = f32::NEG_INFINITY;
        for dx in [0.0f32, (w.saturating_sub(1)) as f32] {
            for dy in [0.0f32, (h.saturating_sub(1)) as f32] {
                let ax = dx + center;
                let ay = dy + center;
                let x_term = a * ax;
                let y_term = b * ay;
                // Check the unfused WGSL expression plus the possible
                // multiply-add contractions permitted by device compilers.
                // Requiring every form to stay on the same out-of-bounds
                // side keeps this proof independent of contraction choice.
                let values = [
                    (x_term + y_term) + c,
                    a.mul_add(ax, y_term) + c,
                    x_term + b.mul_add(ay, c),
                    a.mul_add(ax, b.mul_add(ay, c)),
                ];
                if !ax.is_finite()
                    || !ay.is_finite()
                    || !x_term.is_finite()
                    || !y_term.is_finite()
                    || values.iter().any(|value| !value.is_finite())
                {
                    return None;
                }
                for value in values {
                    minimum = minimum.min(value);
                    maximum = maximum.max(value);
                }
            }
        }
        Some((f64::from(minimum), f64::from(maximum)))
    };
    let Some((min_x_f32, max_x_f32)) = bounds_f32_shader(
        f32_coefficients[0],
        f32_coefficients[1],
        f32_coefficients[2],
        center as f32,
    ) else {
        return false;
    };
    let Some((min_y_f32, max_y_f32)) = bounds_f32_shader(
        f32_coefficients[3],
        f32_coefficients[4],
        f32_coefficients[5],
        center as f32,
    ) else {
        return false;
    };
    (outside(min_x, max_x, source_dimensions.0) || outside(min_y, max_y, source_dimensions.1))
        && (outside(min_x_f32, max_x_f32, source_dimensions.0)
            || outside(min_y_f32, max_y_f32, source_dimensions.1))
}

/// Lower a raw-word nearest rotate through the same fixed-point affine proof
/// used by typed/indexed transforms. The public rotate node is expanded to an
/// affine `Transform` only after Pillow's angle/center/translation geometry
/// has been materialized, so construct that exact intermediate here for the
/// admission check rather than widening every scalar-word rotate. PA carries
/// an opaque index/alpha pair just like its already-admitted affine path;
/// filtered floating-point rotation is intentionally excluded because it
/// interpolates sample values and therefore needs the ordered-f64 arithmetic
/// proof instead of a relocation proof.
fn gpu_rotate_nearest_affine_is_exact(
    op: &PipelineOp,
    image: &DynamicImage,
    mode: Option<&str>,
    source_dimensions: (u32, u32),
) -> bool {
    if !matches!(mode, Some("CMYK" | "F" | "PA")) {
        return false;
    }
    let PipelineOp::Rotate {
        nearest,
        angle,
        expand,
        fill,
        center,
        translate,
        ..
    } = op
    else {
        return false;
    };
    if !*nearest {
        return false;
    }
    let Some((affine, (w, h))) = gpu_rotate_affine(
        *angle,
        *expand,
        *fill,
        *center,
        *translate,
        source_dimensions.0,
        source_dimensions.1,
    ) else {
        return false;
    };
    let transformed = PipelineOp::Transform {
        w,
        h,
        method: TransformMethod::Affine,
        data: Arc::from(affine.to_vec()),
        filter: ResampleFilter::Nearest,
        fill: *fill,
        fill_is_none: fill.is_none(),
        palette_fill: None,
    };
    gpu_nearest_affine_is_exact(&transformed, image, mode, source_dimensions)
}

fn gpu_geometry_requires_exact_host_control(
    ops: &[PipelineOp],
    image: &DynamicImage,
    mode: Option<&str>,
    f_resize_constant_bits: Option<u32>,
    f_resize_box_copy_is_exact: bool,
    f_resize_identity_is_exact: bool,
    f_resize_box_average_is_exact: bool,
    f_resize_dyadic_is_exact: bool,
    f_resize_f64_is_exact: bool,
) -> bool {
    // The affine shader is byte-exact for ordinary packed layouts, and its
    // fixed-point nearest branch additionally owns opaque raw words (CMYK/F)
    // and PA index/alpha pairs. Non-affine coordinates use a different
    // arithmetic contract: the corrected CPU path evaluates perspective maps
    // in f64 at pixel centers, while the shader evaluates raw destination
    // coordinates in f32. Keep every non-proven projective/quad/mesh transform
    // on exact host semantic control; the bounded projective helper below
    // retains only its exhaustive source-selection proof for raw byte samples.
    let rotate_needs_typed_control = gpu_rotate_requires_exact_host_control(image, mode);
    let mut dimensions = image.dimensions();
    for op in ops {
        if mode == Some("F")
            && let PipelineOp::Resize { w, h, .. } = op
            && gpu_f_resize_uses_pillow_tall_order(dimensions, (*w, *h))
        {
            // Pillow's Python-level tall-image optimization changes the
            // observable pass order to vertical-first. The current GPU
            // reducer is horizontal-first, so the CPU exact path owns these
            // rows until the alternate device plan is proven.
            return true;
        }
        let thumbnail_needs_control = matches!(op, PipelineOp::Thumbnail { .. })
            && gpu_thumbnail_requires_exact_host_control(op, dimensions, image, mode);
        let projective_transform_needs_control =
            matches!(
                op,
                PipelineOp::Transform {
                    method: TransformMethod::Perspective
                        | TransformMethod::Quad
                        | TransformMethod::Mesh,
                    ..
                }
            ) && !gpu_projective_nearest_is_exact(op, image, mode, dimensions);
        let typed_transform_needs_control = rotate_needs_typed_control
            && matches!(op, PipelineOp::Transform { .. })
            && !gpu_nearest_affine_is_exact(op, image, mode, dimensions)
            && !gpu_projective_nearest_is_exact(op, image, mode, dimensions)
            && !gpu_transform_all_fill_is_exact(op, image, mode, dimensions);
        if thumbnail_needs_control
            || projective_transform_needs_control
            || typed_transform_needs_control
            || (rotate_needs_typed_control && matches!(op, PipelineOp::Rotate { .. }))
        {
            return true;
        }
        if let Some(next) = op_output_dims(op, dimensions.0, dimensions.1) {
            dimensions = next;
        }
    }
    // Nearest resize only relocates complete four-byte samples for F mode;
    // the raw mode-8 shader preserves those bytes and the affine coordinate
    // contract is already represented by its native relocation path. I mode
    // uses the same opaque-word relocation, with host-generated one-tap
    // tables preserving Pillow's cumulative f64 source walk. Keep filtered I
    // resize on the exact host path until a typed convolution shader carries
    // its integer accumulator and rounding rules. F-mode filtered resize is
    // admitted only when one of the narrow F-mode proofs establishes an
    // exact bit-pattern copy, constant, 2:1 Box average, exact integer-
    // emulated/dyadic arithmetic reduction, or the marker-9 f64 reducer. The
    // marker-9 proof may carry complete-word relocation and nearest stages
    // between filtered resizes; every such intermediate is materialized in
    // the proof before the next reducer. Mixed F samples and unproved
    // arithmetic filters remain on this host-controlled path because ordinary
    // f32 convolution cannot reproduce Pillow's f64 rounding.
    let has_filtered_resize = ops.iter().any(|op| {
        matches!(
            op,
            PipelineOp::Resize { filter, .. }
                if !matches!(filter, ResampleFilter::Nearest)
        )
    });
    (mode == Some("I") && has_filtered_resize && !f_resize_f64_is_exact)
        || (matches!(mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
            && has_filtered_resize
            && !gpu_luma16_resize_f64_is_exact(ops, image, mode))
        || (mode == Some("F")
            && has_filtered_resize
            && f_resize_constant_bits.is_none()
            && !f_resize_box_copy_is_exact
            && !f_resize_identity_is_exact
            && !f_resize_box_average_is_exact
            && !f_resize_dyadic_is_exact
            && !f_resize_f64_is_exact)
}

fn validate_gpu_operations(
    ops: &[PipelineOp],
    image: &DynamicImage,
    mode: Option<&str>,
) -> Result<(), PilError> {
    for op in ops {
        let i_mode_filter = mode == Some("I")
            && matches!(
                op,
                PipelineOp::Filter3x3 { .. } | PipelineOp::Filter5x5 { .. }
            )
            && (gpu_int_filter_is_supported(ops, image)
                || gpu_int_filter_resize_chain_is_supported(ops, image));
        if !gpu_operation_is_safe(op) && !i_mode_filter {
            return Err(PilError::ValueError(format!(
                "GPU operation '{}' exceeds the bounded shader safety limits",
                registry::variant_key(op)
            )));
        }
    }
    Ok(())
}

impl GpuPool {
    fn ensure_init() -> Result<&'static GpuInner, PilError> {
        match GPU.get_or_init(GpuInner::new) {
            Ok(gpu) => Ok(gpu),
            Err(error) => Err(error.clone()),
        }
    }
}

// ─── BackendImpl ───────────────────────────────────────────────────────────

impl BackendImpl for GpuPool {
    fn name(&self) -> Backend {
        Backend::Gpu
    }

    fn priority(&self) -> u8 {
        100
    }

    fn supports(&self, op: &PipelineOp) -> Result<bool, PilError> {
        let healthy = match GPU.get() {
            Some(Ok(gpu)) => gpu.failure_detail().is_none(),
            Some(Err(_)) => false,
            None => Self::ensure_init().is_ok(),
        };
        Ok(healthy
            && (gpu_operation_is_safe(op) || gpu_operation_requires_image_context(op))
            && registry::gpu_supports(op)?)
    }

    fn execute_batch(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
    ) -> Result<DynamicImage, PilError> {
        self.execute_batch_with_policy(ops, img, mode, true)
    }

    fn execute_batch_strict(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
    ) -> Result<DynamicImage, PilError> {
        self.execute_batch_with_policy(ops, img, mode, false)
    }
}

impl GpuPool {
    fn execute_exact_host_result(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
    ) -> Result<DynamicImage, PilError> {
        // Keep receipts honest: this path is an exact semantic bridge, not a
        // native GPU implementation.  The marker is consumed by the outer
        // execution boundary and therefore reports the pixels as CPU-owned
        // even when the packed result is subsequently copied through a real
        // GPU dispatch for transport/parity coverage.
        crate::compute::record_pipeline_backend_fallback("exact host semantic control");
        let exact = crate::compute::CpuPool.execute_batch(ops, img, mode)?;

        // Keep the GPU parity lane honest when the result can use the
        // ordinary packed transport: the host implementation determines the
        // public pixels, then a real GPU Duplicate dispatch carries those
        // bytes through the selected device backend. Typed/empty results are
        // returned directly because the packed copy would narrow their
        // sample representation or require a zero-sized storage binding.
        if exact.width() == 0 || exact.height() == 0 || !gpu_image_layout_is_supported(&exact) {
            return Ok(exact);
        }
        let copied = self
            .execute_batch_with_policy(&[PipelineOp::Duplicate], &exact, None, false)
            .unwrap_or_else(|_| exact.clone());
        Ok(crate::image::preserve_mode(&exact, copied))
    }

    fn preflight_failure(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
        allow_cpu_fallback: bool,
        reason: &str,
    ) -> Result<DynamicImage, PilError> {
        if allow_cpu_fallback {
            crate::compute::record_pipeline_backend_fallback(reason);
            let cpu = crate::compute::CpuPool;
            return cpu.execute_batch(ops, img, mode);
        }

        // An explicitly locked GPU is a parity lane, not a permission to
        // manufacture a public NotImplementedError for a valid Pillow
        // operation.  The preflight guards below describe GPU storage and
        // shader limits; they do not describe the public operation's error
        // contract.  Run the same exact Rust operation here so invalid
        // inputs produce Pillow-compatible errors and valid inputs retain
        // their pixels while the native GPU implementation is completed.
        // Automatic routing still takes the normal CPU fallback branch above.
        let _ = reason;
        self.execute_exact_host_result(ops, img, mode)
    }

    fn execute_batch_with_policy(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
        allow_cpu_fallback: bool,
    ) -> Result<DynamicImage, PilError> {
        if ops.is_empty() {
            return Ok(img.clone());
        }

        // Keep the public operation list intact for routing and parity, but
        // execute contiguous exact point runs as one LUT dispatch when the
        // logical mode matches the source's native byte layout.  The helper
        // below excludes palette and typed modes whose public point contract
        // is not represented by the batch-wide GPU mode word.
        let mut dispatch_ops: Vec<PipelineOp> = ops
            .iter()
            .filter(|op| !gpu_reduce_is_identity(op))
            .cloned()
            .collect();
        if dispatch_ops.is_empty() {
            // There are no pixel invocations for Reduce(1, 1). Return the
            // independent Pillow result without forcing an unsupported native
            // layout through the packed GPU transport.
            crate::compute::record_pipeline_dispatch_count(0);
            return Ok(img.clone());
        }
        dispatch_ops = if gpu_byte_point_mode_allowed(img, mode) {
            fuse_gpu_point_ops(&dispatch_ops, mode_code(img))
        } else {
            dispatch_ops
        };
        if mode.is_none() && gpu_image_layout_is_supported(img) {
            dispatch_ops = fuse_gpu_transpose_ops(&dispatch_ops, img.width(), img.height());
        }
        // Normalize geometry wrappers before deriving operation-aligned
        // auxiliary inputs and preflight state.  Thumbnail can expand into a
        // Reduce + Resize pair, so postponing this step would leave those
        // vectors with different lengths.
        dispatch_ops = expand_gpu_geometry_ops(&dispatch_ops, img, img.dimensions(), mode);
        let f_resize_constant_bits = gpu_f_resize_constant_bits(&dispatch_ops, img, mode);
        let f_resize_box_copy_is_exact = gpu_f_resize_box_copy_is_exact(&dispatch_ops, img, mode);
        let f_resize_identity_is_exact = gpu_f_resize_identity_is_exact(&dispatch_ops, img, mode);
        let f_resize_box_average_is_exact =
            gpu_f_resize_box_average_is_exact(&dispatch_ops, img, mode);
        let f_resize_dyadic_is_exact = gpu_f_resize_dyadic_is_exact(&dispatch_ops, img, mode);
        let f_pad_f64_is_exact = gpu_f_pad_f64_is_exact(&dispatch_ops, img, mode);
        let f_resize_f64_ordered_proof =
            gpu_f_resize_f64_ordered_is_exact(&dispatch_ops, img, mode);
        let f_resize_f64_is_exact = gpu_f_resize_f64_is_exact(&dispatch_ops, img, mode)
            || f_pad_f64_is_exact
            || gpu_luma16_resize_f64_is_exact(&dispatch_ops, img, mode)
            || gpu_i_resize_f64_is_exact(&dispatch_ops, img, mode);
        // Marker 12 is selected only when none of the earlier F proofs owns
        // the operation.  In particular, Box-average/dyadic markers consume
        // the fixed-point coefficient table, whereas marker 12 requires the
        // four-word f64 coefficient arena.
        let f_resize_f64_ordered_is_exact = f_resize_f64_ordered_proof
            && !f_resize_f64_is_exact
            && f_resize_constant_bits.is_none()
            && !f_resize_box_copy_is_exact
            && !f_resize_identity_is_exact
            && !f_resize_box_average_is_exact
            && !f_resize_dyadic_is_exact;
        if gpu_uniform_blur_can_copy(&dispatch_ops, img) {
            // A normalized blur preserves every channel of a constant image,
            // including the edge samples. Replace only the GPU lowering with
            // an identity dispatch; the public operation remains represented
            // by the same exact result and the selected backend stays GPU.
            gpu_log!("[GPU] lowering constant blur to one identity dispatch");
            dispatch_ops = vec![PipelineOp::Duplicate];
        }
        let ops = dispatch_ops.as_slice();

        // Pillow defines both histogram operations as identity operations for
        // an empty image. There is no valid storage-buffer invocation to
        // issue for a zero-area image, so complete this already-validated
        // no-op without entering the device path or manufacturing a CPU
        // fallback receipt.
        if (img.width() == 0 || img.height() == 0)
            && ops
                .iter()
                .all(|op| matches!(op, PipelineOp::Autocontrast { .. } | PipelineOp::Equalize))
        {
            crate::compute::record_pipeline_dispatch_count(0);
            return Ok(img.clone());
        }

        // A packed dispatch has one logical layout uniform. Operations such
        // as Grayscale, ExtractBand, and PutAlpha change that layout for the
        // following node, so they cannot share a dispatch with later work.
        // Execute the first mode-changing node as the terminal operation of a
        // GPU segment, convert its packed result back to the public native
        // image type, and continue with the remaining nodes. Recursion handles
        // multiple transitions in one pipeline and never introduces a CPU
        // fallback in strict mode.
        if let Some(mode_index) = gpu_first_nonterminal_mode_change(ops) {
            let (prefix, suffix) = ops.split_at(mode_index + 1);
            let prefix_result =
                self.execute_batch_with_policy(prefix, img, mode, allow_cpu_fallback)?;
            // A recursive segment publishes its own receipt counters. Drain
            // them before executing the suffix so the outer benchmark sample
            // can retain one terminal aggregate rather than reporting only
            // the final segment's dispatch/resource counts.
            let prefix_override = crate::compute::take_pipeline_backend_override();
            let prefix_resource = crate::compute::take_pipeline_resource_telemetry();
            let prefix_dispatch = crate::compute::take_pipeline_dispatch_count().unwrap_or(0);
            if suffix.is_empty() {
                if let Some(prefix_override) = prefix_override {
                    crate::compute::restore_pipeline_backend_override(prefix_override);
                }
                if let Some(resource) = prefix_resource {
                    crate::compute::record_pipeline_resource_telemetry(resource);
                }
                crate::compute::record_pipeline_dispatch_count(prefix_dispatch);
                return Ok(prefix_result);
            }
            let next_mode = gpu_logical_mode_after_op(mode, &prefix[mode_index], &prefix_result);
            let suffix_result = self.execute_batch_with_policy(
                suffix,
                &prefix_result,
                next_mode.as_deref(),
                allow_cpu_fallback,
            )?;
            let suffix_override = crate::compute::take_pipeline_backend_override();
            let suffix_resource = crate::compute::take_pipeline_resource_telemetry();
            let suffix_dispatch = crate::compute::take_pipeline_dispatch_count().unwrap_or(0);
            // A host-controlled prefix is allowed to feed a native GPU
            // suffix, but its CPU override must not mislabel the terminal
            // executor. Preserve the diagnostic reason while reporting the
            // final segment's backend identity. If the suffix itself fell
            // back, its terminal CPU override wins unchanged.
            if let Some(suffix_override) = suffix_override {
                crate::compute::restore_pipeline_backend_override(suffix_override);
            } else if let Some((_, reason)) = prefix_override {
                crate::compute::restore_pipeline_backend_override((Backend::Gpu, reason));
            }
            let mut resource = None;
            crate::compute::merge_pipeline_resource_telemetry(&mut resource, prefix_resource);
            crate::compute::merge_pipeline_resource_telemetry(&mut resource, suffix_resource);
            if let Some(resource) = resource {
                crate::compute::record_pipeline_resource_telemetry(resource);
            }
            crate::compute::record_pipeline_dispatch_count(
                prefix_dispatch.saturating_add(suffix_dispatch),
            );
            return Ok(suffix_result);
        }

        // Geometry operations whose public contract includes a reducing-gap,
        // fractional crop, or typed-sample rule must publish the exact host
        // result.  The helper still performs a real GPU copy for packed byte
        // results, so this is a controlled host/GPU boundary rather than an
        // operation-level capability outcome.
        if gpu_geometry_requires_exact_host_control(
            ops,
            img,
            mode,
            f_resize_constant_bits,
            f_resize_box_copy_is_exact,
            f_resize_identity_is_exact,
            f_resize_box_average_is_exact,
            f_resize_dyadic_is_exact,
            f_resize_f64_is_exact || f_resize_f64_ordered_is_exact,
        ) {
            return self.execute_exact_host_result(ops, img, mode);
        }

        // GPU shaders consume the packed L/LA/RGB/RGBA representation. P and
        // PA are safe for raw channel-wise Chops because the core exposes
        // their index/(index, alpha) samples as Luma8/LumaA8 buffers; no
        // palette expansion is involved. CMYK is additionally safe for a
        // brightness/color-saturation batch because the core stores its
        // C/M/Y/K bytes in the same four-byte transport and the corresponding
        // shaders explicitly process K. EffectSpread is a separate raw-byte
        // relocation: its host-generated map is gathered as complete packed
        // words, so it is safe for every mode backed by the ordinary byte
        // transport, including the logical modes below.
        let logical_mode_supported = mode.is_none_or(|logical_mode| {
            matches!(logical_mode, "L" | "LA" | "RGB" | "RGBA")
                // ImageDraw's geometry is scan-converted by the exact host
                // canvas before the packed draw shader copies the complete
                // result.  The data-plane therefore preserves raw indexed,
                // typed-word, and native color bytes just like the ordinary
                // byte modes; no shader arithmetic interprets those samples.
                || (matches!(
                    logical_mode,
                    "1" | "P" | "PA" | "RGBX" | "RGBa" | "CMYK" | "HSV" | "YCbCr" | "I" | "F"
                ) && !ops.is_empty()
                    && ops
                        .iter()
                        .all(crate::compute::pool_cpu::ops::draw::is_draw_op))
                || (matches!(logical_mode, "P" | "PA")
                    && ops.iter().all(|op| {
                        matches!(
                            op,
                                PipelineOp::Add { .. }
                                | PipelineOp::Subtract { .. }
                                | PipelineOp::Multiply { .. }
                                | PipelineOp::Screen { .. }
                                | PipelineOp::Darker { .. }
                                | PipelineOp::Lighter { .. }
                                | PipelineOp::Difference { .. }
                                | PipelineOp::Overlay { .. }
                                | PipelineOp::HardLight { .. }
                                | PipelineOp::SoftLight { .. }
                                | PipelineOp::AddModulo { .. }
                                | PipelineOp::SubtractModulo { .. }
                                | PipelineOp::PutAlpha { .. }
                                | PipelineOp::PutAlphaData { .. }
                                | PipelineOp::PutData { .. }
                                | PipelineOp::ExtractBand { .. }
                                | PipelineOp::Eval { .. }
                                | PipelineOp::PutPixel { .. }
                                | PipelineOp::EffectSpread { .. }
                                | PipelineOp::CompositeModule { .. }
                                | PipelineOp::Filter3x3 { .. }
                                | PipelineOp::Filter5x5 { .. }
                                | PipelineOp::RemapPalette { .. }
                                | PipelineOp::InvertChops
                                | PipelineOp::Offset { .. }
                                | PipelineOp::Mirror
                                | PipelineOp::Transpose { .. }
                                | PipelineOp::Crop { .. }
                                | PipelineOp::CropBorder { .. }
                                | PipelineOp::Expand { .. }
                                | PipelineOp::Duplicate
                                | PipelineOp::Flip
                                | PipelineOp::Reduce { .. }
                                | PipelineOp::Paste { .. }
                                | PipelineOp::Scale { .. }
                                | PipelineOp::Contain { .. }
                                | PipelineOp::Cover { .. }
                                | PipelineOp::Pad { .. }
                                | PipelineOp::Fit {
                                    filter: ResampleFilter::Nearest,
                                    ..
                                }
                                | PipelineOp::Transform { .. }
                                | PipelineOp::Resize {
                                    ..
                                }
                                | PipelineOp::Equalize
                        )
                    }))
                || gpu_palette_first_rgb_merge_is_supported(ops, mode)
                || (logical_mode == "1"
                    && ops
                        .iter()
                        .all(|op| {
                            matches!(
                                op,
                                PipelineOp::PutData { .. }
                                    | PipelineOp::PutPixel { .. }
                                    | PipelineOp::EffectSpread { .. }
                                    | PipelineOp::CompositeModule { .. }
                                    | PipelineOp::Filter3x3 { .. }
                                | PipelineOp::Filter5x5 { .. }
                                    | PipelineOp::Eval { .. }
                                    | PipelineOp::LogicalAnd { .. }
                                    | PipelineOp::LogicalOr { .. }
                                    | PipelineOp::LogicalXor { .. }
                                    | PipelineOp::Offset { .. }
                                    | PipelineOp::Transpose { .. }
                                    | PipelineOp::Paste { .. }
                                    | PipelineOp::Duplicate
                                    | PipelineOp::Contain {
                                        filter: crate::pipeline::ResampleFilter::Nearest,
                                        ..
                                    }
                                    | PipelineOp::Cover {
                                        filter: crate::pipeline::ResampleFilter::Nearest,
                                        ..
                                    }
                                    | PipelineOp::Pad {
                                        filter: crate::pipeline::ResampleFilter::Nearest,
                                        ..
                                    }
                                    | PipelineOp::Transform { .. }
                                    | PipelineOp::Resize {
                                        filter: crate::pipeline::ResampleFilter::Nearest,
                                        ..
                                    }
                            )
                        }))
                // RGBX shares RGBA's four-byte storage, but its fourth byte
                // is padding rather than alpha. Transpose is a pure native
                // byte relocation, so the RGBA transport can preserve all
                // four bytes without applying alpha semantics. PutPixel is
                // likewise a raw four-byte write in this logical mode.
                || (logical_mode == "RGBX"
                    && ops.iter().all(|op| {
                        matches!(
                            op,
                            PipelineOp::PutPixel { .. }
                                | PipelineOp::EffectSpread { .. }
                                | PipelineOp::Transpose { .. }
                                | PipelineOp::Paste { mask: None, .. }
                                | PipelineOp::Scale { .. }
                                | PipelineOp::Contain { .. }
                                | PipelineOp::Cover { .. }
                                | PipelineOp::Pad { .. }
                                | PipelineOp::Transform { .. }
                                | PipelineOp::Resize {
                                    ..
                                }
                                | PipelineOp::Convert {
                                    mode: ColorMode::L
                                        | ColorMode::LA
                                        | ColorMode::RGB
                                        | ColorMode::RGBA
                                        | ColorMode::CMYK
                                        | ColorMode::YCbCr
                                        | ColorMode::HSV
                                        | ColorMode::I
                                        | ColorMode::F,
                                    matrix: None,
                                    dither: None,
                                }
                        )
                    }))
                // RGBa uses the same four-byte storage as RGBA. Pillow's
                // Image.resize leaves RGBa in its already-premultiplied
                // representation (PIL/Image.py resize mode dispatch), so
                // boxed Fit can use the same coefficient kernels with
                // `premultiply = 0`; ImageChops likewise operates on these
                // stored bytes directly. Keep this whitelist limited to
                // raw-channel operations and geometry that preserves that
                // four-byte sample contract.
                || (logical_mode == "RGBa"
                    && ops.iter().all(|op| {
                        matches!(
                            op,
                            PipelineOp::PutPixel { .. }
                                | PipelineOp::EffectSpread { .. }
                                | PipelineOp::Add { .. }
                                | PipelineOp::Subtract { .. }
                                | PipelineOp::Multiply { .. }
                                | PipelineOp::Screen { .. }
                                | PipelineOp::Darker { .. }
                                | PipelineOp::Lighter { .. }
                                | PipelineOp::Difference { .. }
                                | PipelineOp::Overlay { .. }
                                | PipelineOp::HardLight { .. }
                                | PipelineOp::SoftLight { .. }
                                | PipelineOp::AddModulo { .. }
                                | PipelineOp::SubtractModulo { .. }
                                | PipelineOp::InvertChops
                                | PipelineOp::Paste { mask: None, .. }
                                | PipelineOp::Scale { .. }
                                | PipelineOp::Contain { .. }
                                | PipelineOp::Cover { .. }
                                | PipelineOp::Pad { .. }
                                | PipelineOp::Transform { .. }
                                | PipelineOp::Fit { .. }
                                | PipelineOp::Resize {
                                    ..
                                }
                        )
                    }))
                || (matches!(logical_mode, "HSV" | "YCbCr")
                    && ops.iter().all(|op| {
                        matches!(
                            op,
                            PipelineOp::Add { .. }
                                | PipelineOp::Subtract { .. }
                                | PipelineOp::Multiply { .. }
                                | PipelineOp::Screen { .. }
                                | PipelineOp::Darker { .. }
                                | PipelineOp::Lighter { .. }
                                | PipelineOp::Difference { .. }
                                | PipelineOp::Overlay { .. }
                                | PipelineOp::HardLight { .. }
                                | PipelineOp::SoftLight { .. }
                                | PipelineOp::AddModulo { .. }
                                | PipelineOp::SubtractModulo { .. }
                                | PipelineOp::Brightness { .. }
                                | PipelineOp::Filter3x3 { .. }
                                | PipelineOp::Filter5x5 { .. }
                                    | PipelineOp::Reduce { .. }
                                    | PipelineOp::PutData { .. }
                                    | PipelineOp::Eval { .. }
                                | PipelineOp::PutPixel { .. }
                                | PipelineOp::EffectSpread { .. }
                                | PipelineOp::Paste { .. }
                                | PipelineOp::Crop { .. }
                                | PipelineOp::Scale { .. }
                                | PipelineOp::Contain { .. }
                                | PipelineOp::Cover { .. }
                                | PipelineOp::Pad { .. }
                                | PipelineOp::Transform { .. }
                                | PipelineOp::Resize {
                                    ..
                                }
                        )
                    }))
                // CMYK, HSV, and YCbCr retain their native channel order in
                // the packed RGBA/RGB transport.  ExtractBand only copies
                // one requested byte and then publishes an L8 result, so it
                // does not reinterpret the samples as RGB or alpha.  Keep
                // PutPixel here as well: the maintained getchannel batches
                // often write one source pixel before extracting its band.
                || (matches!(logical_mode, "CMYK" | "HSV" | "YCbCr")
                    && ops.iter().all(|op| {
                        matches!(
                            op,
                            PipelineOp::ExtractBand { .. }
                                | PipelineOp::PutPixel { .. }
                                | PipelineOp::EffectSpread { .. }
                        )
                    }))
                // CMYK putalpha is a terminal promotion through the exact
                // integer CMYK->RGB conversion in put_alpha.wgsl.  Keep this
                // whitelist terminal-only: after promotion the public mode
                // is RGBA, and a following operation needs a segmented batch
                // with the updated logical layout rather than CMYK metadata.
                || (logical_mode == "CMYK"
                    && ops.len() == 1
                    && matches!(
                        ops[0],
                        PipelineOp::PutAlpha {
                            mode: PixelMode::CMYK,
                            ..
                        } | PipelineOp::PutAlphaData {
                            mode: PixelMode::CMYK,
                            ..
                        }
                    ))
                || (matches!(logical_mode, "RGB" | "RGBA" | "CMYK")
                    && ops
                        .iter()
                        .all(|op| matches!(op, PipelineOp::Color3DLut { .. })))
                // F stores one finite f32 sample in each four-byte word.
                // Its order-statistic shaders compare the decoded samples;
                // the ordinary byte filters would sort IEEE-754 bytes and
                // produce a numerically unrelated result. Mirror remains in
                // this clause because it only relocates complete words.
                || (logical_mode == "F" && gpu_float_filter_is_supported(ops, img))
                // A constant F Pad uses the exact scalar resize marker for
                // its contain step and a raw-word placement shader for the
                // final canvas. The source proof is deliberately limited to
                // a single non-nearest Pad; mixed batches still need the
                // host semantic path until their intermediate contract is
                // proven.
                || (logical_mode == "F"
                    && f_resize_constant_bits.is_some()
                    && ops.len() == 1
                    && matches!(ops[0], PipelineOp::Pad { .. }))
                // A heterogeneous F Pad uses marker 9 for its contain
                // resize and then copies complete words through placement.
                // The admission proof is limited to one changed-axis Pad
                // with an optional PutData(F)-only prefix; nearest, same-size,
                // and unrelated prefixes retain their existing paths.
                || (logical_mode == "F"
                    && f_pad_f64_is_exact)
                // A nearest F Fit is a two-axis one-tap word relocation. Its
                // boxed coefficients are generated on the host after the
                // same f32 crop-boundary conversion as Pillow's affine
                // nearest path; keep it separate from the vertical pass so
                // Metal cannot observe a stale horizontal intermediate.
                || (logical_mode == "F"
                    && ops.len() == 1
                    && matches!(
                        ops[0],
                        PipelineOp::Fit {
                            filter: ResampleFilter::Nearest,
                            ..
                        }
                    ))
                // I-mode nearest Pad carries signed int32 words through a
                // nearest contain resize and a raw-word placement pass. A
                // filtered Pad would need the typed INT32 accumulator and is
                // intentionally kept on exact host semantic control.
                || (logical_mode == "I"
                    && ops.iter().all(|op| {
                        matches!(
                            op,
                            PipelineOp::Pad {
                                filter: ResampleFilter::Nearest,
                                ..
                            }
                                | PipelineOp::Resize {
                                    filter: ResampleFilter::Nearest,
                                    ..
                                }
                        )
                    }))
                || (logical_mode == "I"
                    && (gpu_int_filter_is_supported(ops, img)
                        || gpu_int_filter_resize_chain_is_supported(ops, img)))
                // I/F samples are four raw bytes per pixel at this executor
                // boundary.  These operations only relocate or duplicate
                // the complete sample and therefore do not need to decode it
                // as an integer, float, or color.  Keep arithmetic and fill
                // operations out of this clause: their shader contracts need
                // a native typed buffer rather than packed RGBA semantics.
                || (matches!(logical_mode, "I" | "F")
                    && ops.iter().all(|op| {
                        matches!(
                            op,
                            PipelineOp::PutData { .. }
                                | PipelineOp::Offset { .. }
                                | PipelineOp::EffectSpread { .. }
                                | PipelineOp::Flip
                                | PipelineOp::Mirror
                                | PipelineOp::Transpose { .. }
                                | PipelineOp::Crop { .. }
                                | PipelineOp::CropBorder { .. }
                                | PipelineOp::Duplicate
                                | PipelineOp::Paste { mask: None, .. }
                                | PipelineOp::Scale {
                                    ..
                                }
                                | PipelineOp::Contain {
                                    ..
                                }
                                | PipelineOp::Cover {
                                    ..
                                }
                                | PipelineOp::Resize {
                                    ..
                                }
                                | PipelineOp::Transform { .. }
                        )
                    }))
                // Native I;16* images use one typed u16 sample per pixel.
                // The packed GPU word is only an opaque transport for these
                // relocation operations; it is never narrowed to an 8-bit
                // luma value or interpreted as RGBA.
                || (matches!(logical_mode, "I;16" | "I;16L" | "I;16B" | "I;16N")
                    && (gpu_luma16_geometry_is_supported(ops, img, Some(logical_mode))
                        || gpu_luma16_convert_is_supported(ops, img)
                        || gpu_luma16_paste_is_supported(ops, img)))
                || (logical_mode == "CMYK"
                    && ops
                        .iter()
                        .all(|op| {
                            matches!(
                                op,
                                PipelineOp::Brightness { .. }
                                    | PipelineOp::Contrast { .. }
                                    | PipelineOp::ColorSaturation { .. }
                                    | PipelineOp::Sharpness { .. }
                                    | PipelineOp::InvertChops
                                    | PipelineOp::Add { .. }
                                    | PipelineOp::Subtract { .. }
                                    | PipelineOp::Multiply { .. }
                                    | PipelineOp::Screen { .. }
                                    | PipelineOp::Darker { .. }
                                    | PipelineOp::Lighter { .. }
                                    | PipelineOp::Difference { .. }
                                    | PipelineOp::Overlay { .. }
                                    | PipelineOp::HardLight { .. }
                                    | PipelineOp::SoftLight { .. }
                                    | PipelineOp::AddModulo { .. }
                                    | PipelineOp::SubtractModulo { .. }
                                    | PipelineOp::LogicalAnd { .. }
                                    | PipelineOp::LogicalOr { .. }
                                    | PipelineOp::LogicalXor { .. }
                                    | PipelineOp::Filter3x3 { .. }
                                    | PipelineOp::Filter5x5 { .. }
                                    | PipelineOp::BlendModule { .. }
                                    | PipelineOp::Offset { .. }
                                    | PipelineOp::Mirror
                                    | PipelineOp::Transpose { .. }
                                    | PipelineOp::Crop { .. }
                                    | PipelineOp::CropBorder { .. }
                                    | PipelineOp::Expand { .. }
                                    | PipelineOp::Duplicate
                                    | PipelineOp::Flip
                            | PipelineOp::Reduce { .. }
                                | PipelineOp::CompositeModule { .. }
                                | PipelineOp::PutData { .. }
                                | PipelineOp::Eval { .. }
                                | PipelineOp::PutPixel { .. }
                                | PipelineOp::EffectSpread { .. }
                                | PipelineOp::Paste { .. }
                                | PipelineOp::Scale { .. }
                                | PipelineOp::Contain { .. }
                            | PipelineOp::Cover { .. }
                            | PipelineOp::Pad { .. }
                            | PipelineOp::Transform { .. }
                            | PipelineOp::Resize {
                                    ..
                                }
                            )
                        }))
        });
        if !logical_mode_supported {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to exact host semantic control: logical layout"
            );
            return self.preflight_failure(
                ops,
                img,
                mode,
                allow_cpu_fallback,
                "exact host semantic control",
            );
        }

        // A palette-index PutPixel has the same concrete Luma8 storage as an
        // ordinary L image, so the logical mode is the only distinction that
        // prevents a direct-core caller from accidentally treating the index
        // as a visible luma sample. PA uses the non-indexed flag and is
        // already represented by the LumaA8 layout check below.
        if ops.iter().any(|op| {
            matches!(
                op,
                PipelineOp::PutPixel {
                    palette_index: true,
                    ..
                }
            )
        }) && mode != Some("P")
        {
            return self.preflight_failure(
                ops,
                img,
                mode,
                allow_cpu_fallback,
                "palette index requires P logical mode",
            );
        }

        // The uniform mode word describes the source layout for the whole
        // dispatch batch. Convert, grayscale, and getchannel change that
        // layout; a later shader would otherwise interpret the new packed
        // pixels using the old source mode. Keep the pipeline lazy, but hand
        // mixed-mode batches to the universal sequential CPU executor until
        // GPU batch segmentation carries an updated mode between dispatches.
        if gpu_batch_has_nonterminal_mode_change(ops) {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to CPU: mode-changing op is not terminal"
            );
            return self.preflight_failure(
                ops,
                img,
                mode,
                allow_cpu_fallback,
                "non-terminal mode change",
            );
        }

        if !gpu_image_layout_is_supported(img)
            && !gpu_luma16_geometry_is_supported(ops, img, mode)
            && !gpu_luma16_convert_is_supported(ops, img)
            && !gpu_luma16_paste_is_supported(ops, img)
        {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to CPU: unsupported native pixel layout"
            );
            return self.preflight_failure(
                ops,
                img,
                mode,
                allow_cpu_fallback,
                "unsupported native pixel layout",
            );
        }

        for op in ops {
            let i_mode_filter = mode == Some("I")
                && matches!(
                    op,
                    PipelineOp::Filter3x3 { .. } | PipelineOp::Filter5x5 { .. }
                )
                && (gpu_int_filter_is_supported(ops, img)
                    || gpu_int_filter_resize_chain_is_supported(ops, img));
            if !registry::gpu_supports(op)? && !i_mode_filter {
                return self.preflight_failure(
                    ops,
                    img,
                    mode,
                    allow_cpu_fallback,
                    "no valid single-dispatch shader contract",
                );
            }
        }

        // Keep this guard at execution time as well as in `supports`: explicit
        // backend selection and future callers must not bypass shader bounds.
        if let Err(error) = validate_gpu_operations(ops, img, mode) {
            let reason = error.to_string();
            return self.preflight_failure(ops, img, mode, allow_cpu_fallback, &reason);
        }

        // Check the primary image and every declared output before resolving
        // nested images. An empty/oversized outer image must not initialize a
        // device merely because a later auxiliary pipeline is present; the
        // entire batch will be handled by the CPU fallback.
        let empty_two_input_noop = gpu_empty_two_input_batch_is_noop(ops, img);
        if gpu_dimensions_require_cpu(ops, img, mode) && !empty_two_input_noop {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to CPU: unsafe primary image dimensions"
            );
            return self.preflight_failure(
                ops,
                img,
                mode,
                allow_cpu_fallback,
                "unsafe primary image dimensions",
            );
        }

        // Resolve every nested image before starting GPU work. A nested
        // explicitly locked pipeline may itself need the GPU pool, and Pillow
        // surfaces that materialization failure instead of dispatching the
        // outer shader with an empty auxiliary buffer.
        let auxiliary_images = {
            let mut dimensions = img.dimensions();
            let mut images = Vec::with_capacity(ops.len());
            let mut draw_preview = img.clone();
            let mut draw_preview_valid = true;
            for op in ops {
                let rendered =
                    if draw_preview_valid && crate::compute::pool_cpu::ops::draw::is_draw_op(op) {
                        Some(crate::compute::pool_cpu::ops::draw::execute_draw_batch(
                            &draw_preview,
                            std::slice::from_ref(op),
                            mode,
                        )?)
                    } else {
                        None
                    };
                images.push(extract_auxiliary_images(op, dimensions, rendered.as_ref())?);
                if let Some(rendered) = rendered {
                    draw_preview = rendered;
                } else if !crate::compute::pool_cpu::ops::draw::is_draw_op(op) {
                    // Drawing after an arbitrary GPU operation needs the
                    // preceding native result as its geometry canvas. Keep
                    // the current bounded implementation explicit rather
                    // than rendering against stale source pixels.
                    draw_preview_valid = false;
                }
                if let Some(next) = op_output_dims(op, dimensions.0, dimensions.1) {
                    dimensions = next;
                }
            }
            images
        };

        if empty_two_input_noop {
            if !gpu_empty_two_input_inputs_are_safe(ops, img, &auxiliary_images) {
                gpu_log!(
                    "[GPU] dispatch preflight routed batch to CPU: unsafe or incomplete empty-image inputs"
                );
                return self.preflight_failure(
                    ops,
                    img,
                    mode,
                    allow_cpu_fallback,
                    "unsafe or incomplete empty-image inputs",
                );
            }
            // There are no invocations to submit. Record an explicit zero
            // dispatch so telemetry does not substitute the operation-count
            // estimate for this valid no-op path.
            crate::compute::record_pipeline_dispatch_count(0);
            return Ok(img.clone());
        }

        if gpu_pipeline_requires_cpu(ops, img, &auxiliary_images, mode) {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to CPU: unsafe or incomplete image dimensions"
            );
            return self.preflight_failure(
                ops,
                img,
                mode,
                allow_cpu_fallback,
                "unsafe or incomplete image dimensions",
            );
        }

        // Contrast's midpoint belongs to the current source image. The
        // batch uniform block carries one midpoint, so compute it before
        // upload. A single exact PutPixel prefix can be mirrored without
        // publishing a second backend receipt; all other current-image
        // sequences retain the exact host-control path.
        let contrast_mean = if ops
            .iter()
            .any(|op| matches!(op, PipelineOp::Contrast { .. }))
        {
            match gpu_contrast_mean_after_exact_prefix(img, ops, mode) {
                Some(mean) => Some(mean),
                None => {
                    let reason = if ops
                        .iter()
                        .position(|op| matches!(op, PipelineOp::Contrast { .. }))
                        == Some(0)
                    {
                        "Contrast midpoint is unavailable for this image layout"
                    } else {
                        "Contrast midpoint requires the current host image"
                    };
                    return self.preflight_failure(ops, img, mode, allow_cpu_fallback, reason);
                }
            }
        } else {
            None
        };

        let gpu = Self::ensure_init()?;
        gpu.ensure_healthy("GPU batch start")?;
        if gpu_dispatch_dimensions_require_cpu(
            ops,
            img.dimensions(),
            gpu.device.limits().max_compute_workgroups_per_dimension,
            mode,
        ) {
            gpu_log!("[GPU] dispatch preflight routed batch to CPU: adapter workgroup limit");
            return self.preflight_failure(
                ops,
                img,
                mode,
                allow_cpu_fallback,
                "adapter workgroup limit",
            );
        }
        let capacity = gpu_batch_capacity(ops, img, &auxiliary_images, mode)?;
        let limits = gpu.device.limits();
        if gpu_buffer_capacity_exceeds_limits(
            capacity,
            limits.max_storage_buffer_binding_size,
            limits.max_buffer_size,
        ) {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to CPU: image buffer exceeds adapter limits"
            );
            return self.preflight_failure(
                ops,
                img,
                mode,
                allow_cpu_fallback,
                "image buffer exceeds adapter limits",
            );
        }
        let mut buffers = gpu.acquire_buffers(capacity)?;
        let native_luma16 = gpu_luma16_geometry_is_supported(ops, img, mode);
        let native_luma16_convert = gpu_luma16_convert_is_supported(ops, img);
        let native_luma16_paste = gpu_luma16_paste_is_supported(ops, img);
        let (w, h) = img.dimensions();
        let mcode = execution_mode_code(img, mode);
        let op_keys: Vec<&str> = ops.iter().map(|op| registry::variant_key(op)).collect();
        log::debug!(
            "[GPU] {} op(s) {}x{} mode={}: {:?}",
            ops.len(),
            w,
            h,
            mcode,
            op_keys
        );
        gpu_log!(
            "[GPU] step=upload start native_luma16={native_luma16} native_luma16_convert={native_luma16_convert} native_luma16_paste={native_luma16_paste}"
        );
        if native_luma16 {
            let DynamicImage::ImageLuma16(image) = img else {
                return Err(PilError::InternalError(
                    "GPU typed layout was admitted without an ImageLuma16 source".into(),
                ));
            };
            buffers.upload_luma16(&gpu.queue, image, mode)?;
        } else if native_luma16_convert || native_luma16_paste {
            let DynamicImage::ImageLuma16(image) = img else {
                return Err(PilError::InternalError(
                    "GPU typed operation was admitted without an ImageLuma16 source".into(),
                ));
            };
            buffers.upload_luma16_numeric(&gpu.queue, image)?;
        } else {
            buffers.upload_standard_image(&gpu.queue, img)?;
        }
        gpu_log!("[GPU] step=upload done native_luma16={native_luma16}");
        gpu_log!("[GPU] step=execute_batch_impl start");
        let (final_is_a, final_w, final_h, staging, mut resource_telemetry, dispatch_count) = gpu
            .execute_batch_impl(
            ops,
            &auxiliary_images,
            w,
            h,
            mcode,
            mode,
            contrast_mean,
            f_resize_constant_bits,
            f_resize_box_copy_is_exact,
            f_resize_identity_is_exact,
            f_resize_box_average_is_exact,
            f_resize_dyadic_is_exact,
            f_resize_f64_is_exact,
            f_resize_f64_ordered_is_exact,
            &mut buffers,
        )?;
        gpu_log!(
            "[GPU] step=execute_batch_impl done final=({},{}) is_a={}",
            final_w,
            final_h,
            final_is_a
        );
        // The final copy is recorded in the final compute command buffer, so
        // the lazy pipeline performs one readback submission after all GPU
        // operations instead of creating a second command buffer/submit pair.
        gpu_log!("[GPU] step=readback start");
        let result = if native_luma16 {
            gpu.readback_to_luma16(final_w, final_h, &staging.buffer, mode)?
        } else if native_luma16_paste {
            gpu.readback_to_luma16_numeric(final_w, final_h, &staging.buffer)?
        } else {
            gpu.readback_to_image(final_w, final_h, &staging.buffer)?
        };
        gpu_log!("[GPU] step=readback done");
        // map_async completion proves the command buffer no longer uses the
        // working images. Return successful working sets to the bounded pool;
        // every error path drops its buffers instead of risking reuse of an
        // in-flight or device-invalid resource.
        resource_telemetry.upload_bytes = CheckedDims::new(w, h, 4)?.total_bytes() as u64;
        resource_telemetry.readback_bytes =
            CheckedDims::new(final_w, final_h, 4)?.total_bytes() as u64;
        resource_telemetry.retained_cache_bytes = buffers.retained_bytes();
        resource_telemetry.full_frame_copy_count = 2;
        resource_telemetry.mode_conversion_count = u64::from(
            native_luma16_convert
                || native_luma16_paste
                || !matches!(
                    img,
                    DynamicImage::ImageRgba8(_) | DynamicImage::ImageLuma16(_)
                ),
        );
        crate::compute::record_pipeline_resource_telemetry(resource_telemetry);
        crate::compute::record_pipeline_dispatch_count(dispatch_count);
        gpu.recycle_staging(staging);
        gpu.recycle_buffers(buffers);
        // Track the last mode-changing operation. Geometry and other
        // mode-preserving operations after it do not undo the promotion.
        let mut put_alpha_mode = None;
        let mut out_mode = None;
        for op in ops {
            match op {
                PipelineOp::Grayscale | PipelineOp::ExtractBand { .. } => {
                    put_alpha_mode = None;
                    out_mode = Some(crate::raster::ColorType::L8);
                }
                PipelineOp::Constant { .. } => {
                    put_alpha_mode = None;
                    out_mode = Some(crate::raster::ColorType::L8);
                }
                PipelineOp::Convert { mode, .. } => {
                    put_alpha_mode = None;
                    out_mode = gpu_output_color_type(mode);
                }
                PipelineOp::Colorize { .. } => {
                    put_alpha_mode = None;
                    out_mode = Some(crate::raster::ColorType::Rgb8);
                }
                PipelineOp::Merge { mode, .. } => {
                    put_alpha_mode = None;
                    out_mode = gpu_output_color_type(mode);
                }
                PipelineOp::Color3DLut { target_mode, .. } => {
                    put_alpha_mode = None;
                    out_mode = gpu_pixel_mode_color_type(*target_mode);
                }
                PipelineOp::EffectNoise { .. } => {
                    put_alpha_mode = None;
                    out_mode = Some(crate::raster::ColorType::L8);
                }
                PipelineOp::PutAlpha { mode, .. } => {
                    put_alpha_mode = Some(*mode);
                    out_mode = None;
                }
                PipelineOp::PutAlphaData { mode, .. } => {
                    put_alpha_mode = Some(*mode);
                    out_mode = None;
                }
                _ => {}
            }
        }
        if let Some(mode) = put_alpha_mode {
            return put_alpha_output(result, mode);
        }
        if let Some(ct) = out_mode {
            // Bypass preserve_mode — use the override color type directly.
            return gpu_result_as_color_type(result, ct);
        } else {
            Ok(crate::image::preserve_mode(img, result))
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_endian = "little")]
    use super::expand_rgb_into_rgba;
    use super::{
        GPU_POLL_BACKOFF, GPU_POLL_FAST_BACKOFF, GPU_POLL_FAST_RETRIES,
        encode_resize_compact_box_axis, gpu_buffer_reuse_allowed, gpu_byte_point_mode_allowed,
        gpu_contrast_mean, gpu_contrast_mean_after_exact_prefix, gpu_dimensions_require_cpu,
        gpu_dispatch_count, gpu_dispatch_dimensions_require_cpu, gpu_f_pad_f64_is_exact,
        gpu_f_resize_box_average_is_exact, gpu_f_resize_box_copy_is_exact,
        gpu_f_resize_compact_box_axis, gpu_f_resize_compact_box_is_exact,
        gpu_f_resize_compact_box_vertical_only_geometry, gpu_f_resize_constant_bits,
        gpu_f_resize_dyadic_is_exact, gpu_f_resize_f64_is_exact, gpu_f_resize_f64_ordered_is_exact,
        gpu_f_resize_identity_is_exact, gpu_f_resize_integer_is_exact, gpu_f_source_constant_bits,
        gpu_f_thumbnail_constant_is_exact, gpu_f64_integer_to_f32, gpu_float_filter_is_supported,
        gpu_i_resize_f64_is_exact, gpu_i_resize_identity_is_exact,
        gpu_int_filter_resize_chain_is_supported, gpu_luma16_resize_f64_is_exact,
        gpu_nearest_affine_is_exact, gpu_operation_requires_image_context,
        gpu_palette_alpha_projective_relocation_is_admitted,
        gpu_palette_first_rgb_merge_is_supported,
        gpu_projective_filtered_integer_constant_is_admitted,
        gpu_projective_filtered_relocation_is_admitted, gpu_projective_nearest_is_exact,
        gpu_resize_coefficients, gpu_resize_nearest_uses_coefficients,
        gpu_transform_all_fill_is_exact, gpu_transform_fill, gpu_transform_should_premultiply,
        luma16_resample_big_endian, readback_poll_backoff,
    };
    use crate::ops::imageops::ImageOpsColor;
    use crate::ops::rotate::{RotateExpandInput, RotatePointInput, RotateResampleInput};
    use crate::ops::transform::{TransformData, TransformFill};
    use crate::pipeline::{ColorMode, PipelineOp, PixelMode, ResampleFilter, TransformMethod};
    use crate::raster::{
        DynamicImage, GrayAlphaImage, GrayImage, ImageBuffer, Luma, RgbImage, RgbaImage,
    };
    use crate::{Backend, Image, ResampleInput};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    #[test]
    fn grayscale_f_prefix_keeps_gpu_suffix_receipt_terminal() {
        let source_bytes = [
            1.5f32.to_le_bytes(),
            (-2.25f32).to_le_bytes(),
            300.0f32.to_le_bytes(),
            0.0f32.to_le_bytes(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        let source = Image::frombytes("F", (2, 2), &source_bytes).expect("F source");
        let pipeline = crate::ops::imageops::invert(
            &crate::ops::imageops::grayscale(&source).expect("grayscale pipeline"),
        )
        .expect("invert pipeline");
        let expected = pipeline
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU grayscale/invert");
        assert_eq!(expected, vec![0xfe, 0xff, 0x00, 0xff]);

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match pipeline.use_backend(Backend::Gpu).tobytes() {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU grayscale/invert failed: {error}"),
        };
        assert_eq!(actual, expected);
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("segmented grayscale/invert must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.2, 2);
        assert_eq!(telemetry.6, Some(2));
        assert_eq!(telemetry.7.as_deref(), Some("exact host semantic control"));
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn gpu_buffer_reuse_rejects_oversized_working_sets() {
        assert!(gpu_buffer_reuse_allowed(3_072, 768));
        assert!(gpu_buffer_reuse_allowed(768, 768));
        assert!(!gpu_buffer_reuse_allowed(3_073, 768));
        assert!(!gpu_buffer_reuse_allowed(786_432, 768));
        assert!(gpu_buffer_reuse_allowed(u32::MAX, 0));
        assert!(!gpu_buffer_reuse_allowed(u32::MAX, 1));
    }

    #[cfg(target_endian = "little")]
    #[test]
    fn rgb_staging_expansion_matches_dynamic_image_conversion() {
        let cases = [
            (0, 0, Vec::new()),
            (1, 1, vec![0, 127, 255]),
            (2, 1, vec![0, 0, 0, 255, 255, 255]),
            (
                3,
                2,
                vec![
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 240, 241, 242, 253, 254, 255, 0, 0, 0,
                ],
            ),
        ];

        for (width, height, source) in cases {
            let image = RgbImage::from_raw(width, height, source.clone()).unwrap();
            let expected = DynamicImage::ImageRgb8(image).to_rgba8().into_raw();
            let mut actual = vec![0; expected.len()];
            expand_rgb_into_rgba(&source, &mut actual).unwrap();
            assert_eq!(actual, expected);
        }
    }

    #[cfg(target_endian = "little")]
    #[test]
    fn rgb_staging_expansion_rejects_invalid_lengths() {
        assert!(expand_rgb_into_rgba(&[1, 2], &mut [0; 4]).is_err());
        assert!(expand_rgb_into_rgba(&[1, 2, 3], &mut [0; 3]).is_err());
        assert!(expand_rgb_into_rgba(&[1, 2, 3], &mut [0; 8]).is_err());
    }

    #[test]
    fn readback_poll_backoff_is_bounded_by_interval_and_deadline() {
        let now = Instant::now();
        assert_eq!(
            readback_poll_backoff(true, 1, now, now + Duration::from_millis(2)),
            Some(GPU_POLL_FAST_BACKOFF)
        );
        assert_eq!(
            readback_poll_backoff(
                true,
                GPU_POLL_FAST_RETRIES + 1,
                now,
                now + Duration::from_millis(2)
            ),
            Some(GPU_POLL_BACKOFF)
        );

        let final_backoff = Duration::from_micros(250);
        assert_eq!(
            readback_poll_backoff(false, 1, now, now + Duration::from_millis(2)),
            Some(GPU_POLL_BACKOFF)
        );
        assert_eq!(
            readback_poll_backoff(false, GPU_POLL_FAST_RETRIES + 1, now, now + final_backoff),
            Some(final_backoff)
        );
        assert_eq!(readback_poll_backoff(true, 1, now, now), None);
        assert_eq!(
            readback_poll_backoff(true, 1, now + Duration::from_nanos(1), now),
            None
        );
    }

    #[test]
    fn transform_safety_is_deferred_to_image_aware_preflight() {
        let op = PipelineOp::Transform {
            w: 2,
            h: 1,
            method: TransformMethod::Perspective,
            data: Arc::from(vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, f64::NAN, 0.0]),
            filter: ResampleFilter::Nearest,
            fill: Some((0, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: None,
        };

        // The operation-only router runs before the source image is
        // materialized. A non-finite map must therefore reach the GPU pool's
        // image-aware exact-host-control preflight instead of being reported
        // as a public "GPU does not support Transform" capability error.
        assert!(gpu_operation_requires_image_context(&op));
    }

    #[test]
    fn transform_all_fill_proof_tracks_float_and_typed_bounds() {
        let transform = |data: [f64; 6], filter| PipelineOp::Transform {
            w: 6,
            h: 6,
            method: TransformMethod::Affine,
            data: Arc::from(data.to_vec()),
            filter,
            fill: Some((7, 8, 9, 10)),
            fill_is_none: false,
            palette_fill: None,
        };
        let outside = [1.0, 0.0, 100.0, 0.0, 1.0, 100.0];
        let in_bounds = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let rgba =
            DynamicImage::ImageRgba8(RgbaImage::from_raw(16, 16, vec![0; 16 * 16 * 4]).unwrap());
        let rgb =
            DynamicImage::ImageRgb8(RgbImage::from_raw(16, 16, vec![0; 16 * 16 * 3]).unwrap());
        let luma16 = DynamicImage::ImageLuma16(
            ImageBuffer::<Luma<u16>, Vec<u16>>::from_raw(16, 16, vec![0; 16 * 16]).unwrap(),
        );

        assert!(gpu_transform_all_fill_is_exact(
            &transform(outside, ResampleFilter::Bicubic),
            &rgba,
            Some("F"),
            (16, 16),
        ));
        assert!(gpu_transform_all_fill_is_exact(
            &transform(outside, ResampleFilter::Bicubic),
            &rgb,
            Some("HSV"),
            (16, 16),
        ));
        assert!(gpu_transform_all_fill_is_exact(
            &transform(outside, ResampleFilter::Bicubic),
            &luma16,
            Some("I;16"),
            (16, 16),
        ));
        assert!(!gpu_transform_all_fill_is_exact(
            &transform(in_bounds, ResampleFilter::Bicubic),
            &rgba,
            Some("F"),
            (16, 16),
        ));
        assert!(!gpu_transform_all_fill_is_exact(
            &PipelineOp::Transform {
                w: 6,
                h: 6,
                method: TransformMethod::Perspective,
                data: Arc::from(vec![1.0; 8]),
                filter: ResampleFilter::Bicubic,
                fill: Some((7, 8, 9, 10)),
                fill_is_none: false,
                palette_fill: None,
            },
            &rgba,
            Some("F"),
            (16, 16),
        ));
    }

    #[test]
    fn float_nearest_affine_proof_matches_fixed_coordinate_selection() {
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(16, 16, vec![0; 16 * 16 * 4]).expect("F backing image"),
        );
        let extent = PipelineOp::Transform {
            w: 6,
            h: 6,
            method: TransformMethod::Affine,
            data: Arc::from(vec![7.0 / 6.0, 0.0, -1.0, 0.0, 7.0 / 6.0, -1.0]),
            filter: ResampleFilter::Nearest,
            fill: Some((76, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: None,
        };
        assert!(gpu_nearest_affine_is_exact(
            &extent,
            &image,
            Some("F"),
            (16, 16)
        ));

        // A sub-16.16 boundary difference must stay host-controlled: Pillow
        // truncates the original f64 coordinate to source pixel 0, while the
        // uploaded fixed-point origin rounds to source pixel 1.
        let boundary = PipelineOp::Transform {
            w: 1,
            h: 1,
            method: TransformMethod::Affine,
            data: Arc::from(vec![1.0, 0.0, 0.499999, 0.0, 1.0, 0.0]),
            filter: ResampleFilter::Nearest,
            fill: Some((76, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: None,
        };
        let small_image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(2, 2, vec![0; 2 * 2 * 4]).expect("small F backing image"),
        );
        assert!(!gpu_nearest_affine_is_exact(
            &boundary,
            &small_image,
            Some("F"),
            (2, 2)
        ));

        // Filtered F transforms interpolate scalar samples in Pillow; the
        // mode-8 GPU branch is a raw-word relocation and must not be selected
        // merely because the source coordinates happen to be safe.
        let filtered = PipelineOp::Transform {
            w: 1,
            h: 1,
            method: TransformMethod::Affine,
            data: Arc::from(vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0]),
            filter: ResampleFilter::Bilinear,
            fill: Some((76, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: None,
        };
        assert!(!gpu_nearest_affine_is_exact(
            &filtered,
            &small_image,
            Some("F"),
            (2, 2)
        ));

        let no_fill = PipelineOp::Transform {
            w: 1,
            h: 1,
            method: TransformMethod::Affine,
            data: Arc::from(vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0]),
            filter: ResampleFilter::Nearest,
            fill: None,
            fill_is_none: true,
            palette_fill: None,
        };
        assert_eq!(gpu_transform_fill(&no_fill, Some("F"), 8), 0);
    }

    #[test]
    fn float_nearest_affine_native_gpu_preserves_words() {
        let words = (0..16 * 16)
            .map(|index| (index as f32) + 0.25)
            .collect::<Vec<_>>();
        let bytes = words
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        let source = Image::frombytes("F", (16, 16), &bytes).expect("F source");
        let transformed = source
            .transform_public(
                (6, 6),
                1,
                Some(TransformData::Affine(vec![-1.0, -1.0, 6.0, 6.0])),
                0,
                0,
                None,
            )
            .expect("F extent transform");
        let expected = transformed
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU F extent transform");

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU F extent transform failed: {error}"),
        };
        assert_eq!(actual, expected, "native GPU F extent parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native F extent transform must publish telemetry");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn float_nearest_rotate_native_gpu_preserves_words() {
        let mut values = (0..16 * 16)
            .map(|index| (index as f32) + 0.25)
            .collect::<Vec<_>>();
        values[17] = f32::from_bits(0x8000_0000);
        values[85] = f32::from_bits(0x7fc0_1234);
        values[170] = f32::from_bits(0x7f80_0000);
        values[221] = f32::from_bits(0x0000_0001);
        let bytes = values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        let source = Image::frombytes("F", (16, 16), &bytes).expect("F source");
        let rotated = source
            .rotate_with_input(
                13.0,
                RotateResampleInput::Name("NEAREST".into()),
                RotateExpandInput::Boolean(true),
                RotatePointInput::Default,
                RotatePointInput::Default,
                ImageOpsColor::None,
            )
            .expect("F nearest rotate");
        let expected = rotated
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU F nearest rotate");

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match rotated.use_backend(Backend::Gpu).tobytes() {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU F nearest rotate failed: {error}"),
        };
        assert_eq!(actual, expected, "native GPU F nearest rotate parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU F nearest rotate must publish telemetry");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn projective_nearest_proof_is_bounded() {
        let image = DynamicImage::ImageLuma8(
            GrayImage::from_raw(16, 16, vec![0; 16 * 16]).expect("indexed image"),
        );
        let perspective = PipelineOp::Transform {
            w: 8,
            h: 8,
            method: TransformMethod::Perspective,
            data: Arc::from(vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]),
            filter: ResampleFilter::Nearest,
            fill: Some((7, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: Some(7),
        };
        let quad = PipelineOp::Transform {
            w: 8,
            h: 8,
            method: TransformMethod::Quad,
            data: Arc::from(vec![0.0, 0.0, 8.0, 0.0, 8.0, 8.0, 0.0, 8.0]),
            filter: ResampleFilter::Nearest,
            fill: Some((7, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: Some(7),
        };
        let quad_direct = PipelineOp::Transform {
            w: 8,
            h: 8,
            method: TransformMethod::Quad,
            data: Arc::from(vec![0.0, 0.0, 0.0, 8.0, 8.0, 8.0, 8.0, 0.0]),
            filter: ResampleFilter::Nearest,
            fill: Some((7, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: Some(7),
        };
        let quad_constant = PipelineOp::Transform {
            w: 8,
            h: 8,
            method: TransformMethod::Quad,
            data: Arc::from(vec![3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0]),
            filter: ResampleFilter::Nearest,
            fill: Some((7, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: None,
        };
        let quad_constant_fill = PipelineOp::Transform {
            w: 8,
            h: 8,
            method: TransformMethod::Quad,
            data: Arc::from(vec![-1.0, 5.0, -1.0, 5.0, -1.0, 5.0, -1.0, 5.0]),
            filter: ResampleFilter::Nearest,
            fill: Some((7, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: None,
        };
        let mesh = PipelineOp::Transform {
            w: 8,
            h: 8,
            method: TransformMethod::Mesh,
            data: Arc::from(vec![
                0.0, 0.0, 8.0, 8.0, 0.0, 0.0, 8.0, 0.0, 8.0, 8.0, 0.0, 8.0,
            ]),
            filter: ResampleFilter::Nearest,
            fill: Some((7, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: Some(7),
        };
        let mesh_constant = PipelineOp::Transform {
            w: 8,
            h: 8,
            method: TransformMethod::Mesh,
            data: Arc::from(vec![
                0.0, 0.0, 8.0, 8.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0,
            ]),
            filter: ResampleFilter::Nearest,
            fill: Some((7, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: None,
        };
        let mesh_constant_fill = PipelineOp::Transform {
            w: 8,
            h: 8,
            method: TransformMethod::Mesh,
            data: Arc::from(vec![
                0.0, 0.0, 8.0, 8.0, -1.0, 5.0, -1.0, 5.0, -1.0, 5.0, -1.0, 5.0,
            ]),
            filter: ResampleFilter::Nearest,
            fill: Some((7, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: None,
        };
        for op in [&perspective, &quad, &quad_direct, &mesh] {
            assert!(gpu_projective_nearest_is_exact(
                op,
                &image,
                Some("P"),
                (16, 16)
            ));
        }
        let rgb = DynamicImage::ImageRgb8(
            RgbImage::from_raw(16, 16, vec![0; 16 * 16 * 3]).expect("RGB image"),
        );
        let rgba = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(16, 16, vec![0; 16 * 16 * 4]).expect("RGBA image"),
        );
        for (mode, image) in [("RGB", &rgb), ("RGBA", &rgba)] {
            for op in [&perspective, &quad, &quad_direct, &mesh] {
                assert!(
                    gpu_projective_nearest_is_exact(op, image, Some(mode), (16, 16)),
                    "identity projective proof for {mode}"
                );
            }
        }
        for (name, op) in [
            ("constant Quad", &quad_constant),
            ("constant Quad fill", &quad_constant_fill),
            ("constant Mesh", &mesh_constant),
            ("constant Mesh fill", &mesh_constant_fill),
        ] {
            assert!(
                gpu_projective_nearest_is_exact(op, &rgb, Some("RGB"), (16, 16)),
                "{name} proof"
            );
            assert!(
                gpu_projective_nearest_is_exact(op, &rgba, Some("RGBA"), (16, 16)),
                "{name} proof for RGBA"
            );
        }
        assert!(!gpu_projective_filtered_relocation_is_admitted(
            TransformMethod::Quad,
            &[3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0],
            ResampleFilter::Bilinear,
            Some("RGB"),
            (8, 8),
        ));
        assert!(!gpu_projective_filtered_relocation_is_admitted(
            TransformMethod::Mesh,
            &[0.0, 0.0, 8.0, 8.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0],
            ResampleFilter::Bicubic,
            Some("RGBA"),
            (8, 8),
        ));
        let perspective_translate = PipelineOp::Transform {
            w: 8,
            h: 8,
            method: TransformMethod::Perspective,
            data: Arc::from(vec![1.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0]),
            filter: ResampleFilter::Nearest,
            fill: Some((7, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: None,
        };
        let perspective_axis_swap = PipelineOp::Transform {
            w: 8,
            h: 8,
            method: TransformMethod::Perspective,
            data: Arc::from(vec![0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0]),
            filter: ResampleFilter::Nearest,
            fill: Some((7, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: None,
        };
        for (name, op) in [
            ("integer translation", &perspective_translate),
            ("integer axis swap", &perspective_axis_swap),
        ] {
            for (mode, image) in [("RGB", &rgb), ("RGBA", &rgba)] {
                assert!(
                    gpu_projective_nearest_is_exact(op, image, Some(mode), (16, 16)),
                    "{name} proof for {mode}"
                );
            }
        }
        let perspective_translate_negative = PipelineOp::Transform {
            w: 8,
            h: 8,
            method: TransformMethod::Perspective,
            data: Arc::from(vec![1.0, 0.0, -1.0, 0.0, 1.0, -1.0, 0.0, 0.0]),
            filter: ResampleFilter::Nearest,
            fill: Some((7, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: None,
        };
        assert!(gpu_projective_nearest_is_exact(
            &perspective_translate_negative,
            &rgb,
            Some("RGB"),
            (16, 16)
        ));
        let perspective_signed_unit = [
            [-1.0, 0.0, 8.0, 0.0, 1.0, 0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0, -1.0, 8.0, 0.0, 0.0],
            [-1.0, 0.0, 8.0, 0.0, -1.0, 8.0, 0.0, 0.0],
            [0.0, -1.0, 8.0, 1.0, 0.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, -1.0, 0.0, 8.0, 0.0, 0.0],
            [0.0, -1.0, 8.0, -1.0, 0.0, 8.0, 0.0, 0.0],
            [-1.0, 0.0, 7.0, 0.0, 1.0, 0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0, -1.0, 7.0, 0.0, 0.0],
        ];
        for matrix in perspective_signed_unit {
            let op = PipelineOp::Transform {
                w: 8,
                h: 8,
                method: TransformMethod::Perspective,
                data: Arc::from(matrix.to_vec()),
                filter: ResampleFilter::Nearest,
                fill: Some((7, 0, 0, 255)),
                fill_is_none: false,
                palette_fill: None,
            };
            assert!(gpu_projective_nearest_is_exact(
                &op,
                &rgb,
                Some("RGB"),
                (16, 16),
            ));
        }
        let mesh_translate = PipelineOp::Transform {
            w: 8,
            h: 8,
            method: TransformMethod::Mesh,
            data: Arc::from(vec![
                0.0, 0.0, 8.0, 8.0, 1.0, 1.0, 1.0, 9.0, 9.0, 9.0, 9.0, 1.0,
            ]),
            filter: ResampleFilter::Nearest,
            fill: Some((7, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: None,
        };
        let mesh_axis_swap = PipelineOp::Transform {
            w: 8,
            h: 8,
            method: TransformMethod::Mesh,
            data: Arc::from(vec![
                0.0, 0.0, 8.0, 8.0, 0.0, 0.0, 8.0, 0.0, 8.0, 8.0, 0.0, 8.0,
            ]),
            filter: ResampleFilter::Nearest,
            fill: Some((7, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: None,
        };
        for (name, op) in [
            ("unit mesh translation", &mesh_translate),
            ("unit mesh axis swap", &mesh_axis_swap),
        ] {
            assert!(
                gpu_projective_nearest_is_exact(op, &rgb, Some("RGB"), (16, 16)),
                "{name} proof"
            );
        }
        let mesh_scaled = PipelineOp::Transform {
            w: 8,
            h: 8,
            method: TransformMethod::Mesh,
            data: Arc::from(vec![
                0.0, 0.0, 8.0, 8.0, 0.0, 0.0, 0.0, 16.0, 16.0, 16.0, 16.0, 0.0,
            ]),
            filter: ResampleFilter::Nearest,
            fill: Some((7, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: None,
        };
        assert!(!gpu_projective_nearest_is_exact(
            &mesh_scaled,
            &rgb,
            Some("RGB"),
            (16, 16)
        ));
        let mesh_extra_record = PipelineOp::Transform {
            w: 8,
            h: 8,
            method: TransformMethod::Mesh,
            data: Arc::from(vec![
                0.0, 0.0, 8.0, 8.0, 1.0, 1.0, 1.0, 9.0, 9.0, 9.0, 9.0, 1.0, 0.0, 0.0, 8.0, 8.0,
                0.0, 0.0, 0.0, 8.0, 8.0, 8.0, 8.0, 0.0,
            ]),
            filter: ResampleFilter::Nearest,
            fill: Some((7, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: None,
        };
        assert!(!gpu_projective_nearest_is_exact(
            &mesh_extra_record,
            &rgb,
            Some("RGB"),
            (16, 16)
        ));
        let mesh_constant_extra_record = PipelineOp::Transform {
            w: 8,
            h: 8,
            method: TransformMethod::Mesh,
            data: Arc::from(vec![
                0.0, 0.0, 8.0, 8.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 0.0, 0.0, 8.0, 8.0,
                3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0,
            ]),
            filter: ResampleFilter::Nearest,
            fill: Some((7, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: None,
        };
        assert!(!gpu_projective_nearest_is_exact(
            &mesh_constant_extra_record,
            &rgb,
            Some("RGB"),
            (16, 16)
        ));
        let scaled_quad_axis = PipelineOp::Transform {
            w: 9,
            h: 6,
            method: TransformMethod::Quad,
            data: Arc::from(vec![0.0, 0.0, 9.0, 0.0, 9.0, 6.0, 0.0, 6.0]),
            filter: ResampleFilter::Nearest,
            fill: Some((7, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: None,
        };
        assert!(gpu_projective_nearest_is_exact(
            &scaled_quad_axis,
            &rgb,
            Some("RGB"),
            (16, 16)
        ));
        let f32_max = f64::from(f32::MAX);
        let overflowing_quad = PipelineOp::Transform {
            w: 3,
            h: 2,
            method: TransformMethod::Quad,
            data: Arc::from(vec![
                f32_max, 0.0, f32_max, 2.0, f32_max, 2.0, -f32_max, 0.0,
            ]),
            filter: ResampleFilter::Nearest,
            fill: Some((7, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: None,
        };
        // The f64 map is wholly outside the source, but the generic f32
        // Quad expression overflows and can produce NaN.  Keep this case on
        // exact host semantic control rather than trusting equal fill
        // classifications from a non-finite source-selection result.
        assert!(!gpu_projective_nearest_is_exact(
            &overflowing_quad,
            &rgb,
            Some("RGB"),
            (16, 16)
        ));
        let overflowing_mesh = PipelineOp::Transform {
            w: 3,
            h: 2,
            method: TransformMethod::Mesh,
            data: Arc::from(vec![
                0.0, 0.0, 3.0, 2.0, f32_max, 0.0, -f32_max, 0.0, -f32_max, 2.0, f32_max, 2.0,
            ]),
            filter: ResampleFilter::Nearest,
            fill: Some((7, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: None,
        };
        assert!(!gpu_projective_nearest_is_exact(
            &overflowing_mesh,
            &rgb,
            Some("RGB"),
            (16, 16)
        ));
        let fractional = PipelineOp::Transform {
            w: 8,
            h: 8,
            method: TransformMethod::Perspective,
            data: Arc::from(vec![1.0, 0.0, 0.25, 0.0, 1.0, 0.0, 0.0, 0.0]),
            filter: ResampleFilter::Nearest,
            fill: Some((7, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: Some(7),
        };
        assert!(gpu_projective_nearest_is_exact(
            &fractional,
            &image,
            Some("P"),
            (16, 16)
        ));
        assert!(gpu_projective_nearest_is_exact(
            &fractional,
            &rgb,
            Some("RGB"),
            (16, 16)
        ));
        let scaled = PipelineOp::Transform {
            w: 8,
            h: 8,
            method: TransformMethod::Perspective,
            data: Arc::from(vec![2.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0]),
            filter: ResampleFilter::Nearest,
            fill: Some((7, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: None,
        };
        assert!(gpu_projective_nearest_is_exact(
            &scaled,
            &rgb,
            Some("RGB"),
            (16, 16)
        ));
        let overflowing = PipelineOp::Transform {
            w: 3,
            h: 1,
            method: TransformMethod::Perspective,
            data: Arc::from(vec![
                f64::from(f32::MAX),
                0.0,
                2.0,
                0.0,
                1.0,
                0.0,
                f64::from(f32::MAX),
                0.0,
            ]),
            filter: ResampleFilter::Nearest,
            fill: Some((7, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: None,
        };
        // The raw WGSL path overflows at destination x=2 and produces a
        // NaN coordinate; keep this arithmetic-changing boundary on host
        // control even though the ordinary source-selection proof sees fill
        // on both sides of that pixel.
        assert!(!gpu_projective_nearest_is_exact(
            &overflowing,
            &rgb,
            Some("RGB"),
            (16, 16)
        ));
    }

    #[test]
    fn indexed_projective_nearest_native_gpu_preserves_index_bytes() {
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let mut bytes = vec![0u8; 16 * 16];
        bytes[3 * 16 + 2] = 23;
        let source = Image::frombytes("P", (16, 16), &bytes).expect("indexed source");
        let cases = [
            (
                2,
                TransformData::Affine(vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]),
            ),
            (
                3,
                TransformData::Affine(vec![0.0, 0.0, 8.0, 0.0, 8.0, 8.0, 0.0, 8.0]),
            ),
            (
                4,
                TransformData::Mesh(vec![(
                    vec![0.0, 0.0, 8.0, 8.0],
                    vec![0.0, 0.0, 8.0, 0.0, 8.0, 8.0, 0.0, 8.0],
                )]),
            ),
        ];
        for (method, data) in cases {
            let transformed = source
                .transform_public(
                    (8, 8),
                    method,
                    Some(data),
                    0,
                    0,
                    Some(TransformFill::Scalar(7)),
                )
                .expect("indexed projective transform");
            let expected = transformed
                .clone()
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU indexed transform");
            let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU indexed transform failed: {error}"),
            };
            assert_eq!(actual, expected, "indexed transform method {method}");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("native indexed transform must publish a receipt");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.6, Some(1));
            assert_eq!(telemetry.7, None);
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn byte_projective_nearest_proven_native_gpu_preserves_pixels() {
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for (mode, channels) in [
            ("L", 1usize),
            ("LA", 2usize),
            ("RGB", 3usize),
            ("RGBA", 4usize),
        ] {
            let bytes = (0..16 * 16 * channels)
                .map(|index| (index * 29 + 7) as u8)
                .collect::<Vec<_>>();
            let source = Image::frombytes(mode, (16, 16), &bytes).expect("byte source");
            let cases = [
                (
                    2,
                    TransformData::Affine(vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]),
                ),
                (
                    2,
                    TransformData::Affine(vec![1.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0]),
                ),
                (
                    2,
                    TransformData::Affine(vec![0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0]),
                ),
                (
                    2,
                    TransformData::Affine(vec![1.0, 0.0, -1.0, 0.0, 1.0, -1.0, 0.0, 0.0]),
                ),
                (
                    2,
                    TransformData::Affine(vec![-1.0, 0.0, 8.0, 0.0, 1.0, 0.0, 0.0, 0.0]),
                ),
                (
                    2,
                    TransformData::Affine(vec![1.0, 0.0, 0.0, 0.0, -1.0, 8.0, 0.0, 0.0]),
                ),
                (
                    2,
                    TransformData::Affine(vec![-1.0, 0.0, 8.0, 0.0, -1.0, 8.0, 0.0, 0.0]),
                ),
                (
                    2,
                    TransformData::Affine(vec![0.0, -1.0, 8.0, 1.0, 0.0, 0.0, 0.0, 0.0]),
                ),
                (
                    2,
                    TransformData::Affine(vec![0.0, 1.0, 0.0, -1.0, 0.0, 8.0, 0.0, 0.0]),
                ),
                (
                    2,
                    TransformData::Affine(vec![0.0, -1.0, 8.0, -1.0, 0.0, 8.0, 0.0, 0.0]),
                ),
                (
                    3,
                    TransformData::Affine(vec![0.0, 0.0, 8.0, 0.0, 8.0, 8.0, 0.0, 8.0]),
                ),
                (
                    3,
                    TransformData::Affine(vec![0.0, 0.0, 0.0, 8.0, 8.0, 8.0, 8.0, 0.0]),
                ),
                (
                    3,
                    TransformData::Affine(vec![3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0]),
                ),
                (
                    3,
                    TransformData::Affine(vec![-1.0, 5.0, -1.0, 5.0, -1.0, 5.0, -1.0, 5.0]),
                ),
                (
                    4,
                    TransformData::Mesh(vec![(
                        vec![0.0, 0.0, 8.0, 8.0],
                        vec![0.0, 0.0, 0.0, 8.0, 8.0, 8.0, 8.0, 0.0],
                    )]),
                ),
                (
                    4,
                    TransformData::Mesh(vec![(
                        vec![0.0, 0.0, 8.0, 8.0],
                        vec![1.0, 1.0, 1.0, 9.0, 9.0, 9.0, 9.0, 1.0],
                    )]),
                ),
                (
                    4,
                    TransformData::Mesh(vec![(
                        vec![0.0, 0.0, 8.0, 8.0],
                        vec![-1.0, -1.0, -1.0, 7.0, 7.0, 7.0, 7.0, -1.0],
                    )]),
                ),
                (
                    4,
                    TransformData::Mesh(vec![(
                        vec![0.0, 0.0, 8.0, 8.0],
                        vec![0.0, 0.0, 8.0, 0.0, 8.0, 8.0, 0.0, 8.0],
                    )]),
                ),
                (
                    4,
                    TransformData::Mesh(vec![(
                        vec![0.0, 0.0, 8.0, 8.0],
                        vec![3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0],
                    )]),
                ),
                (
                    4,
                    TransformData::Mesh(vec![(
                        vec![0.0, 0.0, 8.0, 8.0],
                        vec![-1.0, 5.0, -1.0, 5.0, -1.0, 5.0, -1.0, 5.0],
                    )]),
                ),
            ];
            for (method, data) in cases {
                let transformed = source
                    .transform_public((8, 8), method, Some(data), 0, 0, None)
                    .expect("proven projective transform");
                let expected = transformed
                    .clone()
                    .use_backend(Backend::Cpu)
                    .tobytes()
                    .expect("CPU proven projective transform");
                let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
                    Ok(actual) => actual,
                    Err(error)
                        if error.to_string().contains("GPU adapter not available")
                            || error
                                .to_string()
                                .contains("GPU device initialization failed") =>
                    {
                        Backend::set_pipeline_telemetry_enabled(previous);
                        return;
                    }
                    Err(error) => {
                        panic!("native GPU proven projective transform failed: {error}")
                    }
                };
                assert_eq!(
                    actual, expected,
                    "native {mode} proven transform method {method}"
                );
                let telemetry = Backend::take_pipeline_telemetry()
                    .expect("native proven projective transform must publish a receipt");
                assert_eq!(telemetry.0, Some(Backend::Gpu));
                assert_eq!(telemetry.1, Backend::Gpu);
                assert_eq!(telemetry.6, Some(1));
                assert_eq!(telemetry.7, None);
            }
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn byte_projective_integer_affine_nearest_native_gpu_preserves_pixels() {
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let cases = [
            (
                (9u32, 7u32),
                (4u32, 4u32),
                [2.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0],
                251u8,
            ),
            (
                (9u32, 7u32),
                (7u32, 5u32),
                [-2.0, 1.0, 8.0, 1.0, 2.0, -3.0, 0.0, 0.0],
                251u8,
            ),
            (
                (9u32, 7u32),
                (6u32, 5u32),
                [1.0, 2.0, -3.0, 2.0, -1.0, 6.0, 0.0, 0.0],
                17u8,
            ),
            (
                (13u32, 11u32),
                (9u32, 7u32),
                [3.0, -1.0, 5.0, 1.0, 2.0, -4.0, 0.0, 0.0],
                199u8,
            ),
        ];
        for (mode, channels) in [
            ("L", 1usize),
            ("LA", 2usize),
            ("RGB", 3usize),
            ("RGBA", 4usize),
            ("P", 1usize),
        ] {
            for (seed, (source_size, output_size, matrix, fill)) in cases.iter().enumerate() {
                let bytes = (0..source_size.0 as usize * source_size.1 as usize * channels)
                    .map(|index| (index * (29 + 2 * seed) + 7 * channels + seed) as u8)
                    .collect::<Vec<_>>();
                let mut source = Image::frombytes(mode, *source_size, &bytes)
                    .unwrap_or_else(|error| panic!("{mode} source: {error}"));
                if mode == "P" {
                    let palette = (0..768)
                        .map(|index| (index * 11 + 3) as u8)
                        .collect::<Vec<_>>();
                    source
                        .putpalette(&palette, "RGB")
                        .unwrap_or_else(|error| panic!("P palette: {error}"));
                }
                let fill = match mode {
                    "L" | "P" => Some(TransformFill::Scalar(i64::from(*fill))),
                    "LA" => Some(TransformFill::Components(vec![i64::from(*fill), 93])),
                    "RGB" => Some(TransformFill::Components(vec![i64::from(*fill), 93, 41])),
                    "RGBA" => Some(TransformFill::Components(vec![
                        i64::from(*fill),
                        93,
                        41,
                        173,
                    ])),
                    _ => unreachable!(),
                };
                let transformed = source
                    .transform_public(
                        *output_size,
                        2,
                        Some(TransformData::Affine(matrix.to_vec())),
                        0,
                        0,
                        fill,
                    )
                    .unwrap_or_else(|error| panic!("{mode} transform: {error}"));
                let expected = transformed
                    .clone()
                    .use_backend(Backend::Cpu)
                    .tobytes()
                    .unwrap_or_else(|error| panic!("{mode} CPU transform: {error}"));
                let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
                    Ok(actual) => actual,
                    Err(error)
                        if error.to_string().contains("GPU adapter not available")
                            || error
                                .to_string()
                                .contains("GPU device initialization failed") =>
                    {
                        Backend::set_pipeline_telemetry_enabled(previous);
                        return;
                    }
                    Err(error) => panic!("native GPU {mode} transform failed: {error}"),
                };
                assert_eq!(
                    actual, expected,
                    "native {mode} integer projective matrix={matrix:?}"
                );
                let telemetry = Backend::take_pipeline_telemetry()
                    .unwrap_or_else(|| panic!("native GPU {mode} transform missing telemetry"));
                assert_eq!(telemetry.0, Some(Backend::Gpu), "{mode} requested backend");
                assert_eq!(telemetry.1, Backend::Gpu, "{mode} actual backend");
                assert_eq!(telemetry.6, Some(1), "{mode} dispatch count");
                assert_eq!(telemetry.7, None, "{mode} fallback reason");
            }
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn byte_projective_fractional_nearest_native_gpu_preserves_pixels() {
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let cases = [
            (
                (9u32, 8u32),
                (8u32, 7u32),
                [1.0, 0.0, 0.25, 0.0, 1.0, 0.25, 0.0, 0.0],
                251u8,
            ),
            (
                (16u32, 16u32),
                (13u32, 11u32),
                [0.5, 0.25, 0.125, -0.25, 0.5, 0.375, 0.0, 0.0],
                17u8,
            ),
            (
                (5u32, 7u32),
                (9u32, 6u32),
                [0.75, -0.125, 0.25, 0.125, 0.625, 0.375, 0.0, 0.0],
                199u8,
            ),
            (
                (32u32, 32u32),
                (16u32, 16u32),
                [1.0, 0.125, 0.25, -0.125, 1.0, 0.375, 0.0, 0.0],
                61u8,
            ),
        ];
        for (mode, channels) in [
            ("L", 1usize),
            ("LA", 2usize),
            ("RGB", 3usize),
            ("RGBA", 4usize),
            ("P", 1usize),
        ] {
            for (case_index, (source_size, output_size, matrix, fill)) in cases.iter().enumerate() {
                let source_bytes = (0..source_size.0 as usize * source_size.1 as usize * channels)
                    .map(|index| (index * (31 + case_index * 7) + 13 + channels) as u8)
                    .collect::<Vec<_>>();
                let mut source = Image::frombytes(mode, *source_size, &source_bytes)
                    .unwrap_or_else(|error| panic!("{mode} source: {error}"));
                if mode == "P" {
                    let palette = (0..768)
                        .map(|index| (index * 17 + 5) as u8)
                        .collect::<Vec<_>>();
                    source
                        .putpalette(&palette, "RGB")
                        .unwrap_or_else(|error| panic!("P palette: {error}"));
                }
                let fill = match mode {
                    "L" | "P" => Some(TransformFill::Scalar(i64::from(*fill))),
                    "LA" => Some(TransformFill::Components(vec![i64::from(*fill), 93])),
                    "RGB" => Some(TransformFill::Components(vec![i64::from(*fill), 93, 41])),
                    "RGBA" => Some(TransformFill::Components(vec![
                        i64::from(*fill),
                        93,
                        41,
                        173,
                    ])),
                    _ => unreachable!(),
                };
                let transformed = source
                    .transform_public(
                        *output_size,
                        2,
                        Some(TransformData::Affine(matrix.to_vec())),
                        0,
                        0,
                        fill,
                    )
                    .unwrap_or_else(|error| panic!("{mode} fractional transform: {error}"));
                let expected = transformed
                    .clone()
                    .use_backend(Backend::Cpu)
                    .tobytes()
                    .unwrap_or_else(|error| panic!("{mode} fractional CPU transform: {error}"));
                let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
                    Ok(actual) => actual,
                    Err(error)
                        if error.to_string().contains("GPU adapter not available")
                            || error
                                .to_string()
                                .contains("GPU device initialization failed") =>
                    {
                        Backend::set_pipeline_telemetry_enabled(previous);
                        return;
                    }
                    Err(error) => panic!("native GPU {mode} fractional transform failed: {error}"),
                };
                assert_eq!(
                    actual, expected,
                    "native {mode} fractional matrix={matrix:?}"
                );
                let telemetry = Backend::take_pipeline_telemetry().unwrap_or_else(|| {
                    panic!("native GPU {mode} fractional transform missing telemetry")
                });
                assert_eq!(telemetry.0, Some(Backend::Gpu), "{mode} requested backend");
                assert_eq!(telemetry.1, Backend::Gpu, "{mode} actual backend");
                assert_eq!(telemetry.6, Some(1), "{mode} dispatch count");
                assert_eq!(telemetry.7, None, "{mode} fallback reason");
            }
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn byte_projective_nonconstant_denominator_nearest_native_gpu_preserves_pixels() {
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let cases = [
            (
                (9u32, 8u32),
                (8u32, 7u32),
                [1.0, 0.0, 8.0, 0.0, 1.0, 4.0, -0.125, 0.015625],
                251u8,
            ),
            (
                (16u32, 16u32),
                (8u32, 7u32),
                [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0 / 1024.0, 1.0 / 1024.0],
                17u8,
            ),
            (
                (16u32, 16u32),
                (8u32, 7u32),
                [1.0, 0.0, -2.0, 0.0, 1.0, -1.0, 1.0 / 32.0, -1.0 / 32.0],
                83u8,
            ),
        ];
        for (mode, channels) in [
            ("L", 1usize),
            ("LA", 2usize),
            ("RGB", 3usize),
            ("RGBA", 4usize),
        ] {
            for (case_index, (source_size, output_size, matrix, fill)) in cases.iter().enumerate() {
                let source_bytes = (0..source_size.0 as usize * source_size.1 as usize * channels)
                    .map(|index| (index * (37 + case_index * 11) + 13 + channels) as u8)
                    .collect::<Vec<_>>();
                let proof_image = match mode {
                    "L" => DynamicImage::ImageLuma8(
                        GrayImage::from_raw(source_size.0, source_size.1, source_bytes.clone())
                            .expect("L proof image"),
                    ),
                    "LA" => DynamicImage::ImageLumaA8(
                        GrayAlphaImage::from_raw(
                            source_size.0,
                            source_size.1,
                            source_bytes.clone(),
                        )
                        .expect("LA proof image"),
                    ),
                    "RGB" => DynamicImage::ImageRgb8(
                        RgbImage::from_raw(source_size.0, source_size.1, source_bytes.clone())
                            .expect("RGB proof image"),
                    ),
                    "RGBA" => DynamicImage::ImageRgba8(
                        RgbaImage::from_raw(source_size.0, source_size.1, source_bytes.clone())
                            .expect("RGBA proof image"),
                    ),
                    _ => unreachable!(),
                };
                let proof_op = PipelineOp::Transform {
                    w: output_size.0,
                    h: output_size.1,
                    method: TransformMethod::Perspective,
                    data: Arc::from(matrix.to_vec()),
                    filter: ResampleFilter::Nearest,
                    fill: Some((*fill, 93, 41, 173)),
                    fill_is_none: false,
                    palette_fill: None,
                };
                assert!(
                    gpu_projective_nearest_is_exact(
                        &proof_op,
                        &proof_image,
                        Some(mode),
                        *source_size,
                    ),
                    "proof rejected case={case_index} mode={mode} matrix={matrix:?}"
                );
                let source = Image::frombytes(mode, *source_size, &source_bytes)
                    .unwrap_or_else(|error| panic!("{mode} source: {error}"));
                let fill = match mode {
                    "L" => Some(TransformFill::Scalar(i64::from(*fill))),
                    "LA" => Some(TransformFill::Components(vec![i64::from(*fill), 93])),
                    "RGB" => Some(TransformFill::Components(vec![i64::from(*fill), 93, 41])),
                    "RGBA" => Some(TransformFill::Components(vec![
                        i64::from(*fill),
                        93,
                        41,
                        173,
                    ])),
                    _ => unreachable!(),
                };
                let transformed = source
                    .transform_public(
                        *output_size,
                        2,
                        Some(TransformData::Affine(matrix.to_vec())),
                        0,
                        0,
                        fill,
                    )
                    .unwrap_or_else(|error| panic!("{mode} nonconstant transform: {error}"));
                let expected = transformed
                    .clone()
                    .use_backend(Backend::Cpu)
                    .tobytes()
                    .unwrap_or_else(|error| panic!("{mode} CPU nonconstant transform: {error}"));
                let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
                    Ok(actual) => actual,
                    Err(error)
                        if error.to_string().contains("GPU adapter not available")
                            || error
                                .to_string()
                                .contains("GPU device initialization failed") =>
                    {
                        Backend::set_pipeline_telemetry_enabled(previous);
                        return;
                    }
                    Err(error) => panic!("native GPU {mode} nonconstant transform failed: {error}"),
                };
                assert_eq!(actual, expected, "native {mode} matrix={matrix:?}");
                let telemetry = Backend::take_pipeline_telemetry().unwrap_or_else(|| {
                    panic!("native GPU {mode} nonconstant transform missing telemetry")
                });
                assert_eq!(telemetry.0, Some(Backend::Gpu), "{mode} requested backend");
                assert_eq!(
                    telemetry.1,
                    Backend::Gpu,
                    "{mode} actual backend case={case_index} matrix={matrix:?} telemetry={telemetry:?}"
                );
                assert_eq!(telemetry.6, Some(1), "{mode} dispatch count");
                assert_eq!(telemetry.7, None, "{mode} fallback reason");
            }
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn byte_projective_quad_mesh_nearest_proof_native_gpu_preserves_pixels() {
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let cases = [
            (
                3,
                TransformData::Affine(vec![0.0, 0.0, 9.0, 0.0, 9.0, 6.0, 0.0, 6.0]),
                (9, 6),
            ),
            (
                3,
                TransformData::Affine(vec![1.25, 1.0, 1.25, 7.0, 10.25, 7.0, 10.0, 1.0]),
                (9, 6),
            ),
            (
                3,
                TransformData::Affine(vec![1.0, 2.0, 1.0, 8.0, 10.0, 8.0, 10.0, 2.0]),
                (9, 6),
            ),
            (
                3,
                TransformData::Affine(vec![2.0, 1.0, 2.0, 7.0, 8.0, 7.0, 8.0, 1.0]),
                (9, 6),
            ),
            (
                3,
                TransformData::Affine(vec![2.0, 1.0, 8.0, 1.0, 8.0, 10.0, 2.0, 10.0]),
                (9, 6),
            ),
            (
                4,
                TransformData::Mesh(vec![(
                    vec![0.0, 0.0, 9.0, 6.0],
                    vec![0.0, 0.0, 0.0, 6.0, 9.0, 6.0, 9.0, 0.0],
                )]),
                (9, 6),
            ),
            (
                4,
                TransformData::Mesh(vec![(
                    vec![0.0, 0.0, 9.0, 6.0],
                    vec![1.25, 1.0, 1.25, 7.0, 10.25, 7.0, 10.0, 1.0],
                )]),
                (9, 6),
            ),
            (
                4,
                TransformData::Mesh(vec![(
                    vec![0.0, 0.0, 9.0, 6.0],
                    vec![1.0, 2.0, 1.0, 8.0, 10.0, 8.0, 10.0, 2.0],
                )]),
                (9, 6),
            ),
        ];
        for (mode, channels) in [
            ("L", 1usize),
            ("LA", 2usize),
            ("RGB", 3usize),
            ("RGBA", 4usize),
        ] {
            let source_size = (16u32, 16u32);
            let source_bytes = (0..source_size.0 as usize * source_size.1 as usize * channels)
                .map(|index| (index * 29 + 7 + channels) as u8)
                .collect::<Vec<_>>();
            let source = Image::frombytes(mode, source_size, &source_bytes)
                .unwrap_or_else(|error| panic!("{mode} source: {error}"));
            let fill = match mode {
                "LA" => Some(TransformFill::Components(vec![199, 71])),
                "RGBA" => Some(TransformFill::Components(vec![199, 71, 17, 83])),
                "RGB" => Some(TransformFill::Components(vec![199, 71, 17])),
                _ => Some(TransformFill::Scalar(199)),
            };
            for (method, data, output_size) in &cases {
                let transformed = source
                    .transform_public(
                        *output_size,
                        *method,
                        Some(data.clone()),
                        0,
                        0,
                        fill.clone(),
                    )
                    .unwrap_or_else(|error| panic!("{mode} projective transform: {error}"));
                let expected = transformed
                    .clone()
                    .use_backend(Backend::Cpu)
                    .tobytes()
                    .unwrap_or_else(|error| panic!("{mode} CPU projective transform: {error}"));
                let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
                    Ok(actual) => actual,
                    Err(error)
                        if error.to_string().contains("GPU adapter not available")
                            || error
                                .to_string()
                                .contains("GPU device initialization failed") =>
                    {
                        Backend::set_pipeline_telemetry_enabled(previous);
                        return;
                    }
                    Err(error) => panic!("native GPU {mode} projective transform failed: {error}"),
                };
                assert_eq!(
                    actual, expected,
                    "native {mode} method={method} data={data:?}"
                );
                let telemetry = Backend::take_pipeline_telemetry().unwrap_or_else(|| {
                    panic!("native GPU {mode} projective transform missing telemetry")
                });
                assert_eq!(telemetry.0, Some(Backend::Gpu));
                assert_eq!(telemetry.1, Backend::Gpu);
                assert_eq!(telemetry.6, Some(1));
                assert_eq!(telemetry.7, None);
            }
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn byte_projective_fractional_constant_quad_mesh_native_gpu_preserves_pixels() {
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let cases = [
            (
                3,
                TransformData::Affine(vec![3.25, 4.0, 3.25, 4.0, 3.25, 4.0, 3.25, 4.0]),
            ),
            (
                4,
                TransformData::Mesh(vec![(
                    vec![0.0, 0.0, 9.0, 6.0],
                    vec![3.25, 4.0, 3.25, 4.0, 3.25, 4.0, 3.25, 4.0],
                )]),
            ),
        ];
        for (mode, channels) in [
            ("L", 1usize),
            ("LA", 2usize),
            ("RGB", 3usize),
            ("RGBA", 4usize),
            ("P", 1usize),
        ] {
            let source_size = (9u32, 6u32);
            let source_bytes = (0..source_size.0 as usize * source_size.1 as usize * channels)
                .map(|index| (index * 73 + 17 + channels) as u8)
                .collect::<Vec<_>>();
            let mut source = Image::frombytes(mode, source_size, &source_bytes)
                .unwrap_or_else(|error| panic!("{mode} source: {error}"));
            if mode == "P" {
                let palette = (0..768)
                    .map(|index| (index * 19 + 7) as u8)
                    .collect::<Vec<_>>();
                source
                    .putpalette(&palette, "RGB")
                    .unwrap_or_else(|error| panic!("P palette: {error}"));
            }
            let fill = match mode {
                "L" | "P" => Some(TransformFill::Scalar(251)),
                "LA" => Some(TransformFill::Components(vec![251, 93])),
                "RGB" => Some(TransformFill::Components(vec![251, 93, 41])),
                "RGBA" => Some(TransformFill::Components(vec![251, 93, 41, 173])),
                _ => unreachable!(),
            };
            for (method, data) in &cases {
                let transformed = source
                    .transform_public((9, 6), *method, Some(data.clone()), 0, 0, fill.clone())
                    .unwrap_or_else(|error| panic!("{mode} constant transform: {error}"));
                let expected = transformed
                    .clone()
                    .use_backend(Backend::Cpu)
                    .tobytes()
                    .unwrap_or_else(|error| panic!("{mode} constant CPU transform: {error}"));
                let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
                    Ok(actual) => actual,
                    Err(error)
                        if error.to_string().contains("GPU adapter not available")
                            || error
                                .to_string()
                                .contains("GPU device initialization failed") =>
                    {
                        Backend::set_pipeline_telemetry_enabled(previous);
                        return;
                    }
                    Err(error) => panic!("native GPU {mode} constant transform failed: {error}"),
                };
                assert_eq!(actual, expected, "native {mode} constant data={data:?}");
                let telemetry = Backend::take_pipeline_telemetry().unwrap_or_else(|| {
                    panic!("native GPU {mode} constant transform missing telemetry")
                });
                assert_eq!(telemetry.0, Some(Backend::Gpu), "{mode} requested backend");
                assert_eq!(telemetry.1, Backend::Gpu, "{mode} actual backend");
                assert_eq!(telemetry.6, Some(1), "{mode} dispatch count");
                assert_eq!(telemetry.7, None, "{mode} fallback reason");
            }
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn indexed_projective_fractional_nonconstant_nearest_native_gpu_preserves_pixels() {
        let source_size = (9u32, 8u32);
        let source_bytes = (0..source_size.0 as usize * source_size.1 as usize)
            .map(|index| (index * 73 + 19) as u8)
            .collect::<Vec<_>>();
        let mut source = Image::frombytes("P", source_size, &source_bytes).expect("P source");
        let palette = (0..768)
            .map(|index| (index * 23 + 11) as u8)
            .collect::<Vec<_>>();
        source.putpalette(&palette, "RGB").expect("P palette");
        let matrix = [1.0, 0.0, 8.0, 0.0, 1.0, 4.0, -0.125, 0.015625];
        let transformed = source
            .transform_public(
                (8, 7),
                2,
                Some(TransformData::Affine(matrix.to_vec())),
                0,
                0,
                Some(TransformFill::Scalar(251)),
            )
            .expect("P nonconstant projective transform");
        let expected = transformed
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("P nonconstant CPU transform");
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU P nonconstant transform failed: {error}"),
        };
        assert_eq!(actual, expected, "native P nonconstant fractional matrix");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native P nonconstant transform must publish telemetry");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(1));
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn byte_projective_filtered_relocations_native_gpu_preserve_pixels() {
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for (mode, channels) in [
            ("L", 1usize),
            ("LA", 2usize),
            ("RGB", 3usize),
            ("RGBA", 4usize),
        ] {
            let bytes = (0..16 * 16 * channels)
                .map(|index| (index * 29 + 7) as u8)
                .collect::<Vec<_>>();
            let source = Image::frombytes(mode, (16, 16), &bytes).expect("byte source");
            let fill = match mode {
                "LA" => Some(TransformFill::Components(vec![199, 71])),
                "RGBA" => Some(TransformFill::Components(vec![199, 71, 17, 83])),
                _ => None,
            };
            let matrices = [
                [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
                [1.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0],
                [1.0, 0.0, -1.0, 0.0, 1.0, -1.0, 0.0, 0.0],
                [0.0, 1.0, 1.0, 1.0, 0.0, -1.0, 0.0, 0.0],
                [0.0, 0.0, 1.5, 0.0, 0.0, 4.5, 0.0, 0.0],
                [0.0, 0.0, 2.5, 0.0, 0.0, 3.5, 0.0, 0.0],
            ];
            for matrix in matrices {
                for filter in [ResampleFilter::Bilinear, ResampleFilter::Bicubic] {
                    let transformed = source
                        .transform_public(
                            (8, 8),
                            2,
                            Some(TransformData::Affine(matrix.to_vec())),
                            match filter {
                                ResampleFilter::Bilinear => 2,
                                ResampleFilter::Bicubic => 3,
                                _ => unreachable!(),
                            },
                            0,
                            fill.clone(),
                        )
                        .expect("filtered projective relocation");
                    let expected = transformed
                        .clone()
                        .use_backend(Backend::Cpu)
                        .tobytes()
                        .expect("CPU filtered projective relocation");
                    let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
                        Ok(actual) => actual,
                        Err(error)
                            if error.to_string().contains("GPU adapter not available")
                                || error
                                    .to_string()
                                    .contains("GPU device initialization failed") =>
                        {
                            Backend::set_pipeline_telemetry_enabled(previous);
                            return;
                        }
                        Err(error) => {
                            panic!("native GPU filtered projective relocation failed: {error}")
                        }
                    };
                    assert_eq!(actual, expected, "native {mode} filtered relocation");
                    let telemetry = Backend::take_pipeline_telemetry()
                        .expect("filtered relocation must publish a receipt");
                    assert_eq!(
                        telemetry.0,
                        Some(Backend::Gpu),
                        "telemetry={telemetry:?} mode={mode} matrix={matrix:?} filter={filter:?}"
                    );
                    assert_eq!(telemetry.1, Backend::Gpu);
                    assert_eq!(telemetry.6, Some(1));
                    assert_eq!(telemetry.7, None);
                }
            }
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn byte_quad_mesh_filtered_relocations_native_gpu_preserve_pixels() {
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for (mode, channels) in [
            ("L", 1usize),
            ("LA", 2usize),
            ("RGB", 3usize),
            ("RGBA", 4usize),
        ] {
            let source_size = (16u32, 16u32);
            let bytes = (0..source_size.0 as usize * source_size.1 as usize * channels)
                .map(|index| (index * 29 + 7) as u8)
                .collect::<Vec<_>>();
            let source = Image::frombytes(mode, source_size, &bytes).expect("byte source");
            let fill = match mode {
                "LA" => Some(TransformFill::Components(vec![199, 71])),
                "RGBA" => Some(TransformFill::Components(vec![199, 71, 17, 83])),
                _ => None,
            };
            let cases = [
                (
                    3,
                    TransformData::Affine(vec![0.0, 0.0, 0.0, 6.0, 9.0, 6.0, 9.0, 0.0]),
                    (9, 6),
                ),
                (
                    3,
                    TransformData::Affine(vec![0.0, 0.0, 6.0, 0.0, 6.0, 9.0, 0.0, 9.0]),
                    (9, 6),
                ),
                (
                    3,
                    TransformData::Affine(vec![1.0, 2.0, 1.0, 8.0, 10.0, 8.0, 10.0, 2.0]),
                    (9, 6),
                ),
                (
                    3,
                    TransformData::Affine(vec![1.0, 2.0, 7.0, 2.0, 7.0, 11.0, 1.0, 11.0]),
                    (9, 6),
                ),
                (
                    3,
                    TransformData::Affine(vec![1.5, 4.5, 1.5, 4.5, 1.5, 4.5, 1.5, 4.5]),
                    (9, 6),
                ),
                (
                    3,
                    TransformData::Affine(vec![2.5, 3.5, 2.5, 3.5, 2.5, 3.5, 2.5, 3.5]),
                    (9, 6),
                ),
                (
                    4,
                    TransformData::Mesh(vec![(
                        vec![0.0, 0.0, 9.0, 6.0],
                        vec![1.0, 2.0, 1.0, 8.0, 10.0, 8.0, 10.0, 2.0],
                    )]),
                    (9, 6),
                ),
                (
                    4,
                    TransformData::Mesh(vec![(
                        vec![0.0, 0.0, 9.0, 6.0],
                        vec![1.0, 2.0, 7.0, 2.0, 7.0, 11.0, 1.0, 11.0],
                    )]),
                    (9, 6),
                ),
                (
                    4,
                    TransformData::Mesh(vec![(
                        vec![0.0, 0.0, 9.0, 6.0],
                        vec![1.5, 4.5, 1.5, 4.5, 1.5, 4.5, 1.5, 4.5],
                    )]),
                    (9, 6),
                ),
            ];
            for (method, data, output_size) in cases {
                for filter in [ResampleFilter::Bilinear, ResampleFilter::Bicubic] {
                    let transformed = source
                        .transform_public(
                            output_size,
                            method,
                            Some(data.clone()),
                            match filter {
                                ResampleFilter::Bilinear => 2,
                                ResampleFilter::Bicubic => 3,
                                _ => unreachable!(),
                            },
                            0,
                            fill.clone(),
                        )
                        .expect("filtered Quad/Mesh relocation");
                    let expected = transformed
                        .clone()
                        .use_backend(Backend::Cpu)
                        .tobytes()
                        .expect("CPU filtered Quad/Mesh relocation");
                    let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
                        Ok(actual) => actual,
                        Err(error)
                            if error.to_string().contains("GPU adapter not available")
                                || error
                                    .to_string()
                                    .contains("GPU device initialization failed") =>
                        {
                            Backend::set_pipeline_telemetry_enabled(previous);
                            return;
                        }
                        Err(error) => {
                            panic!("native GPU filtered Quad/Mesh relocation failed: {error}")
                        }
                    };
                    assert_eq!(
                        actual, expected,
                        "native {mode} filtered method {method} data={data:?} filter={filter:?}"
                    );
                    let telemetry = Backend::take_pipeline_telemetry()
                        .expect("filtered Quad/Mesh relocation must publish a receipt");
                    assert_eq!(telemetry.0, Some(Backend::Gpu));
                    assert_eq!(
                        telemetry.1,
                        Backend::Gpu,
                        "telemetry={telemetry:?} mode={mode} method={method} data={data:?} filter={filter:?}"
                    );
                    assert_eq!(telemetry.6, Some(1));
                    assert_eq!(telemetry.7, None);
                }
            }
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn byte_projective_partial_mesh_relocations_native_gpu_preserve_pixels() {
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let cases = [
            (
                [1.0, 2.0, 6.0, 6.0],
                [2.0, 3.0, 2.0, 7.0, 7.0, 7.0, 7.0, 3.0],
            ),
            (
                [1.0, 2.0, 6.0, 6.0],
                [2.0, 3.0, 6.0, 3.0, 6.0, 8.0, 2.0, 8.0],
            ),
            (
                [0.0, 3.0, 9.0, 7.0],
                [1.0, 2.0, 1.0, 6.0, 10.0, 6.0, 10.0, 2.0],
            ),
            (
                [0.0, 3.0, 9.0, 7.0],
                [1.0, 2.0, 5.0, 2.0, 5.0, 11.0, 1.0, 11.0],
            ),
        ];
        for (mode, channels) in [
            ("L", 1usize),
            ("LA", 2usize),
            ("RGB", 3usize),
            ("RGBA", 4usize),
        ] {
            let source_size = (16u32, 16u32);
            let bytes = (0..source_size.0 as usize * source_size.1 as usize * channels)
                .map(|index| (index * 37 + channels * 11 + 5) as u8)
                .collect::<Vec<_>>();
            let source = Image::frombytes(mode, source_size, &bytes).expect("byte source");
            let fill = match mode {
                "L" => Some(TransformFill::Scalar(199)),
                "LA" => Some(TransformFill::Components(vec![199, 71])),
                "RGB" => Some(TransformFill::Components(vec![199, 71, 17])),
                "RGBA" => Some(TransformFill::Components(vec![199, 71, 17, 83])),
                _ => unreachable!(),
            };
            for (bbox, quad) in cases {
                let data = TransformData::Mesh(vec![(bbox.to_vec(), quad.to_vec())]);
                for filter in [
                    (0, ResampleFilter::Nearest),
                    (2, ResampleFilter::Bilinear),
                    (3, ResampleFilter::Bicubic),
                ] {
                    let transformed = source
                        .transform_public((9, 8), 4, Some(data.clone()), filter.0, 0, fill.clone())
                        .unwrap_or_else(|error| {
                            panic!(
                                "{mode} partial Mesh transform bbox={bbox:?} quad={quad:?}: {error}"
                            )
                        });
                    let expected = transformed
                        .clone()
                        .use_backend(Backend::Cpu)
                        .tobytes()
                        .expect("CPU partial Mesh transform");
                    let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
                        Ok(actual) => actual,
                        Err(error)
                            if error.to_string().contains("GPU adapter not available")
                                || error
                                    .to_string()
                                    .contains("GPU device initialization failed") =>
                        {
                            Backend::set_pipeline_telemetry_enabled(previous);
                            return;
                        }
                        Err(error) => {
                            panic!("native GPU {mode} partial Mesh transform failed: {error}")
                        }
                    };
                    assert_eq!(
                        actual, expected,
                        "native {mode} partial Mesh filter={filter:?}"
                    );
                    let telemetry = Backend::take_pipeline_telemetry()
                        .expect("partial Mesh transform must publish telemetry");
                    assert_eq!(
                        telemetry.0,
                        Some(Backend::Gpu),
                        "telemetry={telemetry:?} mode={mode} bbox={bbox:?} quad={quad:?} filter={filter:?}"
                    );
                    assert_eq!(
                        telemetry.1,
                        Backend::Gpu,
                        "telemetry={telemetry:?} mode={mode} bbox={bbox:?} quad={quad:?} filter={filter:?}"
                    );
                    assert_eq!(
                        telemetry.6,
                        Some(1),
                        "telemetry={telemetry:?} mode={mode} bbox={bbox:?} quad={quad:?} filter={filter:?}"
                    );
                    assert_eq!(
                        telemetry.7, None,
                        "telemetry={telemetry:?} mode={mode} bbox={bbox:?} quad={quad:?} filter={filter:?}"
                    );
                }
            }
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn palette_projective_partial_mesh_relocations_native_gpu_preserve_pairs() {
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let cases = [
            (
                [1.0, 2.0, 6.0, 6.0],
                [2.0, 3.0, 2.0, 7.0, 7.0, 7.0, 7.0, 3.0],
            ),
            (
                [1.0, 2.0, 6.0, 6.0],
                [2.0, 3.0, 6.0, 3.0, 6.0, 8.0, 2.0, 8.0],
            ),
        ];
        let palette = (0..768)
            .map(|index| (index * 19 + 7) as u8)
            .collect::<Vec<_>>();
        for (mode, channels, fill) in [
            ("P", 1usize, TransformFill::Scalar(251)),
            ("PA", 2usize, TransformFill::Components(vec![199, 71])),
        ] {
            let source_size = (16u32, 16u32);
            let bytes = (0..source_size.0 as usize * source_size.1 as usize * channels)
                .map(|index| (index * 37 + channels * 11 + 5) as u8)
                .collect::<Vec<_>>();
            let storage_mode = if mode == "PA" { "LA" } else { mode };
            let mut source = Image::frombytes(storage_mode, source_size, &bytes)
                .unwrap_or_else(|error| panic!("{mode} source: {error}"));
            source
                .putpalette(&palette, "RGB")
                .unwrap_or_else(|error| panic!("{mode} palette: {error}"));
            assert_eq!(source.mode().expect("palette mode"), mode);
            for (bbox, quad) in cases {
                let data = TransformData::Mesh(vec![(bbox.to_vec(), quad.to_vec())]);
                for filter in [
                    (0, ResampleFilter::Nearest),
                    (2, ResampleFilter::Bilinear),
                    (3, ResampleFilter::Bicubic),
                ] {
                    let transformed = source
                        .transform_public(
                            (9, 8),
                            4,
                            Some(data.clone()),
                            filter.0,
                            0,
                            Some(fill.clone()),
                        )
                        .unwrap_or_else(|error| {
                            panic!(
                                "{mode} partial Mesh transform bbox={bbox:?} quad={quad:?}: {error}"
                            )
                        });
                    let expected = transformed
                        .clone()
                        .use_backend(Backend::Cpu)
                        .tobytes()
                        .expect("CPU partial palette Mesh transform");
                    let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
                        Ok(actual) => actual,
                        Err(error)
                            if error.to_string().contains("GPU adapter not available")
                                || error
                                    .to_string()
                                    .contains("GPU device initialization failed") =>
                        {
                            Backend::set_pipeline_telemetry_enabled(previous);
                            return;
                        }
                        Err(error) => {
                            panic!("native GPU {mode} partial Mesh transform failed: {error}")
                        }
                    };
                    assert_eq!(
                        actual, expected,
                        "native {mode} partial Mesh filter={filter:?}"
                    );
                    let telemetry = Backend::take_pipeline_telemetry()
                        .expect("partial palette Mesh transform must publish telemetry");
                    assert_eq!(telemetry.0, Some(Backend::Gpu));
                    assert_eq!(telemetry.1, Backend::Gpu);
                    assert_eq!(telemetry.6, Some(1));
                    assert_eq!(telemetry.7, None);
                }
            }
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn partial_mesh_without_fill_stays_on_exact_host_control() {
        let image = DynamicImage::ImageRgb8(
            RgbImage::from_raw(16, 16, vec![0; 16 * 16 * 3]).expect("RGB image"),
        );
        let op = PipelineOp::Transform {
            w: 9,
            h: 8,
            method: TransformMethod::Mesh,
            data: Arc::from([1.0, 2.0, 6.0, 6.0, 2.0, 3.0, 2.0, 7.0, 7.0, 7.0, 7.0, 3.0].to_vec()),
            filter: ResampleFilter::Bilinear,
            fill: Some((0, 0, 0, 255)),
            fill_is_none: true,
            palette_fill: None,
        };
        assert!(!gpu_projective_nearest_is_exact(
            &op,
            &image,
            Some("RGB"),
            (16, 16),
        ));
    }

    #[test]
    fn projective_filtered_integer_constant_proof_is_narrow() {
        let source = (16, 16);
        let output = (9, 7);
        let perspective = [0.0, 0.0, 3.0, 0.0, 0.0, 5.0, 0.0, 0.0];
        let quad = [3.0, 5.0, 3.0, 5.0, 3.0, 5.0, 3.0, 5.0];
        let mesh = [0.0, 0.0, 9.0, 7.0, 3.0, 5.0, 3.0, 5.0, 3.0, 5.0, 3.0, 5.0];
        for mode in [
            Some("L"),
            Some("RGB"),
            Some("PA"),
            Some("CMYK"),
            Some("HSV"),
            Some("YCbCr"),
            Some("RGBX"),
            Some("RGBa"),
        ] {
            assert!(gpu_projective_filtered_integer_constant_is_admitted(
                TransformMethod::Perspective,
                &perspective,
                ResampleFilter::Bilinear,
                mode,
                source,
                output,
            ));
            assert!(gpu_projective_filtered_integer_constant_is_admitted(
                TransformMethod::Quad,
                &quad,
                ResampleFilter::Bilinear,
                mode,
                source,
                output,
            ));
            assert!(gpu_projective_filtered_integer_constant_is_admitted(
                TransformMethod::Mesh,
                &mesh,
                ResampleFilter::Bilinear,
                mode,
                source,
                output,
            ));
            assert!(gpu_projective_filtered_integer_constant_is_admitted(
                TransformMethod::Perspective,
                &perspective,
                ResampleFilter::Bicubic,
                mode,
                source,
                output,
            ));
            assert!(gpu_projective_filtered_integer_constant_is_admitted(
                TransformMethod::Quad,
                &quad,
                ResampleFilter::Bicubic,
                mode,
                source,
                output,
            ));
            assert!(gpu_projective_filtered_integer_constant_is_admitted(
                TransformMethod::Mesh,
                &mesh,
                ResampleFilter::Bicubic,
                mode,
                source,
                output,
            ));
        }
        for mode in [Some("LA"), Some("RGBA")] {
            assert!(!gpu_projective_filtered_integer_constant_is_admitted(
                TransformMethod::Perspective,
                &perspective,
                ResampleFilter::Bilinear,
                mode,
                source,
                output,
            ));
            assert!(!gpu_projective_filtered_integer_constant_is_admitted(
                TransformMethod::Perspective,
                &perspective,
                ResampleFilter::Bicubic,
                mode,
                source,
                output,
            ));
        }
        for (x, y) in [
            (0.0, 5.0),
            (16.0, 5.0),
            (3.0, 0.0),
            (3.0, 16.0),
            (3.25, 5.0),
        ] {
            let map = [0.0, 0.0, x, 0.0, 0.0, y, 0.0, 0.0];
            assert!(!gpu_projective_filtered_integer_constant_is_admitted(
                TransformMethod::Perspective,
                &map,
                ResampleFilter::Bilinear,
                Some("RGB"),
                source,
                output,
            ));
            assert!(!gpu_projective_filtered_integer_constant_is_admitted(
                TransformMethod::Perspective,
                &map,
                ResampleFilter::Bicubic,
                Some("RGB"),
                source,
                output,
            ));
        }
        for (x, y) in [(1.0, 5.0), (15.0, 5.0), (3.0, 1.0), (3.0, 15.0)] {
            let map = [0.0, 0.0, x, 0.0, 0.0, y, 0.0, 0.0];
            assert!(!gpu_projective_filtered_integer_constant_is_admitted(
                TransformMethod::Perspective,
                &map,
                ResampleFilter::Bicubic,
                Some("CMYK"),
                source,
                output,
            ));
        }
        let partial_mesh = [1.0, 1.0, 8.0, 6.0, 3.0, 5.0, 3.0, 5.0, 3.0, 5.0, 3.0, 5.0];
        assert!(!gpu_projective_filtered_integer_constant_is_admitted(
            TransformMethod::Mesh,
            &partial_mesh,
            ResampleFilter::Bilinear,
            Some("RGB"),
            source,
            output,
        ));
    }

    #[test]
    fn projective_filtered_integer_constants_native_gpu_preserve_pixels() {
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let source_size = (16, 16);
        let output_size = (9, 7);
        let maps = [
            (
                2,
                TransformData::Affine(vec![0.0, 0.0, 3.0, 0.0, 0.0, 5.0, 0.0, 0.0]),
            ),
            (
                3,
                TransformData::Affine(vec![3.0, 5.0, 3.0, 5.0, 3.0, 5.0, 3.0, 5.0]),
            ),
            (
                4,
                TransformData::Mesh(vec![(
                    vec![0.0, 0.0, 9.0, 7.0],
                    vec![3.0, 5.0, 3.0, 5.0, 3.0, 5.0, 3.0, 5.0],
                )]),
            ),
        ];
        for (mode, channels) in [
            ("L", 1usize),
            ("RGB", 3usize),
            ("PA", 2usize),
            ("CMYK", 4usize),
            ("HSV", 3usize),
            ("YCbCr", 3usize),
            ("RGBX", 4usize),
            ("RGBa", 4usize),
        ] {
            let bytes = (0..source_size.0 as usize * source_size.1 as usize * channels)
                .map(|index| (index * 53 + 17) as u8)
                .collect::<Vec<_>>();
            let mut source =
                Image::frombytes(if mode == "PA" { "LA" } else { mode }, source_size, &bytes)
                    .expect("integer-constant projective source");
            if mode == "PA" {
                let palette = (0..768)
                    .map(|index| (index * 19 + 7) as u8)
                    .collect::<Vec<_>>();
                source
                    .putpalette(&palette, "RGB")
                    .expect("integer-constant PA palette");
            }
            let fill = match mode {
                "L" => TransformFill::Scalar(199),
                "RGB" => TransformFill::Components(vec![199, 71, 17]),
                "PA" => TransformFill::Components(vec![199, 71]),
                "CMYK" | "RGBX" | "RGBa" => TransformFill::Components(vec![199, 71, 17, 233]),
                "HSV" | "YCbCr" => TransformFill::Components(vec![199, 71, 17]),
                _ => unreachable!(),
            };
            for filter in [ResampleFilter::Bilinear, ResampleFilter::Bicubic] {
                for (method, data) in &maps {
                    let transformed = source
                        .transform_public(
                            output_size,
                            *method,
                            Some(data.clone()),
                            match filter {
                                ResampleFilter::Bilinear => 2,
                                ResampleFilter::Bicubic => 3,
                                _ => unreachable!(),
                            },
                            0,
                            Some(fill.clone()),
                        )
                        .expect("integer-constant projective transform");
                    let expected = transformed
                        .clone()
                        .use_backend(Backend::Cpu)
                        .tobytes()
                        .expect("CPU integer-constant projective transform");
                    let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
                        Ok(actual) => actual,
                        Err(error)
                            if error.to_string().contains("GPU adapter not available")
                                || error
                                    .to_string()
                                    .contains("GPU device initialization failed") =>
                        {
                            Backend::set_pipeline_telemetry_enabled(previous);
                            return;
                        }
                        Err(error) => {
                            panic!(
                                "native GPU integer-constant projective transform failed: {error}"
                            )
                        }
                    };
                    assert_eq!(
                        actual, expected,
                        "native {mode} method={method} filter={filter:?} integer constant"
                    );
                    let telemetry = Backend::take_pipeline_telemetry()
                        .expect("integer-constant projective transform must publish telemetry");
                    assert_eq!(telemetry.0, Some(Backend::Gpu));
                    assert_eq!(telemetry.1, Backend::Gpu);
                    assert_eq!(telemetry.6, Some(1));
                    assert_eq!(telemetry.7, None);
                }
            }
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn palette_alpha_projective_relocation_proof_is_narrow() {
        assert!(gpu_transform_should_premultiply(
            1,
            None,
            ResampleFilter::Bilinear,
            TransformMethod::Quad,
        ));
        let image = DynamicImage::ImageLuma8(
            GrayImage::from_raw(16, 16, vec![0; 16 * 16]).expect("luma image"),
        );
        let direct = [1.0, 0.0, -2.0, 0.0, 1.0, 1.0, 0.0, 0.0];
        let swapped = [0.0, 1.0, 3.0, 1.0, 0.0, -4.0, 0.0, 0.0];
        assert!(gpu_palette_alpha_projective_relocation_is_admitted(
            TransformMethod::Perspective,
            &direct,
            ResampleFilter::Nearest,
            (9, 6),
        ));
        assert!(gpu_palette_alpha_projective_relocation_is_admitted(
            TransformMethod::Perspective,
            &swapped,
            ResampleFilter::Nearest,
            (5, 7),
        ));
        let reflected = [-1.0, 0.0, 9.0, 0.0, 1.0, 6.0, 0.0, 0.0];
        assert!(gpu_palette_alpha_projective_relocation_is_admitted(
            TransformMethod::Perspective,
            &reflected,
            ResampleFilter::Nearest,
            (9, 6),
        ));
        assert!(!gpu_palette_alpha_projective_relocation_is_admitted(
            TransformMethod::Perspective,
            &direct,
            ResampleFilter::Bilinear,
            (9, 6),
        ));
        assert!(!gpu_palette_alpha_projective_relocation_is_admitted(
            TransformMethod::Quad,
            &direct,
            ResampleFilter::Nearest,
            (9, 6),
        ));
        let quad_axis_swap = [0.0, 0.0, 6.0, 0.0, 6.0, 9.0, 0.0, 9.0];
        assert!(gpu_palette_alpha_projective_relocation_is_admitted(
            TransformMethod::Quad,
            &quad_axis_swap,
            ResampleFilter::Nearest,
            (9, 6),
        ));
        let quad_direct = [0.0, 0.0, 0.0, 6.0, 9.0, 6.0, 9.0, 0.0];
        assert!(gpu_palette_alpha_projective_relocation_is_admitted(
            TransformMethod::Quad,
            &quad_direct,
            ResampleFilter::Nearest,
            (9, 6),
        ));
        let quad_constant = [3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0];
        assert!(gpu_palette_alpha_projective_relocation_is_admitted(
            TransformMethod::Quad,
            &quad_constant,
            ResampleFilter::Nearest,
            (9, 6),
        ));
        let quad_constant_fractional = [3.25, 4.0, 3.25, 4.0, 3.25, 4.0, 3.25, 4.0];
        assert!(gpu_palette_alpha_projective_relocation_is_admitted(
            TransformMethod::Quad,
            &quad_constant_fractional,
            ResampleFilter::Nearest,
            (9, 6),
        ));
        let quad_constant_half = [1.5, 4.5, 1.5, 4.5, 1.5, 4.5, 1.5, 4.5];
        assert!(gpu_projective_filtered_relocation_is_admitted(
            TransformMethod::Quad,
            &quad_constant_half,
            ResampleFilter::Bilinear,
            Some("PA"),
            (9, 6),
        ));
        assert!(!gpu_projective_filtered_relocation_is_admitted(
            TransformMethod::Quad,
            &quad_constant_fractional,
            ResampleFilter::Bilinear,
            Some("PA"),
            (9, 6),
        ));
        let mesh_relocation = [0.0, 0.0, 9.0, 6.0, 1.0, 2.0, 1.0, 8.0, 10.0, 8.0, 10.0, 2.0];
        let mesh_filtered = PipelineOp::Transform {
            w: 9,
            h: 6,
            method: TransformMethod::Mesh,
            data: Arc::from(mesh_relocation.to_vec()),
            filter: ResampleFilter::Bilinear,
            fill: Some((7, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: None,
        };
        assert!(gpu_projective_nearest_is_exact(
            &mesh_filtered,
            &image,
            Some("L"),
            (16, 16)
        ));
        assert!(gpu_palette_alpha_projective_relocation_is_admitted(
            TransformMethod::Mesh,
            &mesh_relocation,
            ResampleFilter::Nearest,
            (9, 6),
        ));
        let mesh_constant = [0.0, 0.0, 9.0, 6.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0];
        assert!(gpu_palette_alpha_projective_relocation_is_admitted(
            TransformMethod::Mesh,
            &mesh_constant,
            ResampleFilter::Nearest,
            (9, 6),
        ));
        let mesh_constant_half = [0.0, 0.0, 9.0, 6.0, 1.5, 4.5, 1.5, 4.5, 1.5, 4.5, 1.5, 4.5];
        assert!(gpu_projective_filtered_relocation_is_admitted(
            TransformMethod::Mesh,
            &mesh_constant_half,
            ResampleFilter::Bicubic,
            Some("PA"),
            (9, 6),
        ));
        assert!(!gpu_projective_filtered_relocation_is_admitted(
            TransformMethod::Mesh,
            &mesh_constant,
            ResampleFilter::Bicubic,
            Some("PA"),
            (9, 6),
        ));
        let mesh_constant_extra = [
            0.0, 0.0, 9.0, 6.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 0.0, 0.0,
        ];
        assert!(!gpu_palette_alpha_projective_relocation_is_admitted(
            TransformMethod::Mesh,
            &mesh_constant_extra,
            ResampleFilter::Nearest,
            (9, 6),
        ));
        assert!(!gpu_palette_alpha_projective_relocation_is_admitted(
            TransformMethod::Perspective,
            &[1.0, 0.0, 0.25, 0.0, 1.0, 0.0, 0.0, 0.0],
            ResampleFilter::Nearest,
            (9, 6),
        ));
        assert!(gpu_projective_filtered_relocation_is_admitted(
            TransformMethod::Perspective,
            &direct,
            ResampleFilter::Bilinear,
            Some("PA"),
            (9, 6),
        ));
        assert!(gpu_projective_filtered_relocation_is_admitted(
            TransformMethod::Perspective,
            &swapped,
            ResampleFilter::Bicubic,
            Some("PA"),
            (5, 7),
        ));
        let perspective_constant_half = [0.0, 0.0, 1.5, 0.0, 0.0, 4.5, 0.0, 0.0];
        assert!(gpu_projective_filtered_relocation_is_admitted(
            TransformMethod::Perspective,
            &perspective_constant_half,
            ResampleFilter::Bilinear,
            Some("PA"),
            (9, 6),
        ));
        let perspective_constant_fractional = [0.0, 0.0, 1.25, 0.0, 0.0, 4.5, 0.0, 0.0];
        assert!(!gpu_projective_filtered_relocation_is_admitted(
            TransformMethod::Perspective,
            &perspective_constant_fractional,
            ResampleFilter::Bilinear,
            Some("PA"),
            (9, 6),
        ));
        assert!(gpu_projective_filtered_relocation_is_admitted(
            TransformMethod::Perspective,
            &direct,
            ResampleFilter::Bilinear,
            Some("LA"),
            (9, 6),
        ));
        assert!(gpu_projective_filtered_relocation_is_admitted(
            TransformMethod::Perspective,
            &swapped,
            ResampleFilter::Bicubic,
            Some("RGBA"),
            (5, 7),
        ));
        let fractional = [1.0, 0.0, 0.25, 0.0, 1.0, 0.0, 0.0, 0.0];
        assert!(!gpu_projective_filtered_relocation_is_admitted(
            TransformMethod::Perspective,
            &fractional,
            ResampleFilter::Bilinear,
            Some("PA"),
            (9, 6),
        ));
        assert!(!gpu_projective_filtered_relocation_is_admitted(
            TransformMethod::Perspective,
            &fractional,
            ResampleFilter::Bilinear,
            Some("LA"),
            (9, 6),
        ));
        assert!(!gpu_projective_filtered_relocation_is_admitted(
            TransformMethod::Quad,
            &direct,
            ResampleFilter::Bilinear,
            Some("PA"),
            (9, 6),
        ));
        assert!(gpu_projective_filtered_relocation_is_admitted(
            TransformMethod::Quad,
            &quad_direct,
            ResampleFilter::Bilinear,
            Some("PA"),
            (9, 6),
        ));
        assert!(gpu_projective_filtered_relocation_is_admitted(
            TransformMethod::Quad,
            &quad_axis_swap,
            ResampleFilter::Bicubic,
            Some("PA"),
            (9, 6),
        ));
        assert!(gpu_projective_filtered_relocation_is_admitted(
            TransformMethod::Mesh,
            &mesh_relocation,
            ResampleFilter::Bicubic,
            Some("PA"),
            (9, 6),
        ));
    }

    #[test]
    fn palette_alpha_projective_nearest_native_gpu_preserves_pairs() {
        let source_bytes = (0..7 * 5)
            .flat_map(|index| [(index * 17 + 3) as u8, (index * 29 + 11) as u8])
            .collect::<Vec<_>>();
        let mut source = Image::frombytes("LA", (7, 5), &source_bytes).expect("LA source");
        source
            .putpalette(&[10, 20, 30, 40, 50, 60], "RGB")
            .expect("PA palette");
        assert_eq!(source.mode().expect("PA mode"), "PA");

        let cases = [
            ((7, 5), [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0], None),
            (
                (9, 6),
                [1.0, 0.0, 1.0, 0.0, 1.0, 2.0, 0.0, 0.0],
                Some(TransformFill::Components(vec![199, 71])),
            ),
            (
                (9, 6),
                [1.0, 0.0, -2.0, 0.0, 1.0, -1.0, 0.0, 0.0],
                Some(TransformFill::Components(vec![61, 233])),
            ),
            (
                (5, 7),
                [0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
                Some(TransformFill::Components(vec![17, 203])),
            ),
            (
                (7, 5),
                [-1.0, 0.0, 7.0, 0.0, 1.0, 0.0, 0.0, 0.0],
                Some(TransformFill::Components(vec![61, 233])),
            ),
            (
                (7, 5),
                [1.0, 0.0, 0.0, 0.0, -1.0, 5.0, 0.0, 0.0],
                Some(TransformFill::Components(vec![61, 233])),
            ),
            (
                (7, 5),
                [-1.0, 0.0, 7.0, 0.0, -1.0, 5.0, 0.0, 0.0],
                Some(TransformFill::Components(vec![61, 233])),
            ),
            (
                (7, 5),
                [0.0, -1.0, 5.0, 1.0, 0.0, 0.0, 0.0, 0.0],
                Some(TransformFill::Components(vec![61, 233])),
            ),
            (
                (7, 5),
                [0.0, 1.0, 0.0, -1.0, 0.0, 7.0, 0.0, 0.0],
                Some(TransformFill::Components(vec![61, 233])),
            ),
            (
                (7, 5),
                [0.0, -1.0, 5.0, -1.0, 0.0, 7.0, 0.0, 0.0],
                Some(TransformFill::Components(vec![61, 233])),
            ),
        ];

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for (size, matrix, fill) in cases {
            let transformed = source
                .transform_public(
                    size,
                    2,
                    Some(TransformData::Affine(matrix.to_vec())),
                    0,
                    0,
                    fill,
                )
                .expect("PA projective transform");
            let expected = transformed
                .clone()
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU PA projective transform");
            let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU PA projective transform failed: {error}"),
            };
            assert_eq!(actual, expected, "PA projective pair parity for {size:?}");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("native PA projective transform must publish a receipt");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.6, Some(1));
            assert_eq!(telemetry.7, None);
        }
        let constant_cases = [
            (
                (9, 6),
                3,
                TransformData::Affine(vec![3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0]),
                Some(TransformFill::Components(vec![199, 71])),
            ),
            (
                (9, 6),
                3,
                TransformData::Affine(vec![-1.0, 5.0, -1.0, 5.0, -1.0, 5.0, -1.0, 5.0]),
                Some(TransformFill::Components(vec![61, 233])),
            ),
            (
                (9, 6),
                4,
                TransformData::Mesh(vec![(
                    vec![0.0, 0.0, 9.0, 6.0],
                    vec![3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0],
                )]),
                Some(TransformFill::Components(vec![199, 71])),
            ),
            (
                (9, 6),
                4,
                TransformData::Mesh(vec![(
                    vec![0.0, 0.0, 9.0, 6.0],
                    vec![-1.0, 5.0, -1.0, 5.0, -1.0, 5.0, -1.0, 5.0],
                )]),
                Some(TransformFill::Components(vec![61, 233])),
            ),
            (
                (9, 6),
                3,
                TransformData::Affine(vec![3.25, 4.0, 3.25, 4.0, 3.25, 4.0, 3.25, 4.0]),
                Some(TransformFill::Components(vec![199, 71])),
            ),
            (
                (9, 6),
                4,
                TransformData::Mesh(vec![(
                    vec![0.0, 0.0, 9.0, 6.0],
                    vec![3.25, 4.0, 3.25, 4.0, 3.25, 4.0, 3.25, 4.0],
                )]),
                Some(TransformFill::Components(vec![199, 71])),
            ),
        ];
        for (size, method, data, fill) in constant_cases {
            let transformed = source
                .transform_public(size, method, Some(data), 0, 0, fill)
                .expect("PA constant projective transform");
            assert_eq!(transformed.mode().expect("constant PA mode"), "PA");
            assert_eq!(transformed.palette(), Some(vec![10, 20, 30, 40, 50, 60]));
            let expected = transformed
                .clone()
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU PA constant projective transform");
            let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU PA constant projective transform failed: {error}"),
            };
            assert_eq!(actual, expected, "PA constant projective pair parity");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("native PA constant projective transform must publish telemetry");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.6, Some(1));
            assert_eq!(telemetry.7, None);
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn palette_alpha_projective_filtered_relocations_native_gpu_preserve_pairs() {
        let cases = [
            (
                (1, 1),
                (1, 1),
                [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
                None,
            ),
            (
                (2, 3),
                (2, 3),
                [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
                None,
            ),
            (
                (4, 3),
                (6, 5),
                [1.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0],
                Some(TransformFill::Components(vec![199, 71])),
            ),
            (
                (7, 5),
                (5, 7),
                [0.0, 1.0, 1.0, 1.0, 0.0, -1.0, 0.0, 0.0],
                Some(TransformFill::Components(vec![61, 233])),
            ),
            (
                (16, 9),
                (18, 11),
                [1.0, 0.0, -2.0, 0.0, 1.0, 1.0, 0.0, 0.0],
                Some(TransformFill::Components(vec![17, 203])),
            ),
            (
                (33, 17),
                (31, 19),
                [0.0, 1.0, 2.0, 1.0, 0.0, -1.0, 0.0, 0.0],
                Some(TransformFill::Components(vec![113, 7])),
            ),
            (
                (16, 16),
                (8, 8),
                [0.0, 0.0, 1.5, 0.0, 0.0, 4.5, 0.0, 0.0],
                None,
            ),
            (
                (16, 16),
                (8, 8),
                [0.0, 0.0, -0.5, 0.0, 0.0, -0.5, 0.0, 0.0],
                Some(TransformFill::Components(vec![17, 203])),
            ),
        ];
        let palette = vec![10, 20, 30, 40, 50, 60];
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for (source_size, output_size, matrix, fill) in cases {
            let (source_width, source_height) = source_size;
            let source_bytes = (0..source_width * source_height)
                .flat_map(|index| [(index * 17 + 3) as u8, (index * 29 + 11) as u8])
                .collect::<Vec<_>>();
            let mut source = Image::frombytes("LA", source_size, &source_bytes).expect("LA source");
            source.putpalette(&palette, "RGB").expect("PA palette");
            assert_eq!(source.mode().expect("PA mode"), "PA");

            for filter in [(2, ResampleFilter::Bilinear), (3, ResampleFilter::Bicubic)] {
                let transformed = source
                    .transform_public(
                        output_size,
                        2,
                        Some(TransformData::Affine(matrix.to_vec())),
                        filter.0,
                        0,
                        fill.clone(),
                    )
                    .expect("PA filtered projective transform");
                assert_eq!(transformed.mode().expect("transformed PA mode"), "PA");
                assert_eq!(transformed.palette(), Some(palette.clone()));
                let expected_image = transformed.clone().use_backend(Backend::Cpu);
                let expected = expected_image
                    .tobytes()
                    .expect("CPU PA filtered projective transform");
                let gpu_image = transformed.clone().use_backend(Backend::Gpu);
                assert_eq!(gpu_image.mode().expect("GPU PA mode"), "PA");
                assert_eq!(gpu_image.palette(), Some(palette.clone()));
                let actual = match gpu_image.tobytes() {
                    Ok(actual) => actual,
                    Err(error)
                        if error.to_string().contains("GPU adapter not available")
                            || error
                                .to_string()
                                .contains("GPU device initialization failed") =>
                    {
                        Backend::set_pipeline_telemetry_enabled(previous);
                        return;
                    }
                    Err(error) => {
                        panic!("native GPU PA filtered projective transform failed: {error}")
                    }
                };
                assert_eq!(
                    actual, expected,
                    "PA filtered projective parity for {source_size:?} -> {output_size:?}, filter={:?}",
                    filter.1
                );
                let telemetry = Backend::take_pipeline_telemetry()
                    .expect("native PA filtered projective transform must publish a receipt");
                assert_eq!(telemetry.0, Some(Backend::Gpu));
                assert_eq!(telemetry.1, Backend::Gpu, "telemetry={telemetry:?}");
                assert_eq!(telemetry.6, Some(1));
                assert_eq!(telemetry.7, None);
            }
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn palette_alpha_quad_mesh_filtered_relocations_native_gpu_preserve_pairs() {
        let source_size = (16u32, 16u32);
        let source_bytes = (0..source_size.0 as usize * source_size.1 as usize)
            .flat_map(|index| [(index * 17 + 3) as u8, (index * 29 + 11) as u8])
            .collect::<Vec<_>>();
        let mut source = Image::frombytes("LA", source_size, &source_bytes).expect("LA source");
        let palette = vec![10, 20, 30, 40, 50, 60];
        source.putpalette(&palette, "RGB").expect("PA palette");
        assert_eq!(source.mode().expect("PA mode"), "PA");

        let cases = [
            (
                3,
                TransformData::Affine(vec![0.0, 0.0, 0.0, 6.0, 9.0, 6.0, 9.0, 0.0]),
            ),
            (
                3,
                TransformData::Affine(vec![0.0, 0.0, 6.0, 0.0, 6.0, 9.0, 0.0, 9.0]),
            ),
            (
                4,
                TransformData::Mesh(vec![(
                    vec![0.0, 0.0, 9.0, 6.0],
                    vec![1.0, 2.0, 1.0, 8.0, 10.0, 8.0, 10.0, 2.0],
                )]),
            ),
            (
                4,
                TransformData::Mesh(vec![(
                    vec![0.0, 0.0, 9.0, 6.0],
                    vec![1.0, 2.0, 7.0, 2.0, 7.0, 11.0, 1.0, 11.0],
                )]),
            ),
            (
                3,
                TransformData::Affine(vec![1.5, 4.5, 1.5, 4.5, 1.5, 4.5, 1.5, 4.5]),
            ),
            (
                4,
                TransformData::Mesh(vec![(
                    vec![0.0, 0.0, 9.0, 6.0],
                    vec![1.5, 4.5, 1.5, 4.5, 1.5, 4.5, 1.5, 4.5],
                )]),
            ),
        ];

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for (method, data) in cases {
            for filter in [ResampleFilter::Bilinear, ResampleFilter::Bicubic] {
                let transformed = source
                    .transform_public(
                        (9, 6),
                        method,
                        Some(data.clone()),
                        match filter {
                            ResampleFilter::Bilinear => 2,
                            ResampleFilter::Bicubic => 3,
                            _ => unreachable!(),
                        },
                        0,
                        Some(TransformFill::Components(vec![199, 71])),
                    )
                    .expect("filtered PA Quad/Mesh relocation");
                assert_eq!(transformed.mode().expect("PA mode"), "PA");
                assert_eq!(transformed.palette(), Some(palette.clone()));
                let expected = transformed
                    .clone()
                    .use_backend(Backend::Cpu)
                    .tobytes()
                    .expect("CPU filtered PA Quad/Mesh relocation");
                let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
                    Ok(actual) => actual,
                    Err(error)
                        if error.to_string().contains("GPU adapter not available")
                            || error
                                .to_string()
                                .contains("GPU device initialization failed") =>
                    {
                        Backend::set_pipeline_telemetry_enabled(previous);
                        return;
                    }
                    Err(error) => {
                        panic!("native GPU filtered PA Quad/Mesh relocation failed: {error}")
                    }
                };
                assert_eq!(actual, expected, "PA filtered method {method}");
                let telemetry = Backend::take_pipeline_telemetry()
                    .expect("filtered PA Quad/Mesh relocation must publish a receipt");
                assert_eq!(telemetry.0, Some(Backend::Gpu));
                assert_eq!(telemetry.1, Backend::Gpu, "telemetry={telemetry:?}");
                assert_eq!(telemetry.6, Some(1));
                assert_eq!(telemetry.7, None);
            }
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn palette_alpha_quad_mesh_nearest_native_gpu_preserves_pairs() {
        let source_bytes = (0..8 * 8)
            .flat_map(|index| [(index * 19 + 5) as u8, (index * 31 + 13) as u8])
            .collect::<Vec<_>>();
        let mut source = Image::frombytes("LA", (8, 8), &source_bytes).expect("LA source");
        source
            .putpalette(&[10, 20, 30, 40, 50, 60], "RGB")
            .expect("PA palette");
        assert_eq!(source.mode().expect("PA mode"), "PA");

        let cases = [
            (
                (8, 8),
                3,
                TransformData::Affine(vec![0.0, 0.0, 8.0, 0.0, 8.0, 8.0, 0.0, 8.0]),
                None,
            ),
            (
                (8, 8),
                3,
                TransformData::Affine(vec![0.0, 0.0, 0.0, 8.0, 8.0, 8.0, 8.0, 0.0]),
                Some(TransformFill::Components(vec![211, 83])),
            ),
            (
                (10, 10),
                4,
                TransformData::Mesh(vec![(
                    vec![0.0, 0.0, 10.0, 10.0],
                    vec![1.0, 2.0, 1.0, 12.0, 11.0, 12.0, 11.0, 2.0],
                )]),
                Some(TransformFill::Components(vec![199, 71])),
            ),
            (
                (8, 8),
                4,
                TransformData::Mesh(vec![(
                    vec![0.0, 0.0, 8.0, 8.0],
                    vec![0.0, 0.0, 8.0, 0.0, 8.0, 8.0, 0.0, 8.0],
                )]),
                None,
            ),
            (
                (8, 8),
                3,
                TransformData::Affine(vec![1.0, 2.0, 1.0, 10.0, 9.0, 10.0, 9.0, 2.0]),
                Some(TransformFill::Components(vec![61, 233])),
            ),
            (
                (8, 8),
                3,
                TransformData::Affine(vec![1.0, 2.0, 9.0, 2.0, 9.0, 10.0, 1.0, 10.0]),
                Some(TransformFill::Components(vec![17, 203])),
            ),
        ];

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for (size, method, data, fill) in cases {
            let transformed = source
                .transform_public(size, method, Some(data), 0, 0, fill)
                .expect("PA Quad/Mesh transform");
            let expected = transformed
                .clone()
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU PA Quad/Mesh transform");
            let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU PA Quad/Mesh transform failed: {error}"),
            };
            assert_eq!(actual, expected, "PA method {method} pair parity");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("native PA Quad/Mesh transform must publish a receipt");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu, "telemetry={telemetry:?}");
            assert_eq!(telemetry.6, Some(1));
            assert_eq!(telemetry.7, None);
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn fractional_projective_nearest_host_controls_corrected_cpu() {
        let width = 9;
        let height = 8;
        let source_bytes: Vec<u8> = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x * 37 + y * 11 + 3) as u8))
            .collect();
        let source = Image::frombytes("L", (width, height), &source_bytes).expect("luma source");
        let transformed = source
            .transform_public(
                (8, 7),
                2,
                Some(TransformData::Affine(vec![
                    1.0, 0.07, 0.4, -0.03, 1.0, 0.2, 0.001, -0.002,
                ])),
                0,
                0,
                Some(TransformFill::Scalar(17)),
            )
            .expect("perspective transform");
        let expected = transformed
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU perspective transform");
        let expected_oracle = [
            3, 40, 77, 114, 151, 188, 225, 6, 51, 88, 125, 162, 162, 199, 236, 17, 62, 99, 136,
            173, 210, 247, 28, 65, 73, 110, 147, 184, 221, 2, 39, 76, 84, 121, 158, 195, 232, 13,
            50, 87, 95, 132, 169, 206, 243, 24, 61, 98, 106, 143, 180, 217, 254, 35, 72, 109,
        ];
        assert_eq!(expected, expected_oracle);

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("fractional GPU perspective transform failed: {error}"),
        };
        assert_eq!(actual, expected_oracle);
        if let Some(telemetry) = Backend::take_pipeline_telemetry() {
            // The selected backend is the semantic fact this regression
            // protects. Other tests may toggle the process-wide telemetry
            // switch while the harness is running, so tolerate a missing or
            // replaced sample in parallel test execution.
            assert_eq!(telemetry.1, Backend::Cpu);
            assert_eq!(telemetry.7.as_deref(), Some("exact host semantic control"));
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn palette_alpha_nearest_affine_native_gpu_preserves_pairs() {
        let source_bytes = [
            1u8, 17, 2, 33, 3, 49, 4, 65, 5, 81, 6, 97, 7, 113, 8, 129, 9, 145, 10, 161, 11, 177,
            12, 193,
        ];
        let mut source = Image::frombytes("LA", (4, 3), &source_bytes).expect("LA source");
        source
            .putpalette(&[10, 20, 30, 40, 50, 60], "RGB")
            .expect("PA palette");
        assert_eq!(source.mode().expect("PA mode"), "PA");
        let transformed = source
            .transform_public(
                (4, 3),
                0,
                Some(TransformData::Affine(vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0])),
                0,
                0,
                None,
            )
            .expect("PA affine transform");
        let expected = transformed
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU PA transform");
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU PA affine transform failed: {error}"),
        };
        assert_eq!(actual, expected, "native GPU PA affine parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU PA affine transform must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(1));
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn palette_alpha_nearest_rotate_native_gpu_preserves_pairs() {
        let source_bytes = [
            1u8, 17, 2, 33, 3, 49, 4, 65, 5, 81, 6, 97, 7, 113, 8, 129, 9, 145, 10, 161, 11, 177,
            12, 193,
        ];
        let mut source = Image::frombytes("LA", (4, 3), &source_bytes).expect("LA source");
        source
            .putpalette(&[10, 20, 30, 40, 50, 60], "RGB")
            .expect("PA palette");
        assert_eq!(source.mode().expect("PA mode"), "PA");
        let cases = [
            source
                .rotate_with_input(
                    37.5,
                    RotateResampleInput::Name("NEAREST".into()),
                    RotateExpandInput::Boolean(true),
                    RotatePointInput::Default,
                    RotatePointInput::Default,
                    ImageOpsColor::Components(vec![1, 128]),
                )
                .expect("PA nearest rotate"),
            source
                .rotate_with_input(
                    17.5,
                    RotateResampleInput::Code(0),
                    RotateExpandInput::Boolean(true),
                    RotatePointInput::Values(vec![1.25, 0.75]),
                    RotatePointInput::Values(vec![0.25, -0.5]),
                    ImageOpsColor::Components(vec![7, 201]),
                )
                .expect("PA custom nearest rotate"),
        ];
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for rotated in cases {
            let expected = rotated
                .clone()
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU PA rotate");
            let actual = match rotated.use_backend(Backend::Gpu).tobytes() {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU PA rotate failed: {error}"),
            };
            assert_eq!(actual, expected, "PA rotate parity");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("native GPU PA rotate must publish a receipt");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.6, Some(1));
            assert_eq!(telemetry.7, None);
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn byte_point_fusion_requires_matching_native_mode() {
        let luma = DynamicImage::ImageLuma8(GrayImage::from_raw(1, 1, vec![7]).unwrap());
        let gray_alpha =
            DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(1, 1, vec![7, 9]).unwrap());
        let rgb = DynamicImage::ImageRgb8(RgbImage::from_raw(1, 1, vec![7, 9, 11]).unwrap());
        let rgba = DynamicImage::ImageRgba8(RgbaImage::from_raw(1, 1, vec![7, 9, 11, 13]).unwrap());

        assert!(gpu_byte_point_mode_allowed(&luma, None));
        assert!(gpu_byte_point_mode_allowed(&luma, Some("L")));
        assert!(!gpu_byte_point_mode_allowed(&luma, Some("P")));
        assert!(gpu_byte_point_mode_allowed(&gray_alpha, Some("LA")));
        assert!(!gpu_byte_point_mode_allowed(&gray_alpha, Some("L")));
        assert!(gpu_byte_point_mode_allowed(&rgb, Some("RGB")));
        assert!(gpu_byte_point_mode_allowed(&rgba, Some("RGBA")));
        assert!(!gpu_byte_point_mode_allowed(&rgba, Some("RGBX")));
    }

    #[test]
    fn palette_first_rgb_merge_admission_is_narrow() {
        let mut palette = Image::frombytes("P", (2, 1), &[1, 2]).expect("P band");
        palette
            .putpalette(&[10, 20, 30, 40, 50, 60], "RGB")
            .expect("palette");
        let l0 = Image::frombytes("L", (2, 1), &[32, 33]).expect("G band");
        let l1 = Image::frombytes("L", (2, 1), &[48, 49]).expect("B band");
        let rgb = PipelineOp::Merge {
            mode: ColorMode::RGB,
            logical_mode: "RGB".to_owned(),
            bands: vec![palette.clone(), l0.clone(), l1.clone()].into(),
        };
        assert!(gpu_palette_first_rgb_merge_is_supported(&[rgb], Some("P")));

        let lab = PipelineOp::Merge {
            mode: ColorMode::RGB,
            logical_mode: "LAB".to_owned(),
            bands: vec![palette, l0, l1].into(),
        };
        assert!(!gpu_palette_first_rgb_merge_is_supported(&[lab], Some("P")));
    }

    #[test]
    fn palette_first_rgb_merge_native_gpu_preserves_index_bytes() {
        let mut palette = Image::frombytes("P", (2, 1), &[1, 2]).expect("P band");
        palette
            .putpalette(&[10, 20, 30, 40, 50, 60], "RGB")
            .expect("palette");
        let green = Image::frombytes("L", (2, 1), &[32, 33]).expect("G band");
        let blue = Image::frombytes("L", (2, 1), &[48, 49]).expect("B band");
        let merged = crate::image_merge("RGB", &[palette, green, blue]).expect("RGB merge");
        let expected = merged
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU merge");
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match merged.use_backend(Backend::Gpu).tobytes() {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU palette-first merge failed: {error}"),
        };
        assert_eq!(actual, [1, 32, 48, 2, 33, 49]);
        assert_eq!(actual, expected);
        let telemetry =
            Backend::take_pipeline_telemetry().expect("native GPU merge must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(1));
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn contrast_mean_after_exact_putpixel_prefix_matches_materialized_prefix() {
        let cases = [
            (
                DynamicImage::ImageLuma8(GrayImage::from_raw(2, 1, vec![10, 20]).unwrap()),
                Some("L"),
                (1, 0, (80, 0, 0, 255)),
            ),
            (
                DynamicImage::ImageLumaA8(
                    GrayAlphaImage::from_raw(2, 1, vec![10, 200, 20, 100]).unwrap(),
                ),
                Some("LA"),
                (1, 0, (80, 0, 0, 220)),
            ),
            (
                DynamicImage::ImageRgb8(
                    RgbImage::from_raw(2, 1, vec![10, 20, 30, 40, 50, 60]).unwrap(),
                ),
                Some("RGB"),
                (1, 0, (80, 90, 100, 255)),
            ),
            (
                DynamicImage::ImageRgba8(
                    RgbaImage::from_raw(2, 1, vec![10, 20, 30, 40, 50, 60, 70, 80]).unwrap(),
                ),
                Some("RGBA"),
                (1, 0, (80, 90, 100, 220)),
            ),
            (
                DynamicImage::ImageRgba8(
                    RgbaImage::from_raw(2, 1, vec![10, 20, 30, 40, 50, 60, 70, 80]).unwrap(),
                ),
                Some("CMYK"),
                (1, 0, (80, 90, 100, 220)),
            ),
        ];
        for (image, mode, (x, y, color)) in cases {
            let ops = [
                PipelineOp::PutPixel {
                    x,
                    y,
                    color,
                    palette_index: false,
                },
                PipelineOp::Contrast { factor: 0.5 },
            ];
            let prefixed =
                crate::compute::pool_cpu::ops::effects::op_put_pixel(&image, x, y, color)
                    .expect("exact PutPixel prefix");
            assert_eq!(
                gpu_contrast_mean_after_exact_prefix(&image, &ops, mode),
                gpu_contrast_mean(&prefixed, mode),
                "mode {mode:?}"
            );
        }

        let indexed = [
            PipelineOp::PutPixel {
                x: 0,
                y: 0,
                color: (1, 2, 3, 255),
                palette_index: true,
            },
            PipelineOp::Contrast { factor: 0.5 },
        ];
        let image = DynamicImage::ImageLuma8(GrayImage::from_raw(1, 1, vec![7]).unwrap());
        assert_eq!(
            gpu_contrast_mean_after_exact_prefix(&image, &indexed, Some("P")),
            None
        );
    }

    #[test]
    fn extract_band_native_gpu_preserves_raw_color_mode_channels() {
        let cases = [
            ("CMYK", 3, vec![1, 2, 3, 4, 11, 12, 13, 14], vec![4, 14]),
            ("HSV", 1, vec![21, 22, 23, 31, 32, 33], vec![22, 32]),
            ("YCbCr", 2, vec![41, 42, 43, 51, 52, 53], vec![43, 53]),
        ];
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for (mode, channel, source_bytes, expected_bytes) in cases {
            let source = Image::frombytes(mode, (2, 1), &source_bytes).expect("source image");
            let expected = source
                .getchannel(channel)
                .expect("channel operation")
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU channel extraction");
            assert_eq!(expected, expected_bytes);
            let actual = match source
                .getchannel(channel)
                .expect("channel operation")
                .use_backend(Backend::Gpu)
                .tobytes()
            {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU {mode} channel extraction failed: {error}"),
            };
            assert_eq!(actual, expected, "{mode} channel extraction parity");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("native GPU channel extraction must publish a receipt");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.6, Some(1));
            assert_eq!(telemetry.7, None);
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn effect_spread_native_gpu_preserves_raw_byte_modes() {
        let cases = [
            ("1", vec![0x80, 0x40]),
            ("L", vec![7, 8]),
            ("LA", vec![7, 9, 8, 10]),
            ("P", vec![17, 18]),
            ("RGB", vec![1, 2, 3, 4, 5, 6]),
            ("RGBa", vec![1, 2, 3, 4, 5, 6, 7, 8]),
            ("RGBX", vec![11, 12, 13, 14, 15, 16, 17, 18]),
            ("RGBA", vec![21, 22, 23, 24, 25, 26, 27, 28]),
            ("CMYK", vec![31, 32, 33, 34, 35, 36, 37, 38]),
            ("HSV", vec![41, 42, 43, 44, 45, 46]),
            ("YCbCr", vec![51, 52, 53, 54, 55, 56]),
            (
                "I",
                (-123i32)
                    .to_le_bytes()
                    .into_iter()
                    .chain(456i32.to_le_bytes())
                    .collect(),
            ),
            (
                "F",
                1.25f32
                    .to_le_bytes()
                    .into_iter()
                    .chain(2.5f32.to_le_bytes())
                    .collect(),
            ),
        ];
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for (mode, source_bytes) in cases {
            let source = Image::frombytes(mode, (2, 1), &source_bytes).expect("source image");
            let expected = crate::image_effect_spread(&source, 0)
                .expect("effect_spread operation")
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU effect_spread");
            let actual = match crate::image_effect_spread(&source, 0)
                .expect("effect_spread operation")
                .use_backend(Backend::Gpu)
                .tobytes()
            {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU {mode} effect_spread failed: {error}"),
            };
            assert_eq!(actual, expected, "{mode} effect_spread parity");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("native GPU effect_spread must publish a receipt");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.6, Some(1));
            assert_eq!(telemetry.7, None);
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn contrast_after_putpixel_native_gpu_uses_current_image_midpoint() {
        let cases = [
            ("L", vec![10, 20, 30, 40]),
            ("LA", vec![10, 200, 20, 180, 30, 160, 40, 140]),
            (
                "RGB",
                vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120],
            ),
            (
                "RGBA",
                vec![
                    10, 20, 30, 200, 40, 50, 60, 180, 70, 80, 90, 160, 100, 110, 120, 140,
                ],
            ),
            (
                "CMYK",
                vec![
                    10, 20, 30, 40, 40, 50, 60, 80, 70, 80, 90, 160, 100, 110, 120, 140,
                ],
            ),
        ];
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for (mode, bytes) in cases {
            let mut source = Image::frombytes(mode, (2, 2), &bytes)
                .unwrap_or_else(|error| panic!("{mode} source image: {error}"));
            source
                .putpixel(1, 0, 180, 90, 60, 220)
                .unwrap_or_else(|error| panic!("{mode} PutPixel: {error}"));
            let expected = source
                .enhance_contrast(0.5)
                .expect("Contrast operation")
                .use_backend(Backend::Cpu)
                .tobytes()
                .unwrap_or_else(|error| panic!("{mode} CPU Contrast: {error}"));
            let actual = match source
                .enhance_contrast(0.5)
                .expect("Contrast operation")
                .use_backend(Backend::Gpu)
                .tobytes()
            {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU {mode} Contrast failed: {error}"),
            };
            assert_eq!(actual, expected, "{mode} Contrast parity");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("native GPU Contrast must publish a receipt");
            assert_eq!(telemetry.0, Some(Backend::Gpu), "{mode} requested backend");
            assert_eq!(telemetry.1, Backend::Gpu, "{mode} actual backend");
            assert_eq!(telemetry.6, Some(2), "{mode} dispatch count");
            assert_eq!(telemetry.7, None, "{mode} fallback reason");
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn cmyk_putalpha_native_gpu_promotes_exactly() {
        let source_bytes = vec![
            10, 20, 30, 40, // C/M/Y/K
            80, 90, 100, 110,
        ];
        let previous = Backend::set_pipeline_telemetry_enabled(true);

        let mut scalar = Image::frombytes("CMYK", (2, 1), &source_bytes).expect("CMYK source");
        scalar.putalpha(173).expect("CMYK scalar PutAlpha");
        let expected = scalar
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU CMYK scalar PutAlpha");
        let actual = match scalar.use_backend(Backend::Gpu).tobytes() {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU CMYK scalar PutAlpha failed: {error}"),
        };
        assert_eq!(actual, expected, "CMYK scalar PutAlpha parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU CMYK scalar PutAlpha must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(1));
        assert_eq!(telemetry.7, None);

        let mask = Image::frombytes("L", (2, 1), &[7, 231]).expect("L alpha mask");
        let mut masked = Image::frombytes("CMYK", (2, 1), &source_bytes).expect("CMYK source");
        masked.putalpha_data(&mask).expect("CMYK image PutAlpha");
        let expected = masked
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU CMYK image PutAlpha");
        let actual = match masked.use_backend(Backend::Gpu).tobytes() {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU CMYK image PutAlpha failed: {error}"),
        };
        assert_eq!(actual, expected, "CMYK image PutAlpha parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU CMYK image PutAlpha must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(1));
        assert_eq!(telemetry.7, None);

        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn draw_point_native_gpu_preserves_raw_byte_modes() {
        let cases = [
            ("1", vec![0x00, 0xff]),
            ("P", vec![17, 18]),
            ("PA", vec![17, 18]),
            ("RGBX", vec![1, 2, 3, 4, 5, 6, 7, 8]),
            ("RGBa", vec![11, 12, 13, 14, 15, 16, 17, 18]),
            ("CMYK", vec![21, 22, 23, 24, 25, 26, 27, 28]),
            ("HSV", vec![31, 32, 33, 34, 35, 36]),
            ("YCbCr", vec![41, 42, 43, 44, 45, 46]),
            (
                "I",
                (-123i32)
                    .to_le_bytes()
                    .into_iter()
                    .chain(456i32.to_le_bytes())
                    .collect(),
            ),
            (
                "F",
                1.25f32
                    .to_le_bytes()
                    .into_iter()
                    .chain(2.5f32.to_le_bytes())
                    .collect(),
            ),
        ];
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for (mode, source_bytes) in cases {
            let source = if mode == "PA" {
                let mut indexed = Image::frombytes("P", (2, 1), &source_bytes)
                    .unwrap_or_else(|error| panic!("{mode} source image: {error}"));
                indexed.putalpha(201).expect("PA source alpha");
                indexed
            } else {
                Image::frombytes(mode, (2, 1), &source_bytes)
                    .unwrap_or_else(|error| panic!("{mode} source image: {error}"))
            };
            let op = PipelineOp::DrawPoint {
                points: Arc::from([(0, 0), (1, 0)]),
                fill: (165, 90, 60, 195),
                alpha_blend_rgb: false,
            };
            let expected = Image::push_op(&source, op.clone())
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU draw point");
            let actual = match Image::push_op(&source, op)
                .use_backend(Backend::Gpu)
                .tobytes()
            {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU {mode} draw point failed: {error}"),
            };
            assert_eq!(actual, expected, "{mode} draw point parity");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("native GPU draw point must publish a receipt");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.6, Some(1));
            assert_eq!(telemetry.7, None);
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn fit_nearest_native_gpu_preserves_indexed_modes() {
        let cases = [("P", vec![17u8; 12]), ("PA", vec![17u8; 12])];
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for (mode, source_bytes) in cases {
            let source = if mode == "PA" {
                let mut indexed = Image::frombytes("P", (4, 3), &source_bytes)
                    .unwrap_or_else(|error| panic!("{mode} source image: {error}"));
                indexed.putalpha(201).expect("PA source alpha");
                indexed
            } else {
                Image::frombytes(mode, (4, 3), &source_bytes)
                    .unwrap_or_else(|error| panic!("{mode} source image: {error}"))
            };
            let fitted = crate::ops::imageops::fit(&source, 3, 2, Some("NEAREST"), 0.0, (0.5, 0.5))
                .expect("Fit operation");
            let expected = fitted
                .clone()
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU Fit");
            let actual = match fitted.use_backend(Backend::Gpu).tobytes() {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU {mode} Fit failed: {error}"),
            };
            assert_eq!(actual, expected, "{mode} Fit parity");
            let telemetry =
                Backend::take_pipeline_telemetry().expect("native GPU Fit must publish a receipt");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.6, Some(2));
            assert_eq!(telemetry.7, None);
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_fit_nearest_native_gpu_preserves_scalar_words() {
        let values = [
            0.25f32, -1.5, 3.0, 7.25, 11.0, 13.5, 17.75, 19.0, 23.25, 29.5, 31.0, 37.75, 41.0,
            43.5, 47.25, 53.0, 59.5, 61.25, 67.0, 71.75,
        ];
        let source_bytes = values
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>();
        let source = Image::frombytes("F", (5, 4), &source_bytes).expect("F source");
        let fitted = crate::ops::imageops::fit(&source, 3, 2, Some("NEAREST"), 0.0, (0.5, 0.5))
            .expect("F Fit operation");
        let expected = fitted
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU F Fit");

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match fitted.use_backend(Backend::Gpu).tobytes() {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU F Fit failed: {error}"),
        };
        assert_eq!(actual, expected, "native GPU F nearest Fit parity");
        let telemetry =
            Backend::take_pipeline_telemetry().expect("native GPU F Fit must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(2));
        assert_eq!(telemetry.7, None);

        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn i_cover_pad_nearest_native_gpu_preserves_signed_words() {
        let values = [
            -12345i32,
            -7,
            0,
            206,
            17,
            31,
            4096,
            i32::MAX,
            -1,
            73,
            91,
            127,
        ];
        let source_bytes = values
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect::<Vec<_>>();
        let source = Image::frombytes("I", (12, 1), &source_bytes).expect("I source");
        let covered =
            crate::ops::imageops::cover_with_input(&source, 8, 1, Some(ResampleInput::Code(0)))
                .expect("I Cover operation");
        let padded = crate::ops::imageops::pad_with_input(
            &covered,
            10,
            2,
            Some(ResampleInput::Code(0)),
            crate::ops::imageops::ImageOpsColor::Scalar(7),
            crate::ops::imageops::CenteringInput::Default,
        )
        .expect("I Pad operation");
        let expected = padded
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU I Cover/Pad");

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match padded.use_backend(Backend::Gpu).tobytes() {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU I Cover/Pad failed: {error}"),
        };
        assert_eq!(actual, expected, "native GPU I Cover/Pad parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU I Cover/Pad must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(5));
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn i_filter_nearest_resize_native_gpu_preserves_signed_words() {
        let source_bytes = (0..99i32)
            .map(|index| index.saturating_mul(257).saturating_sub(20_000))
            .flat_map(i32::to_le_bytes)
            .collect::<Vec<_>>();
        let source = Image::frombytes("I", (11, 9), &source_bytes).expect("I source");
        let filter = PipelineOp::Filter3x3 {
            kernel: [0.0, -1.0, 0.0, -1.0, 5.0, -1.0, 0.0, -1.0, 0.0],
            scale: 1.0,
            offset: 0.0,
        };
        let filtered = Image::push_op(&source, filter);
        let resized = filtered
            .resize((8, 6), Some(ResampleInput::Code(0)), None)
            .expect("I nearest resize");
        let expected = resized
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU I filter/resize");

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match resized.use_backend(Backend::Gpu).tobytes() {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU I filter/resize failed: {error}"),
        };
        assert_eq!(actual, expected, "native GPU I filter/resize parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU I filter/resize must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(3));
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn i_filtered_resize_native_gpu_preserves_int32_rounding() {
        let mut values = vec![0i32; 9 * 8];
        values[3 * 9 + 2] = 100_000;
        values[6 * 9 + 7] = -100_000;
        let source_bytes = values
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect::<Vec<_>>();
        let source = Image::frombytes("I", (9, 8), &source_bytes).expect("I source");
        let cases = [
            ((9, 3), ResampleInput::Name("BILINEAR".into())),
            ((9, 3), ResampleInput::Name("BICUBIC".into())),
            ((9, 3), ResampleInput::Name("LANCZOS".into())),
            ((9, 3), ResampleInput::Name("HAMMING".into())),
            ((9, 3), ResampleInput::Name("BOX".into())),
            ((3, 3), ResampleInput::Name("BICUBIC".into())),
        ];

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for (size, filter) in cases {
            let resized = source
                .resize(size, Some(filter), None)
                .expect("I filtered resize");
            let expected = resized
                .clone()
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU I filtered resize");
            let actual = match resized.use_backend(Backend::Gpu).tobytes() {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU I filtered resize failed: {error}"),
            };
            assert_eq!(actual, expected, "native GPU I filtered resize parity");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("native GPU I filtered resize must publish a receipt");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.6, Some(2));
            assert_eq!(telemetry.7, None);
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn i_filtered_box_resize_native_gpu_handles_extreme_words() {
        let values = [
            i32::MIN,
            i32::MAX,
            0,
            -1,
            i32::MAX,
            i32::MIN,
            1,
            7,
            100_000,
            -100_000,
            2_000_000_000,
            -2_000_000_000,
            17,
            31,
            4096,
            -4096,
        ];
        let source_bytes = values
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect::<Vec<_>>();
        let source = Image::frombytes("I", (4, 4), &source_bytes).expect("I source");
        let resized = source
            .resize((2, 2), Some(ResampleInput::Name("BOX".into())), None)
            .expect("I Box resize");
        let expected = resized
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU I Box resize");

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match resized.use_backend(Backend::Gpu).tobytes() {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU I Box resize failed: {error}"),
        };
        assert_eq!(actual, expected, "native GPU I Box resize parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU I Box resize must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(2));
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn i_resize_identity_native_gpu_preserves_signed_words() {
        let values = [
            i32::MIN,
            -12345,
            -1,
            0,
            1,
            12345,
            i32::MAX,
            -2_000_000_001,
            2_000_000_001,
        ];
        let source_bytes = (0..72usize)
            .flat_map(|index| values[index % values.len()].to_le_bytes())
            .collect::<Vec<_>>();
        let source = Image::frombytes("I", (9, 8), &source_bytes).expect("I source");
        let resized = source
            .resize((9, 8), Some(ResampleInput::Code(3)), None)
            .expect("I identity resize");
        let expected = resized
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU I identity resize");

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match resized.use_backend(Backend::Gpu).tobytes() {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU I identity resize failed: {error}"),
        };
        assert_eq!(actual, expected, "native GPU I identity resize parity");
        assert_eq!(actual, source.tobytes().expect("I source bytes"));
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU I identity resize must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(1));
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn i_resize_identity_proof_is_narrow() {
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(4, 3, i32::MIN.to_le_bytes().repeat(12)).unwrap(),
        );
        let same_size = PipelineOp::Resize {
            w: 4,
            h: 3,
            filter: ResampleFilter::Lanczos,
        };
        assert!(gpu_i_resize_identity_is_exact(
            std::slice::from_ref(&same_size),
            &image,
            Some("I")
        ));
        assert!(!gpu_i_resize_identity_is_exact(
            std::slice::from_ref(&same_size),
            &image,
            Some("F")
        ));
        assert!(!gpu_i_resize_identity_is_exact(
            &[same_size.clone(), PipelineOp::Mirror],
            &image,
            Some("I")
        ));
        assert!(!gpu_i_resize_identity_is_exact(
            &[PipelineOp::Resize {
                w: 3,
                h: 3,
                filter: ResampleFilter::Bilinear,
            }],
            &image,
            Some("I")
        ));
    }

    #[test]
    fn i_filtered_resize_f64_proof_matches_bounded_samples() {
        let mut bytes = vec![0u8; 9 * 8 * 4];
        let index = (3 * 9 + 2) * 4;
        bytes[index..index + 4].copy_from_slice(&100_000i32.to_le_bytes());
        let image = DynamicImage::ImageRgba8(RgbaImage::from_raw(9, 8, bytes).unwrap());
        let resize = PipelineOp::Resize {
            w: 9,
            h: 3,
            filter: ResampleFilter::Bilinear,
        };
        assert!(gpu_i_resize_f64_is_exact(
            std::slice::from_ref(&resize),
            &image,
            Some("I")
        ));
        assert!(!gpu_i_resize_f64_is_exact(
            std::slice::from_ref(&resize),
            &image,
            Some("F")
        ));
    }

    #[test]
    fn indexed_nearest_rotate_native_gpu_preserves_raw_samples() {
        let cases = [
            (
                "P",
                (5, 4),
                vec![
                    0, 17, 255, 42, 3, 99, 128, 7, 64, 201, 11, 240, 31, 88, 156, 4, 222, 13, 77,
                    190,
                ],
                Some((23, 0, 0, 255)),
            ),
            (
                "1",
                (8, 4),
                vec![0b1001_0110, 0b0110_1001, 0b1111_0000, 0b0000_1111],
                Some((255, 255, 255, 255)),
            ),
        ];
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for (mode, size, raw, fill) in cases {
            let source = Image::frombytes(mode, size, &raw)
                .unwrap_or_else(|error| panic!("{mode} source image: {error}"));
            let rotated = source
                .rotate(13.0, true, fill)
                .unwrap_or_else(|error| panic!("{mode} rotate: {error}"));
            let expected = rotated
                .clone()
                .use_backend(Backend::Cpu)
                .tobytes()
                .unwrap_or_else(|error| panic!("{mode} CPU rotate: {error}"));
            let actual = match rotated.use_backend(Backend::Gpu).tobytes() {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU {mode} rotate failed: {error}"),
            };
            assert_eq!(actual, expected, "{mode} rotate parity");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("native GPU indexed rotate must publish a receipt");
            assert_eq!(telemetry.0, Some(Backend::Gpu), "{mode} requested backend");
            assert_eq!(telemetry.1, Backend::Gpu, "{mode} actual backend");
            assert_eq!(telemetry.7, None, "{mode} fallback reason");
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn indexed_rotate_forces_nearest_and_reports_transposed_size() {
        let cases = [
            ("P", (2, 1), vec![17, 203]),
            ("1", (8, 2), vec![0b1001_0110, 0b0110_1001]),
        ];
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for (mode, size, raw) in cases {
            let source = Image::frombytes(mode, size, &raw)
                .unwrap_or_else(|error| panic!("{mode} source image: {error}"));
            let rotated = source
                .rotate_with_input(
                    90.0,
                    RotateResampleInput::Name("BILINEAR".to_owned()),
                    RotateExpandInput::Boolean(true),
                    RotatePointInput::Default,
                    RotatePointInput::Default,
                    ImageOpsColor::None,
                )
                .unwrap_or_else(|error| panic!("{mode} rotate: {error}"));
            assert_eq!(
                rotated.size().unwrap(),
                (size.1, size.0),
                "{mode} lazy size"
            );
            let expected = rotated
                .clone()
                .use_backend(Backend::Cpu)
                .tobytes()
                .unwrap_or_else(|error| panic!("{mode} CPU rotate: {error}"));
            let actual = match rotated.use_backend(Backend::Gpu).tobytes() {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU {mode} rotate failed: {error}"),
            };
            assert_eq!(actual, expected, "{mode} rotate parity");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("native GPU indexed right-angle rotate must publish a receipt");
            assert_eq!(telemetry.0, Some(Backend::Gpu), "{mode} requested backend");
            assert_eq!(telemetry.1, Backend::Gpu, "{mode} actual backend");
            assert_eq!(telemetry.7, None, "{mode} fallback reason");
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn typed_scalar_right_angle_rotate_uses_native_transpose() {
        // A right-angle rotate is Pillow's complete-sample relocation fast
        // path even when a non-nearest filter token was supplied.  Exercise
        // every typed storage representation: I/F words must survive as-is,
        // and I;16 must preserve its declared byte order in the low word.
        let cases = [
            (
                "I",
                (3, 2),
                vec![
                    0i32.to_le_bytes(),
                    (-17i32).to_le_bytes(),
                    65_535i32.to_le_bytes(),
                    i32::MIN.to_le_bytes(),
                    42i32.to_le_bytes(),
                    i32::MAX.to_le_bytes(),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>(),
            ),
            (
                "F",
                (3, 2),
                vec![
                    0.25f32.to_le_bytes(),
                    (-17.5f32).to_le_bytes(),
                    f32::from_bits(0x7f7f_ffff).to_le_bytes(),
                    (-0.0f32).to_le_bytes(),
                    42.0f32.to_le_bytes(),
                    f32::from_bits(0x8000_0001).to_le_bytes(),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>(),
            ),
            (
                "I;16",
                (3, 2),
                [1u16, 258, 32_767, 65_535, 42, 54_321]
                    .into_iter()
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>(),
            ),
            (
                "I;16B",
                (3, 2),
                [1u16, 258, 32_767, 65_535, 42, 54_321]
                    .into_iter()
                    .flat_map(u16::to_be_bytes)
                    .collect::<Vec<_>>(),
            ),
        ];
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for (mode, size, raw) in cases {
            let source = Image::frombytes(mode, size, &raw)
                .unwrap_or_else(|error| panic!("{mode} source image: {error}"));
            let rotated = source
                .rotate_with_input(
                    90.0,
                    RotateResampleInput::Name("BICUBIC".to_owned()),
                    RotateExpandInput::Boolean(true),
                    RotatePointInput::Default,
                    RotatePointInput::Default,
                    ImageOpsColor::None,
                )
                .unwrap_or_else(|error| panic!("{mode} rotate: {error}"));
            let expected = rotated
                .clone()
                .use_backend(Backend::Cpu)
                .tobytes()
                .unwrap_or_else(|error| panic!("{mode} CPU rotate: {error}"));
            let actual = match rotated.use_backend(Backend::Gpu).tobytes() {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU {mode} rotate failed: {error}"),
            };
            assert_eq!(
                actual, expected,
                "native GPU {mode} right-angle rotate parity"
            );
            let telemetry = Backend::take_pipeline_telemetry()
                .unwrap_or_else(|| panic!("native GPU {mode} rotate missing telemetry"));
            assert_eq!(telemetry.0, Some(Backend::Gpu), "{mode} requested backend");
            assert_eq!(telemetry.1, Backend::Gpu, "{mode} actual backend");
            assert_eq!(telemetry.7, None, "{mode} fallback reason");
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn cmyk_nearest_rotate_native_gpu_preserves_raw_samples() {
        let source = Image::frombytes(
            "CMYK",
            (3, 2),
            &[
                0, 17, 34, 51, 68, 85, 102, 119, 136, 153, 170, 187, 204, 221, 238, 255, 13, 47,
                89, 131, 173, 215, 241, 7,
            ],
        )
        .expect("CMYK source");
        let cases = [
            source
                .rotate(1.0, true, None)
                .expect("CMYK fractional rotate"),
            source
                .rotate_with_input(
                    90.0,
                    RotateResampleInput::Name("BICUBIC".to_owned()),
                    RotateExpandInput::Boolean(true),
                    RotatePointInput::Default,
                    RotatePointInput::Default,
                    ImageOpsColor::None,
                )
                .expect("CMYK right-angle rotate"),
            source
                .rotate_with_input(
                    180.0,
                    RotateResampleInput::Name("BICUBIC".to_owned()),
                    RotateExpandInput::Boolean(false),
                    RotatePointInput::Default,
                    RotatePointInput::Default,
                    ImageOpsColor::None,
                )
                .expect("CMYK 180-degree rotate"),
            source
                .rotate(90.0, true, None)
                .expect("CMYK 90-degree rotate"),
            source
                .rotate(270.0, true, None)
                .expect("CMYK 270-degree rotate"),
            source
                .rotate_with_input(
                    17.5,
                    RotateResampleInput::Name("NEAREST".to_owned()),
                    RotateExpandInput::Boolean(true),
                    RotatePointInput::Values(vec![1.25, 0.75]),
                    RotatePointInput::Values(vec![0.25, -0.5]),
                    ImageOpsColor::Components(vec![10, 20, 30, 40]),
                )
                .expect("CMYK custom nearest rotate"),
        ];
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for rotated in cases {
            let expected = rotated
                .clone()
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU CMYK rotate");
            let actual = match rotated.use_backend(Backend::Gpu).tobytes() {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU CMYK rotate failed: {error}"),
            };
            assert_eq!(actual, expected, "CMYK rotate parity");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("native GPU CMYK rotate must publish a receipt");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.6, Some(1));
            assert_eq!(telemetry.7, None);
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn cmyk_filtered_rotate_stays_on_exact_host_control() {
        let source = Image::frombytes(
            "CMYK",
            (3, 2),
            &[
                0, 17, 34, 51, 68, 85, 102, 119, 136, 153, 170, 187, 204, 221, 238, 255, 13, 47,
                89, 131, 173, 215, 241, 7,
            ],
        )
        .expect("CMYK source");
        let rotated = source
            .rotate_with_input(
                17.5,
                RotateResampleInput::Name("BICUBIC".to_owned()),
                RotateExpandInput::Boolean(true),
                RotatePointInput::Default,
                RotatePointInput::Default,
                ImageOpsColor::None,
            )
            .expect("CMYK filtered rotate");
        let expected = rotated
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU CMYK filtered rotate");

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match rotated.use_backend(Backend::Gpu).tobytes() {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("CMYK filtered rotate failed: {error}"),
        };
        assert_eq!(actual, expected, "CMYK filtered rotate parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("CMYK filtered rotate must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Cpu);
        assert_eq!(telemetry.7.as_deref(), Some("exact host semantic control"));
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn i_filter_nearest_resize_chain_guard_is_narrow() {
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(4, 4, (-12345i32).to_le_bytes().repeat(16)).unwrap(),
        );
        let filter = PipelineOp::Filter3x3 {
            kernel: [0.0, -1.0, 0.0, -1.0, 5.0, -1.0, 0.0, -1.0, 0.0],
            scale: 1.0,
            offset: 0.0,
        };
        assert!(gpu_int_filter_resize_chain_is_supported(
            &[
                filter.clone(),
                PipelineOp::Resize {
                    w: 3,
                    h: 2,
                    filter: ResampleFilter::Nearest,
                },
            ],
            &image,
        ));
        assert!(!gpu_int_filter_resize_chain_is_supported(
            &[
                filter.clone(),
                PipelineOp::Resize {
                    w: 3,
                    h: 2,
                    filter: ResampleFilter::Bilinear,
                },
            ],
            &image,
        ));
        assert!(!gpu_int_filter_resize_chain_is_supported(
            &[
                PipelineOp::Resize {
                    w: 3,
                    h: 2,
                    filter: ResampleFilter::Nearest,
                },
                filter,
            ],
            &image,
        ));
        assert!(!gpu_int_filter_resize_chain_is_supported(
            &[
                PipelineOp::Filter5x5 {
                    kernel: [0.0; 25],
                    scale: 1.0,
                    offset: 0.0,
                },
                PipelineOp::Resize {
                    w: 3,
                    h: 2,
                    filter: ResampleFilter::Nearest,
                },
            ],
            &image,
        ));
    }

    #[test]
    fn f_resize_constant_lowering_requires_exact_finite_source() {
        let resize = PipelineOp::Resize {
            w: 2,
            h: 3,
            filter: ResampleFilter::Bicubic,
        };
        let bits = 37.0f32.to_bits();
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(3, 2, 37.0f32.to_le_bytes().repeat(6)).unwrap(),
        );
        assert_eq!(
            gpu_f_resize_constant_bits(std::slice::from_ref(&resize), &image, Some("F")),
            Some(bits)
        );

        let mut mixed = 37.0f32.to_le_bytes().repeat(6);
        mixed[4..8].copy_from_slice(&113.0f32.to_le_bytes());
        let mixed = DynamicImage::ImageRgba8(RgbaImage::from_raw(3, 2, mixed).unwrap());
        assert_eq!(
            gpu_f_resize_constant_bits(std::slice::from_ref(&resize), &mixed, Some("F")),
            None
        );

        let nearest = PipelineOp::Resize {
            w: 2,
            h: 3,
            filter: ResampleFilter::Nearest,
        };
        assert_eq!(
            gpu_f_resize_constant_bits(std::slice::from_ref(&nearest), &image, Some("F")),
            None
        );

        let negative_zero = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(3, 2, (-0.0f32).to_le_bytes().repeat(6)).unwrap(),
        );
        assert_eq!(
            gpu_f_resize_constant_bits(std::slice::from_ref(&resize), &negative_zero, Some("F")),
            None
        );
    }

    #[test]
    fn f_nearest_resize_uses_cumulative_coefficients() {
        assert!(gpu_resize_nearest_uses_coefficients(Some("F")));
        let coefficients = gpu_resize_coefficients(7, 2, ResampleFilter::Nearest);
        assert_eq!(&coefficients.xmin, &[0, 0, 0, 0, 1, 1, 1]);
        assert!(coefficients.count.iter().all(|&count| count == 1));
        assert!(
            coefficients
                .weights
                .iter()
                .all(|&weight| weight == 1i64 << 22)
        );
    }

    #[test]
    fn f_resize_box_copy_lowering_requires_non_downscaling_box_chain() {
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(
                3,
                2,
                [1.0f32, -2.5, 3.75, 4.5, -7.0, 9.25]
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect(),
            )
            .unwrap(),
        );
        let box_resize = |w, h| PipelineOp::Resize {
            w,
            h,
            filter: ResampleFilter::Box,
        };

        assert!(gpu_f_resize_box_copy_is_exact(
            &[box_resize(6, 4)],
            &image,
            Some("F")
        ));
        assert!(gpu_f_resize_box_copy_is_exact(
            &[box_resize(3, 2), box_resize(12, 8)],
            &image,
            Some("F")
        ));
        assert!(gpu_f_resize_box_copy_is_exact(
            &[
                PipelineOp::PutData {
                    data: Arc::from(vec![0u8; 24].into_boxed_slice()),
                    mode: PixelMode::F,
                },
                box_resize(6, 4),
            ],
            &image,
            Some("F")
        ));
        assert!(!gpu_f_resize_box_copy_is_exact(
            &[
                PipelineOp::PutData {
                    data: Arc::from(vec![0u8; 4].into_boxed_slice()),
                    mode: PixelMode::F,
                },
                box_resize(6, 4),
            ],
            &image,
            Some("F")
        ));
        assert!(!gpu_f_resize_box_copy_is_exact(
            &[box_resize(2, 2)],
            &image,
            Some("F")
        ));
        assert!(!gpu_f_resize_box_copy_is_exact(
            &[PipelineOp::Resize {
                w: 6,
                h: 4,
                filter: ResampleFilter::Bilinear,
            }],
            &image,
            Some("F")
        ));
        assert!(!gpu_f_resize_box_copy_is_exact(
            &[box_resize(6, 4)],
            &image,
            Some("I")
        ));
        assert!(!gpu_f_resize_box_copy_is_exact(
            &[box_resize(6, 4), PipelineOp::Mirror],
            &image,
            Some("F")
        ));

        let negative_zero = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(3, 2, (-0.0f32).to_le_bytes().repeat(6)).unwrap(),
        );
        assert!(gpu_f_resize_box_copy_is_exact(
            &[box_resize(6, 4)],
            &negative_zero,
            Some("F")
        ));
        let nan = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(3, 2, f32::NAN.to_le_bytes().repeat(6)).unwrap(),
        );
        assert!(gpu_f_resize_box_copy_is_exact(
            &[box_resize(6, 4)],
            &nan,
            Some("F")
        ));
        let infinity = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(3, 2, f32::INFINITY.to_le_bytes().repeat(6)).unwrap(),
        );
        assert!(gpu_f_resize_box_copy_is_exact(
            &[box_resize(6, 4)],
            &infinity,
            Some("F")
        ));
    }

    #[test]
    fn f_resize_box_average_lowering_requires_2x_axes_and_safe_values() {
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(
                4,
                4,
                [
                    1.0f32, 2.5, 3.75, 4.5, 7.0, 9.25, 12.0, 15.5, 2.0, 3.5, 5.75, 6.5, 8.0, 10.25,
                    13.0, 16.5,
                ]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect(),
            )
            .unwrap(),
        );
        let box_resize = |w, h| PipelineOp::Resize {
            w,
            h,
            filter: ResampleFilter::Box,
        };

        let coefficients = super::gpu_resize_coefficients(2, 4, ResampleFilter::Box);
        assert_eq!(coefficients.count, [2, 2]);
        assert_eq!(coefficients.weights, [1 << 21, 1 << 21, 1 << 21, 1 << 21]);

        assert!(gpu_f_resize_box_average_is_exact(
            &[box_resize(2, 4)],
            &image,
            Some("F")
        ));
        assert!(gpu_f_resize_box_average_is_exact(
            &[box_resize(4, 2)],
            &image,
            Some("F")
        ));
        assert!(gpu_f_resize_box_average_is_exact(
            &[box_resize(2, 2)],
            &image,
            Some("F")
        ));
        assert!(!gpu_f_resize_box_average_is_exact(
            &[box_resize(3, 4)],
            &image,
            Some("F")
        ));
        assert!(!gpu_f_resize_box_average_is_exact(
            &[box_resize(2, 4), box_resize(1, 4)],
            &image,
            Some("F")
        ));

        let mixed_sign = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(
                4,
                4,
                [
                    1.0f32, -2.5, 3.75, 4.5, 7.0, 9.25, 12.0, 15.5, 2.0, 3.5, 5.75, 6.5, 8.0,
                    10.25, 13.0, 16.5,
                ]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect(),
            )
            .unwrap(),
        );
        assert!(!gpu_f_resize_box_average_is_exact(
            &[box_resize(2, 4)],
            &mixed_sign,
            Some("F")
        ));

        let subnormal = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(4, 4, (f32::MIN_POSITIVE).to_le_bytes().repeat(16)).unwrap(),
        );
        assert!(!gpu_f_resize_box_average_is_exact(
            &[box_resize(2, 4)],
            &subnormal,
            Some("F")
        ));
        let floor = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(4, 4, f32::from_bits(0x3580_0000).to_le_bytes().repeat(16))
                .unwrap(),
        );
        assert!(gpu_f_resize_box_average_is_exact(
            &[box_resize(2, 4)],
            &floor,
            Some("F")
        ));
        let below_floor = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(
                4,
                4,
                [f32::from_bits(0x3500_0000)]
                    .into_iter()
                    .chain([f32::from_bits(0x3580_0000); 15])
                    .flat_map(f32::to_le_bytes)
                    .collect(),
            )
            .unwrap(),
        );
        assert!(!gpu_f_resize_box_average_is_exact(
            &[box_resize(2, 4)],
            &below_floor,
            Some("F")
        ));
        let negative_zero = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(4, 4, (-0.0f32).to_le_bytes().repeat(16)).unwrap(),
        );
        assert!(!gpu_f_resize_box_average_is_exact(
            &[box_resize(2, 4)],
            &negative_zero,
            Some("F")
        ));
    }

    #[test]
    fn f_resize_dyadic_lowering_requires_proven_filter_and_source_domain() {
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(
                4,
                2,
                [1.0f32, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0]
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect(),
            )
            .unwrap(),
        );
        let bilinear = PipelineOp::Resize {
            w: 8,
            h: 4,
            filter: ResampleFilter::Bilinear,
        };
        let box_resize = |w, h| PipelineOp::Resize {
            w,
            h,
            filter: ResampleFilter::Box,
        };

        // Bilinear 2x rows contain only exact dyadic weights.  A 4:1 Box
        // reduction has four equal dyadic taps; both exercise marker 6.
        assert!(gpu_f_resize_dyadic_is_exact(
            std::slice::from_ref(&bilinear),
            &image,
            Some("F")
        ));
        assert!(gpu_f_resize_dyadic_is_exact(
            &[box_resize(1, 2)],
            &image,
            Some("F")
        ));
        assert!(gpu_f_resize_dyadic_is_exact(
            &[
                PipelineOp::PutData {
                    data: Arc::from(image.as_bytes().to_vec().into_boxed_slice()),
                    mode: PixelMode::F,
                },
                box_resize(1, 2),
            ],
            &image,
            Some("F")
        ));

        // Two changed power-of-two Box axes remain exact because every
        // horizontal dyadic average is exactly representable before the
        // vertical pass consumes it.
        assert!(gpu_f_resize_dyadic_is_exact(
            &[box_resize(1, 1)],
            &image,
            Some("F")
        ));
        // Chained Box reductions retain exactness when the cumulative dyadic
        // shift stays within the same 24-bit significand bound.  The second
        // resize consumes f32 intermediates that are no longer single powers
        // of two, so this guards the chain proof rather than the one-pass
        // source-word check above.
        assert!(gpu_f_resize_dyadic_is_exact(
            &[box_resize(2, 1), box_resize(1, 1)],
            &image,
            Some("F")
        ));
        assert!(gpu_f_resize_dyadic_is_exact(
            &[
                box_resize(3, 2),
                PipelineOp::PutData {
                    data: Arc::from(
                        [1.0f32, 2.0, 4.0, 8.0, 16.0, 32.0]
                            .into_iter()
                            .flat_map(f32::to_le_bytes)
                            .collect::<Vec<_>>()
                            .into_boxed_slice(),
                    ),
                    mode: PixelMode::F,
                },
                box_resize(2, 1),
                box_resize(1, 1),
            ],
            &image,
            Some("F")
        ));
        // Mixing an arithmetic filter into the chain is still outside the
        // proof: only Box's power-of-two scaling preserves the exact value
        // domain after an intermediate f32 store.
        assert!(!gpu_f_resize_dyadic_is_exact(
            &[
                box_resize(2, 1),
                PipelineOp::Resize {
                    w: 1,
                    h: 1,
                    filter: ResampleFilter::Bilinear,
                },
            ],
            &image,
            Some("F")
        ));
        // Non-divisor Box geometry can still be dyadic when each output row
        // uses only one or two equal taps.  The 4 -> 3 table is [1, 2, 1]
        // taps, so it exercises the generalized row proof rather than the
        // integral power-of-two ratio shortcut.
        assert!(gpu_f_resize_dyadic_is_exact(
            &[box_resize(3, 4)],
            &image,
            Some("F")
        ));
        let non_dyadic_box_source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(
                7,
                1,
                [1.0f32, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0]
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect(),
            )
            .unwrap(),
        );
        // The middle 7 -> 3 Box row has three 1/3 taps, which is outside the
        // dyadic device proof even though the fixed shader table is valid.
        assert!(!gpu_f_resize_dyadic_is_exact(
            &[box_resize(3, 1)],
            &non_dyadic_box_source,
            Some("F")
        ));

        // The smallest two-sample reduction has exact, non-negative 1/2
        // rows for these arithmetic filters.  Their kernels differ away
        // from this geometry, so this matrix guards the proof's row-level
        // filter admission rather than treating every arithmetic filter as
        // dyadic.
        let two_sample = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(
                2,
                2,
                [1.0f32, 2.0, 4.0, 8.0]
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect(),
            )
            .unwrap(),
        );
        for filter in [
            ResampleFilter::Bicubic,
            ResampleFilter::Lanczos,
            ResampleFilter::Hamming,
        ] {
            assert!(gpu_f_resize_dyadic_is_exact(
                &[PipelineOp::Resize { w: 1, h: 1, filter }],
                &two_sample,
                Some("F")
            ));
        }
        let three_sample = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(
                3,
                2,
                [1.0f32, 2.0, 4.0, 8.0, 16.0, 32.0]
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect(),
            )
            .unwrap(),
        );
        // A 3 -> 2 arithmetic resize crosses the admission boundary: the
        // wider kernels need more than two taps, and Hamming's two taps are
        // non-dyadic.  Keep all three filters on host control here.
        for filter in [
            ResampleFilter::Bicubic,
            ResampleFilter::Lanczos,
            ResampleFilter::Hamming,
        ] {
            assert!(!gpu_f_resize_dyadic_is_exact(
                &[PipelineOp::Resize { w: 2, h: 1, filter }],
                &three_sample,
                Some("F")
            ));
        }
        let wide_two_axis = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(
                4,
                2,
                [
                    2.0f32.powi(-80),
                    2.0f32.powi(-59),
                    2.0f32.powi(-59),
                    2.0f32.powi(-59),
                    2.0f32.powi(-59),
                    2.0f32.powi(-59),
                    2.0f32.powi(-59),
                    2.0f32.powi(-59),
                ]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect(),
            )
            .unwrap(),
        );
        // A second reduction would expose a 24-bit-plus significand span;
        // retain exact host control rather than relying on device rounding.
        assert!(!gpu_f_resize_dyadic_is_exact(
            &[box_resize(1, 1)],
            &wide_two_axis,
            Some("F")
        ));
        assert!(!gpu_f_resize_dyadic_is_exact(
            &[PipelineOp::Resize {
                w: 8,
                h: 4,
                filter: ResampleFilter::Bicubic,
            }],
            &image,
            Some("F")
        ));

        let rejects = [
            (
                [(-0.0f32), 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0],
                "negative-zero",
            ),
            ([f32::NAN, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0], "nan"),
            (
                [f32::INFINITY, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0],
                "infinity",
            ),
            (
                [f32::from_bits(1), 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0],
                "subnormal",
            ),
            (
                [2.0f32.powi(-81), 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0],
                "below-coefficient-bound",
            ),
            (
                [2.0f32.powi(-16), 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0],
                "wide-exponent-span",
            ),
        ];
        for (values, _label) in rejects {
            let rejected = DynamicImage::ImageRgba8(
                RgbaImage::from_raw(
                    4,
                    2,
                    values.into_iter().flat_map(f32::to_le_bytes).collect(),
                )
                .unwrap(),
            );
            assert!(
                !gpu_f_resize_dyadic_is_exact(
                    std::slice::from_ref(&bilinear),
                    &rejected,
                    Some("F")
                ),
                "unexpected admission for {_label}"
            );
        }

        assert!(!gpu_f_resize_dyadic_is_exact(
            std::slice::from_ref(&bilinear),
            &image,
            Some("I")
        ));
    }

    #[test]
    fn f_resize_f64_lowering_proves_finite_subnormal_words_and_two_axes() {
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(
                2,
                2,
                [0.1f32, -0.3, 1.7, 2.9]
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect(),
            )
            .unwrap(),
        );
        let horizontal = PipelineOp::Resize {
            w: 1,
            h: 2,
            filter: ResampleFilter::Bilinear,
        };
        assert!(gpu_f_resize_f64_is_exact(
            std::slice::from_ref(&horizontal),
            &image,
            Some("F")
        ));

        // The encoder separates the two reducers, and the host proof checks
        // the rounded horizontal words before proving the vertical pass.
        assert!(gpu_f_resize_f64_is_exact(
            &[PipelineOp::Resize {
                w: 1,
                h: 1,
                filter: ResampleFilter::Bilinear,
            }],
            &image,
            Some("F")
        ));

        // A non-dyadic vertical table keeps this case outside marker 6 while
        // the exact f64 reducer still proves its two-axis intermediate.
        let non_dyadic_two_axis = PipelineOp::Resize {
            w: 1,
            h: 5,
            filter: ResampleFilter::Bilinear,
        };
        assert!(!gpu_f_resize_integer_is_exact(
            std::slice::from_ref(&non_dyadic_two_axis),
            &image,
            Some("F")
        ));
        assert!(gpu_f_resize_f64_is_exact(
            std::slice::from_ref(&non_dyadic_two_axis),
            &image,
            Some("F")
        ));
        let filtered_chain = [
            PipelineOp::Resize {
                w: 3,
                h: 3,
                filter: ResampleFilter::Bicubic,
            },
            PipelineOp::Resize {
                w: 2,
                h: 2,
                filter: ResampleFilter::Lanczos,
            },
        ];
        assert!(gpu_f_resize_f64_is_exact(
            &filtered_chain,
            &image,
            Some("F")
        ));

        // PutData(F) replaces the deferred source words before the resize;
        // the marker-9 proof must validate and consume that replacement rather
        // than conservatively routing an otherwise exact non-dyadic resize.
        let putdata: Arc<[u8]> = Arc::from(
            [0.1f32, -0.3, 1.7, 2.9]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        assert!(gpu_f_resize_f64_is_exact(
            &[
                PipelineOp::PutData {
                    data: putdata.clone(),
                    mode: PixelMode::F,
                },
                PipelineOp::Resize {
                    w: 1,
                    h: 5,
                    filter: ResampleFilter::Bilinear,
                },
            ],
            &image,
            Some("F")
        ));
        assert!(!gpu_f_resize_f64_is_exact(
            &[
                PipelineOp::PutData {
                    data: Arc::from(vec![0u8; 4].into_boxed_slice()),
                    mode: PixelMode::F,
                },
                PipelineOp::Resize {
                    w: 1,
                    h: 5,
                    filter: ResampleFilter::Bilinear,
                },
            ],
            &image,
            Some("F")
        ));
        assert!(!gpu_f_resize_f64_is_exact(
            &[
                PipelineOp::PutData {
                    data: putdata,
                    mode: PixelMode::I,
                },
                PipelineOp::Resize {
                    w: 1,
                    h: 5,
                    filter: ResampleFilter::Bilinear,
                },
            ],
            &image,
            Some("F")
        ));
        // The explicit horizontal/vertical compute-pass boundary also makes
        // the opposite mixed geometry safe: horizontal upscaling is fully
        // materialized before the vertical downscale consumes it.
        assert!(gpu_f_resize_f64_is_exact(
            &[PipelineOp::Resize {
                w: 4,
                h: 1,
                filter: ResampleFilter::Bilinear,
            }],
            &image,
            Some("F")
        ));
        // Marker 6 and the broader dyadic Box proof retain their own
        // conservative mixed-geometry guard; marker 9's explicit pass
        // boundary is not a proof for those older reducers.
        let mixed_box_source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(
                1,
                2,
                [-9.7966165f32, 6.5041304]
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect(),
            )
            .unwrap(),
        );
        let mixed_box = PipelineOp::Resize {
            w: 2,
            h: 1,
            filter: ResampleFilter::Box,
        };
        assert!(!gpu_f_resize_integer_is_exact(
            std::slice::from_ref(&mixed_box),
            &mixed_box_source,
            Some("F")
        ));
        assert!(!gpu_f_resize_dyadic_is_exact(
            std::slice::from_ref(&mixed_box),
            &mixed_box_source,
            Some("F")
        ));
        assert!(gpu_f_resize_f64_is_exact(
            std::slice::from_ref(&mixed_box),
            &mixed_box_source,
            Some("F")
        ));
        assert!(!gpu_f_resize_f64_is_exact(
            std::slice::from_ref(&horizontal),
            &image,
            Some("I")
        ));

        for value in [f32::NAN.to_bits(), f32::INFINITY.to_bits()] {
            let special = DynamicImage::ImageRgba8(
                RgbaImage::from_raw(
                    2,
                    2,
                    [value, 0x3f80_0000, 0x4000_0000, 0x4040_0000]
                        .into_iter()
                        .flat_map(u32::to_le_bytes)
                        .collect(),
                )
                .unwrap(),
            );
            assert!(
                gpu_f_resize_f64_is_exact(std::slice::from_ref(&horizontal), &special, Some("F")),
                "finite-coefficient special-value rows should use the exact marker"
            );
        }

        // Finite subnormal words use the explicit 2^-149 source scale and
        // remain eligible for the same exact integer reducer.  Keep both
        // signs here; the filtered path canonicalizes only an exact zero
        // result, while nonzero negative subnormals retain their sign bit.
        for values in [
            [
                f32::from_bits(1),
                f32::from_bits(2),
                f32::from_bits(3),
                f32::from_bits(4),
            ],
            [
                -f32::from_bits(1),
                -f32::from_bits(2),
                -f32::from_bits(3),
                -f32::from_bits(4),
            ],
        ] {
            let subnormal = DynamicImage::ImageRgba8(
                RgbaImage::from_raw(
                    2,
                    2,
                    values.into_iter().flat_map(f32::to_le_bytes).collect(),
                )
                .unwrap(),
            );
            assert!(gpu_f_resize_f64_is_exact(
                std::slice::from_ref(&horizontal),
                &subnormal,
                Some("F")
            ));
        }
    }

    #[test]
    fn f_order_filter_putdata_requires_finite_words() {
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(
                2,
                2,
                [1.0f32, 2.0, 3.0, 4.0]
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect(),
            )
            .unwrap(),
        );
        let rank = PipelineOp::RankFilter { size: 3, rank: 4 };
        let words = |values: [f32; 4]| -> Arc<[u8]> {
            Arc::from(
                values
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            )
        };

        assert!(gpu_float_filter_is_supported(
            &[
                PipelineOp::PutData {
                    data: words([4.0, 3.0, 2.0, 1.0]),
                    mode: PixelMode::F,
                },
                rank.clone(),
            ],
            &image,
        ));
        for special in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(!gpu_float_filter_is_supported(
                &[
                    PipelineOp::PutData {
                        data: words([special, 3.0, 2.0, 1.0]),
                        mode: PixelMode::F,
                    },
                    rank.clone(),
                ],
                &image,
            ));
        }
        assert!(!gpu_float_filter_is_supported(
            &[
                PipelineOp::PutData {
                    data: Arc::from(vec![0u8; 4].into_boxed_slice()),
                    mode: PixelMode::F,
                },
                rank.clone(),
            ],
            &image,
        ));
        assert!(!gpu_float_filter_is_supported(
            &[
                PipelineOp::PutData {
                    data: words([4.0, 3.0, 2.0, 1.0]),
                    mode: PixelMode::I,
                },
                rank,
            ],
            &image,
        ));
    }

    #[test]
    fn f_resize_f64_subnormal_rounding_is_ties_to_even() {
        assert_eq!(gpu_f64_integer_to_f32(1, -149), Some(0x0000_0001));
        assert_eq!(gpu_f64_integer_to_f32(1, -150), Some(0));
        assert_eq!(gpu_f64_integer_to_f32(3, -151), Some(0x0000_0001));
        assert_eq!(gpu_f64_integer_to_f32(3, -150), Some(0x0000_0002));
        assert_eq!(
            gpu_f64_integer_to_f32((1i128 << 23) - 1, -149),
            Some(0x007f_ffff)
        );
        assert_eq!(gpu_f64_integer_to_f32(1i128 << 23, -149), Some(0x0080_0000));
        assert_eq!(gpu_f64_integer_to_f32(-1, -149), Some(0x8000_0001));
        assert_eq!(gpu_f64_integer_to_f32(-1, -200), Some(0x8000_0000));
        assert_eq!(gpu_f64_integer_to_f32(1, 128), Some(0x7f80_0000));
        assert_eq!(gpu_f64_integer_to_f32(-1, 128), Some(0xff80_0000));
    }

    #[test]
    fn f_resize_integer_lowering_proves_signed_arbitrary_significands_and_two_axes() {
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(
                2,
                2,
                [0.1f32, 0.3, 1.7, 2.9]
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect(),
            )
            .unwrap(),
        );
        let one_axis = PipelineOp::Resize {
            w: 1,
            h: 2,
            filter: ResampleFilter::Bilinear,
        };
        assert!(gpu_f_resize_dyadic_is_exact(
            std::slice::from_ref(&one_axis),
            &image,
            Some("F")
        ));

        // The horizontal integer proof materializes the exact rounded f32
        // intermediate before proving the vertical pass.
        assert!(gpu_f_resize_dyadic_is_exact(
            &[PipelineOp::Resize {
                w: 1,
                h: 1,
                filter: ResampleFilter::Bilinear,
            }],
            &image,
            Some("F")
        ));

        let mixed_sign = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(
                2,
                2,
                [0.1f32, -0.3, 1.7, 2.9]
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect(),
            )
            .unwrap(),
        );
        assert!(gpu_f_resize_dyadic_is_exact(
            std::slice::from_ref(&one_axis),
            &mixed_sign,
            Some("F")
        ));
    }

    #[test]
    fn f_resize_dyadic_gpu_bytes_match_reference_and_publish_native_receipt() {
        fn bytes(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        let cases = [
            (
                (1, 2),
                (1, 7),
                ResampleInput::Name("NEAREST".into()),
                vec![1.0f32, 2.0],
                vec![
                    1.0f32.to_bits(),
                    1.0f32.to_bits(),
                    1.0f32.to_bits(),
                    1.0f32.to_bits(),
                    2.0f32.to_bits(),
                    2.0f32.to_bits(),
                    2.0f32.to_bits(),
                ],
            ),
            (
                (1, 2),
                (1, 7),
                ResampleInput::Name("NEAREST".into()),
                vec![(-0.0f32), f32::NAN],
                vec![
                    (-0.0f32).to_bits(),
                    (-0.0f32).to_bits(),
                    (-0.0f32).to_bits(),
                    (-0.0f32).to_bits(),
                    f32::NAN.to_bits(),
                    f32::NAN.to_bits(),
                    f32::NAN.to_bits(),
                ],
            ),
            (
                (2, 2),
                (4, 4),
                ResampleInput::Name("BILINEAR".into()),
                vec![1.0f32, 2.0, 4.0, 8.0],
                vec![
                    0x3f80_0000,
                    0x3fa0_0000,
                    0x3fe0_0000,
                    0x4000_0000,
                    0x3fe0_0000,
                    0x400c_0000,
                    0x4044_0000,
                    0x4060_0000,
                    0x4050_0000,
                    0x4082_0000,
                    0x40b6_0000,
                    0x40d0_0000,
                    0x4080_0000,
                    0x40a0_0000,
                    0x40e0_0000,
                    0x4100_0000,
                ],
            ),
            (
                (2, 2),
                (1, 2),
                ResampleInput::Name("BILINEAR".into()),
                vec![0.1f32, -0.3, 1.7, 2.9],
                vec![0xbdcc_ccce, 0x4013_3334],
            ),
            (
                (8, 1),
                (2, 1),
                ResampleInput::Name("BOX".into()),
                vec![
                    2.0f32.powi(-80),
                    2.0f32.powi(-79),
                    2.0f32.powi(-78),
                    2.0f32.powi(-77),
                    2.0f32.powi(-76),
                    2.0f32.powi(-75),
                    2.0f32.powi(-74),
                    2.0f32.powi(-73),
                ],
                vec![0x1870_0000, 0x1a70_0000],
            ),
            (
                (4, 2),
                (1, 1),
                ResampleInput::Name("BOX".into()),
                vec![1.0f32, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0],
                vec![0x41ff_0000],
            ),
            (
                (4, 2),
                (1, 1),
                ResampleInput::Name("BOX".into()),
                vec![
                    2.0f32.powi(-80),
                    2.0f32.powi(-79),
                    2.0f32.powi(-78),
                    2.0f32.powi(-77),
                    2.0f32.powi(-76),
                    2.0f32.powi(-75),
                    2.0f32.powi(-74),
                    2.0f32.powi(-73),
                ]
                .into_iter()
                .map(|value| -value)
                .collect(),
                vec![0x99ff_0000],
            ),
            (
                (2, 2),
                (1, 1),
                ResampleInput::Name("BICUBIC".into()),
                vec![1.0f32, 2.0, 4.0, 8.0],
                vec![0x4070_0000],
            ),
            (
                (2, 2),
                (1, 1),
                ResampleInput::Name("LANCZOS".into()),
                vec![1.0f32, 2.0, 4.0, 8.0],
                vec![0x4070_0000],
            ),
            (
                (2, 2),
                (1, 1),
                ResampleInput::Name("HAMMING".into()),
                vec![1.0f32, 2.0, 4.0, 8.0],
                vec![0x4070_0000],
            ),
            (
                (4, 1),
                (3, 1),
                ResampleInput::Name("BOX".into()),
                vec![1.0f32, 2.0, 4.0, 8.0],
                vec![0x3f80_0000, 0x4040_0000, 0x4100_0000],
            ),
        ];

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for (source_size, output_size, filter, values, expected_words) in cases {
            let source_bytes = bytes(
                &values
                    .iter()
                    .map(|value| value.to_bits())
                    .collect::<Vec<_>>(),
            );
            let source = Image::frombytes("F", source_size, &source_bytes).expect("F source");
            let actual = match source
                .resize(
                    (i64::from(output_size.0), i64::from(output_size.1)),
                    Some(filter),
                    None,
                )
                .expect("resize operation")
                .use_backend(Backend::Gpu)
                .tobytes()
            {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU F resize failed: {error}"),
            };
            assert_eq!(actual, bytes(&expected_words));
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("native GPU resize must publish a telemetry receipt");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.6, Some(2));
            assert_eq!(telemetry.7, None);
        }

        // Two sequential 2:1 Box reductions exercise the chain proof: the
        // first pass stores non-power-of-two f32 averages, and the second
        // pass must still match Pillow's f64 accumulation and f32 store.
        let chain_values = [1.0f32, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0];
        let chain_source = Image::frombytes("F", (4, 2), &bytes(&chain_values.map(f32::to_bits)))
            .expect("F chain source");
        let chain_actual = match chain_source
            .resize((2, 1), Some(ResampleInput::Name("BOX".into())), None)
            .expect("first Box resize")
            .resize((1, 1), Some(ResampleInput::Name("BOX".into())), None)
            .expect("second Box resize")
            .use_backend(Backend::Gpu)
            .tobytes()
        {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU F Box chain failed: {error}"),
        };
        assert_eq!(chain_actual, bytes(&[0x41ff_0000]));
        let chain_telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU F Box chain must publish a telemetry receipt");
        assert_eq!(chain_telemetry.0, Some(Backend::Gpu));
        assert_eq!(chain_telemetry.1, Backend::Gpu);
        assert_eq!(chain_telemetry.6, Some(4));
        assert_eq!(chain_telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_box_copy_native_preserves_special_words() {
        let words: [u32; 7] = [
            0x7fc0_1234, // quiet NaN with a payload
            0xffc0_1234, // negative NaN with a payload
            0x7f80_0000, // positive infinity
            0xff80_0000, // negative infinity
            0x8000_0000, // negative zero
            0x0000_0000, // positive zero
            0x3fc0_0000, // finite control word
        ];
        let source_bytes: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
        let source = Image::frombytes("F", (7, 1), &source_bytes).expect("F source");
        let expected_words: Vec<u32> = words
            .iter()
            .flat_map(|word| {
                let copied = if *word == 0x8000_0000 { 0 } else { *word };
                [copied, copied]
            })
            .chain(words.iter().flat_map(|word| {
                let copied = if *word == 0x8000_0000 { 0 } else { *word };
                [copied, copied]
            }))
            .collect();
        let expected: Vec<u8> = expected_words
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect();
        let cpu = source
            .resize((14, 2), Some(ResampleInput::Name("BOX".into())), None)
            .expect("CPU resize operation")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU special-word resize");
        assert_eq!(cpu, expected, "Pillow Box copy special words");

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match source
            .resize((14, 2), Some(ResampleInput::Name("BOX".into())), None)
            .expect("GPU resize operation")
            .use_backend(Backend::Gpu)
            .tobytes()
        {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU special-word Box resize failed: {error}"),
        };
        assert_eq!(actual, expected, "native GPU Box copy special words");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU special-word Box resize must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(2));
        assert_eq!(telemetry.7, None);

        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_integer_emulation_matches_cpu_for_heterogeneous_significands() {
        let values = [0.1f32, 0.3, 1.7, 2.9];
        let source_bytes: Vec<u8> = values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let source = Image::frombytes("F", (2, 2), &source_bytes).expect("F source");
        let filters = ["BILINEAR", "BICUBIC"];
        let sizes = [(1i64, 2i64), (2, 1)];
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for filter in filters {
            for size in sizes {
                let expected = source
                    .resize(size, Some(ResampleInput::Name(filter.into())), None)
                    .expect("CPU resize operation")
                    .use_backend(Backend::Cpu)
                    .tobytes()
                    .expect("CPU F resize");
                let actual = match source
                    .resize(size, Some(ResampleInput::Name(filter.into())), None)
                    .expect("GPU resize operation")
                    .use_backend(Backend::Gpu)
                    .tobytes()
                {
                    Ok(actual) => actual,
                    Err(error)
                        if error.to_string().contains("GPU adapter not available")
                            || error
                                .to_string()
                                .contains("GPU device initialization failed") =>
                    {
                        Backend::set_pipeline_telemetry_enabled(previous);
                        return;
                    }
                    Err(error) => panic!("native GPU F {filter} resize failed: {error}"),
                };
                assert_eq!(actual, expected, "F {filter} integer emulation parity");
                let telemetry = Backend::take_pipeline_telemetry()
                    .expect("native GPU F resize must publish a telemetry receipt");
                assert_eq!(telemetry.0, Some(Backend::Gpu));
                assert_eq!(telemetry.1, Backend::Gpu);
                assert_eq!(telemetry.6, Some(2));
                assert_eq!(telemetry.7, None);
            }
        }

        // Exercise the signed accumulator and the second f32 boundary in a
        // single native two-axis Bilinear dispatch.
        let signed_values = [0.1f32, -0.3, 1.7, 2.9];
        let signed_bytes: Vec<u8> = signed_values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let signed_source = Image::frombytes("F", (2, 2), &signed_bytes).expect("F source");
        let expected = signed_source
            .resize((1, 1), Some(ResampleInput::Name("BILINEAR".into())), None)
            .expect("CPU two-axis resize operation")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU two-axis F resize");
        let actual = match signed_source
            .resize((1, 1), Some(ResampleInput::Name("BILINEAR".into())), None)
            .expect("GPU two-axis resize operation")
            .use_backend(Backend::Gpu)
            .tobytes()
        {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU signed two-axis F resize failed: {error}"),
        };
        assert_eq!(actual, expected, "signed two-axis F resize parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU signed two-axis F resize must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(2));
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_f64_two_axis_native_non_dyadic_matches_cpu() {
        let values = [0.1f32, -0.3, 1.7, 2.9];
        let source_bytes: Vec<u8> = values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let source = Image::frombytes("F", (2, 2), &source_bytes).expect("F source");
        let expected = source
            .resize((1, 5), Some(ResampleInput::Name("BILINEAR".into())), None)
            .expect("CPU resize operation")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU F resize");
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match source
            .resize((1, 5), Some(ResampleInput::Name("BILINEAR".into())), None)
            .expect("GPU resize operation")
            .use_backend(Backend::Gpu)
            .tobytes()
        {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU F two-axis resize failed: {error}"),
        };
        assert_eq!(actual, expected, "non-dyadic two-axis F resize parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU F two-axis resize must publish a telemetry receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(2));
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_f64_native_mixed_upscale_downscale_matches_cpu() {
        let values = [1.0f32, 2.0, 4.0, 8.0];
        let source_bytes: Vec<u8> = values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let source = Image::frombytes("F", (2, 2), &source_bytes).expect("F source");
        let expected = source
            .resize((4, 1), Some(ResampleInput::Name("BILINEAR".into())), None)
            .expect("CPU mixed-geometry resize operation")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU mixed-geometry F resize");

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match source
            .resize((4, 1), Some(ResampleInput::Name("BILINEAR".into())), None)
            .expect("GPU mixed-geometry resize operation")
            .use_backend(Backend::Gpu)
            .tobytes()
        {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU mixed-geometry F resize failed: {error}"),
        };
        assert_eq!(actual, expected, "mixed-axis F resize parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU mixed-axis F resize must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(2));
        assert_eq!(telemetry.7, None);

        for filter in ["BICUBIC", "LANCZOS", "HAMMING"] {
            let expected = source
                .resize((4, 1), Some(ResampleInput::Name(filter.into())), None)
                .expect("CPU mixed-geometry filtered resize operation")
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU mixed-geometry filtered F resize");
            let actual = source
                .resize((4, 1), Some(ResampleInput::Name(filter.into())), None)
                .expect("GPU mixed-geometry filtered resize operation")
                .use_backend(Backend::Gpu)
                .tobytes()
                .expect("native GPU mixed-geometry filtered F resize");
            assert_eq!(actual, expected, "mixed-axis {filter} F resize parity");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("native GPU mixed-axis filtered F resize must publish a receipt");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.6, Some(2));
            assert_eq!(telemetry.7, None);
        }

        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_f64_native_preserves_proven_overflow_outputs() {
        fn bytes(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        let source_words = [0x7f7f_ffff, 0x7f7f_ffff, 0xff7f_ffff];
        let source = Image::frombytes("F", (3, 1), &bytes(&source_words)).expect("F source");
        let cases = [
            (
                ResampleInput::Name("BICUBIC".into()),
                [0x7f80_0000, 0xfe99_aea3],
            ),
            (
                ResampleInput::Name("LANCZOS".into()),
                [0x7f80_0000, 0xfe9d_8858],
            ),
            (
                ResampleInput::Name("HAMMING".into()),
                [0x7f7f_ffff, 0xfee4_133b],
            ),
        ];

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for (filter, expected_words) in cases {
            let expected = source
                .resize((2, 1), Some(filter.clone()), None)
                .expect("CPU overflow resize operation")
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU overflow F resize");
            assert_eq!(expected, bytes(&expected_words));

            let actual = match source
                .resize((2, 1), Some(filter), None)
                .expect("GPU overflow resize operation")
                .use_backend(Backend::Gpu)
                .tobytes()
            {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU overflow F resize failed: {error}"),
            };
            assert_eq!(actual, expected, "native GPU overflow F resize");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("native GPU overflow F resize must publish a receipt");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.7, None);
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_ordered_f64_two_tap_native_matches_cpu() {
        // The marker-9 exact-real reducer rejects this row because the tiny
        // second product is below the first product's f64 ulp.  Pillow rounds
        // the accumulator after the first FMA, then performs the second FMA;
        // marker 12 reproduces that ordered boundary without relaxed device
        // floats, including wider rows up to the bounded tap cap.
        let source_bytes = [
            44, 149, 220, 51, // 1.0271683e-7
            155, 14, 222, 56, // 1.0588505e-4
            62, 115, 42, 56, // 4.0638486e-5
        ];
        let source = Image::frombytes("F", (3, 1), &source_bytes).expect("F source");
        let source_dynamic = source.materialize().expect("materialize F source");
        let op = PipelineOp::Resize {
            w: 2,
            h: 1,
            filter: ResampleFilter::Bilinear,
        };
        assert!(!gpu_f_resize_f64_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        assert!(gpu_f_resize_f64_ordered_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        let expected = source
            .resize((2, 1), Some(ResampleInput::Name("BILINEAR".into())), None)
            .expect("CPU ordered-f64 resize")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU F resize bytes");

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match source
            .resize((2, 1), Some(ResampleInput::Name("BILINEAR".into())), None)
            .expect("GPU ordered-f64 resize")
            .use_backend(Backend::Gpu)
            .tobytes()
        {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU ordered-f64 resize failed: {error}"),
        };
        assert_eq!(actual, expected, "ordered f64 two-tap F resize");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("ordered-f64 F resize must publish a telemetry receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(2));
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_ordered_f64_wider_tap_native_matches_cpu() {
        fn bytes(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        // A three-tap Lanczos upscale is the first wider row that exposed the
        // old marker-12 count guard. Pillow still rounds each ordered f64 FMA
        // before the final f32 store; the bounded integer reducer now follows
        // that sequence for this finite heterogeneous row.
        let source = Image::frombytes(
            "F",
            (3, 1),
            &bytes(&[0x3fc3_2777, 0xbeb0_5e6a, 0x44db_fac1]),
        )
        .expect("F source");
        let source_dynamic = source.materialize().expect("materialize F source");
        let op = PipelineOp::Resize {
            w: 5,
            h: 1,
            filter: ResampleFilter::Lanczos,
        };
        assert!(!gpu_f_resize_f64_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        assert!(gpu_f_resize_f64_ordered_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        let filter = ResampleInput::Name("LANCZOS".into());
        let expected = source
            .resize((5, 1), Some(filter.clone()), None)
            .expect("CPU wider ordered-f64 resize")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU F resize bytes");

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match source
            .resize((5, 1), Some(filter), None)
            .expect("GPU wider ordered-f64 resize")
            .use_backend(Backend::Gpu)
            .tobytes()
        {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU wider ordered-f64 resize failed: {error}"),
        };
        assert_eq!(actual, expected, "wider ordered f64 F resize");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("wider ordered-f64 F resize must publish a telemetry receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(2));
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_arm64_wide_horizontal_vector_native_matches_pillow() {
        fn bytes(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        // Pillow 12.2.0's arm64 FLOAT32 horizontal kernel changes from the
        // scalar FMA loop to separate product/ordered-add accumulation after
        // 15 taps. These rows were the first deterministic divergences when
        // the host and shader admitted every finite row through the FMA model:
        // Pillow produced c8be3d3d and bbc8afba, while that model produced
        // c9be3d3d and b9afc8bb respectively.
        let cases = [(16usize, 91u32, 0x3d3d_bec8), (32, 275, 0xbbc8_afba)];
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for (width, trial, expected_word) in cases {
            let mut seed = trial.wrapping_mul(0x9e37_79b9).wrapping_add(width as u32);
            let words: Vec<u32> = (0..width)
                .map(|_| {
                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    (125 << 23) | (seed & 0x8000_0000) | (seed & 0x007f_ffff)
                })
                .collect();
            let source_bytes = bytes(&words);
            let source =
                Image::frombytes("F", (width as u32, 1), &source_bytes).expect("wide F source");
            let source_dynamic = source.materialize().expect("materialize wide F source");
            let op = PipelineOp::Resize {
                w: 1,
                h: 1,
                filter: ResampleFilter::Bilinear,
            };
            assert!(!gpu_f_resize_f64_is_exact(
                std::slice::from_ref(&op),
                &source_dynamic,
                Some("F")
            ));
            assert!(gpu_f_resize_f64_ordered_is_exact(
                std::slice::from_ref(&op),
                &source_dynamic,
                Some("F")
            ));
            let expected = bytes(&[expected_word]);
            let cpu = source
                .resize((1, 1), Some(ResampleInput::Name("BILINEAR".into())), None)
                .expect("CPU wide F resize operation")
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU wide F resize");
            assert_eq!(cpu, expected, "CPU wide F resize at width {width}");

            let actual = match source
                .resize((1, 1), Some(ResampleInput::Name("BILINEAR".into())), None)
                .expect("GPU wide F resize operation")
                .use_backend(Backend::Gpu)
                .tobytes()
            {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU wide F resize failed: {error}"),
            };
            assert_eq!(actual, expected, "GPU wide F resize at width {width}");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("wide F resize must publish a telemetry receipt");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.6, Some(2));
            assert_eq!(telemetry.7, None);
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_wide_box_overflow_stays_host_controlled() {
        fn bytes(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        // A 16-tap Box row has a max-normal word followed by ordinary values.
        // Its exact average is 0x7d7fffff; any proof that drops the high
        // exponent term can incorrectly produce the small-value average.
        let source = Image::frombytes(
            "F",
            (16, 1),
            &bytes(
                &[0x7f7f_ffff]
                    .into_iter()
                    .chain([0x3f80_0000; 15])
                    .collect::<Vec<_>>(),
            ),
        )
        .expect("wide Box F source");
        let source_dynamic = source.materialize().expect("materialize wide Box F source");
        let op = PipelineOp::Resize {
            w: 1,
            h: 1,
            filter: ResampleFilter::Box,
        };
        assert!(!gpu_f_resize_integer_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        assert!(!gpu_f_resize_dyadic_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        assert!(!gpu_f_resize_f64_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));

        let expected = bytes(&[0x7d7f_ffff]);
        let cpu = source
            .resize((1, 1), Some(ResampleInput::Name("BOX".into())), None)
            .expect("CPU wide Box resize operation")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU wide Box resize");
        assert_eq!(cpu, expected);

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match source
            .resize((1, 1), Some(ResampleInput::Name("BOX".into())), None)
            .expect("GPU wide Box resize operation")
            .use_backend(Backend::Gpu)
            .tobytes()
        {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU wide Box resize failed: {error}"),
        };
        assert_eq!(actual, expected);
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("wide Box host-control resize must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Cpu);
        assert_eq!(telemetry.7.as_deref(), Some("exact host semantic control"));
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_f64_wide_lanczos_cancellation_native_matches_pillow() {
        fn bytes(values: &[f32]) -> Vec<u8> {
            values
                .iter()
                .flat_map(|value| value.to_le_bytes())
                .collect()
        }

        // This 48-tap Lanczos row was the first proven failure above the old
        // marker-12 cap: marker 9's exact-real GPU coefficients cancelled the
        // middle result to +0.0, while Pillow's src/libImaging/Resample.c
        // arm64 ordered reduction stores -2.0. The ordered reducer now models
        // that wider row after using the native multiply/divide order for the
        // Lanczos coefficient.
        let values = [2f32.powi(60), -2f32.powi(60)]
            .into_iter()
            .cycle()
            .take(48)
            .collect::<Vec<_>>();
        let source = Image::frombytes("F", (48, 1), &bytes(&values)).expect("wide F source");
        let source_dynamic = source.materialize().expect("materialize wide F source");
        let op = PipelineOp::Resize {
            w: 3,
            h: 1,
            filter: ResampleFilter::Lanczos,
        };
        assert!(!gpu_f_resize_f64_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F"),
        ));
        assert!(gpu_f_resize_f64_ordered_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F"),
        ));

        let expected = bytes(&[
            f32::from_bits(0x5aa1_ab41),
            f32::from_bits(0xc000_0000),
            f32::from_bits(0xdaa1_ab41),
        ]);
        let cpu = source
            .resize((3, 1), Some(ResampleInput::Name("LANCZOS".into())), None)
            .expect("CPU wide Lanczos resize operation")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU wide Lanczos resize");
        assert_eq!(cpu, expected);

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match source
            .resize((3, 1), Some(ResampleInput::Name("LANCZOS".into())), None)
            .expect("GPU wide Lanczos resize operation")
            .use_backend(Backend::Gpu)
            .tobytes()
        {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU wide Lanczos resize failed: {error}"),
        };
        assert_eq!(actual, expected, "wide Lanczos native parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("wide Lanczos native resize must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_f64_native_preserves_proven_negative_zero_output() {
        fn bytes(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        // Pillow's ordered f64 Box accumulation leaves a tiny negative
        // residual here; the final f32 store underflows it to signed zero.
        // The exact marker-9 reducer must preserve that sign instead of
        // routing the row back to host control.
        let source_words = [
            f32::from_bits(0x8000_0001).to_bits(),
            0.0f32.to_bits(),
            0.0f32.to_bits(),
        ];
        let source = Image::frombytes("F", (1, 3), &bytes(&source_words)).expect("F source");
        let filter = ResampleInput::Name("BOX".into());
        let source_dynamic = source.materialize().expect("materialize F source");
        assert!(gpu_f_resize_f64_is_exact(
            &[PipelineOp::Resize {
                w: 1,
                h: 1,
                filter: ResampleFilter::Box,
            }],
            &source_dynamic,
            Some("F")
        ));
        let expected = source
            .resize((1, 1), Some(filter.clone()), None)
            .expect("CPU signed-zero resize operation")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU signed-zero F resize");
        assert_eq!(expected, bytes(&[0x8000_0000]));

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match source
            .resize((1, 1), Some(filter), None)
            .expect("GPU signed-zero resize operation")
            .use_backend(Backend::Gpu)
            .tobytes()
        {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU signed-zero F resize failed: {error}"),
        };
        assert_eq!(actual, expected, "native GPU signed-zero F resize");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU signed-zero F resize must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_f64_native_preserves_special_value_outputs() {
        fn bytes(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        // Marker 9 handles special values in the coefficient reducer without
        // relaxed-f32 arithmetic.  Include a signaling NaN to verify the
        // Pillow/C quieting rule, infinities with both signs, and an
        // opposite-infinity pair that must become the canonical quiet NaN.
        let cases = [
            (
                (2u32, 2u32),
                (1i64, 2i64),
                ResampleInput::Name("BILINEAR".into()),
                vec![0x7fa0_0001, 0x3f80_0000, 0x4000_0000, 0x4040_0000],
            ),
            (
                (2, 2),
                (1, 2),
                ResampleInput::Name("BILINEAR".into()),
                vec![0x7f80_0000, 0x3f80_0000, 0x4000_0000, 0x4040_0000],
            ),
            (
                (2, 2),
                (1, 2),
                ResampleInput::Name("BILINEAR".into()),
                vec![0xff80_0000, 0x3f80_0000, 0x4000_0000, 0x4040_0000],
            ),
            (
                (3, 1),
                (2, 1),
                ResampleInput::Name("BICUBIC".into()),
                vec![0x7f80_0000, 0x3f80_0000, 0xff80_0000],
            ),
            (
                (3, 1),
                (2, 1),
                ResampleInput::Name("BICUBIC".into()),
                vec![0x7fc1_2345, 0x3f80_0000, 0x3f80_0000],
            ),
            (
                (2, 1),
                (6, 1),
                ResampleInput::Name("BOX".into()),
                vec![0x7fa0_0001, 0x3f80_0000],
            ),
            (
                (2, 1),
                (6, 1),
                ResampleInput::Name("BICUBIC".into()),
                vec![0x7f80_0000, 0x3f80_0000],
            ),
        ];

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for (source_dimensions, destination, filter, words) in cases {
            let source = Image::frombytes("F", source_dimensions, &bytes(&words))
                .expect("F special-value source");
            let expected = source
                .resize(destination, Some(filter.clone()), None)
                .expect("CPU special-value resize operation")
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU special-value F resize");
            let actual = match source
                .resize(destination, Some(filter), None)
                .expect("GPU special-value resize operation")
                .use_backend(Backend::Gpu)
                .tobytes()
            {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU special-value F resize failed: {error}"),
            };
            assert_eq!(actual, expected, "native GPU special-value F resize");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("native GPU special-value F resize must publish a receipt");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.7, None);
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_f64_wide_special_value_outputs_native() {
        fn bytes(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        // Marker 12 deliberately stops at 8388607 taps. Marker 9's special
        // prepass is independent of the arm64 vector product/add split, so a
        // 257-tap row can still be native when the host proof agrees on the
        // exact NaN or infinity bits and its encoded table fits one binding.
        // Finite rows use marker 12 when its ordered proof is representable.
        let cases = [
            (
                ResampleFilter::Bilinear,
                0x7fa1_2345u32,
                257u32,
                1u32,
                0usize,
            ),
            (ResampleFilter::Bicubic, 0x7fc2_3456, 257, 1, 0),
            (ResampleFilter::Lanczos, 0x7f80_0000, 257, 1, 0),
            (ResampleFilter::Hamming, 0xff80_0000, 257, 1, 0),
            (ResampleFilter::Box, 0x7f80_0000, 1, 257, 128),
        ];
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for (filter, special, source_w, source_h, special_index) in cases {
            let mut words = vec![0x3f80_0000; (source_w * source_h) as usize];
            words[special_index] = special;
            let source = Image::frombytes("F", (source_w, source_h), &bytes(&words))
                .expect("wide special F source");
            let source_dynamic = source
                .materialize()
                .expect("materialize wide special F source");
            let op = PipelineOp::Resize { w: 1, h: 1, filter };
            assert!(gpu_f_resize_f64_is_exact(
                std::slice::from_ref(&op),
                &source_dynamic,
                Some("F")
            ));
            let filter_name = match filter {
                ResampleFilter::Bilinear => "BILINEAR",
                ResampleFilter::Bicubic => "BICUBIC",
                ResampleFilter::Lanczos => "LANCZOS",
                ResampleFilter::Hamming => "HAMMING",
                ResampleFilter::Box => "BOX",
                _ => unreachable!(),
            };
            let expected = source
                .resize((1, 1), Some(ResampleInput::Name(filter_name.into())), None)
                .expect("CPU wide special F resize")
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU wide special F bytes");
            let actual = match source
                .resize((1, 1), Some(ResampleInput::Name(filter_name.into())), None)
                .expect("GPU wide special F resize")
                .use_backend(Backend::Gpu)
                .tobytes()
            {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU wide special F resize failed: {error}"),
            };
            assert_eq!(actual, expected, "wide special {filter_name} F resize");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("wide special F resize must publish telemetry");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.7, None);
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_ordered_f64_over_64_taps_native_matches_cpu() {
        fn bytes(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        // The ordered reducer is a dynamic tap loop; exercise rows larger
        // than the former policy cap with heterogeneous finite values. The
        // host proof must still reject any state that cannot be represented,
        // so this test only asserts native execution for rows it certifies.
        for (width, filter) in [
            (65usize, ResampleFilter::Bilinear),
            (96usize, ResampleFilter::Bicubic),
            (128usize, ResampleFilter::Lanczos),
            (96usize, ResampleFilter::Hamming),
            (128usize, ResampleFilter::Box),
        ] {
            let words: Vec<u32> = (0..width)
                .map(|index| {
                    let value = 0.5f32 + ((index * 37 % 100) as f32) * 0.01f32;
                    value.to_bits()
                })
                .collect();
            let source =
                Image::frombytes("F", (width as u32, 1), &bytes(&words)).expect("wide F source");
            let source_dynamic = source.materialize().expect("materialize wide F source");
            let op = PipelineOp::Resize { w: 1, h: 1, filter };
            assert!(
                gpu_f_resize_f64_ordered_is_exact(
                    std::slice::from_ref(&op),
                    &source_dynamic,
                    Some("F")
                ),
                "ordered proof should cover {width}-tap {filter:?} row"
            );
            let filter_name = match filter {
                ResampleFilter::Bilinear => "BILINEAR",
                ResampleFilter::Bicubic => "BICUBIC",
                ResampleFilter::Lanczos => "LANCZOS",
                ResampleFilter::Hamming => "HAMMING",
                ResampleFilter::Box => "BOX",
                _ => unreachable!(),
            };
            let expected = source
                .resize((1, 1), Some(ResampleInput::Name(filter_name.into())), None)
                .expect("CPU wide F resize")
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU wide F bytes");
            let previous = Backend::set_pipeline_telemetry_enabled(true);
            let actual = match source
                .resize((1, 1), Some(ResampleInput::Name(filter_name.into())), None)
                .expect("GPU wide F resize")
                .use_backend(Backend::Gpu)
                .tobytes()
            {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU wide F resize failed: {error}"),
            };
            assert_eq!(actual, expected, "wide {filter_name} F resize");
            let telemetry =
                Backend::take_pipeline_telemetry().expect("wide F resize must publish telemetry");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.7, None);
            Backend::set_pipeline_telemetry_enabled(previous);
        }
    }

    #[test]
    fn f_resize_ordered_f64_two_axes_over_64_taps_native_matches_cpu() {
        fn bytes(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        let width = 65usize;
        let height = 65usize;
        let words: Vec<u32> = (0..width * height)
            .map(|index| {
                let value = 0.75f32 + ((index * 17 % 80) as f32) * 0.01f32;
                value.to_bits()
            })
            .collect();
        let source = Image::frombytes("F", (width as u32, height as u32), &bytes(&words))
            .expect("wide two-axis F source");
        let source_dynamic = source
            .materialize()
            .expect("materialize wide two-axis F source");
        let op = PipelineOp::Resize {
            w: 1,
            h: 1,
            filter: ResampleFilter::Bilinear,
        };
        assert!(gpu_f_resize_f64_ordered_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        let filter = ResampleInput::Name("BILINEAR".into());
        let expected = source
            .resize((1, 1), Some(filter.clone()), None)
            .expect("CPU wide two-axis F resize")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU wide two-axis F bytes");
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match source
            .resize((1, 1), Some(filter), None)
            .expect("GPU wide two-axis F resize")
            .use_backend(Backend::Gpu)
            .tobytes()
        {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU wide two-axis F resize failed: {error}"),
        };
        assert_eq!(actual, expected, "wide two-axis F resize");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("wide two-axis F resize must publish telemetry");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_ordered_f64_through_8388607_taps_native_matches_cpu() {
        fn bytes(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        for (width, filter) in [
            (129usize, ResampleFilter::Bilinear),
            (192usize, ResampleFilter::Bicubic),
            (256usize, ResampleFilter::Lanczos),
            (192usize, ResampleFilter::Hamming),
            (256usize, ResampleFilter::Box),
            (384usize, ResampleFilter::Bilinear),
            (512usize, ResampleFilter::Lanczos),
            (768usize, ResampleFilter::Bicubic),
            (1024usize, ResampleFilter::Box),
            (1536usize, ResampleFilter::Hamming),
            (2048usize, ResampleFilter::Bilinear),
            (3072usize, ResampleFilter::Lanczos),
            (4096usize, ResampleFilter::Box),
            (8192usize, ResampleFilter::Bilinear),
            (8192usize, ResampleFilter::Bicubic),
            (8192usize, ResampleFilter::Lanczos),
            (8192usize, ResampleFilter::Hamming),
            (8192usize, ResampleFilter::Box),
            (16384usize, ResampleFilter::Bilinear),
            (16384usize, ResampleFilter::Bicubic),
            (16384usize, ResampleFilter::Lanczos),
            (16384usize, ResampleFilter::Hamming),
            (16384usize, ResampleFilter::Box),
            (32768usize, ResampleFilter::Bilinear),
            (32768usize, ResampleFilter::Bicubic),
            (32768usize, ResampleFilter::Lanczos),
            (32768usize, ResampleFilter::Hamming),
            (32768usize, ResampleFilter::Box),
            (32769usize, ResampleFilter::Bilinear),
            (32769usize, ResampleFilter::Bicubic),
            (32769usize, ResampleFilter::Lanczos),
            (32769usize, ResampleFilter::Hamming),
            (32769usize, ResampleFilter::Box),
            (65536usize, ResampleFilter::Bilinear),
            (65536usize, ResampleFilter::Bicubic),
            (65536usize, ResampleFilter::Lanczos),
            (65536usize, ResampleFilter::Hamming),
            (65536usize, ResampleFilter::Box),
            (131072usize, ResampleFilter::Bilinear),
            (131072usize, ResampleFilter::Bicubic),
            (131072usize, ResampleFilter::Lanczos),
            (131072usize, ResampleFilter::Hamming),
            (131072usize, ResampleFilter::Box),
            (262144usize, ResampleFilter::Bilinear),
            (262144usize, ResampleFilter::Bicubic),
            (262144usize, ResampleFilter::Lanczos),
            (262144usize, ResampleFilter::Hamming),
            (262144usize, ResampleFilter::Box),
            (524288usize, ResampleFilter::Bilinear),
            (524288usize, ResampleFilter::Bicubic),
            (524288usize, ResampleFilter::Lanczos),
            (524288usize, ResampleFilter::Hamming),
            (524288usize, ResampleFilter::Box),
            (1_048_576usize, ResampleFilter::Bilinear),
            (1_048_576usize, ResampleFilter::Bicubic),
            (1_048_576usize, ResampleFilter::Lanczos),
            (1_048_576usize, ResampleFilter::Hamming),
            (1_048_576usize, ResampleFilter::Box),
            (2_097_152usize, ResampleFilter::Bilinear),
            (2_097_152usize, ResampleFilter::Bicubic),
            (2_097_152usize, ResampleFilter::Lanczos),
            (2_097_152usize, ResampleFilter::Hamming),
            (2_097_152usize, ResampleFilter::Box),
            (4_194_304usize, ResampleFilter::Bilinear),
            (4_194_304usize, ResampleFilter::Bicubic),
            (4_194_304usize, ResampleFilter::Lanczos),
            (4_194_304usize, ResampleFilter::Hamming),
            (4_194_304usize, ResampleFilter::Box),
            (8_388_607usize, ResampleFilter::Bilinear),
        ] {
            let words: Vec<u32> = (0..width)
                .map(|index| {
                    let value = 0.25f32 + ((index * 29 % 120) as f32) * 0.01f32;
                    value.to_bits()
                })
                .collect();
            let source = Image::frombytes("F", (width as u32, 1), &bytes(&words))
                .expect("wide ordered F source");
            let source_dynamic = source
                .materialize()
                .expect("materialize wide ordered source");
            let op = PipelineOp::Resize { w: 1, h: 1, filter };
            assert!(
                gpu_f_resize_f64_ordered_is_exact(
                    std::slice::from_ref(&op),
                    &source_dynamic,
                    Some("F")
                ),
                "ordered proof should cover {width}-tap {filter:?} row"
            );
            let filter_name = match filter {
                ResampleFilter::Bilinear => "BILINEAR",
                ResampleFilter::Bicubic => "BICUBIC",
                ResampleFilter::Lanczos => "LANCZOS",
                ResampleFilter::Hamming => "HAMMING",
                ResampleFilter::Box => "BOX",
                _ => unreachable!(),
            };
            let expected = source
                .resize((1, 1), Some(ResampleInput::Name(filter_name.into())), None)
                .expect("CPU wide ordered F resize")
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU wide ordered F bytes");
            let previous = Backend::set_pipeline_telemetry_enabled(true);
            let actual = match source
                .resize((1, 1), Some(ResampleInput::Name(filter_name.into())), None)
                .expect("GPU wide ordered F resize")
                .use_backend(Backend::Gpu)
                .tobytes()
            {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU wide ordered F resize failed: {error}"),
            };
            assert_eq!(actual, expected, "{width}-tap {filter_name} F resize");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("wide ordered F resize must publish telemetry");
            assert_eq!(telemetry.0, Some(Backend::Gpu), "{width}-tap {filter_name}");
            assert_eq!(telemetry.1, Backend::Gpu, "{width}-tap {filter_name}");
            assert_eq!(telemetry.7, None, "{width}-tap {filter_name}");
            Backend::set_pipeline_telemetry_enabled(previous);
        }
    }

    #[test]
    fn f_resize_ordered_f64_2097152_two_axis_native_matches_cpu() {
        fn bytes(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        let width = 2_097_152usize;
        let height = 2usize;
        let words: Vec<u32> = (0..width * height)
            .map(|index| {
                let value = 0.25f32 + ((index * 37 % 180) as f32) * 0.005f32;
                value.to_bits()
            })
            .collect();
        let source = Image::frombytes("F", (width as u32, height as u32), &bytes(&words))
            .expect("wide two-axis ordered F source");
        let source_dynamic = source
            .materialize()
            .expect("materialize wide two-axis ordered source");
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for filter in [
            ResampleFilter::Bilinear,
            ResampleFilter::Bicubic,
            ResampleFilter::Lanczos,
            ResampleFilter::Hamming,
            ResampleFilter::Box,
        ] {
            let op = PipelineOp::Resize { w: 1, h: 1, filter };
            assert!(
                gpu_f_resize_f64_ordered_is_exact(
                    std::slice::from_ref(&op),
                    &source_dynamic,
                    Some("F")
                ),
                "ordered proof should cover two-axis {filter:?} resize"
            );
            let filter_name = match filter {
                ResampleFilter::Bilinear => "BILINEAR",
                ResampleFilter::Bicubic => "BICUBIC",
                ResampleFilter::Lanczos => "LANCZOS",
                ResampleFilter::Hamming => "HAMMING",
                ResampleFilter::Box => "BOX",
                _ => unreachable!(),
            };
            let expected = source
                .resize((1, 1), Some(ResampleInput::Name(filter_name.into())), None)
                .expect("CPU wide two-axis ordered F resize")
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU wide two-axis ordered F bytes");
            let actual = source
                .resize((1, 1), Some(ResampleInput::Name(filter_name.into())), None)
                .expect("GPU wide two-axis ordered F resize")
                .use_backend(Backend::Gpu)
                .tobytes()
                .expect("GPU wide two-axis ordered F bytes");
            assert_eq!(
                actual, expected,
                "2097152-tap two-axis {filter_name} resize"
            );
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("wide two-axis ordered F resize must publish telemetry");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.6, Some(2));
            assert_eq!(telemetry.7, None);
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_ordered_f64_524289_taps_native_matches_cpu() {
        fn bytes(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        let width = 524289usize;
        let words: Vec<u32> = (0..width)
            .map(|index| (0.5f32 + ((index * 13 % 90) as f32) * 0.01f32).to_bits())
            .collect();
        let source =
            Image::frombytes("F", (width as u32, 1), &bytes(&words)).expect("over-bound F source");
        let source_dynamic = source
            .materialize()
            .expect("materialize over-bound F source");
        let op = PipelineOp::Resize {
            w: 1,
            h: 1,
            filter: ResampleFilter::Bilinear,
        };
        assert!(gpu_f_resize_f64_ordered_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        assert!(!gpu_f_resize_f64_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        let filter = ResampleInput::Name("BILINEAR".into());
        let expected = source
            .resize((1, 1), Some(filter.clone()), None)
            .expect("CPU over-bound F resize")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU over-bound F bytes");
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = source
            .resize((1, 1), Some(filter), None)
            .expect("GPU over-bound F resize")
            .use_backend(Backend::Gpu)
            .tobytes()
            .expect("native 524289-tap F resize");
        assert_eq!(actual, expected);
        let telemetry =
            Backend::take_pipeline_telemetry().expect("524289-tap F resize must publish telemetry");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_ordered_f64_over_8388607_taps_stays_host_controlled() {
        fn bytes(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        let width = 8_388_608usize;
        let words: Vec<u32> = (0..width)
            .map(|index| (0.5f32 + ((index * 13 % 90) as f32) * 0.01f32).to_bits())
            .collect();
        let source =
            Image::frombytes("F", (width as u32, 1), &bytes(&words)).expect("over-bound F source");
        let source_dynamic = source
            .materialize()
            .expect("materialize over-bound F source");
        let op = PipelineOp::Resize {
            w: 1,
            h: 1,
            filter: ResampleFilter::Bilinear,
        };
        assert!(!gpu_f_resize_f64_ordered_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        assert!(!gpu_f_resize_f64_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        let filter = ResampleInput::Name("BILINEAR".into());
        let expected = source
            .resize((1, 1), Some(filter.clone()), None)
            .expect("CPU over-bound F resize")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU over-bound F bytes");
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = source
            .resize((1, 1), Some(filter), None)
            .expect("GPU over-bound F resize")
            .use_backend(Backend::Gpu)
            .tobytes()
            .expect("host-controlled over-bound F resize");
        assert_eq!(actual, expected);
        let telemetry =
            Backend::take_pipeline_telemetry().expect("over-bound F resize must publish telemetry");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Cpu);
        assert_eq!(telemetry.7.as_deref(), Some("exact host semantic control"));
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_compact_box_proof_covers_horizontal_over_binding_row() {
        fn bytes(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        let height = 8_388_608usize;
        let pattern = [0.125f32, -17.25, 1.0, -2.5, 3.75, -0.0, 100.0, -99.0];
        let words: Vec<u32> = (0..height)
            .map(|index| pattern[index % pattern.len()].to_bits())
            .collect();
        let source = Image::frombytes("F", (height as u32, 1), &bytes(&words))
            .expect("horizontal compact Box source");
        let source_dynamic = source
            .materialize()
            .expect("materialize horizontal compact Box source");
        let op = PipelineOp::Resize {
            w: 1,
            h: 1,
            filter: ResampleFilter::Box,
        };
        assert!(gpu_f_resize_compact_box_vertical_only_geometry(
            &op,
            (1, height as u32),
            (1, 1),
            Some("F")
        ));
        assert!(gpu_f_resize_compact_box_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        let compact_words =
            encode_resize_compact_box_axis(height as u32, 1).expect("compact Box coefficient row");
        assert_eq!(compact_words.len(), 7);
        assert_eq!(&compact_words[0..3], &[0, height as u32, 0]);
        assert!(!gpu_f_resize_f64_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        assert!(!gpu_f_resize_dyadic_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        assert!(gpu_f_resize_f64_ordered_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        assert!(!gpu_dimensions_require_cpu(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        assert!(!gpu_dispatch_dimensions_require_cpu(
            std::slice::from_ref(&op),
            (1, height as u32),
            65_535,
            Some("F")
        ));
        assert_eq!(
            gpu_dispatch_count(std::slice::from_ref(&op), Some("F"), (1, height as u32)),
            1
        );
    }

    #[test]
    fn f_resize_compact_box_proof_covers_multiple_output_rows() {
        fn bytes(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        // Two output columns each consume an integer 8,388,608-tap Box row.
        // The complete per-tap table would exceed the binding limit, while
        // Pillow's Resample.c geometry gives both rows the same 1/ratio
        // coefficient and contiguous source ranges.
        let ratio = 8_388_608usize;
        let width = ratio * 2;
        let pattern = [0.125f32, -17.25, 1.0, -2.5, 3.75, -0.0, 100.0, -99.0];
        let words: Vec<u32> = (0..width)
            .map(|index| pattern[index % pattern.len()].to_bits())
            .collect();
        let source = Image::frombytes("F", (width as u32, 1), &bytes(&words))
            .expect("multi-output compact Box source");
        let source_dynamic = source
            .materialize()
            .expect("materialize multi-output compact Box source");
        let op = PipelineOp::Resize {
            w: 2,
            h: 1,
            filter: ResampleFilter::Box,
        };
        assert!(gpu_f_resize_compact_box_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        let compact_words = encode_resize_compact_box_axis(width as u32, 2)
            .expect("multi-output compact Box coefficient rows");
        assert_eq!(compact_words.len(), 10);
        assert_eq!(
            &compact_words[0..6],
            &[0, ratio as u32, 0, ratio as u32, ratio as u32, 0]
        );
        assert_eq!(
            &compact_words[6..],
            &encode_resize_compact_box_axis(ratio as u32, 1)
                .expect("single-output compact Box coefficient row")[3..]
        );
        assert!(!gpu_dimensions_require_cpu(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        assert!(!gpu_dispatch_dimensions_require_cpu(
            std::slice::from_ref(&op),
            (width as u32, 1),
            65_535,
            Some("F")
        ));
        let filter = ResampleInput::Name("BOX".into());
        let expected = source
            .resize((2, 1), Some(filter.clone()), None)
            .expect("CPU multi-output compact Box resize")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU multi-output compact Box bytes");
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = source
            .resize((2, 1), Some(filter), None)
            .expect("GPU multi-output compact Box resize")
            .use_backend(Backend::Gpu)
            .tobytes()
            .expect("native GPU multi-output compact Box bytes");
        assert_eq!(actual, expected);
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("multi-output compact Box resize must publish telemetry");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);

        assert!(!gpu_f_resize_compact_box_axis(width as u32 + 1, 2));
        assert!(encode_resize_compact_box_axis(width as u32 + 1, 2).is_err());
    }

    #[test]
    fn f_resize_compact_box_proof_covers_vertical_over_binding_row() {
        fn bytes(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        let height = 8_388_608usize;
        let pattern = [0.125f32, -17.25, 1.0, -2.5, 3.75, -0.0, 100.0, -99.0];
        let words: Vec<u32> = (0..height)
            .map(|index| pattern[index % pattern.len()].to_bits())
            .collect();
        let source = Image::frombytes("F", (1, height as u32), &bytes(&words))
            .expect("vertical compact Box source");
        let source_dynamic = source
            .materialize()
            .expect("materialize vertical compact Box source");
        let op = PipelineOp::Resize {
            w: 1,
            h: 1,
            filter: ResampleFilter::Box,
        };
        assert!(gpu_f_resize_compact_box_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        let compact_words =
            encode_resize_compact_box_axis(height as u32, 1).expect("compact Box coefficient row");
        assert_eq!(compact_words.len(), 7);
        assert_eq!(&compact_words[0..3], &[0, height as u32, 0]);
        assert!(!gpu_f_resize_f64_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        assert!(!gpu_f_resize_dyadic_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        assert!(gpu_f_resize_f64_ordered_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
    }

    #[test]
    fn f_resize_compact_box_proof_covers_changed_second_axis() {
        fn bytes(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        // The horizontal compact row is followed by a two-tap ordinary Box
        // row.  This exercises the pass ordering that was previously rejected
        // solely because the non-compact axis changed.
        let width = 8_388_608usize;
        let pattern = [0.125f32, -17.25, 1.0, -2.5, 3.75, -0.0, 100.0, -99.0];
        let words: Vec<u32> = (0..width * 2)
            .map(|index| pattern[index % pattern.len()].to_bits())
            .collect();
        let source = Image::frombytes("F", (width as u32, 2), &bytes(&words))
            .expect("compact chained Box source");
        let source_dynamic = source
            .materialize()
            .expect("materialize compact chained Box source");
        let op = PipelineOp::Resize {
            w: 1,
            h: 1,
            filter: ResampleFilter::Box,
        };
        assert!(gpu_f_resize_compact_box_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        assert!(!gpu_f_resize_compact_box_vertical_only_geometry(
            &op,
            (width as u32, 2),
            (1, 1),
            Some("F")
        ));
        let mut special_words = words.clone();
        special_words[width / 2] = 0x7fc1_2345;
        let special_source = Image::frombytes("F", (width as u32, 2), &bytes(&special_words))
            .expect("compact chained special Box source")
            .materialize()
            .expect("materialize compact chained special Box source");
        assert!(!gpu_f_resize_compact_box_is_exact(
            std::slice::from_ref(&op),
            &special_source,
            Some("F")
        ));
        let filter = ResampleInput::Name("BOX".into());
        let expected = source
            .resize((1, 1), Some(filter.clone()), None)
            .expect("CPU compact chained Box resize")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU compact chained Box bytes");
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = source
            .resize((1, 1), Some(filter), None)
            .expect("GPU compact chained Box resize")
            .use_backend(Backend::Gpu)
            .tobytes()
            .expect("native GPU compact chained Box bytes");
        assert_eq!(actual, expected);
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("compact chained Box resize must publish telemetry");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_compact_box_special_over_binding_native_matches_cpu() {
        fn bytes(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        // Marker 13 repeats a nonzero 1/source-axis coefficient, so its
        // compact special scan can preserve the same first-NaN payload and
        // signed-infinity cancellation as marker 9 without a full table.
        // Exercise both shader directions at the adapter-fitting boundary.
        let width = 8_388_608usize;
        let cases = [
            (
                width as u32,
                1u32,
                vec![(width / 2, 0x7fa1_2345u32)],
                "horizontal NaN",
            ),
            (
                width as u32,
                1u32,
                vec![(width / 3, 0x7f80_0000u32), (width * 2 / 3, 0xff80_0000)],
                "horizontal opposite infinities",
            ),
            (
                1u32,
                width as u32,
                vec![(width / 3, 0x7f80_0000u32), (width * 2 / 3, 0xff80_0000)],
                "vertical opposite infinities",
            ),
            (
                1u32,
                width as u32,
                vec![(width / 2, 0x7fc2_3456u32)],
                "vertical NaN",
            ),
        ];
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for (source_w, source_h, specials, label) in cases {
            let mut words = vec![0x3f80_0000u32; width];
            for (index, value) in specials {
                words[index] = value;
            }
            let source = Image::frombytes("F", (source_w, source_h), &bytes(&words))
                .expect("compact special F source");
            let source_dynamic = source
                .materialize()
                .expect("materialize compact special F source");
            let op = PipelineOp::Resize {
                w: 1,
                h: 1,
                filter: ResampleFilter::Box,
            };
            assert!(
                gpu_f_resize_compact_box_is_exact(
                    std::slice::from_ref(&op),
                    &source_dynamic,
                    Some("F")
                ),
                "compact proof should cover {label}"
            );
            let filter = ResampleInput::Name("BOX".into());
            let expected = source
                .resize((1, 1), Some(filter.clone()), None)
                .expect("CPU compact special F resize")
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU compact special F bytes");
            let actual = match source
                .resize((1, 1), Some(filter), None)
                .expect("GPU compact special F resize")
                .use_backend(Backend::Gpu)
                .tobytes()
            {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU compact special F resize failed: {error}"),
            };
            assert_eq!(actual, expected, "native compact special {label}");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("compact special F resize must publish telemetry");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.7, None);
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_f64_special_over_8388607_taps_stays_host_controlled() {
        fn bytes(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        // Marker 9's special-value prepass must obey the same adapter-fitting
        // coefficient cap as marker 12. Before this guard, an 8388608-tap
        // NaN row bypassed the finite-row limit and reached bind-group
        // creation with a 134217984-byte range, exceeding wgpu's 128-MiB
        // storage binding limit instead of taking exact host control.
        let width = 8_388_608usize;
        let mut words = vec![0x3f80_0000; width];
        words[width / 2] = 0x7fc1_2345;
        let source = Image::frombytes("F", (width as u32, 1), &bytes(&words))
            .expect("over-bound special F source");
        let source_dynamic = source
            .materialize()
            .expect("materialize over-bound special F source");
        let op = PipelineOp::Resize {
            w: 1,
            h: 1,
            filter: ResampleFilter::Bilinear,
        };
        assert!(!gpu_f_resize_f64_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));

        let filter = ResampleInput::Name("BILINEAR".into());
        let expected = source
            .resize((1, 1), Some(filter.clone()), None)
            .expect("CPU over-bound special F resize")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU over-bound special F bytes");
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = source
            .resize((1, 1), Some(filter), None)
            .expect("GPU over-bound special F resize")
            .use_backend(Backend::Gpu)
            .tobytes()
            .expect("host-controlled over-bound special F resize");
        assert_eq!(actual, expected);
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("over-bound special F resize must publish telemetry");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Cpu);
        assert_eq!(telemetry.7.as_deref(), Some("exact host semantic control"));
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_f64_coeff_table_over_binding_stays_host_controlled() {
        fn bytes(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        // Each output row below is individually within the ordered reducer's
        // tap cap, but the two-row horizontal table is 201326592 bytes after
        // f64 coefficient encoding. Admission must account for the complete
        // binding range rather than checking rows in isolation.
        let width = 8_388_607usize;
        let words: Vec<u32> = (0..width)
            .map(|index| (0.5f32 + ((index * 13 % 90) as f32) * 0.01f32).to_bits())
            .collect();
        let source = Image::frombytes("F", (width as u32, 1), &bytes(&words))
            .expect("multi-row coefficient F source");
        let source_dynamic = source
            .materialize()
            .expect("materialize multi-row coefficient F source");
        let op = PipelineOp::Resize {
            w: 2,
            h: 1,
            filter: ResampleFilter::Bilinear,
        };
        assert!(!gpu_f_resize_f64_ordered_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        assert!(!gpu_f_resize_f64_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));

        let filter = ResampleInput::Name("BILINEAR".into());
        let expected = source
            .resize((2, 1), Some(filter.clone()), None)
            .expect("CPU multi-row coefficient F resize")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU multi-row coefficient F bytes");
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = source
            .resize((2, 1), Some(filter), None)
            .expect("GPU multi-row coefficient F resize")
            .use_backend(Backend::Gpu)
            .tobytes()
            .expect("host-controlled multi-row coefficient F resize");
        assert_eq!(actual, expected);
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("multi-row coefficient F resize must publish telemetry");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Cpu);
        assert_eq!(telemetry.7.as_deref(), Some("exact host semantic control"));
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_ordered_f64_subnormal_and_extreme_finite_native_matches_cpu() {
        fn bytes(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        // f32 subnormals are still ordinary finite values to Pillow's f64
        // accumulator.  Verify the explicit 2^-149 representation survives
        // the wider marker-12 row, alongside the largest finite f32 word.
        for (filter, value) in [
            (ResampleFilter::Bilinear, 0x0000_0001u32),
            (ResampleFilter::Lanczos, 0x0000_0003u32),
            (ResampleFilter::Box, 0x0000_0007u32),
            (ResampleFilter::Bicubic, 0x7f7f_ffffu32),
            (ResampleFilter::Hamming, 0x7f7f_ffffu32),
        ] {
            let width = 32768usize;
            let words = vec![value; width];
            let source = Image::frombytes("F", (width as u32, 1), &bytes(&words))
                .expect("subnormal/extreme F source");
            let source_dynamic = source
                .materialize()
                .expect("materialize subnormal/extreme F source");
            let op = PipelineOp::Resize { w: 1, h: 1, filter };
            assert!(
                gpu_f_resize_f64_ordered_is_exact(
                    std::slice::from_ref(&op),
                    &source_dynamic,
                    Some("F")
                ),
                "ordered proof should cover 32768-tap {filter:?} row"
            );
            let filter_name = match filter {
                ResampleFilter::Bilinear => "BILINEAR",
                ResampleFilter::Bicubic => "BICUBIC",
                ResampleFilter::Lanczos => "LANCZOS",
                ResampleFilter::Hamming => "HAMMING",
                ResampleFilter::Box => "BOX",
                _ => unreachable!(),
            };
            let expected = source
                .resize((1, 1), Some(ResampleInput::Name(filter_name.into())), None)
                .expect("CPU subnormal/extreme F resize")
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU subnormal/extreme F bytes");
            let previous = Backend::set_pipeline_telemetry_enabled(true);
            let actual = match source
                .resize((1, 1), Some(ResampleInput::Name(filter_name.into())), None)
                .expect("GPU subnormal/extreme F resize")
                .use_backend(Backend::Gpu)
                .tobytes()
            {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU subnormal/extreme F resize failed: {error}"),
            };
            assert_eq!(actual, expected, "32768-tap {filter_name} F resize");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("subnormal/extreme F resize must publish telemetry");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.7, None);
            Backend::set_pipeline_telemetry_enabled(previous);
        }
    }

    #[test]
    fn f_resize_ordered_f64_subnormal_vertical_native_matches_cpu() {
        fn bytes(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        // Exercise the separate vertical shader reducer as well as the
        // horizontal path above.  Vertical FLOAT32 resampling stays on the
        // scalar FMA ordering, including when its source words are f32
        // subnormals or largest-finite values.
        for (filter, value) in [
            (ResampleFilter::Bilinear, 0x0000_0001u32),
            (ResampleFilter::Bicubic, 0x0000_0003u32),
            (ResampleFilter::Lanczos, 0x8000_0007u32),
            (ResampleFilter::Hamming, 0x7f7f_ffffu32),
            (ResampleFilter::Box, 0x7f7f_ffffu32),
        ] {
            let height = 32768usize;
            let words = vec![value; height];
            let source = Image::frombytes("F", (1, height as u32), &bytes(&words))
                .expect("vertical subnormal/extreme F source");
            let source_dynamic = source
                .materialize()
                .expect("materialize vertical subnormal/extreme F source");
            let op = PipelineOp::Resize { w: 1, h: 1, filter };
            assert!(gpu_f_resize_f64_ordered_is_exact(
                std::slice::from_ref(&op),
                &source_dynamic,
                Some("F")
            ));
            let filter_name = match filter {
                ResampleFilter::Bilinear => "BILINEAR",
                ResampleFilter::Bicubic => "BICUBIC",
                ResampleFilter::Lanczos => "LANCZOS",
                ResampleFilter::Hamming => "HAMMING",
                ResampleFilter::Box => "BOX",
                _ => unreachable!(),
            };
            let expected = source
                .resize((1, 1), Some(ResampleInput::Name(filter_name.into())), None)
                .expect("CPU vertical subnormal/extreme F resize")
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU vertical subnormal/extreme F bytes");
            let previous = Backend::set_pipeline_telemetry_enabled(true);
            let actual = match source
                .resize((1, 1), Some(ResampleInput::Name(filter_name.into())), None)
                .expect("GPU vertical subnormal/extreme F resize")
                .use_backend(Backend::Gpu)
                .tobytes()
            {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU vertical F resize failed: {error}"),
            };
            assert_eq!(
                actual, expected,
                "32768-tap vertical {filter_name} F resize"
            );
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("vertical F resize must publish telemetry");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.7, None);
            Backend::set_pipeline_telemetry_enabled(previous);
        }
    }

    #[test]
    fn f_resize_ordered_f64_wide_cancellation_native_matches_cpu() {
        fn bytes(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        for (width, filter) in [
            (65usize, ResampleFilter::Bilinear),
            (96usize, ResampleFilter::Lanczos),
        ] {
            let words: Vec<u32> = (0..width)
                .map(|index| {
                    if index % 2 == 0 {
                        0x3f80_0000
                    } else {
                        0xbf80_0000
                    }
                })
                .collect();
            let source = Image::frombytes("F", (width as u32, 1), &bytes(&words))
                .expect("wide cancellation F source");
            let source_dynamic = source
                .materialize()
                .expect("materialize cancellation source");
            let op = PipelineOp::Resize { w: 1, h: 1, filter };
            assert!(gpu_f_resize_f64_ordered_is_exact(
                std::slice::from_ref(&op),
                &source_dynamic,
                Some("F")
            ));
            let filter_name = match filter {
                ResampleFilter::Bilinear => "BILINEAR",
                ResampleFilter::Lanczos => "LANCZOS",
                _ => unreachable!(),
            };
            let expected = source
                .resize((1, 1), Some(ResampleInput::Name(filter_name.into())), None)
                .expect("CPU cancellation resize")
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU cancellation bytes");
            let previous = Backend::set_pipeline_telemetry_enabled(true);
            let actual = source
                .resize((1, 1), Some(ResampleInput::Name(filter_name.into())), None)
                .expect("GPU cancellation resize")
                .use_backend(Backend::Gpu)
                .tobytes()
                .expect("GPU cancellation bytes");
            assert_eq!(actual, expected, "wide cancellation {filter_name} resize");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("wide cancellation resize must publish telemetry");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.7, None);
            Backend::set_pipeline_telemetry_enabled(previous);
        }
    }

    #[test]
    fn f_pad_constant_source_preserves_scalar_fill_on_gpu() {
        // `ImageOps.pad` keeps F samples as opaque four-byte words through
        // the contain resize and final placement.  This constant source is
        // eligible for the exact scalar resize marker; the named fill is
        // resolved to Pillow's F luma value (76.0) before it reaches the
        // raw-word placement shader.
        let source = Image::new(16, 16, "F", (0, 0, 0, 0)).expect("F source");
        let expected = crate::ops::imageops::pad_with_input(
            &source,
            20,
            12,
            None,
            crate::ops::imageops::ImageOpsColor::Name("red".into()),
            crate::ops::imageops::CenteringInput::Default,
        )
        .expect("CPU pad operation")
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU F pad");

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match crate::ops::imageops::pad_with_input(
            &source,
            20,
            12,
            None,
            crate::ops::imageops::ImageOpsColor::Name("red".into()),
            crate::ops::imageops::CenteringInput::Default,
        )
        .expect("GPU pad operation")
        .use_backend(Backend::Gpu)
        .tobytes()
        {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU F pad failed: {error}"),
        };
        assert_eq!(actual, expected, "native GPU F pad scalar fill");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU F pad must publish a telemetry receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(3));
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_pad_f64_native_preserves_heterogeneous_words() {
        let values = [
            0.1f32, -0.3, 1.7, 2.9, 3.5, -4.25, 9.0, 0.0, 8.0, 1.0, 5.0, 7.0,
        ];
        let source_bytes = values
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>();
        let source = Image::new(4, 3, "F", (0, 0, 0, 0)).expect("initial F source");
        let source_dynamic = source.materialize().expect("materialize initial F source");
        let putdata: Arc<[u8]> = Arc::from(source_bytes.clone().into_boxed_slice());
        let filters = [
            (ResampleFilter::Bilinear, 2i64),
            (ResampleFilter::Bicubic, 3),
            (ResampleFilter::Lanczos, 1),
            (ResampleFilter::Hamming, 5),
            (ResampleFilter::Box, 4),
        ];
        for (filter, _) in filters {
            let pad = PipelineOp::Pad {
                w: 5,
                h: 5,
                filter,
                color: None,
                centering: (0.5, 0.5),
            };
            assert!(gpu_f_pad_f64_is_exact(
                std::slice::from_ref(&pad),
                &source_dynamic,
                Some("F")
            ));
            assert!(gpu_f_pad_f64_is_exact(
                &[
                    PipelineOp::PutData {
                        data: putdata.clone(),
                        mode: PixelMode::F,
                    },
                    pad,
                ],
                &source_dynamic,
                Some("F")
            ));
        }

        let mut expected_source = source.clone();
        expected_source
            .putdata(&source_bytes)
            .expect("CPU F PutData");
        let mut actual_source = source;
        actual_source.putdata(&source_bytes).expect("GPU F PutData");
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for (filter, code) in filters {
            let expected = crate::ops::imageops::pad_with_input(
                &expected_source,
                5,
                5,
                Some(ResampleInput::Code(code)),
                ImageOpsColor::Scalar(0),
                crate::ops::imageops::CenteringInput::Default,
            )
            .expect("CPU F Pad operation")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU F Pad");
            let actual = match crate::ops::imageops::pad_with_input(
                &actual_source,
                5,
                5,
                Some(ResampleInput::Code(code)),
                ImageOpsColor::Scalar(0),
                crate::ops::imageops::CenteringInput::Default,
            )
            .expect("GPU F Pad operation")
            .use_backend(Backend::Gpu)
            .tobytes()
            {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU F Pad {filter:?} failed: {error}"),
            };
            assert_eq!(actual, expected, "native GPU F Pad {filter:?}");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("native GPU F Pad must publish a telemetry receipt");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.6, Some(4));
            assert_eq!(telemetry.7, None);
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_thumbnail_constant_source_uses_exact_gpu_resize() {
        let source = Image::new(16, 16, "F", (0, 0, 0, 0)).expect("constant F thumbnail source");
        let mut expected_image = source.clone();
        expected_image
            .thumbnail((4, 4), None)
            .expect("CPU constant F thumbnail operation");
        let expected = expected_image
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU constant F thumbnail");

        let mut actual_image = source;
        actual_image
            .thumbnail((4, 4), None)
            .expect("GPU constant F thumbnail operation");
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match actual_image.use_backend(Backend::Gpu).tobytes() {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU constant F thumbnail failed: {error}"),
        };
        assert_eq!(actual, expected, "constant F thumbnail parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("constant F thumbnail must publish a telemetry receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_thumbnail_constant_admission_rejects_reducing_overflow() {
        let words = 25usize.saturating_mul(38);
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(
                25,
                38,
                (0..words)
                    .flat_map(|_| 0x7f27_69eeu32.to_le_bytes())
                    .collect(),
            )
            .expect("constant finite F source"),
        );
        assert_eq!(
            gpu_f_source_constant_bits(&source, Some("F")),
            Some(0x7f27_69ee)
        );
        let thumbnail = PipelineOp::Thumbnail {
            w: 10,
            h: 7,
            filter: ResampleFilter::Bilinear,
        };
        assert!(!gpu_f_thumbnail_constant_is_exact(
            &thumbnail,
            (25, 38),
            &source,
            Some("F"),
        ));
    }

    #[test]
    fn f_thumbnail_without_reducing_gap_uses_exact_f64_resize() {
        // Pillow skips the reducing-gap pass when both factors are one, then
        // runs the same filtered F resize used by Image.resize. The planner
        // previously kept every non-nearest F Thumbnail on host control even
        // in this no-reduce case, so marker 9 could not carry heterogeneous
        // non-dyadic words through the native path.
        let values = [0.1f32, -0.3, 1.7, 2.9];
        let source_bytes = values
            .iter()
            .cycle()
            .take(8 * 4)
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        let source = Image::frombytes("F", (8, 4), &source_bytes).expect("F thumbnail source");
        let filters = ["BOX", "BILINEAR", "BICUBIC", "LANCZOS", "HAMMING"];
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for filter in filters {
            let mut expected_image = source.clone();
            expected_image
                .thumbnail((6, 3), Some(ResampleInput::Name(filter.into())))
                .expect("CPU no-reduce F thumbnail");
            let expected = expected_image
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU no-reduce F thumbnail bytes");

            let mut actual_image = source.clone();
            actual_image
                .thumbnail((6, 3), Some(ResampleInput::Name(filter.into())))
                .expect("GPU no-reduce F thumbnail");
            let actual = match actual_image.use_backend(Backend::Gpu).tobytes() {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native no-reduce F thumbnail failed ({filter}): {error}"),
            };
            assert_eq!(actual, expected, "no-reduce F thumbnail parity ({filter})");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("native no-reduce F thumbnail must publish telemetry");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.6, Some(2));
            assert_eq!(telemetry.7, None);
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_f64_filtered_chain_native_matches_cpu() {
        let values = [0.1f32, -0.3, 1.7, 2.9];
        let source_bytes: Vec<u8> = values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let source = Image::frombytes("F", (2, 2), &source_bytes).expect("F source");
        let expected = source
            .resize((3, 3), Some(ResampleInput::Name("BICUBIC".into())), None)
            .expect("CPU first chain resize")
            .resize((2, 2), Some(ResampleInput::Name("LANCZOS".into())), None)
            .expect("CPU second chain resize")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU filtered F chain");
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match source
            .resize((3, 3), Some(ResampleInput::Name("BICUBIC".into())), None)
            .expect("GPU first chain resize")
            .resize((2, 2), Some(ResampleInput::Name("LANCZOS".into())), None)
            .expect("GPU second chain resize")
            .use_backend(Backend::Gpu)
            .tobytes()
        {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU filtered F chain failed: {error}"),
        };
        assert_eq!(actual, expected, "filtered F chain parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU filtered F chain must publish a telemetry receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.2, 2);
        assert_eq!(telemetry.6, Some(4));
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_f64_relocation_chain_native_matches_cpu() {
        fn source() -> Image {
            Image::frombytes(
                "F",
                (2, 2),
                &[0.1f32, -0.3, 1.7, 2.9]
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect::<Vec<_>>(),
            )
            .expect("F relocation-chain source")
        }

        let cases = [
            ((3, 2), PipelineOp::Mirror, (2, 2), "mirror"),
            ((3, 2), PipelineOp::Flip, (2, 2), "flip"),
            (
                (3, 2),
                PipelineOp::Transpose {
                    method: crate::pipeline::TransposeMethod::Rotate90,
                },
                (2, 2),
                "transpose",
            ),
            ((3, 2), PipelineOp::Offset { x: 1, y: -1 }, (2, 2), "offset"),
            ((3, 2), PipelineOp::Duplicate, (2, 2), "duplicate"),
            (
                (3, 2),
                PipelineOp::Crop {
                    left: 0,
                    top: 0,
                    right: 3,
                    bottom: 2,
                },
                (2, 2),
                "crop",
            ),
            (
                (4, 4),
                PipelineOp::CropBorder { border: 1 },
                (3, 3),
                "crop-border",
            ),
        ];

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for (first_dimensions, relocation, final_dimensions, name) in cases {
            let first = source()
                .resize(
                    (i64::from(first_dimensions.0), i64::from(first_dimensions.1)),
                    Some(ResampleInput::Name("BICUBIC".into())),
                    None,
                )
                .expect("CPU first relocation-chain resize");
            let expected = Image::push_op(&first, relocation.clone())
                .resize(
                    (i64::from(final_dimensions.0), i64::from(final_dimensions.1)),
                    Some(ResampleInput::Name("LANCZOS".into())),
                    None,
                )
                .expect("CPU second relocation-chain resize")
                .use_backend(Backend::Cpu)
                .tobytes()
                .unwrap_or_else(|error| panic!("CPU {name} relocation chain failed: {error}"));

            let first = source()
                .resize(
                    (i64::from(first_dimensions.0), i64::from(first_dimensions.1)),
                    Some(ResampleInput::Name("BICUBIC".into())),
                    None,
                )
                .expect("GPU first relocation-chain resize");
            let actual = match Image::push_op(&first, relocation)
                .resize(
                    (i64::from(final_dimensions.0), i64::from(final_dimensions.1)),
                    Some(ResampleInput::Name("LANCZOS".into())),
                    None,
                )
                .expect("GPU second relocation-chain resize")
                .use_backend(Backend::Gpu)
                .tobytes()
            {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU {name} relocation chain failed: {error}"),
            };
            assert_eq!(
                actual, expected,
                "native GPU F {name} relocation chain parity"
            );
            let telemetry = Backend::take_pipeline_telemetry()
                .unwrap_or_else(|| panic!("native GPU F {name} chain missing telemetry"));
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.2, 3);
            assert_eq!(telemetry.6, Some(5));
            assert_eq!(telemetry.7, None);
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_f64_nearest_relocation_chain_native_matches_cpu() {
        fn source() -> Image {
            Image::frombytes(
                "F",
                (2, 2),
                &[0.1f32, -0.3, 1.7, 2.9]
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect::<Vec<_>>(),
            )
            .expect("F nearest-chain source")
        }

        let pipeline = |image: Image| {
            image
                .resize((3, 3), Some(ResampleInput::Name("BICUBIC".into())), None)
                .expect("first nearest-chain resize")
                .resize((2, 2), Some(ResampleInput::Name("NEAREST".into())), None)
                .expect("nearest-chain copy resize")
                .resize((1, 3), Some(ResampleInput::Name("LANCZOS".into())), None)
                .expect("last nearest-chain resize")
        };
        let expected = pipeline(source())
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU F nearest relocation chain");

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match pipeline(source()).use_backend(Backend::Gpu).tobytes() {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU F nearest relocation chain failed: {error}"),
        };
        assert_eq!(
            actual, expected,
            "native GPU F nearest relocation chain parity"
        );
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU F nearest relocation chain must publish telemetry");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.2, 3);
        assert_eq!(telemetry.6, Some(6));
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_f64_putdata_prefix_native_matches_cpu() {
        fn bytes(words: &[f32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        // The deferred PutData is deliberately different from the initial
        // image. Pillow replaces the source words before evaluating the
        // non-dyadic resize; the GPU must prove and consume that same update
        // instead of applying the coefficients to stale upload bytes.
        let initial =
            Image::frombytes("F", (2, 2), &bytes(&[9.0, 8.0, 7.0, 6.0])).expect("initial F source");
        let replacement = bytes(&[0.1, -0.3, 1.7, 2.9]);

        let mut expected_image = initial.clone();
        expected_image.putdata(&replacement).expect("CPU F PutData");
        let expected = expected_image
            .resize((1, 5), Some(ResampleInput::Name("BILINEAR".into())), None)
            .expect("CPU F PutData resize")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU F PutData resize bytes");

        let mut actual_image = initial;
        actual_image.putdata(&replacement).expect("GPU F PutData");
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match actual_image
            .resize((1, 5), Some(ResampleInput::Name("BILINEAR".into())), None)
            .expect("GPU F PutData resize")
            .use_backend(Backend::Gpu)
            .tobytes()
        {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU F PutData resize failed: {error}"),
        };
        assert_eq!(actual, expected, "native GPU F PutData resize parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU F PutData resize must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.2, 2);
        assert_eq!(telemetry.6, Some(3));
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_mixed_upscale_downscale_native_matches_cpu() {
        let values = [-9.7966165f32, 6.5041304];
        let source_bytes: Vec<u8> = values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let source = Image::frombytes("F", (1, 2), &source_bytes).expect("F source");
        let expected = source
            .resize((2, 1), Some(ResampleInput::Name("BOX".into())), None)
            .expect("CPU mixed-geometry resize")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU mixed-geometry F resize");

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = source
            .resize((2, 1), Some(ResampleInput::Name("BOX".into())), None)
            .expect("GPU mixed-geometry resize operation")
            .use_backend(Backend::Gpu)
            .tobytes()
            .expect("native GPU mixed-geometry F resize");
        assert_eq!(actual, expected, "mixed F resize parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native mixed-geometry F resize must publish a telemetry receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(2));
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_f64_subnormal_native_matches_cpu() {
        fn bytes(words: &[u32]) -> Vec<u8> {
            words.iter().flat_map(|word| word.to_le_bytes()).collect()
        }

        let source = Image::frombytes(
            "F",
            (2, 2),
            &bytes(&[0x0000_0001, 0x0000_0002, 0x0000_0003, 0x0000_0004]),
        )
        .expect("F subnormal source");
        let cases = [
            ((1i64, 2i64), ResampleInput::Name("BILINEAR".into())),
            ((1, 5), ResampleInput::Name("BICUBIC".into())),
            ((1, 5), ResampleInput::Name("LANCZOS".into())),
            ((1, 5), ResampleInput::Name("HAMMING".into())),
            ((1, 5), ResampleInput::Name("BOX".into())),
        ];

        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for (size, filter) in cases {
            let expected = source
                .resize(size, Some(filter.clone()), None)
                .expect("CPU subnormal resize operation")
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU subnormal F resize");
            let actual = match source
                .resize(size, Some(filter), None)
                .expect("GPU subnormal resize operation")
                .use_backend(Backend::Gpu)
                .tobytes()
            {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU subnormal F resize failed: {error}"),
            };
            assert_eq!(actual, expected, "subnormal F resize parity");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("native GPU subnormal F resize must publish a receipt");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.6, Some(2));
            assert_eq!(telemetry.7, None);
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn typed_luma16_nearest_affine_transform_uses_native_word_path() {
        // I;16 affine transforms use Pillow's native nearest sampler even
        // when a non-nearest filter token is supplied.  The mode-5 shader
        // must therefore relocate the complete low 16-bit word, while its
        // affine plan uses the typed `floor(source + 0.5)` coordinate
        // contract rather than the byte path's destination-center offset.
        let values = [
            0x0102u16, 0x1122, 0x2233, 0x3344, 0x4455, 0x5566, 0x6677, 0x7788, 0x8899, 0x99aa,
            0xaabb, 0xbbcc,
        ];
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for mode in ["I;16", "I;16L", "I;16B", "I;16N"] {
            let bytes = values
                .iter()
                .flat_map(|value| {
                    if mode == "I;16B" {
                        value.to_be_bytes()
                    } else {
                        value.to_le_bytes()
                    }
                })
                .collect::<Vec<_>>();
            let source = Image::frombytes(mode, (4, 3), &bytes)
                .unwrap_or_else(|error| panic!("{mode} source: {error}"));
            let data = TransformData::Affine(vec![0.5, 0.0, 0.0, 0.0, 0.5, 0.0]);
            let transformed = source
                .transform_public(
                    (3, 2),
                    0,
                    Some(data),
                    3,
                    0,
                    Some(TransformFill::Scalar(258)),
                )
                .unwrap_or_else(|error| panic!("{mode} transform: {error}"));
            let expected = transformed
                .clone()
                .use_backend(Backend::Cpu)
                .tobytes()
                .unwrap_or_else(|error| panic!("{mode} CPU transform: {error}"));
            let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU {mode} transform failed: {error}"),
            };
            assert_eq!(actual, expected, "native GPU {mode} affine parity");
            let telemetry = Backend::take_pipeline_telemetry()
                .unwrap_or_else(|| panic!("native GPU {mode} transform missing telemetry"));
            assert_eq!(telemetry.0, Some(Backend::Gpu), "{mode} requested backend");
            assert_eq!(telemetry.1, Backend::Gpu, "{mode} actual backend");
            assert_eq!(telemetry.7, None, "{mode} fallback reason");

            let op = PipelineOp::Transform {
                w: 3,
                h: 2,
                method: TransformMethod::Affine,
                data: Arc::from(vec![0.5, 0.0, 0.0, 0.0, 0.5, 0.0]),
                filter: ResampleFilter::Bicubic,
                fill: Some((2, 1, 0, 0)),
                fill_is_none: false,
                palette_fill: None,
            };
            let image = source.materialize().expect("materialize typed source");
            assert!(
                gpu_nearest_affine_is_exact(&op, &image, Some(mode), (4, 3)),
                "{mode} nearest-affine proof"
            );
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn f_resize_identity_lowering_copies_nonfinite_words() {
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(
                4,
                4,
                [
                    f32::NAN.to_bits(),
                    f32::INFINITY.to_bits(),
                    (-0.0f32).to_bits(),
                    (-1.25f32).to_bits(),
                    0.0f32.to_bits(),
                    1.0f32.to_bits(),
                    2.0f32.to_bits(),
                    3.0f32.to_bits(),
                    4.0f32.to_bits(),
                    5.0f32.to_bits(),
                    6.0f32.to_bits(),
                    7.0f32.to_bits(),
                    8.0f32.to_bits(),
                    9.0f32.to_bits(),
                    10.0f32.to_bits(),
                    11.0f32.to_bits(),
                ]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect(),
            )
            .unwrap(),
        );
        let same_size = PipelineOp::Resize {
            w: 4,
            h: 4,
            filter: ResampleFilter::Bicubic,
        };
        assert!(gpu_f_resize_identity_is_exact(
            std::slice::from_ref(&same_size),
            &image,
            Some("F")
        ));
        assert!(gpu_f_resize_identity_is_exact(
            &[
                PipelineOp::PutData {
                    data: Arc::from(vec![0xde, 0xad, 0xbe, 0xef].repeat(16).into_boxed_slice()),
                    mode: PixelMode::F,
                },
                same_size.clone(),
            ],
            &image,
            Some("F")
        ));
        assert!(!gpu_f_resize_identity_is_exact(
            &[PipelineOp::Resize {
                w: 3,
                h: 4,
                filter: ResampleFilter::Bilinear,
            }],
            &image,
            Some("F")
        ));
        assert!(!gpu_f_resize_identity_is_exact(
            &[same_size.clone(), PipelineOp::Mirror],
            &image,
            Some("F")
        ));
        assert!(!gpu_f_resize_identity_is_exact(
            std::slice::from_ref(&same_size),
            &image,
            Some("I")
        ));
    }

    #[test]
    fn luma16_filtered_resize_proof_covers_declared_byte_orders() {
        let values = [
            1u16, 258, 32_767, 65_535, 32_768, 43_981, 12_345, 54_321, 999, 40_000, 22_222, 65_534,
        ];
        let op = PipelineOp::Resize {
            w: 2,
            h: 2,
            filter: ResampleFilter::Bilinear,
        };
        for mode in ["I;16", "I;16L", "I;16B", "I;16N"] {
            let big_endian = mode == "I;16B";
            let bytes = values
                .iter()
                .flat_map(|value| {
                    if big_endian {
                        value.to_be_bytes()
                    } else {
                        value.to_le_bytes()
                    }
                })
                .collect::<Vec<_>>();
            let source = Image::frombytes(mode, (4, 3), &bytes).expect("I;16 source");
            let image = source.materialize().expect("materialize I;16 source");
            assert!(
                gpu_luma16_resize_f64_is_exact(std::slice::from_ref(&op), &image, Some(mode)),
                "filtered I;16 proof should cover {mode}"
            );
        }
    }

    #[test]
    fn luma16_filtered_resize_native_gpu_matches_cpu() {
        let values = [
            1u16, 258, 32_767, 65_535, 32_768, 43_981, 12_345, 54_321, 999, 40_000, 22_222, 65_534,
            7, 511, 4096, 60000,
        ];
        let cases = [
            ((2u32, 2u32), ResampleFilter::Bilinear),
            ((7, 4), ResampleFilter::Lanczos),
            ((4, 5), ResampleFilter::Lanczos),
            ((5, 3), ResampleFilter::Box),
            ((3, 6), ResampleFilter::Hamming),
            ((6, 3), ResampleFilter::Bicubic),
        ];
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        for mode in ["I;16", "I;16L", "I;16B", "I;16N"] {
            let big_endian = mode == "I;16B";
            let bytes = values
                .iter()
                .flat_map(|value| {
                    if big_endian {
                        value.to_be_bytes()
                    } else {
                        value.to_le_bytes()
                    }
                })
                .collect::<Vec<_>>();
            let source = Image::frombytes(mode, (4, 4), &bytes).expect("I;16 source");
            assert_eq!(source.mode().expect("I;16 mode"), mode);
            assert_eq!(
                luma16_resample_big_endian(Some(mode)),
                matches!(mode, "I;16B" | "I;16N")
            );
            for &((width, height), filter) in &cases {
                let filter_name = ResampleInput::Name(format!("{filter:?}").to_uppercase());
                let expected = source
                    .resize(
                        (width as i64, height as i64),
                        Some(filter_name.clone()),
                        None,
                    )
                    .expect("CPU I;16 resize")
                    .use_backend(Backend::Cpu)
                    .tobytes()
                    .expect("CPU I;16 bytes");
                let actual = match source
                    .resize((width as i64, height as i64), Some(filter_name), None)
                    .expect("GPU I;16 resize")
                    .use_backend(Backend::Gpu)
                    .tobytes()
                {
                    Ok(actual) => actual,
                    Err(error)
                        if error.to_string().contains("GPU adapter not available")
                            || error
                                .to_string()
                                .contains("GPU device initialization failed") =>
                    {
                        Backend::set_pipeline_telemetry_enabled(previous);
                        return;
                    }
                    Err(error) => panic!("native GPU {mode} resize failed: {error}"),
                };
                assert_eq!(actual, expected, "native GPU I;16 {mode} {filter:?} resize");
                let telemetry = Backend::take_pipeline_telemetry()
                    .expect("native GPU I;16 resize must publish a receipt");
                assert_eq!(telemetry.0, Some(Backend::Gpu));
                assert_eq!(telemetry.1, Backend::Gpu);
                assert_eq!(telemetry.6, Some(2));
                assert_eq!(telemetry.7, None);
            }
        }
        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn rgb_a_fit_uses_native_raw_channel_resize() {
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let mut source_bytes = vec![0u8; 9 * 8 * 4];
        source_bytes[(3 * 9 + 2) * 4..][..4].copy_from_slice(&[200, 100, 50, 128]);
        let source = Image::frombytes("RGBa", (9, 8), &source_bytes).expect("RGBa source");
        let fitted = crate::ops::imageops::fit(&source, 4, 3, Some("BILINEAR"), 0.0, (0.5, 0.5))
            .expect("RGBa Fit operation");
        let actual = match fitted.use_backend(Backend::Gpu).tobytes() {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU RGBa Fit failed: {error}"),
        };
        assert_eq!(
            actual,
            vec![
                4, 2, 1, 3, 5, 3, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 14, 7, 3, 9, 19, 9, 5, 12, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]
        );
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU RGBa Fit must publish a telemetry receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(2));
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }
}
