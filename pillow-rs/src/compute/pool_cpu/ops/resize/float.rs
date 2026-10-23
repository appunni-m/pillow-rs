//! Pillow-compatible floating-point resize kernels.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::ops::pil_resize::{
    F64MulAdd, FilterCoeffsF64, PortableFma, f_resize_coefficients_allow_finite_fma,
    f_resize_samples_allow_finite_fma, f32_samples_from_le_bytes, precompute_coeffs_f64,
};
use crate::pipeline::ResampleFilter;
use crate::raster::{DynamicImage, GenericImageView};

use super::resample_kernel;

#[path = "float_resize.rs"]
mod float_resize;
#[cfg(all(test, not(feature = "parallel")))]
#[path = "float_resize_tests.rs"]
mod float_resize_tests;

// ── Helpers ──

// Pillow 12.2.0's arm64 FLOAT32 horizontal resampler uses scalar FMA for
// rows with at most 15 taps, then switches to complete 16-tap vector
// product/add blocks; any tail remains scalar FMA. The vertical resampler
// remains scalar FMA for every tap count. Keep the same split in the exact CPU
// implementation so heterogeneous wide reductions match the native Pillow
// build rather than the compiler's fused Rust loop.
const F_RESIZE_VECTOR_WIDTH: usize = 16;

pub(crate) fn f_resize_mul_add<const FINITE_OPERANDS: bool, F: F64MulAdd>(
    fma: &F,
    weight: f64,
    sample: f64,
    accumulator: f64,
) -> f64 {
    if FINITE_OPERANDS {
        fma.mul_add_finite(weight, sample, accumulator)
    } else {
        fma.mul_add(weight, sample, accumulator)
    }
}

pub(crate) fn f_resize_accumulate<const FINITE_OPERANDS: bool, F: F64MulAdd>(
    accumulator: &mut f64,
    weight: f64,
    sample: f32,
    separate_product_add: bool,
    fma: &F,
) {
    let sample = f64::from(sample);
    if separate_product_add {
        // Keep the product out of the following add. LLVM may otherwise
        // contract this expression back into an FMA, defeating the arm64
        // wide-row contract that Pillow uses after 15 taps.
        let product = std::hint::black_box(weight * sample);
        *accumulator += product;
    } else {
        *accumulator = f_resize_mul_add::<FINITE_OPERANDS, _>(fma, weight, sample, *accumulator);
    }
}

#[inline]
pub(crate) fn f_resize_horizontal_eight_tap_sample<const FINITE_OPERANDS: bool, F: F64MulAdd>(
    source_row: &[f32],
    source_x: usize,
    weights: &[f64; 8],
    fma: &F,
) -> Option<f32> {
    let [
        weight0,
        weight1,
        weight2,
        weight3,
        weight4,
        weight5,
        weight6,
        weight7,
    ] = weights;
    let samples = source_row.get(source_x..)?.get(..8)?;
    let [
        sample0,
        sample1,
        sample2,
        sample3,
        sample4,
        sample5,
        sample6,
        sample7,
    ] = samples
    else {
        return None;
    };

    let mut accumulator =
        f_resize_mul_add::<FINITE_OPERANDS, _>(fma, *weight0, f64::from(*sample0), 0.0);
    accumulator =
        f_resize_mul_add::<FINITE_OPERANDS, _>(fma, *weight1, f64::from(*sample1), accumulator);
    accumulator =
        f_resize_mul_add::<FINITE_OPERANDS, _>(fma, *weight2, f64::from(*sample2), accumulator);
    accumulator =
        f_resize_mul_add::<FINITE_OPERANDS, _>(fma, *weight3, f64::from(*sample3), accumulator);
    accumulator =
        f_resize_mul_add::<FINITE_OPERANDS, _>(fma, *weight4, f64::from(*sample4), accumulator);
    accumulator =
        f_resize_mul_add::<FINITE_OPERANDS, _>(fma, *weight5, f64::from(*sample5), accumulator);
    accumulator =
        f_resize_mul_add::<FINITE_OPERANDS, _>(fma, *weight6, f64::from(*sample6), accumulator);
    accumulator =
        f_resize_mul_add::<FINITE_OPERANDS, _>(fma, *weight7, f64::from(*sample7), accumulator);
    Some(accumulator as f32)
}

#[inline]
pub(crate) fn f_resize_horizontal_sample<const FINITE_OPERANDS: bool, F: F64MulAdd>(
    source_row: &[f32],
    first_source_x: i64,
    weights: &[f64],
    fma: &F,
) -> f32 {
    if let Ok(weights) = <&[f64; 8]>::try_from(weights)
        && let Ok(source_x) = usize::try_from(first_source_x)
        && let Some(sample) = f_resize_horizontal_eight_tap_sample::<FINITE_OPERANDS, _>(
            source_row, source_x, weights, fma,
        )
    {
        return sample;
    }

    let vector_product_count = (weights.len() / F_RESIZE_VECTOR_WIDTH) * F_RESIZE_VECTOR_WIDTH;
    let mut accumulator = 0.0f64;
    if vector_product_count == 0 {
        for (offset, &weight) in weights.iter().enumerate() {
            let source_x = (first_source_x + offset as i64) as usize;
            accumulator = f_resize_mul_add::<FINITE_OPERANDS, _>(
                fma,
                weight,
                f64::from(source_row[source_x]),
                accumulator,
            );
        }
    } else {
        for (offset, &weight) in weights.iter().enumerate() {
            let source_x = (first_source_x + offset as i64) as usize;
            f_resize_accumulate::<FINITE_OPERANDS, _>(
                &mut accumulator,
                weight,
                source_row[source_x],
                offset < vector_product_count,
                fma,
            );
        }
    }
    accumulator as f32
}

#[cfg(not(feature = "parallel"))]
pub(crate) fn f_resize_contiguous_eight_tap_coefficients(
    coefficients: &FilterCoeffsF64,
) -> Option<(usize, Vec<(usize, [f64; 8])>)> {
    let first = coefficients
        .weights
        .iter()
        .position(|weights| weights.len() == 8)?;
    let last = coefficients
        .weights
        .iter()
        .rposition(|weights| weights.len() == 8)?;
    let mut fixed = Vec::with_capacity(last - first + 1);
    for index in first..=last {
        let source_x = usize::try_from(*coefficients.xmin.get(index)?).ok()?;
        let weights = <&[f64; 8]>::try_from(coefficients.weights.get(index)?.as_slice()).ok()?;
        fixed.push((source_x, *weights));
    }
    Some((first, fixed))
}

#[inline]
pub(crate) fn f_resize_vertical_sample<const FINITE_OPERANDS: bool, F: F64MulAdd>(
    source: &[f32],
    source_row_stride: usize,
    first_source_y: i64,
    source_x: usize,
    weights: &[f64],
    fma: &F,
) -> f32 {
    if let [
        weight0,
        weight1,
        weight2,
        weight3,
        weight4,
        weight5,
        weight6,
        weight7,
    ] = weights
        && let Ok(first_source_y) = usize::try_from(first_source_y)
    {
        // `precompute_coeffs_f64` clips every coefficient row to the source
        // geometry, and callers pass a source x from that same row's width.
        let first_source_index = first_source_y * source_row_stride + source_x;
        let mut accumulator = f_resize_mul_add::<FINITE_OPERANDS, _>(
            fma,
            *weight0,
            f64::from(source[first_source_index]),
            0.0,
        );
        accumulator = f_resize_mul_add::<FINITE_OPERANDS, _>(
            fma,
            *weight1,
            f64::from(source[first_source_index + source_row_stride]),
            accumulator,
        );
        accumulator = f_resize_mul_add::<FINITE_OPERANDS, _>(
            fma,
            *weight2,
            f64::from(source[first_source_index + source_row_stride * 2]),
            accumulator,
        );
        accumulator = f_resize_mul_add::<FINITE_OPERANDS, _>(
            fma,
            *weight3,
            f64::from(source[first_source_index + source_row_stride * 3]),
            accumulator,
        );
        accumulator = f_resize_mul_add::<FINITE_OPERANDS, _>(
            fma,
            *weight4,
            f64::from(source[first_source_index + source_row_stride * 4]),
            accumulator,
        );
        accumulator = f_resize_mul_add::<FINITE_OPERANDS, _>(
            fma,
            *weight5,
            f64::from(source[first_source_index + source_row_stride * 5]),
            accumulator,
        );
        accumulator = f_resize_mul_add::<FINITE_OPERANDS, _>(
            fma,
            *weight6,
            f64::from(source[first_source_index + source_row_stride * 6]),
            accumulator,
        );
        accumulator = f_resize_mul_add::<FINITE_OPERANDS, _>(
            fma,
            *weight7,
            f64::from(source[first_source_index + source_row_stride * 7]),
            accumulator,
        );
        return accumulator as f32;
    }

    let mut accumulator = 0.0f64;
    for (offset, &weight) in weights.iter().enumerate() {
        let source_y = (first_source_y + offset as i64) as usize;
        accumulator = f_resize_mul_add::<FINITE_OPERANDS, _>(
            fma,
            weight,
            f64::from(source[source_y * source_row_stride + source_x]),
            accumulator,
        );
    }
    accumulator as f32
}

pub(crate) fn f_resize_horizontal_pass<const FINITE_OPERANDS: bool, F: F64MulAdd>(
    source: &[f32],
    output: &mut [f32],
    source_width: usize,
    destination_width: usize,
    _source_height: usize,
    coefficients: &FilterCoeffsF64,
    fma: &F,
) {
    #[cfg(feature = "parallel")]
    crate::par_rows_mut_typed!(
        output,
        destination_width,
        _source_height,
        |_row_start, _row_end, source_y, row| {
            let source_row_start = source_y as usize * source_width;
            let source_row = &source[source_row_start..source_row_start + source_width];
            for (destination_x, output) in row.iter_mut().enumerate() {
                *output = f_resize_horizontal_sample::<FINITE_OPERANDS, _>(
                    source_row,
                    coefficients.xmin[destination_x],
                    &coefficients.weights[destination_x],
                    fma,
                );
            }
        }
    );
    #[cfg(not(feature = "parallel"))]
    {
        let eight_tap_coefficients = f_resize_contiguous_eight_tap_coefficients(coefficients);
        for (source_y, row) in output.chunks_mut(destination_width).enumerate() {
            let source_row_start = source_y * source_width;
            let source_row = &source[source_row_start..source_row_start + source_width];
            if let Some((first_eight_tap, fixed_weights)) = &eight_tap_coefficients {
                let end_eight_tap = *first_eight_tap + fixed_weights.len();
                for (destination_x, output) in row[..*first_eight_tap].iter_mut().enumerate() {
                    *output = f_resize_horizontal_sample::<FINITE_OPERANDS, _>(
                        source_row,
                        coefficients.xmin[destination_x],
                        &coefficients.weights[destination_x],
                        fma,
                    );
                }
                for (offset, output) in row[*first_eight_tap..end_eight_tap].iter_mut().enumerate()
                {
                    let destination_x = *first_eight_tap + offset;
                    let (source_x, weights) = fixed_weights[offset];
                    *output = f_resize_horizontal_eight_tap_sample::<FINITE_OPERANDS, _>(
                        source_row, source_x, &weights, fma,
                    )
                    .unwrap_or_else(|| {
                        f_resize_horizontal_sample::<FINITE_OPERANDS, _>(
                            source_row,
                            coefficients.xmin[destination_x],
                            &coefficients.weights[destination_x],
                            fma,
                        )
                    });
                }
                for (offset, output) in row[end_eight_tap..].iter_mut().enumerate() {
                    let destination_x = end_eight_tap + offset;
                    *output = f_resize_horizontal_sample::<FINITE_OPERANDS, _>(
                        source_row,
                        coefficients.xmin[destination_x],
                        &coefficients.weights[destination_x],
                        fma,
                    );
                }
            } else {
                for (destination_x, output) in row.iter_mut().enumerate() {
                    *output = f_resize_horizontal_sample::<FINITE_OPERANDS, _>(
                        source_row,
                        coefficients.xmin[destination_x],
                        &coefficients.weights[destination_x],
                        fma,
                    );
                }
            }
        }
    }
}

pub(crate) fn f_resize_vertical_pass<const FINITE_OPERANDS: bool, F: F64MulAdd>(
    source: &[f32],
    output: &mut [f32],
    destination_width: usize,
    _destination_height: usize,
    coefficients: &FilterCoeffsF64,
    fma: &F,
) {
    #[cfg(feature = "parallel")]
    crate::par_rows_mut_typed!(
        output,
        destination_width,
        _destination_height,
        |_row_start, _row_end, destination_y, row| {
            let source_y = coefficients.xmin[destination_y as usize];
            let weights = &coefficients.weights[destination_y as usize];
            for (source_x, output) in row.iter_mut().enumerate() {
                // Pillow stores the f32 accumulator directly; retain its
                // signed-zero result at this observable pass boundary.
                *output = f_resize_vertical_sample::<FINITE_OPERANDS, _>(
                    source,
                    destination_width,
                    source_y,
                    source_x,
                    weights,
                    fma,
                );
            }
        }
    );
    #[cfg(not(feature = "parallel"))]
    {
        // Keep one f64 accumulator per output column and visit source rows in
        // tap order. Each column still receives the same ordered FMA sequence
        // as the scalar sampler, while the source is read a row at a time.
        let mut accumulators = vec![0.0f64; destination_width];
        for (destination_y, row) in output.chunks_mut(destination_width).enumerate() {
            let source_y = coefficients.xmin[destination_y];
            let weights = &coefficients.weights[destination_y];
            float_resize::accumulate_row::<FINITE_OPERANDS, _>(
                source,
                destination_width,
                source_y,
                weights,
                &mut accumulators,
                fma,
            );
            for (output, &accumulator) in row.iter_mut().zip(&accumulators) {
                // Preserve Pillow's f32 pass boundary, including signed zero.
                *output = accumulator as f32;
            }
        }
    }
}

/// Run Pillow's tall-image resample ordering for an F image.
///
/// `PIL.Image.Image.resize` avoids a numerically unstable horizontal-first
/// reduction when the source is more than 100 times taller than it is wide.
/// It first resizes vertically to `(source_width, destination_height)`, then
/// resizes that intermediate horizontally.  The intermediate is still stored
/// as FLOAT32 between the two native `Resample.c` passes.
pub(crate) fn resize_f_tall_order<F: F64MulAdd>(
    src_floats: &[f32],
    source_width: u32,
    source_height: u32,
    destination_width: u32,
    destination_height: u32,
    filter: &ResampleFilter,
    fma: &F,
) -> Vec<f32> {
    let (kernel, support) = resample_kernel(filter);
    let vertical = precompute_coeffs_f64(destination_height, source_height, kernel, support);
    let source_width_usize = source_width as usize;
    let destination_height_usize = destination_height as usize;
    let mut vertical_output = vec![0.0f32; source_width_usize * destination_height_usize];

    // This is the first pass in Pillow's tall-image branch.  Each destination
    // row is independent, so retain the normal row-parallel CPU contract.
    #[cfg(feature = "parallel")]
    crate::par_rows_mut_typed!(
        &mut vertical_output,
        source_width_usize,
        destination_height_usize,
        |_row_start, _row_end, destination_y, row| {
            let y0 = vertical.xmin[destination_y as usize];
            let weights = &vertical.weights[destination_y as usize];
            for (source_x, output) in row.iter_mut().enumerate() {
                *output = f_resize_vertical_sample::<false, _>(
                    src_floats,
                    source_width_usize,
                    y0,
                    source_x,
                    weights,
                    fma,
                );
            }
        }
    );
    #[cfg(not(feature = "parallel"))]
    for (destination_y, row) in vertical_output.chunks_mut(source_width_usize).enumerate() {
        let y0 = vertical.xmin[destination_y];
        let weights = &vertical.weights[destination_y];
        for (source_x, output) in row.iter_mut().enumerate() {
            *output = f_resize_vertical_sample::<false, _>(
                src_floats,
                source_width_usize,
                y0,
                source_x,
                weights,
                fma,
            );
        }
    }

    // The second pass resamples each already-materialized vertical row from
    // source_width to destination_width.  The >15-tap horizontal product/add
    // split is the same arm64 FLOAT32 contract used by the ordinary path.
    let horizontal = precompute_coeffs_f64(destination_width, source_width, kernel, support);
    let destination_width_usize = destination_width as usize;
    let mut output = vec![0.0f32; destination_width_usize * destination_height_usize];
    #[cfg(feature = "parallel")]
    crate::par_rows_mut_typed!(
        &mut output,
        destination_width_usize,
        destination_height_usize,
        |_row_start, _row_end, destination_y, row| {
            let source_row_start = destination_y as usize * source_width_usize;
            for (destination_x, output) in row.iter_mut().enumerate() {
                let x0 = horizontal.xmin[destination_x];
                let vector_product_count = (horizontal.weights[destination_x].len()
                    / F_RESIZE_VECTOR_WIDTH)
                    * F_RESIZE_VECTOR_WIDTH;
                let mut accumulator = 0.0f64;
                for (offset, &weight) in horizontal.weights[destination_x].iter().enumerate() {
                    let source_x = (x0 + offset as i64) as usize;
                    f_resize_accumulate::<false, _>(
                        &mut accumulator,
                        weight,
                        vertical_output[source_row_start + source_x],
                        offset < vector_product_count,
                        fma,
                    );
                }
                *output = accumulator as f32;
            }
        }
    );
    #[cfg(not(feature = "parallel"))]
    for (destination_y, row) in output.chunks_mut(destination_width_usize).enumerate() {
        let source_row_start = destination_y * source_width_usize;
        for (destination_x, output) in row.iter_mut().enumerate() {
            let x0 = horizontal.xmin[destination_x];
            let vector_product_count = (horizontal.weights[destination_x].len()
                / F_RESIZE_VECTOR_WIDTH)
                * F_RESIZE_VECTOR_WIDTH;
            let mut accumulator = 0.0f64;
            for (offset, &weight) in horizontal.weights[destination_x].iter().enumerate() {
                let source_x = (x0 + offset as i64) as usize;
                f_resize_accumulate::<false, _>(
                    &mut accumulator,
                    weight,
                    vertical_output[source_row_start + source_x],
                    offset < vector_product_count,
                    fma,
                );
            }
            *output = accumulator as f32;
        }
    }
    output
}

// ── F-mode / I-mode resize ──

/// Resize an F-mode image (32-bit floats stored as RGBA8 bytes).
/// Uses PIL-compatible direct 2D interpolation with f64 precision,
/// so the result matches PIL's Image.resize() on mode F images.
pub(crate) fn resize_f(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    filter: &ResampleFilter,
) -> Result<DynamicImage, PilError> {
    #[cfg(target_arch = "x86_64")]
    if let Some(fma) = X86FmaToken::detect() {
        return resize_f_with_fma(img, dst_w, dst_h, filter, &fma);
    }
    resize_f_with_fma(img, dst_w, dst_h, filter, &PortableFma)
}

pub(crate) fn resize_f_with_fma<F: F64MulAdd>(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    filter: &ResampleFilter,
    fma: &F,
) -> Result<DynamicImage, PilError> {
    let (sw, sh) = img.dimensions();

    if dst_w == 0 || dst_h == 0 || sw == 0 || sh == 0 {
        return Ok(DynamicImage::new_rgba8(dst_w, dst_h));
    }
    if (dst_w, dst_h) == (sw, sh) {
        return Ok(img.clone());
    }

    // F-mode images arrive here as their native Rgba8 storage.  Inspect that
    // storage directly before the generic `to_rgba8` clone and f32 decode;
    // Pillow's normalized F-mode resampler preserves an ordinary finite
    // constant sample exactly, so the cache-control resize can be filled
    // without either pass.  Keep negative zero out of this fast path: the
    // scalar path below preserves its sign bit when Pillow's convolution
    // arithmetic produces one.
    if let DynamicImage::ImageRgba8(rgba) = img {
        let raw = rgba.as_raw();
        if let Some(first) = raw.get(..4) {
            let bits = u32::from_le_bytes([first[0], first[1], first[2], first[3]]);
            let constant = f32::from_bits(bits);
            if constant.is_finite()
                && constant.to_bits() != (-0.0f32).to_bits()
                && raw.chunks_exact(4).all(|sample| {
                    u32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]) == bits
                })
            {
                let output_len = (dst_w as usize)
                    .saturating_mul(dst_h as usize)
                    .saturating_mul(4);
                let rgba_bytes = constant.to_le_bytes().repeat(output_len / 4);
                let out = crate::raster::RgbaImage::from_raw(dst_w, dst_h, rgba_bytes)
                    .expect("resize_f constant output shape must match its dimensions");
                return Ok(DynamicImage::ImageRgba8(out));
            }
        }
    }

    // F-mode is stored in ImageRgba8 only as a four-byte carrier for scalar
    // f32 samples. Borrow those words directly; `to_rgba8()` clones the whole
    // input even though it performs no channel conversion for this layout.
    let source_bytes = match img {
        DynamicImage::ImageRgba8(rgba) => rgba.as_raw().as_slice(),
        _ => {
            return Err(PilError::ValueError(
                "F-mode resize requires four-byte sample storage".into(),
            ));
        }
    };
    let source_pixels = CheckedDims::new_allow_empty(sw, sh, 4)?.total_pixels();
    let src_floats = f32_samples_from_le_bytes(source_bytes, source_pixels);

    // Pillow's F-mode ImagingResample keeps an ordinary finite constant sample
    // unchanged because each normalized horizontal/vertical coefficient row
    // sums to one.  Avoid decoding the same value through both convolution
    // passes for this common cache-control workload; negative zero, non-finite,
    // and mixed samples retain the exact scalar path below.
    if let Some(&constant) = src_floats.first()
        && constant.is_finite()
        && constant.to_bits() != (-0.0f32).to_bits()
        && src_floats
            .iter()
            .all(|value| value.to_bits() == constant.to_bits())
    {
        let output_len = (dst_w as usize)
            .saturating_mul(dst_h as usize)
            .saturating_mul(4);
        let rgba_bytes = constant.to_le_bytes().repeat(output_len / 4);
        let out = crate::raster::RgbaImage::from_raw(dst_w, dst_h, rgba_bytes)
            .expect("resize_f constant output shape must match its dimensions");
        return Ok(DynamicImage::ImageRgba8(out));
    }

    if matches!(filter, ResampleFilter::Nearest) {
        return Ok(float_resize::nearest(&src_floats, (sw, sh), (dst_w, dst_h)));
    }

    // Pillow's high-level Image.resize takes a vertical-first path for very
    // tall images before invoking Resample.c.  A direct horizontal-first
    // reduction is observably different for long F rows (for example, the
    // native 2x16384 -> 1x1 BICUBIC and BOX results differ by one ULP), so
    // preserve that axis order and FLOAT32 intermediate boundary here.
    if sh > sw.saturating_mul(100) && dst_h < sh && dst_w != sw {
        let out_floats = resize_f_tall_order(&src_floats, sw, sh, dst_w, dst_h, filter, fma);
        let rgba_bytes: Vec<u8> = out_floats.iter().flat_map(|f| f.to_le_bytes()).collect();
        let out = crate::raster::RgbaImage::from_raw(dst_w, dst_h, rgba_bytes)
            .expect("resize_f tall-order output shape must match its dimensions");
        return Ok(DynamicImage::ImageRgba8(out));
    }

    // The x86 finite-only FMA route is admitted only after proving the source
    // samples finite. Portable and other architecture paths skip this scan.
    let finite_source_samples = f_resize_samples_allow_finite_fma(fma, &src_floats);
    let (kernel, support) = resample_kernel(filter);
    let needs_horizontal = dst_w != sw;
    let needs_vertical = dst_h != sh;

    // Resample.c skips a pass when its destination axis already matches the
    // source axis. When a horizontal pass is needed, its FLOAT32 output is
    // stored in the intermediate image before the vertical pass reads it;
    // retaining f64 values here introduces tiny side lobes that Pillow does
    // not serialize.
    let mut intermediate = vec![0.0f32; (sh * dst_w) as usize];
    if needs_horizontal {
        let h_coeffs = precompute_coeffs_f64(dst_w, sw, kernel, support);
        if finite_source_samples && f_resize_coefficients_allow_finite_fma(&h_coeffs) {
            f_resize_horizontal_pass::<true, _>(
                &src_floats,
                &mut intermediate,
                sw as usize,
                dst_w as usize,
                sh as usize,
                &h_coeffs,
                fma,
            );
        } else {
            f_resize_horizontal_pass::<false, _>(
                &src_floats,
                &mut intermediate,
                sw as usize,
                dst_w as usize,
                sh as usize,
                &h_coeffs,
                fma,
            );
        }
    } else {
        #[cfg(feature = "parallel")]
        crate::par_rows_mut_typed!(
            &mut intermediate,
            dst_w as usize,
            sh as usize,
            |_row_start, _row_end, sy, row| {
                let source_start = (sy * sw) as usize;
                row.copy_from_slice(&src_floats[source_start..source_start + sw as usize]);
            }
        );
        #[cfg(not(feature = "parallel"))]
        for (sy, row) in intermediate.chunks_mut(dst_w as usize).enumerate() {
            let source_start = sy * sw as usize;
            row.copy_from_slice(&src_floats[source_start..source_start + sw as usize]);
        }
    }

    // On the serial CPU route, the vertical pass is the final F32 stage. Write
    // each rounded sample into its packed pixel slot and flatten the array
    // vector without copying, instead of allocating an F32 output and then
    // allocating and filling a second byte vector.
    #[cfg(not(feature = "parallel"))]
    if needs_vertical {
        return float_resize::packed_vertical_output(
            &intermediate,
            sh,
            dst_w,
            dst_h,
            needs_horizontal,
            finite_source_samples,
            filter,
            fma,
        );
    }

    let out_floats: Vec<f32> = if needs_vertical {
        let v_coeffs = precompute_coeffs_f64(dst_h, sh, kernel, support);
        let mut output = vec![0.0f32; (dst_w * dst_h) as usize];
        let vertical_samples_are_finite = if needs_horizontal {
            f_resize_samples_allow_finite_fma(fma, &intermediate)
        } else {
            finite_source_samples
        };
        if vertical_samples_are_finite && f_resize_coefficients_allow_finite_fma(&v_coeffs) {
            f_resize_vertical_pass::<true, _>(
                &intermediate,
                &mut output,
                dst_w as usize,
                dst_h as usize,
                &v_coeffs,
                fma,
            );
        } else {
            f_resize_vertical_pass::<false, _>(
                &intermediate,
                &mut output,
                dst_w as usize,
                dst_h as usize,
                &v_coeffs,
                fma,
            );
        }
        output
    } else {
        intermediate
    };

    // Re-pack each f32 as 4 RGBA8 bytes (little-endian).
    let rgba_bytes: Vec<u8> = out_floats.iter().flat_map(|f| f.to_le_bytes()).collect();
    let out = crate::raster::RgbaImage::from_raw(dst_w, dst_h, rgba_bytes)
        .expect("resize_f output shape must match its dimensions");
    Ok(DynamicImage::ImageRgba8(out))
}
