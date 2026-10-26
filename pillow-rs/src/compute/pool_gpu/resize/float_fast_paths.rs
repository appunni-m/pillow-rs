//! float fast paths for GPU resize exactness.

use super::super::*;

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
pub(in crate::compute::pool_gpu) fn gpu_f_resize_constant_bits(
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
pub(in crate::compute::pool_gpu) fn gpu_f_source_constant_bits(
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> Option<u32> {
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
pub(in crate::compute::pool_gpu) fn gpu_f_resize_box_copy_is_exact(
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
pub(in crate::compute::pool_gpu) fn gpu_f_resize_identity_is_exact(
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
pub(in crate::compute::pool_gpu) fn gpu_i_resize_identity_is_exact(
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
pub(in crate::compute::pool_gpu) fn gpu_f_resize_box_average_is_exact(
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
    const MIN_SAFE_VALUE: f32 = f32::from_bits(0x3580_0000); // exact f32 value 2^-20
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
