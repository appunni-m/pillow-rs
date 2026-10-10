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
use crate::image_utils::raw_bytes_to_image;
#[cfg(target_arch = "x86_64")]
use crate::ops::pil_resize::X86FmaToken;
use crate::ops::pil_resize::{
    F64MulAdd, FilterCoeffsF64, PortableFma, f_resize_coefficients_allow_finite_fma,
    f_resize_samples_allow_finite_fma, f32_samples_from_le_bytes, i32_samples_from_le_bytes,
    pil_resize, pil_resize_boxed, pillow_sin_f64, precompute_coeffs_f64,
    precompute_coeffs_f64_boxed, premultiply_alpha, round_up, unpremultiply_alpha,
};
use crate::pipeline::{PipelineOp, ResampleFilter, TransposeMethod};

// ── PIL-compatible filter kernels (f64 precision) ──

/// Box / Nearest-neighbor kernel.
fn f_kernel_box(x: f64) -> f64 {
    if x > -0.5 && x <= 0.5 { 1.0 } else { 0.0 }
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
        // Match Pillow's Resample.c Horner evaluation exactly.  Expanding
        // this polynomial with powi changes the last f64 bits for some
        // heterogeneous F samples, which becomes a visible f32 ULP after
        // ImagingResample stores the accumulated value.
        let t = 1.5f64.mul_add(a, -2.5);
        a.mul_add(t * a, 1.0)
    } else if a < 2.0 {
        let t = (a - 5.0).mul_add(a, 8.0);
        a.mul_add(t, -4.0) * -0.5
    } else {
        0.0
    }
}

/// Lanczos kernel with window `a`.
fn f_kernel_lanczos(x: f64, a: f64) -> f64 {
    // Resample.c uses `-a <= x && x < a`, so the negative support edge is
    // evaluated (and can produce a tiny signed coefficient) while the
    // positive edge is excluded.
    if x < -a || x >= a {
        return 0.0;
    }
    let sinc = |value: f64| {
        if value == 0.0 {
            1.0
        } else {
            let pix = value * std::f64::consts::PI;
            pillow_sin_f64(pix) / pix
        }
    };
    sinc(x) * sinc(x / a)
}

/// Hamming kernel.
fn f_kernel_hamming(x: f64) -> f64 {
    let x = x.abs();
    if x == 0.0 {
        // Resample.c has an exact-zero special case.  Values merely close to
        // zero still run through sin/cos; the float constants then leave a
        // visible residual (0.54f + 0.46f) in cancellation-sensitive F rows.
        return 1.0;
    }
    if x >= 1.0 {
        return 0.0;
    }
    // Keep the numeric F/I paths aligned with Pillow's Hamming windowed-sinc
    // resampler (Resample.c), including its sinc factor.
    let pix = std::f64::consts::PI * x;
    // Resample.c uses sincos and contracts the `0.46f * cos + 0.54f` window
    // expression; preserving those operations matters for exact f32
    // cancellation residuals.
    let (sin, cos) = pix.sin_cos();
    (sin / pix) * cos.mul_add(0.46_f32 as f64, 0.54_f32 as f64)
}

fn f_kernel_lanczos3(x: f64) -> f64 {
    f_kernel_lanczos(x, 3.0)
}

/// Returns the scalar kernel and support used to build Pillow-compatible
/// coefficients. SIMD uses this only for its control-plane coefficient table;
/// pixel accumulation remains in the SIMD adapter.
pub(crate) fn resample_kernel(filter: &ResampleFilter) -> (fn(f64) -> f64, f64) {
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

// Pillow 12.2.0's arm64 FLOAT32 horizontal resampler uses scalar FMA for
// rows with at most 15 taps, then switches to complete 16-tap vector
// product/add blocks; any tail remains scalar FMA. The vertical resampler
// remains scalar FMA for every tap count. Keep the same split in the exact CPU
// implementation so heterogeneous wide reductions match the native Pillow
// build rather than the compiler's fused Rust loop.
const F_RESIZE_VECTOR_WIDTH: usize = 16;

fn f_resize_mul_add<const FINITE_OPERANDS: bool, F: F64MulAdd>(
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

fn f_resize_accumulate<const FINITE_OPERANDS: bool, F: F64MulAdd>(
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
fn f_resize_horizontal_sample<const FINITE_OPERANDS: bool, F: F64MulAdd>(
    source: &[f32],
    source_row_start: usize,
    first_source_x: i64,
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
        && let Ok(source_x) = usize::try_from(first_source_x)
        && let Some(source_row) = source.get(source_row_start..)
        && let Some(source_row) = source_row.get(source_x..)
        && let Some(
            [
                sample0,
                sample1,
                sample2,
                sample3,
                sample4,
                sample5,
                sample6,
                sample7,
            ],
        ) = source_row.get(..8)
    {
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
        return accumulator as f32;
    }

    let vector_product_count = (weights.len() / F_RESIZE_VECTOR_WIDTH) * F_RESIZE_VECTOR_WIDTH;
    let mut accumulator = 0.0f64;
    if vector_product_count == 0 {
        for (offset, &weight) in weights.iter().enumerate() {
            let source_x = (first_source_x + offset as i64) as usize;
            accumulator = f_resize_mul_add::<FINITE_OPERANDS, _>(
                fma,
                weight,
                f64::from(source[source_row_start + source_x]),
                accumulator,
            );
        }
    } else {
        for (offset, &weight) in weights.iter().enumerate() {
            let source_x = (first_source_x + offset as i64) as usize;
            f_resize_accumulate::<FINITE_OPERANDS, _>(
                &mut accumulator,
                weight,
                source[source_row_start + source_x],
                offset < vector_product_count,
                fma,
            );
        }
    }
    accumulator as f32
}

#[inline]
fn f_resize_vertical_sample<const FINITE_OPERANDS: bool, F: F64MulAdd>(
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

fn f_resize_horizontal_pass<const FINITE_OPERANDS: bool, F: F64MulAdd>(
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
            for (destination_x, output) in row.iter_mut().enumerate() {
                *output = f_resize_horizontal_sample::<FINITE_OPERANDS, _>(
                    source,
                    source_row_start,
                    coefficients.xmin[destination_x],
                    &coefficients.weights[destination_x],
                    fma,
                );
            }
        }
    );
    #[cfg(not(feature = "parallel"))]
    for (source_y, row) in output.chunks_mut(destination_width).enumerate() {
        let source_row_start = source_y * source_width;
        for (destination_x, output) in row.iter_mut().enumerate() {
            *output = f_resize_horizontal_sample::<FINITE_OPERANDS, _>(
                source,
                source_row_start,
                coefficients.xmin[destination_x],
                &coefficients.weights[destination_x],
                fma,
            );
        }
    }
}

fn f_resize_vertical_pass<const FINITE_OPERANDS: bool, F: F64MulAdd>(
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
            accumulators.fill(0.0);
            for (offset, &weight) in weights.iter().enumerate() {
                let source_row_start = (source_y + offset as i64) as usize * destination_width;
                let source_row = &source[source_row_start..source_row_start + destination_width];
                for (accumulator, &sample) in accumulators.iter_mut().zip(source_row) {
                    *accumulator = f_resize_mul_add::<FINITE_OPERANDS, _>(
                        fma,
                        weight,
                        f64::from(sample),
                        *accumulator,
                    );
                }
            }
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
fn resize_f_tall_order<F: F64MulAdd>(
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
fn resize_f(
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

fn resize_f_with_fma<F: F64MulAdd>(
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

    // Pillow's F-mode NEAREST resize uses the affine point-sampling path,
    // not the BOX convolution path used by the other filters. The source
    // coordinate advances cumulatively from half a destination pixel, which
    // is observable when a narrow impulse falls between sampled rows.
    if matches!(filter, ResampleFilter::Nearest) {
        let scale_x = sw as f64 / dst_w as f64;
        let scale_y = sh as f64 / dst_h as f64;
        let mut xintab = Vec::with_capacity(dst_w as usize);
        let mut source_x = scale_x * 0.5;
        for _ in 0..dst_w {
            let sx = source_x as u32;
            xintab.push(sx.min(sw - 1));
            source_x += scale_x;
        }
        // Pillow's `libImaging/Geometry.c::ImagingScaleAffine` initializes the
        // vertical coordinate once and advances it after each row. Keep that
        // cumulative f64 sequence
        // instead of recomputing `(y + 0.5) * scale_y`: the two expressions
        // differ at exact-integer boundaries (for example, 2 / 7 reaches
        // 1.0 when multiplied directly but remains just below 1.0 after
        // three cumulative additions), changing the selected source row.
        let mut yintab = Vec::with_capacity(dst_h as usize);
        let mut source_y = scale_y * 0.5;
        for _ in 0..dst_h {
            let sy = source_y as u32;
            yintab.push(sy.min(sh - 1));
            source_y += scale_y;
        }
        let mut out_floats = vec![0.0f32; (dst_w * dst_h) as usize];
        #[cfg(feature = "parallel")]
        crate::par_rows_mut_typed!(
            &mut out_floats,
            dst_w as usize,
            dst_h as usize,
            |_row_start, _row_end, y, row| {
                let sy = yintab[y as usize];
                for (out, &sx) in row.iter_mut().zip(&xintab) {
                    *out = src_floats[(sy * sw + sx) as usize];
                }
            }
        );
        #[cfg(not(feature = "parallel"))]
        for (y, row) in out_floats.chunks_mut(dst_w as usize).enumerate() {
            let sy = yintab[y];
            for (out, &sx) in row.iter_mut().zip(&xintab) {
                *out = src_floats[(sy * sw + sx) as usize];
            }
        }
        let rgba_bytes: Vec<u8> = out_floats.iter().flat_map(|f| f.to_le_bytes()).collect();
        // The loop emits exactly four bytes per checked output pixel, so a
        // shape failure here indicates an internal arithmetic regression, not
        // a public input error.
        let out = crate::raster::RgbaImage::from_raw(dst_w, dst_h, rgba_bytes)
            .expect("resize_f nearest output shape must match its dimensions");
        return Ok(DynamicImage::ImageRgba8(out));
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

/// Resize an I-mode image (32-bit signed integers stored as RGBA8 bytes LE).
/// Uses PIL's two-pass separable approach matching ImagingResample.
/// The common eight-tap interior keeps Pillow's sequential FMA order; clipped
/// edge spans retain the general tap loop.
#[inline(always)]
fn resize_i_mul_add<F: F64MulAdd>(fma: &F, weight: f64, sample: i32, accumulator: f64) -> f64 {
    fma.mul_add(weight, f64::from(sample), accumulator)
}

#[inline(always)]
fn resize_i_sum_eight<F: F64MulAdd>(
    weights: &[f64; 8],
    first_sample: usize,
    sample: impl Fn(usize) -> i32,
    fma: &F,
) -> f64 {
    let mut accumulator = resize_i_mul_add(fma, weights[0], sample(first_sample), 0.0);
    accumulator = resize_i_mul_add(fma, weights[1], sample(first_sample + 1), accumulator);
    accumulator = resize_i_mul_add(fma, weights[2], sample(first_sample + 2), accumulator);
    accumulator = resize_i_mul_add(fma, weights[3], sample(first_sample + 3), accumulator);
    accumulator = resize_i_mul_add(fma, weights[4], sample(first_sample + 4), accumulator);
    accumulator = resize_i_mul_add(fma, weights[5], sample(first_sample + 5), accumulator);
    accumulator = resize_i_mul_add(fma, weights[6], sample(first_sample + 6), accumulator);
    resize_i_mul_add(fma, weights[7], sample(first_sample + 7), accumulator)
}

#[inline(always)]
#[cfg(not(feature = "parallel"))]
fn resize_i_sum_eight_contiguous<F: F64MulAdd>(
    weights: &[f64; 8],
    source: &[i32],
    first_sample: usize,
    fma: &F,
) -> f64 {
    let samples: &[i32; 8] = source[first_sample..first_sample + 8]
        .try_into()
        .expect("eight-tap resize span must contain eight samples");
    let mut accumulator = resize_i_mul_add(fma, weights[0], samples[0], 0.0);
    accumulator = resize_i_mul_add(fma, weights[1], samples[1], accumulator);
    accumulator = resize_i_mul_add(fma, weights[2], samples[2], accumulator);
    accumulator = resize_i_mul_add(fma, weights[3], samples[3], accumulator);
    accumulator = resize_i_mul_add(fma, weights[4], samples[4], accumulator);
    accumulator = resize_i_mul_add(fma, weights[5], samples[5], accumulator);
    accumulator = resize_i_mul_add(fma, weights[6], samples[6], accumulator);
    resize_i_mul_add(fma, weights[7], samples[7], accumulator)
}

#[inline(always)]
#[cfg(not(feature = "parallel"))]
fn resize_i_sum_general<F: F64MulAdd>(
    weights: &[f64],
    first_sample: usize,
    sample: impl Fn(usize) -> i32,
    fma: &F,
) -> f64 {
    let mut accumulator = 0.0_f64;
    for (offset, &weight) in weights.iter().enumerate() {
        accumulator = resize_i_mul_add(fma, weight, sample(first_sample + offset), accumulator);
    }
    accumulator
}

/// Convert a serial I-resize accumulator with Pillow's sign-aware half offset.
/// Casting to i32 already truncates, so avoid a separate float truncation step.
#[inline(always)]
#[cfg(not(feature = "parallel"))]
fn resize_i_round_up_to_i32(value: f64) -> i32 {
    (value + 0.5_f64.copysign(value)) as i32
}

/// Copy one contiguous run of eight-tap coefficients into a dense fixed-width
/// table so row resampling does not reclassify the same horizontal spans.
#[cfg(not(feature = "parallel"))]
fn resize_i_contiguous_eight_tap_weights(
    coefficients: &[Vec<f64>],
) -> Option<(usize, Vec<[f64; 8]>)> {
    let first = coefficients.iter().position(|weights| weights.len() == 8)?;
    let last = coefficients
        .iter()
        .rposition(|weights| weights.len() == 8)?
        .checked_add(1)?;
    let spans = coefficients.get(first..last)?;
    if spans.iter().any(|weights| weights.len() != 8) {
        return None;
    }

    let mut fixed_width = Vec::with_capacity(spans.len());
    for weights in spans {
        let fixed: &[f64; 8] = weights.as_slice().try_into().ok()?;
        fixed_width.push(*fixed);
    }
    Some((first, fixed_width))
}

#[cfg(not(feature = "parallel"))]
fn resize_i_horizontal_row<F: F64MulAdd>(
    source_row: &[i32],
    output_row: &mut [i32],
    coefficients: &FilterCoeffsF64,
    fixed_weights: Option<&(usize, Vec<[f64; 8]>)>,
    fma: &F,
) {
    if let Some((first_eight_tap, fixed_weights)) = fixed_weights {
        for dx in 0..*first_eight_tap {
            let x0 = coefficients.xmin[dx];
            let accumulator = resize_i_sum_general(
                &coefficients.weights[dx],
                x0 as usize,
                |sx| source_row[sx],
                fma,
            );
            output_row[dx] = resize_i_round_up_to_i32(accumulator);
        }

        let eight_tap_end = first_eight_tap + fixed_weights.len();
        for (dx, weights) in (*first_eight_tap..eight_tap_end).zip(fixed_weights) {
            let accumulator = resize_i_sum_eight_contiguous(
                weights,
                source_row,
                coefficients.xmin[dx] as usize,
                fma,
            );
            output_row[dx] = resize_i_round_up_to_i32(accumulator);
        }

        for dx in eight_tap_end..output_row.len() {
            let x0 = coefficients.xmin[dx];
            let accumulator = resize_i_sum_general(
                &coefficients.weights[dx],
                x0 as usize,
                |sx| source_row[sx],
                fma,
            );
            output_row[dx] = resize_i_round_up_to_i32(accumulator);
        }
        return;
    }

    for (dx, output) in output_row.iter_mut().enumerate() {
        let x0 = coefficients.xmin[dx];
        let weights = &coefficients.weights[dx];
        let accumulator = if let Ok(weights) = <&[f64; 8]>::try_from(weights.as_slice()) {
            resize_i_sum_eight_contiguous(weights, source_row, x0 as usize, fma)
        } else {
            resize_i_sum_general(weights, x0 as usize, |sx| source_row[sx], fma)
        };
        *output = resize_i_round_up_to_i32(accumulator);
    }
}

#[cfg(not(feature = "parallel"))]
fn resize_i_streaming_vertical_pass<F: F64MulAdd>(
    source: &[i32],
    source_width: usize,
    destination_width: usize,
    vertical: &FilterCoeffsF64,
    horizontal: &FilterCoeffsF64,
    fixed_horizontal_weights: Option<&(usize, Vec<[f64; 8]>)>,
    fma: &F,
    ring: &mut [i32],
    ring_mask: usize,
    output_bytes: &mut [u8],
    output_stride: usize,
) {
    let mut next_source_row = 0_usize;
    for (dy, output_row) in output_bytes.chunks_mut(output_stride).enumerate() {
        let y0 = vertical.xmin[dy] as usize;
        let weights = &vertical.weights[dy];
        let source_end = y0 + weights.len();
        while next_source_row < source_end {
            let source_start = next_source_row * source_width;
            let source_row = &source[source_start..source_start + source_width];
            let ring_start = (next_source_row & ring_mask) * destination_width;
            let horizontal_row = &mut ring[ring_start..ring_start + destination_width];
            resize_i_horizontal_row(
                source_row,
                horizontal_row,
                horizontal,
                fixed_horizontal_weights,
                fma,
            );
            next_source_row += 1;
        }

        if let Ok(weights) = <&[f64; 8]>::try_from(weights.as_slice()) {
            for (dx, output) in output_row.chunks_exact_mut(4).enumerate() {
                let accumulator = resize_i_sum_eight(
                    weights,
                    y0,
                    |sy| ring[(sy & ring_mask) * destination_width + dx],
                    fma,
                );
                output.copy_from_slice(&resize_i_round_up_to_i32(accumulator).to_le_bytes());
            }
        } else {
            for (dx, output) in output_row.chunks_exact_mut(4).enumerate() {
                let accumulator = resize_i_sum_general(
                    weights,
                    y0,
                    |sy| ring[(sy & ring_mask) * destination_width + dx],
                    fma,
                );
                output.copy_from_slice(&resize_i_round_up_to_i32(accumulator).to_le_bytes());
            }
        }
    }
}

#[cfg(not(feature = "parallel"))]
fn resize_i_streaming_with_fma<F: F64MulAdd>(
    source: &[i32],
    source_width: u32,
    destination_width: u32,
    destination_height: u32,
    horizontal: &FilterCoeffsF64,
    vertical: &FilterCoeffsF64,
    fixed_horizontal_weights: Option<&(usize, Vec<[f64; 8]>)>,
    fma: &F,
) -> Result<DynamicImage, PilError> {
    let max_vertical_taps = vertical
        .weights
        .iter()
        .map(|weights| weights.len())
        .max()
        .unwrap_or(0);
    let ring_rows = max_vertical_taps
        .checked_next_power_of_two()
        .ok_or_else(|| {
            PilError::ValueError("resize_i: vertical coefficient span is too large".into())
        })?;
    let ring_samples = ring_rows
        .checked_mul(destination_width as usize)
        .ok_or_else(|| PilError::ValueError("resize_i: vertical ring is too large".into()))?;
    let mut ring = vec![0_i32; ring_samples];
    let output_dims = CheckedDims::new(destination_width, destination_height, 4)?;
    let output_stride = output_dims.row_stride();
    let mut output_bytes = output_dims.alloc_buffer();
    resize_i_streaming_vertical_pass(
        source,
        source_width as usize,
        destination_width as usize,
        vertical,
        horizontal,
        fixed_horizontal_weights,
        fma,
        &mut ring,
        ring_rows - 1,
        &mut output_bytes,
        output_stride,
    );
    let out =
        crate::raster::RgbaImage::from_raw(destination_width, destination_height, output_bytes)
            .ok_or_else(|| {
                PilError::ValueError("resize_i: failed to create output buffer".into())
            })?;
    Ok(DynamicImage::ImageRgba8(out))
}

fn resize_i(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    filter: &ResampleFilter,
) -> Result<DynamicImage, PilError> {
    // Keep Pillow's ordered fused accumulation on x86 even in generic release
    // builds that do not enable FMA at compile time.
    #[cfg(target_arch = "x86_64")]
    if let Some(fma) = X86FmaToken::detect() {
        return resize_i_with_fma(img, dst_w, dst_h, filter, &fma);
    }
    resize_i_with_fma(img, dst_w, dst_h, filter, &PortableFma)
}

fn resize_i_with_fma<F: F64MulAdd>(
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

    let (_, _, source_bytes) = i32_sample_bytes_from_native_storage(img)?;
    let source_pixels = CheckedDims::new_allow_empty(sw, sh, 4)?.total_pixels();
    let src_ints = i32_samples_from_le_bytes(source_bytes, source_pixels);

    let (kernel, support) = resample_kernel(filter);
    let sw_f = sw as f64;
    let sh_f = sh as f64;
    let dw_f = dst_w as f64;
    let dh_f = dst_h as f64;

    // PIL-compatible scale factor for kernel widening during downscaling
    let _sx_scale = (sw_f / dw_f).max(1.0);
    let _sy_scale = (sh_f / dh_f).max(1.0);

    let output_dims = CheckedDims::new(dst_w, dst_h, 4)?;
    let output_stride = output_dims.row_stride();

    // NEAREST: Pillow's mode-I path uses the same half-destination-pixel
    // point samples as its native point resampler:
    //   sx = (int)((dx + 0.5) * sw/dw)
    //   sy = (int)((dy + 0.5) * sh/dh)
    //
    // The older affine-style formula shifts a downsampled I image by one
    // source row/column at the leading edge.  That only becomes visible when
    // a public pipeline composes a typed filter with a non-integral resize,
    // so keep the correction in the native I branch rather than changing the
    // byte-image transform contract.
    if matches!(filter, ResampleFilter::Nearest) {
        let mut output_bytes = output_dims.alloc_buffer();
        #[cfg(feature = "parallel")]
        crate::par_rows_mut!(
            &mut output_bytes,
            output_stride,
            dst_h as usize,
            |_row_start, _row_end, dy, row| {
                let sy = ((f64::from(dy) + 0.5) * sh_f / dh_f).floor() as i64;
                let sy = sy.clamp(0, sh as i64 - 1) as u32;
                for (dx, output) in row.chunks_exact_mut(4).enumerate() {
                    let sx = ((dx as f64 + 0.5) * sw_f / dw_f).floor() as i64;
                    let sx = sx.clamp(0, sw as i64 - 1) as u32;
                    output.copy_from_slice(&src_ints[(sy * sw + sx) as usize].to_le_bytes());
                }
            }
        );
        #[cfg(not(feature = "parallel"))]
        for (dy, row) in output_bytes.chunks_mut(output_stride).enumerate() {
            let sy = ((dy as f64 + 0.5) * sh_f / dh_f).floor() as i64;
            let sy = sy.clamp(0, sh as i64 - 1) as u32;
            for (dx, output) in row.chunks_exact_mut(4).enumerate() {
                let sx = ((dx as f64 + 0.5) * sw_f / dw_f).floor() as i64;
                let sx = sx.clamp(0, sw as i64 - 1) as u32;
                output.copy_from_slice(&src_ints[(sy * sw + sx) as usize].to_le_bytes());
            }
        }
        let out = crate::raster::RgbaImage::from_raw(dst_w, dst_h, output_bytes)
            .expect("resize_i output shape must match its dimensions");
        return Ok(DynamicImage::ImageRgba8(out));
    }

    // PIL: for INT32 images, coefficients stay as double-precision (not fixed-point).
    // Use f64 accumulation + ROUND_UP matching PIL's ImagingResample for 32-bit types.
    let h_coeffs_f64 = precompute_coeffs_f64(dst_w, sw, kernel, support);
    let v_coeffs_f64 = precompute_coeffs_f64(dst_h, sh, kernel, support);
    #[cfg(not(feature = "parallel"))]
    let h_eight_tap_weights = resize_i_contiguous_eight_tap_weights(&h_coeffs_f64.weights);

    #[cfg(not(feature = "parallel"))]
    return resize_i_streaming_with_fma(
        &src_ints,
        sw,
        dst_w,
        dst_h,
        &h_coeffs_f64,
        &v_coeffs_f64,
        h_eight_tap_weights.as_ref(),
        fma,
    );

    // ImagingResample stores the horizontal INT32 pass in an INT32 image
    // before the vertical pass. Keeping this buffer as f64 changes overflow
    // cases (the C cast saturates to INT32_MIN on the supported platforms).
    #[cfg(feature = "parallel")]
    let mut intermediate: Vec<i32> = vec![0; (sh * dst_w) as usize];

    // Horizontal pass: f64 accumulation, matching PIL's double-precision path
    #[cfg(feature = "parallel")]
    crate::par_rows_mut_typed!(
        &mut intermediate,
        dst_w as usize,
        sh as usize,
        |_row_start, _row_end, sy, row| {
            let src_row_base = (sy * sw) as usize;
            for (dx, output) in row.iter_mut().enumerate() {
                let x0 = h_coeffs_f64.xmin[dx];
                let weights = &h_coeffs_f64.weights[dx];
                let accumulator = if let Ok(weights) = <&[f64; 8]>::try_from(weights.as_slice()) {
                    resize_i_sum_eight(weights, x0 as usize, |sx| src_ints[src_row_base + sx], fma)
                } else {
                    let mut accumulator: f64 = 0.0;
                    for (cix, &weight) in weights.iter().enumerate() {
                        let sx = (x0 + cix as i64) as usize;
                        accumulator =
                            resize_i_mul_add(fma, weight, src_ints[src_row_base + sx], accumulator);
                    }
                    accumulator
                };
                *output = round_up(accumulator) as i32;
            }
        }
    );
    // Vertical pass
    #[cfg(feature = "parallel")]
    let mut output_bytes = output_dims.alloc_buffer();
    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        &mut output_bytes,
        output_stride,
        dst_h as usize,
        |_row_start, _row_end, dy, row| {
            let output_y = dy as usize;
            let y0 = v_coeffs_f64.xmin[output_y];
            let weights = &v_coeffs_f64.weights[output_y];
            for (dx, output) in row.chunks_exact_mut(4).enumerate() {
                let accumulator = if let Ok(weights) = <&[f64; 8]>::try_from(weights.as_slice()) {
                    resize_i_sum_eight(
                        weights,
                        y0 as usize,
                        |sy| intermediate[sy * dst_w as usize + dx],
                        fma,
                    )
                } else {
                    let mut accumulator: f64 = 0.0;
                    for (cix, &weight) in weights.iter().enumerate() {
                        let sy = (y0 + cix as i64) as usize;
                        accumulator = resize_i_mul_add(
                            fma,
                            weight,
                            intermediate[(sy * dst_w as usize) + dx],
                            accumulator,
                        );
                    }
                    accumulator
                };
                output.copy_from_slice(&(round_up(accumulator) as i32).to_le_bytes());
            }
        }
    );
    #[cfg(feature = "parallel")]
    let out = crate::raster::RgbaImage::from_raw(dst_w, dst_h, output_bytes)
        .ok_or_else(|| PilError::ValueError("resize_i: failed to create output buffer".into()))?;
    #[cfg(feature = "parallel")]
    Ok(DynamicImage::ImageRgba8(out))
}

/// Borrow I-mode's little-endian four-byte carrier after validating its
/// concrete storage. `I` samples are signed scalar words, not RGBA channels.
fn i32_sample_bytes_from_native_storage(img: &DynamicImage) -> Result<(u32, u32, &[u8]), PilError> {
    let (width, height) = img.dimensions();
    let source_bytes = match img {
        DynamicImage::ImageRgba8(rgba) => rgba.as_raw().as_slice(),
        _ => {
            return Err(PilError::ValueError(
                "I-mode resize requires four-byte sample storage".into(),
            ));
        }
    };
    CheckedDims::new_allow_empty(width, height, 4)?;
    Ok((width, height, source_bytes))
}

/// Borrow aligned little-endian I samples for boxed resize. Keep an exact
/// decoder fallback for unaligned or other-endian source storage.
fn i32_samples_from_native_storage<'a>(
    img: &'a DynamicImage,
) -> Result<(u32, u32, std::borrow::Cow<'a, [i32]>), PilError> {
    let (width, height, source_bytes) = i32_sample_bytes_from_native_storage(img)?;
    let source_pixels = CheckedDims::new_allow_empty(width, height, 4)?.total_pixels();
    let samples = i32_samples_from_le_bytes(source_bytes, source_pixels);
    Ok((width, height, samples))
}

/// Resize an `I` image through a fractional source box.
///
/// Pillow's reducing-gap thumbnail path passes the original source box after
/// the integer reduction. The intermediate image is still an INT32 image, so
/// both separable passes round their f64 sums to i32 before the next pass.
fn resize_i_boxed(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    filter: ResampleFilter,
) -> Result<DynamicImage, PilError> {
    let (source_width, source_height) = img.dimensions();
    if dst_w == 0 || dst_h == 0 || source_width == 0 || source_height == 0 {
        return Ok(DynamicImage::new_rgba8(dst_w, dst_h));
    }
    if box_left == 0.0
        && box_top == 0.0
        && box_right == f64::from(source_width)
        && box_bottom == f64::from(source_height)
    {
        // A full-image box uses the same coefficient domain as ordinary I
        // resize. Reuse its fixed-width tap path and runtime FMA dispatch.
        return resize_i(img, dst_w, dst_h, &filter);
    }
    let (_, _, source) = i32_samples_from_native_storage(img)?;
    let horizontal = precompute_coeffs_f64_boxed(dst_w, source_width, box_left, box_right, filter);
    let vertical = precompute_coeffs_f64_boxed(dst_h, source_height, box_top, box_bottom, filter);

    let mut intermediate = vec![0i32; (source_height * dst_w) as usize];
    for source_y in 0..source_height as usize {
        let source_row = source_y * source_width as usize;
        let intermediate_row = source_y * dst_w as usize;
        for output_x in 0..dst_w as usize {
            let x0 = horizontal.xmin[output_x];
            let mut sum = 0.0f64;
            for (tap, &weight) in horizontal.weights[output_x].iter().enumerate() {
                let source_x = (x0 + tap as i64) as usize;
                sum = weight.mul_add(f64::from(source[source_row + source_x]), sum);
            }
            intermediate[intermediate_row + output_x] = round_up(sum) as i32;
        }
    }

    let output_dimensions = CheckedDims::new_allow_empty(dst_w, dst_h, 4)?;
    let mut output_bytes = Vec::with_capacity(output_dimensions.total_bytes());
    for output_y in 0..dst_h as usize {
        let y0 = vertical.xmin[output_y];
        for output_x in 0..dst_w as usize {
            let mut sum = 0.0f64;
            for (tap, &weight) in vertical.weights[output_y].iter().enumerate() {
                let source_y = (y0 + tap as i64) as usize;
                sum = weight.mul_add(
                    f64::from(intermediate[source_y * dst_w as usize + output_x]),
                    sum,
                );
            }
            output_bytes.extend_from_slice(&(round_up(sum) as i32).to_le_bytes());
        }
    }
    let out = crate::raster::RgbaImage::from_raw(dst_w, dst_h, output_bytes).ok_or_else(|| {
        PilError::ValueError("resize_i boxed: failed to create output buffer".into())
    })?;
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
    if destination_width == 0 || destination_height == 0 {
        return;
    }

    let process_row = |y: u32, row: &mut [u8]| {
        let mut source_x = origin_x + i64::from(y) * step_y_x;
        let mut source_y = origin_y + i64::from(y) * step_y_y;
        for x in 0..destination_width {
            let input_x = source_x >> 16;
            let input_y = source_y >> 16;
            let output_index = x as usize * channels;
            if input_x >= 0
                && input_x < i64::from(source_width)
                && input_y >= 0
                && input_y < i64::from(source_height)
            {
                let input_index =
                    (input_y as u32 * source_width + input_x as u32) as usize * channels;
                row[output_index..output_index + channels]
                    .copy_from_slice(&source[input_index..input_index + channels]);
            } else {
                for channel in 0..channels.min(4) {
                    row[output_index + channel] = if channels == 2 && channel == 1 {
                        // LA/PA normalize their second sample as alpha in
                        // fill.3; fill.1 is only the duplicated luma/index
                        // component used by the host-neutral color record.
                        fill.3
                    } else {
                        match channel {
                            0 => fill.0,
                            1 => fill.1,
                            2 => fill.2,
                            _ => fill.3,
                        }
                    };
                }
            }
            source_x += step_x_x;
            source_y += step_x_y;
        }
    };

    #[cfg(feature = "parallel")]
    const PARALLEL_PIXEL_THRESHOLD: usize = 512 * 512;
    let output_stride = destination_width as usize * channels;
    #[cfg(feature = "parallel")]
    if (destination_width as usize).saturating_mul(destination_height as usize)
        >= PARALLEL_PIXEL_THRESHOLD
    {
        crate::par_rows_mut!(
            output,
            output_stride,
            destination_height as usize,
            |_row_start, _row_end, y, row| {
                process_row(y, row);
            }
        );
    } else {
        for (y, row) in output
            .chunks_exact_mut(output_stride)
            .take(destination_height as usize)
            .enumerate()
        {
            process_row(y as u32, row);
        }
    }
    #[cfg(not(feature = "parallel"))]
    for (y, row) in output
        .chunks_exact_mut(output_stride)
        .take(destination_height as usize)
        .enumerate()
    {
        process_row(y as u32, row);
    }
}

/// Rotate an image by an arbitrary angle, working on the native number of channels.
/// When `nearest` is true, uses nearest-neighbor sampling.
fn rotate_arbitrary_generic(
    img: &DynamicImage,
    angle: f64,
    expand: bool,
    fill: Option<(u8, u8, u8, u8)>,
    filter: ResampleFilter,
    nearest: bool,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    explicit_mode: Option<&str>,
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
    let aff_a = crate::ops::rotate::round_rotate_coefficient(rad.cos());
    let aff_b = crate::ops::rotate::round_rotate_coefficient(rad.sin());
    let aff_d = crate::ops::rotate::round_rotate_coefficient(-rad.sin());
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
    // Python's rotate() constructs the matrix and expanded bounds with
    // ordinary Python float operations. Keep that order for the matrix
    // translation/bounds pass; the native Geometry.c callback uses the
    // target's fused multiply-add grouping only when sampling pixels.
    let transform =
        |x: f64, y: f64, c: f64, f: f64| (aff_a * x + aff_b * y + c, aff_d * x + aff_e * y + f);
    let sample_transform = |x: f64, y: f64, c: f64, f: f64| {
        (
            aff_a.mul_add(x, aff_b * y) + c,
            aff_d.mul_add(x, aff_e * y) + f,
        )
    };

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

    if let DynamicImage::ImageLuma16(source) = img {
        let fill_value = fill.map_or(0, |value| u16::from_le_bytes([value.0, value.1]));
        let mut output = vec![fill_value; (dw as usize) * (dh as usize)];
        if nearest {
            // SPECIAL modes use Geometry.c's generic floating-point mapping,
            // then copy both bytes. They bypass the ordinary 16.16 sampler.
            for y in 0..dh {
                for x in 0..dw {
                    let (sx, sy) =
                        sample_transform(f64::from(x) + 0.5, f64::from(y) + 0.5, aff_c, aff_f);
                    if sx >= 0.0 && sx < sw && sy >= 0.0 && sy < sh {
                        output[(y * dw + x) as usize] = source.get_pixel(sx as u32, sy as u32)[0];
                    }
                }
            }
        } else {
            // Pillow's filtered I;16 path selects filter8: it samples the
            // first `width` bytes of each two-byte source row and writes only
            // byte zero of each destination sample. Preserve that observable
            // byte ABI, including the untouched fill byte, instead of
            // interpreting the filter as a numeric u16 convolution.
            let big_endian = explicit_mode == Some("I;16B")
                || (cfg!(target_endian = "big") && matches!(explicit_mode, Some("I;16" | "I;16N")));
            let encode = |value: u16| {
                if big_endian {
                    value.to_be_bytes()
                } else {
                    value.to_le_bytes()
                }
            };
            let fill_bytes = encode(fill_value);
            let mut plane = Vec::with_capacity((w as usize) * (h as usize));
            for y in 0..h as usize {
                plane.extend(
                    source.as_raw()[y * w as usize..(y + 1) * w as usize]
                        .iter()
                        .flat_map(|&value| encode(value))
                        .take(w as usize),
                );
            }
            let plane = crate::raster::GrayImage::from_raw(w, h, plane).ok_or_else(|| {
                PilError::InternalError("rotate I;16 source shape mismatch".into())
            })?;
            let sampled = rotate_arbitrary_generic(
                &DynamicImage::ImageLuma8(plane),
                angle,
                expand,
                fill.map(|_| (fill_bytes[0], 0, 0, 0)),
                filter,
                false,
                center,
                translate,
                Some("L"),
            )?;
            for (value, &byte) in output.iter_mut().zip(sampled.as_bytes()) {
                let bytes = [byte, fill_bytes[1]];
                *value = if big_endian {
                    u16::from_be_bytes(bytes)
                } else {
                    u16::from_le_bytes(bytes)
                };
            }
        }
        return crate::raster::ImageBuffer::from_raw(dw, dh, output)
            .map(DynamicImage::ImageLuma16)
            .ok_or_else(|| PilError::InternalError("rotate I;16 result shape mismatch".into()));
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
    } else if matches!(explicit_mode, Some("F") | Some("I")) && channels == 4 {
        rotate_arbitrary_scalar(
            raw,
            (w, h),
            (dw, dh),
            channels,
            [aff_a, aff_b, aff_c, aff_d, aff_e, aff_f],
            fill_color,
            filter,
            explicit_mode == Some("F"),
            &mut out,
        );
    } else {
        for dy in 0..dh {
            for dx in 0..dw {
                // Geometry.c first maps the destination pixel center and the
                // bilinear filter then converts that center to the source
                // pixel's corner coordinate by subtracting 0.5. Keep the
                // two stages explicit so arbitrary rotations use the same
                // source coordinates as Pillow's affine kernel.
                let (sx_rel, sy_rel) =
                    sample_transform(dx as f64 + 0.5, dy as f64 + 0.5, aff_c, aff_f);
                // Geometry.c subtracts half a pixel for every filtered
                // byte layout, including PA's independent index/alpha bands.
                let (sx_rel, sy_rel) = (sx_rel - 0.5, sy_rel - 0.5);
                let out_idx = (dy * dw + dx) as usize * channels;
                let in_filter_support =
                    sx_rel >= -0.5 && sx_rel < sw - 0.5 && sy_rel >= -0.5 && sy_rel < sh - 0.5;
                if in_filter_support {
                    let sx_rel = if matches!(filter, ResampleFilter::Bicubic) {
                        sx_rel
                    } else {
                        sx_rel.clamp(0.0, sw - 1.0)
                    };
                    let sy_rel = if matches!(filter, ResampleFilter::Bicubic) {
                        sy_rel
                    } else {
                        sy_rel.clamp(0.0, sh - 1.0)
                    };
                    let bilinear_x = sx_rel.floor() as i64;
                    let bilinear_y = sy_rel.floor() as i64;
                    let bicubic_x = bilinear_x - 1;
                    let bicubic_y = bilinear_y - 1;
                    let fx = sx_rel - bilinear_x as f64;
                    let fy = sy_rel - bilinear_y as f64;
                    for c in 0..channels {
                        let value = match filter {
                            ResampleFilter::Bicubic => {
                                let cubic_fx = sx_rel - (bicubic_x + 1) as f64;
                                let cubic_fy = sy_rel - (bicubic_y + 1) as f64;
                                let mut rows = [0.0f64; 4];
                                for (row, output) in rows.iter_mut().enumerate() {
                                    let yy = bicubic_y + row as i64;
                                    let cy = yy.clamp(0, i64::from(h - 1)) as usize;
                                    let samples = [
                                        raw[(cy * w as usize
                                            + bicubic_x.clamp(0, i64::from(w - 1)) as usize)
                                            * channels
                                            + c] as f64,
                                        raw[(cy * w as usize
                                            + (bicubic_x + 1).clamp(0, i64::from(w - 1)) as usize)
                                            * channels
                                            + c] as f64,
                                        raw[(cy * w as usize
                                            + (bicubic_x + 2).clamp(0, i64::from(w - 1)) as usize)
                                            * channels
                                            + c] as f64,
                                        raw[(cy * w as usize
                                            + (bicubic_x + 3).clamp(0, i64::from(w - 1)) as usize)
                                            * channels
                                            + c] as f64,
                                    ];
                                    *output = rotate_cubic_horizontal_f64(samples, cubic_fx);
                                }
                                rotate_cubic_vertical_f64(rows, cubic_fy)
                            }
                            _ => {
                                let sx = bilinear_x.clamp(0, i64::from(w - 1)) as u32;
                                let sy = bilinear_y.clamp(0, i64::from(h - 1)) as u32;
                                let sx1 = (bilinear_x + 1).clamp(0, i64::from(w - 1)) as u32;
                                let sy1 = (bilinear_y + 1).clamp(0, i64::from(h - 1)) as u32;
                                let p00 = raw[(sy * w + sx) as usize * channels + c] as f64;
                                let p10 = raw[(sy * w + sx1) as usize * channels + c] as f64;
                                let p01 = raw[(sy1 * w + sx) as usize * channels + c] as f64;
                                let p11 = raw[(sy1 * w + sx1) as usize * channels + c] as f64;
                                // Geometry.c's BILINEAR macro evaluates each
                                // horizontal row as `a + (b-a) * d`, then
                                // applies the same form vertically. Keep the
                                // fused operations explicit so byte filters
                                // use the native rounding order, including
                                // CMYK and palette-alpha rows.
                                let top = (p10 - p00).mul_add(fx, p00);
                                let bottom = (p11 - p01).mul_add(fx, p01);
                                (bottom - top).mul_add(fy, top)
                            }
                        };
                        // Geometry.c's UINT8 filters store a C cast of the
                        // interpolated value, truncating toward zero. This
                        // matters for the premultiplied alpha round trip.
                        out[out_idx + c] = if matches!(filter, ResampleFilter::Bicubic) {
                            value.clamp(0.0, 255.0) as u8
                        } else {
                            value as u8
                        };
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

#[inline]
fn rotate_cubic_horizontal_f32(samples: [f32; 4], distance: f64) -> f64 {
    let [v1, v2, v3, v4] = samples;
    let p1 = f64::from(v2);
    let p2 = f64::from(v3 - v1);
    let p3 = f64::from((v1 - v2).mul_add(2.0, v3) - v4);
    let p4 = f64::from((v2 - v1 - v3) + v4);
    let inner = distance.mul_add(p4, p3);
    let middle = distance.mul_add(inner, p2);
    distance.mul_add(middle, p1)
}

#[inline]
fn rotate_cubic_horizontal_f64(samples: [f64; 4], distance: f64) -> f64 {
    let [v1, v2, v3, v4] = samples;
    let p1 = v2;
    let p2 = -v1 + v3;
    let p3 = 2.0 * (v1 - v2) + v3 - v4;
    let p4 = -v1 + v2 - v3 + v4;
    // Geometry.c's optimized arm64 build contracts each Horner step into an
    // FMA.  Keep the contraction explicit; the final UINT8 cast can change
    // at a one-ULP boundary when this is evaluated as separate multiply/add
    // operations.
    let inner = distance.mul_add(p4, p3);
    let middle = distance.mul_add(inner, p2);
    distance.mul_add(middle, p1)
}

#[inline]
pub(crate) fn rotate_cubic_horizontal_i32(samples: [i32; 4], distance: f64) -> f64 {
    let [v1, v2, v3, v4] = samples;
    // Geometry.c's INT32 BICUBIC coefficients are formed in the source
    // integer type before the Horner chain is promoted to double. Explicit
    // wrapping keeps Rust's defined arithmetic aligned with Pillow's native
    // two's-complement operations for large signed samples.
    let p1 = f64::from(v2);
    let p2 = f64::from(v1.wrapping_neg().wrapping_add(v3));
    let p3 = f64::from(
        v1.wrapping_sub(v2)
            .wrapping_mul(2)
            .wrapping_add(v3)
            .wrapping_sub(v4),
    );
    let p4 = f64::from(
        v1.wrapping_neg()
            .wrapping_add(v2)
            .wrapping_sub(v3)
            .wrapping_add(v4),
    );
    // As in the UINT8 kernel, the native INT32 path uses an FMA Horner
    // chain after the integer coefficient preparation.
    let inner = distance.mul_add(p4, p3);
    let middle = distance.mul_add(inner, p2);
    distance.mul_add(middle, p1)
}

#[inline]
fn rotate_cubic_vertical_f64_fma(samples: [f64; 4], distance: f64) -> f64 {
    let [v1, v2, v3, v4] = samples;
    let p1 = v2;
    let p2 = -v1 + v3;
    let p3 = (v1 - v2).mul_add(2.0, v3) - v4;
    let p4 = -v1 + v2 - v3 + v4;
    let inner = distance.mul_add(p4, p3);
    let middle = distance.mul_add(inner, p2);
    distance.mul_add(middle, p1)
}

#[inline]
pub(crate) fn rotate_cubic_vertical_f64(samples: [f64; 4], distance: f64) -> f64 {
    rotate_cubic_horizontal_f64(samples, distance)
}

/// Apply Pillow's bilinear/bicubic transform to native four-byte `I`/`F`
/// samples.
///
/// These modes are represented by four bytes in the backend image, but the
/// transform kernel operates on one signed 32-bit or float32 sample. Pillow's
/// scalar affine kernels accumulate the bilinear value in double precision;
/// `F` then stores float32 and `I` truncates the result toward zero.
fn rotate_arbitrary_scalar(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    channels: usize,
    affine: [f64; 6],
    fill: (u8, u8, u8, u8),
    filter: ResampleFilter,
    is_float: bool,
    output: &mut [u8],
) {
    let [a, b, c, d, e, f] = affine;
    let (source_width, source_height) = source_size;
    let (destination_width, destination_height) = destination_size;
    let scalar_fill = if is_float {
        f32::from_le_bytes([fill.0, fill.1, fill.2, fill.3]) as f64
    } else {
        i32::from_le_bytes([fill.0, fill.1, fill.2, fill.3]) as f64
    };

    if source_width == 0 || source_height == 0 {
        for output_index in (0..destination_width as usize * destination_height as usize)
            .map(|index| index * channels)
        {
            let bytes = if is_float {
                (scalar_fill as f32).to_le_bytes()
            } else {
                (scalar_fill as i32).to_le_bytes()
            };
            output[output_index..output_index + 4].copy_from_slice(&bytes);
        }
        return;
    }

    for dy in 0..destination_height {
        for dx in 0..destination_width {
            let xin = dx as f64 + 0.5;
            let yin = dy as f64 + 0.5;
            // Geometry.c evaluates affine coordinates as a product followed
            // by a fused multiply-add and a separate translation add for both
            // typed 32-bit and FLOAT32 images. The filter subtracts 0.5 only
            // after the center-space bounds check.
            let sx = a.mul_add(xin, b * yin) + c;
            let sy = d.mul_add(xin, e * yin) + f;
            let output_index = (dy * destination_width + dx) as usize * channels;

            let value = if sx >= 0.0
                && sx < source_width as f64
                && sy >= 0.0
                && sy < source_height as f64
            {
                let sample_x = sx - 0.5;
                let sample_y = sy - 0.5;
                let read = |x: u32, y: u32| {
                    let index = ((y * source_width + x) as usize) * channels;
                    if is_float {
                        f32::from_le_bytes([
                            source[index],
                            source[index + 1],
                            source[index + 2],
                            source[index + 3],
                        ]) as f64
                    } else {
                        i32::from_le_bytes([
                            source[index],
                            source[index + 1],
                            source[index + 2],
                            source[index + 3],
                        ]) as f64
                    }
                };
                let read_f32 = |x: u32, y: u32| {
                    let index = ((y * source_width + x) as usize) * channels;
                    f32::from_le_bytes([
                        source[index],
                        source[index + 1],
                        source[index + 2],
                        source[index + 3],
                    ])
                };
                let read_i32 = |x: u32, y: u32| {
                    let index = ((y * source_width + x) as usize) * channels;
                    i32::from_le_bytes([
                        source[index],
                        source[index + 1],
                        source[index + 2],
                        source[index + 3],
                    ])
                };
                if matches!(filter, ResampleFilter::Bicubic) {
                    let floor_x = sample_x.floor() as i64;
                    let floor_y = sample_y.floor() as i64;
                    let base_x = floor_x - 1;
                    let base_y = floor_y - 1;
                    let fx = sample_x - floor_x as f64;
                    let fy = sample_y - floor_y as f64;
                    let mut rows = [0.0f64; 4];
                    for (row, output) in rows.iter_mut().enumerate() {
                        let yy =
                            (base_y + row as i64).clamp(0, i64::from(source_height - 1)) as u32;
                        let taps = [
                            (base_x).clamp(0, i64::from(source_width - 1)) as u32,
                            (base_x + 1).clamp(0, i64::from(source_width - 1)) as u32,
                            (base_x + 2).clamp(0, i64::from(source_width - 1)) as u32,
                            (base_x + 3).clamp(0, i64::from(source_width - 1)) as u32,
                        ];
                        *output = if is_float {
                            rotate_cubic_horizontal_f32(
                                [
                                    read_f32(taps[0], yy),
                                    read_f32(taps[1], yy),
                                    read_f32(taps[2], yy),
                                    read_f32(taps[3], yy),
                                ],
                                fx,
                            )
                        } else {
                            rotate_cubic_horizontal_i32(
                                [
                                    read_i32(taps[0], yy),
                                    read_i32(taps[1], yy),
                                    read_i32(taps[2], yy),
                                    read_i32(taps[3], yy),
                                ],
                                fx,
                            )
                        };
                    }
                    if is_float {
                        rotate_cubic_vertical_f64_fma(rows, fy)
                    } else {
                        rotate_cubic_vertical_f64(rows, fy)
                    }
                } else {
                    let floor_x = sample_x.floor() as i64;
                    let floor_y = sample_y.floor() as i64;
                    let x0 = floor_x.clamp(0, i64::from(source_width - 1)) as u32;
                    let y0 = floor_y.clamp(0, i64::from(source_height - 1)) as u32;
                    let x1 = (floor_x + 1).clamp(0, i64::from(source_width - 1)) as u32;
                    let y1 = (floor_y + 1).clamp(0, i64::from(source_height - 1)) as u32;
                    let fx = sample_x - floor_x as f64;
                    let fy = sample_y - floor_y as f64;
                    let p00 = read(x0, y0);
                    let p10 = read(x1, y0);
                    let p01 = read(x0, y1);
                    let p11 = read(x1, y1);
                    let horizontal_top = if is_float {
                        f64::from((p10 as f32) - (p00 as f32)).mul_add(fx, p00)
                    } else {
                        let p00 = p00 as i32;
                        let p10 = p10 as i32;
                        f64::from(p10.wrapping_sub(p00)) * fx + f64::from(p00)
                    };
                    let horizontal_bottom = if is_float {
                        f64::from((p11 as f32) - (p01 as f32)).mul_add(fx, p01)
                    } else {
                        let p01 = p01 as i32;
                        let p11 = p11 as i32;
                        f64::from(p11.wrapping_sub(p01)) * fx + f64::from(p01)
                    };
                    if is_float {
                        (horizontal_bottom - horizontal_top).mul_add(fy, horizontal_top)
                    } else {
                        (horizontal_bottom - horizontal_top) * fy + horizontal_top
                    }
                }
            } else {
                scalar_fill
            };

            let bytes = if is_float {
                (value as f32).to_le_bytes()
            } else {
                (value as i32).to_le_bytes()
            };
            output[output_index..output_index + 4].copy_from_slice(&bytes);
        }
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

/// Resize a floating source region with Pillow's typed and pass-order rules.
pub fn execute_resize_boxed(
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: ResampleFilter,
    bounds: (f64, f64, f64, f64),
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let resize = |image: &DynamicImage, width, height, bounds: (f64, f64, f64, f64), mode| {
        if mode == Some("I") && !matches!(filter, ResampleFilter::Nearest) {
            resize_i_boxed(
                image, width, height, bounds.0, bounds.1, bounds.2, bounds.3, filter,
            )
        } else {
            // Nearest copies complete stored samples, including F's scalar
            // words. Keep F's logical mode so the typed F path can copy those
            // words directly; other modes use the generic raw-sample path.
            let mode = if matches!(filter, ResampleFilter::Nearest) && mode != Some("F") {
                None
            } else {
                mode
            };
            Ok(pil_resize_boxed(
                image, width, height, bounds.0, bounds.1, bounds.2, bounds.3, filter, mode,
            ))
        }
    };
    if u64::from(img.height()) <= u64::from(img.width()) * 100 || h >= img.height() {
        return resize(img, w, h, bounds, mode);
    }
    // Pillow changes pass order for very tall inputs. Keep alpha premultiplied
    // across both passes: an intermediate unpremultiply/re-premultiply rounds
    // the colors again and changes the final bytes.
    let alpha_mode = match (img, mode) {
        (DynamicImage::ImageRgba8(_), None | Some("RGBA")) => Some("RGBa"),
        (DynamicImage::ImageLumaA8(_), None | Some("LA")) => Some("La"),
        _ => None,
    }
    .filter(|_| !matches!(filter, ResampleFilter::Nearest));
    let source = alpha_mode.map(|_| premultiply_alpha(img));
    let working_mode = alpha_mode.or(mode);
    let vertical = resize(
        source.as_ref().unwrap_or(img),
        img.width(),
        h,
        (0.0, bounds.1, f64::from(img.width()), bounds.3),
        working_mode,
    )?;
    let result = resize(
        &vertical,
        w,
        h,
        (bounds.0, 0.0, bounds.2, f64::from(h)),
        working_mode,
    )?;
    Ok(if alpha_mode.is_some() {
        unpremultiply_alpha(&result)
    } else {
        result
    })
}

/// Execute a Crop operation.
///
/// `Image::crop` owns Pillow's signed/out-of-bounds normalization before it
/// queues `PipelineOp::Crop`. The compute operation therefore receives only a
/// positive, in-bounds box; keeping padding logic here would duplicate that
/// public contract and leave an unreachable second crop implementation.
pub fn execute_crop(
    img: &DynamicImage,
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
) -> Result<DynamicImage, PilError> {
    let (iw, ih) = (img.width(), img.height());
    debug_assert!(
        left < iw && top < ih && right <= iw && bottom <= ih,
        "crop pipeline coordinates must be normalized before execution"
    );
    let width = right.checked_sub(left).ok_or_else(|| {
        PilError::InternalError("crop pipeline width underflow after normalization".into())
    })?;
    let height = bottom.checked_sub(top).ok_or_else(|| {
        PilError::InternalError("crop pipeline height underflow after normalization".into())
    })?;

    // Native byte layouts can copy complete rows directly.  Keep the image
    // crate path for typed samples such as I;16, whose byte stride is wider
    // than its logical channel count and therefore needs its own layout
    // handling.
    let channels = img.color().channel_count() as usize;
    if matches!(
        img.color(),
        crate::raster::ColorType::L8
            | crate::raster::ColorType::La8
            | crate::raster::ColorType::Rgb8
            | crate::raster::ColorType::Rgba8
    ) {
        let source = img.as_bytes();
        let source_stride = iw as usize * channels;
        let output_stride = width as usize * channels;
        let output_dims = CheckedDims::new(width, height, channels as u8)?;
        #[cfg(feature = "parallel")]
        if output_dims.total_bytes() >= 4 * 1024 * 1024 && output_stride != 0 {
            let mut output = output_dims.alloc_buffer();
            crate::par_rows_mut!(
                &mut output,
                output_stride,
                height as usize,
                |_row_start, _row_end, y, row| {
                    let source_start =
                        (top as usize + y as usize) * source_stride + left as usize * channels;
                    row.copy_from_slice(&source[source_start..source_start + output_stride]);
                }
            );
            return raw_bytes_to_image(width, height, output, channels);
        }

        // Every output row comes from one source-row span, so
        // avoid zero-filling a destination immediately before copying it.
        let mut output = Vec::with_capacity(output_dims.total_bytes());
        crate::compute::record_pipeline_allocation(output_dims.total_bytes());
        for y in 0..height as usize {
            let source_start = (top as usize + y) * source_stride + left as usize * channels;
            output.extend_from_slice(&source[source_start..source_start + output_stride]);
        }
        return raw_bytes_to_image(width, height, output, channels);
    }
    Ok(img.crop_imm(left, top, width, height))
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

    // Pillow centers the expanded 90-degree result with independent edge
    // rounding. Rust integer division truncates negative halves toward zero,
    // so calculate the floor/ceil offsets explicitly for odd dimension gaps.
    let width_gap = w as f64 - h as f64;
    let height_gap = h as f64 - w as f64;
    let dx_off_90 = (width_gap / 2.0).floor() as i32;
    let dy_off_90 = (height_gap / 2.0).ceil() as i32;
    let dx_off_270 = (width_gap / 2.0).ceil() as i32;
    let dy_off_270 = (height_gap / 2.0).floor() as i32;

    for dy in 0..h {
        for dx in 0..w {
            let (sx, sy) = if clockwise_270 {
                // 270° CCW = 90° CW: expand_true uses input(dy, h-1-dx)
                // dx_true = dx - dx_off, dy_true = dy - dy_off
                // sx = dy_true = dy - dy_off, sy = h - 1 - dx_true
                //     = h - 1 - dx + dx_off
                (
                    dy as i32 - dy_off_270,
                    h as i32 - 1 - dx as i32 + dx_off_270,
                )
            } else {
                // 90° CCW: expand_true uses input(w-1-dy, dx)
                // dx_true = dx - dx_off, dy_true = dy - dy_off
                // sx = w - 1 - dy_true = w - 1 - dy + dy_off, sy = dx_true = dx - dx_off
                (w as i32 - 1 - dy as i32 + dy_off_90, dx as i32 - dx_off_90)
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
    filter: ResampleFilter,
    requested_nearest: bool,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let has_custom_transform = center.is_some() || translate.is_some();
    let nearest = requested_nearest || explicit_mode == Some("P") || explicit_mode == Some("1");
    let filter = if nearest {
        ResampleFilter::Nearest
    } else {
        filter
    };
    // LA/La/PA use a public (gray, gray, gray, alpha) fill tuple. Normalize
    // it to native two-band storage; PA and La bypass the alpha round trip.
    let fill = if matches!(explicit_mode, Some("PA" | "LA" | "La")) {
        fill.map(|(index, _, _, alpha)| (index, alpha, 0, alpha))
    } else {
        fill
    };
    // Fast path: exact 90-degree multiples
    // PIL rotates counterclockwise; image crate rotates clockwise.
    // PIL 90° CCW = image crate 270° CW, PIL 270° CCW = image crate 90° CW.
    // Unexpanded non-square 90/270 rotations still use the affine sampler,
    // including its filtered alpha round trip.
    let normalized_angle = angle.rem_euclid(360.0);
    let result = if !has_custom_transform && normalized_angle.abs() <= f64::EPSILON {
        // Pillow's public rotate() returns an exact copy at angle 0 (and
        // every multiple of 360) before considering the requested filter.
        // Keep this fast path ahead of filtered affine sampling so LA/RGBA
        // bytes and alpha channels are not rounded needlessly.
        img.clone()
    } else if !has_custom_transform
        && normalized_angle == 90.0
        && (expand || img.width() == img.height())
    {
        if expand {
            img.rotate270() // 270° CW = 90° CCW (PIL)
        } else {
            rotate_90_non_expand(img, false, fill)?
        }
    } else if !has_custom_transform && normalized_angle == 180.0 {
        img.rotate180()
    } else if !has_custom_transform
        && normalized_angle == 270.0
        && (expand || img.width() == img.height())
    {
        if expand {
            img.rotate90() // 90° CW = 270° CCW (PIL)
        } else {
            rotate_90_non_expand(img, true, fill)?
        }
    } else {
        // Pillow routes non-nearest LA/RGBA transforms through their
        // premultiplied modes (La/RGBa), then converts the interpolated
        // samples back. RGBa is already premultiplied and must remain a direct
        // native-channel path, just as in pil_resize.
        let needs_alpha_roundtrip = !nearest
            && !matches!(
                explicit_mode,
                Some("PA")
                    | Some("RGBa")
                    | Some("La")
                    | Some("RGBX")
                    | Some("CMYK")
                    | Some("F")
                    | Some("I")
            )
            && matches!(
                img.color(),
                crate::raster::ColorType::La8 | crate::raster::ColorType::Rgba8
            );
        let work = if needs_alpha_roundtrip {
            premultiply_alpha(img)
        } else {
            img.clone()
        };
        let rotated = rotate_arbitrary_generic(
            &work,
            normalized_angle,
            expand,
            fill,
            filter,
            nearest,
            center,
            translate,
            explicit_mode,
        )?;
        if needs_alpha_roundtrip {
            unpremultiply_alpha(&rotated)
        } else {
            rotated
        }
    };
    Ok(preserve_mode(img, result))
}

/// Execute a Transpose operation.
const TRANSPOSE_TILE_SIZE: u32 = 32;
#[cfg(not(feature = "parallel"))]
const TRANSPOSE_RGBA_TILE_SIZE: u32 = 16;
const TRANSPOSE_TILE_THRESHOLD_PIXELS: usize = 256 * 1024;

#[inline]
fn should_tile_transpose(width: u32, height: u32) -> bool {
    width >= TRANSPOSE_TILE_SIZE
        && height >= TRANSPOSE_TILE_SIZE
        && (width as usize).saturating_mul(height as usize) >= TRANSPOSE_TILE_THRESHOLD_PIXELS
}

/// Transpose a large byte image in bounded output-row tiles.
///
/// Each task owns complete output rows and visits bounded rectangles within
/// them. Writing each row segment contiguously keeps destination indexing out
/// of the pixel loop while the source gathers stay within one small rectangle.
/// The small-image path deliberately keeps its old order.
fn transpose_bytes_tiled<const CHANNELS: usize>(
    source: &[u8],
    output: &mut [u8],
    width: u32,
    height: u32,
    method: &TransposeMethod,
    tile_size: u32,
) {
    let width = width as usize;
    let height = height as usize;
    let tile_size = tile_size as usize;
    let output_stride = height * CHANNELS;
    let source_stride = width * CHANNELS;
    #[cfg(feature = "parallel")]
    let tile_stride = output_stride * tile_size;
    let tile_rows = width.div_ceil(tile_size);
    let (reverse_x, reverse_y) = match method {
        TransposeMethod::Transpose => (false, false),
        TransposeMethod::Transverse => (true, true),
        TransposeMethod::Rotate90 => (true, false),
        TransposeMethod::Rotate270 => (false, true),
        _ => unreachable!("unsupported tiled transpose method"),
    };
    let process_tile = |tile_index: usize, rows: &mut [u8]| {
        for block_x in (0..height).step_by(tile_size) {
            let end_x = (block_x + tile_size).min(height);
            for (local_y, row) in rows.chunks_exact_mut(output_stride).enumerate() {
                let output_y = tile_index * tile_size + local_y;
                let source_x = if reverse_x {
                    width - 1 - output_y
                } else {
                    output_y
                };
                let pixels = row[block_x * CHANNELS..end_x * CHANNELS].chunks_exact_mut(CHANNELS);
                // Each segment contains complete native samples. Its source
                // indices advance by one row, bounded by this rectangle even
                // when the final tile is partial or the orientation reverses.
                if reverse_y {
                    let first = (height - 1 - block_x) * source_stride + source_x * CHANNELS;
                    for (offset, pixel) in pixels.enumerate() {
                        let source_index = first - offset * source_stride;
                        pixel.copy_from_slice(&source[source_index..source_index + CHANNELS]);
                    }
                } else {
                    let first = block_x * source_stride + source_x * CHANNELS;
                    for (offset, pixel) in pixels.enumerate() {
                        let source_index = first + offset * source_stride;
                        pixel.copy_from_slice(&source[source_index..source_index + CHANNELS]);
                    }
                }
            }
        }
    };

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        output,
        tile_stride,
        tile_rows,
        |_row_start, _row_end, tile_index, rows| {
            process_tile(tile_index as usize, rows);
        }
    );

    #[cfg(not(feature = "parallel"))]
    for tile_index in 0..tile_rows {
        let output_y_start = tile_index * tile_size;
        let output_y_end = (output_y_start + tile_size).min(width);
        let row_start = output_y_start * output_stride;
        let row_end = output_y_end * output_stride;
        let rows = &mut output[row_start..row_end];
        process_tile(tile_index, rows);
    }
}

/// Transpose small native byte images with complete row and pixel slices.
/// This avoids repeated coordinate validation and saturating arithmetic in
/// generic pixel accessors. Trim trailing storage before reversing source
/// rows: ImageBuffer permits extra samples after the logical raster.
fn transpose_bytes_serial<const CHANNELS: usize>(
    source: &[u8],
    output: &mut [u8],
    width: u32,
    height: u32,
    method: &TransposeMethod,
) {
    let source = &source[..output.len()];
    let source_stride = width as usize * CHANNELS;
    let output_stride = height as usize * CHANNELS;
    let reverse_x = matches!(
        method,
        TransposeMethod::Rotate90 | TransposeMethod::Transverse
    );
    let reverse_y = matches!(
        method,
        TransposeMethod::Rotate270 | TransposeMethod::Transverse
    );
    for (y, row) in output.chunks_exact_mut(output_stride).enumerate() {
        let source_x = if reverse_x { width as usize - 1 - y } else { y };
        let offset = source_x * CHANNELS;
        let source_rows = source.chunks_exact(source_stride);
        if reverse_y {
            for (pixel, original) in row.chunks_exact_mut(CHANNELS).zip(source_rows.rev()) {
                pixel.copy_from_slice(&original[offset..offset + CHANNELS]);
            }
        } else {
            for (pixel, original) in row.chunks_exact_mut(CHANNELS).zip(source_rows) {
                pixel.copy_from_slice(&original[offset..offset + CHANNELS]);
            }
        }
    }
}

/// Copy complete native rows for flips and half turns. Pixel chunks keep
/// channels (including opaque packed I/F words) together without repeated
/// coordinate validation or a per-pixel image accessor.
fn transpose_bytes_rows<const CHANNELS: usize>(
    source: &[u8],
    output: &mut [u8],
    dimensions: &CheckedDims,
    reverse_x: bool,
    reverse_y: bool,
) {
    let height = dimensions.height as usize;
    let stride = dimensions.row_stride();
    let copy_rows = |first_y: usize, rows: &mut [u8]| {
        for (local_y, row) in rows.chunks_exact_mut(stride).enumerate() {
            let y = first_y + local_y;
            let source_y = if reverse_y { height - 1 - y } else { y };
            let input = &source[source_y * stride..(source_y + 1) * stride];
            if reverse_x {
                for (pixel, original) in row
                    .chunks_exact_mut(CHANNELS)
                    .zip(input.chunks_exact(CHANNELS).rev())
                {
                    pixel.copy_from_slice(original);
                }
            } else {
                row.copy_from_slice(input);
            }
        }
    };
    #[cfg(feature = "parallel")]
    if dimensions.total_pixels() >= TRANSPOSE_TILE_THRESHOLD_PIXELS {
        // Group complete rows by bytes, so a tall one-pixel image does not
        // create a task per tiny row. Checked output dimensions bound stride
        // and every full group; the final group may contain fewer rows.
        let rows_per_group = (32 * 1024usize).div_ceil(stride).min(height);
        let group_stride = rows_per_group * stride;
        let groups = height.div_ceil(rows_per_group);
        crate::par_rows_mut!(
            output,
            group_stride,
            groups,
            |_row_start, _row_end, group, rows| {
                copy_rows(group as usize * rows_per_group, rows);
            }
        );
    } else {
        copy_rows(0, output);
    }
    #[cfg(not(feature = "parallel"))]
    copy_rows(0, output);
}

pub fn execute_transpose(
    img: &DynamicImage,
    method: &TransposeMethod,
) -> Result<DynamicImage, PilError> {
    if matches!(
        method,
        TransposeMethod::FlipLeftRight
            | TransposeMethod::FlipTopBottom
            | TransposeMethod::Rotate180
    ) && matches!(
        img.color(),
        crate::raster::ColorType::L8
            | crate::raster::ColorType::La8
            | crate::raster::ColorType::Rgb8
            | crate::raster::ColorType::Rgba8
    ) {
        let (width, height) = img.dimensions();
        if width != 0 && height != 0 {
            let channels = img.color().channel_count() as usize;
            let dimensions = CheckedDims::new(width, height, channels as u8)?;
            let mut output = dimensions.alloc_buffer();
            let reverse_x = !matches!(method, TransposeMethod::FlipTopBottom);
            let reverse_y = !matches!(method, TransposeMethod::FlipLeftRight);
            macro_rules! copy_rows {
                ($channels:literal) => {
                    transpose_bytes_rows::<$channels>(
                        img.as_bytes(),
                        &mut output,
                        &dimensions,
                        reverse_x,
                        reverse_y,
                    )
                };
            }
            match channels {
                1 => copy_rows!(1),
                2 => copy_rows!(2),
                3 => copy_rows!(3),
                4 => copy_rows!(4),
                _ => unreachable!("native byte image has one to four channels"),
            }
            return raw_bytes_to_image(width, height, output, channels);
        }
    }
    if matches!(
        method,
        TransposeMethod::Rotate90
            | TransposeMethod::Rotate270
            | TransposeMethod::Transpose
            | TransposeMethod::Transverse
    ) && matches!(
        img.color(),
        crate::raster::ColorType::L8
            | crate::raster::ColorType::La8
            | crate::raster::ColorType::Rgb8
            | crate::raster::ColorType::Rgba8
    ) {
        let (width, height) = img.dimensions();
        if width != 0 && height != 0 && (width != 1 || height != 1) {
            let channels = img.color().channel_count() as usize;
            let output_dims = CheckedDims::new(height, width, channels as u8)?;
            let mut output = output_dims.alloc_buffer();
            #[cfg(feature = "parallel")]
            let transpose_tile_size = TRANSPOSE_TILE_SIZE;
            #[cfg(not(feature = "parallel"))]
            let transpose_tile_size = if channels == 4 {
                TRANSPOSE_RGBA_TILE_SIZE
            } else {
                TRANSPOSE_TILE_SIZE
            };
            // Specialize the opaque pixel size once per image so each tile
            // copies a fixed-size sample instead of calling a variable-size
            // slice copy for every pixel.
            macro_rules! transpose_bytes {
                ($channels:literal) => {
                    if should_tile_transpose(width, height) {
                        transpose_bytes_tiled::<$channels>(
                            img.as_bytes(),
                            &mut output,
                            width,
                            height,
                            method,
                            transpose_tile_size,
                        );
                    } else {
                        transpose_bytes_serial::<$channels>(
                            img.as_bytes(),
                            &mut output,
                            width,
                            height,
                            method,
                        );
                    }
                };
            }
            match channels {
                1 => transpose_bytes!(1),
                2 => transpose_bytes!(2),
                3 => transpose_bytes!(3),
                4 => transpose_bytes!(4),
                _ => unreachable!("native byte image has one to four channels"),
            }
            return raw_bytes_to_image(height, width, output, channels);
        }
    }
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
            Ok(img.transpose_diagonal(matches!(method, TransposeMethod::Transverse)))
        }
    }
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
    // `Image::thumbnail` performs Pillow's aspect-preserving `round_aspect`
    // calculation before queuing this operation so lazy shape metadata and
    // the eventual pixels agree. The operation therefore carries the final
    // dimensions; do not apply the aspect calculation a second time here.
    let new_w = w.max(1).min(cur_w);
    let new_h = h.max(1).min(cur_h);
    // PIL forces NEAREST for mode "1" and "P" to avoid non-binary/interpolated values
    let effective_filter = match explicit_mode {
        Some("1") | Some("P") => ResampleFilter::Nearest,
        _ => *filter,
    };
    // PIL's thumbnail uses reducing_gap=2.0 by default: first integer-reduce
    // by up to scale/reducing_gap, then resize the rest.
    // This matches PIL's ImagingReduce then ImagingResample two-step.
    // Skip reducing_gap for modes with alpha (LA, RGBA) to avoid premultiply
    // issues. F/I use RGBA storage internally but are scalar modes, and CMYK
    // uses the same four-byte storage without an alpha channel.
    let has_alpha = matches!(
        img.color(),
        crate::raster::ColorType::La8 | crate::raster::ColorType::Rgba8
    ) && !matches!(explicit_mode, Some("F" | "I" | "CMYK" | "RGBa" | "RGBX"));
    let needs_reduce = !matches!(effective_filter, ResampleFilter::Nearest) && !has_alpha;
    let mut resize_box = None;
    let work_img = if needs_reduce {
        let scale_x = cur_w as f64 / new_w as f64;
        let scale_y = cur_h as f64 / new_h as f64;
        // Image.resize computes these independently for the two axes before
        // calling Image.reduce(factor=(factor_x, factor_y)).
        let factor_x = ((scale_x / 2.0) as u32).max(1);
        let factor_y = ((scale_y / 2.0) as u32).max(1);
        if factor_x > 1 || factor_y > 1 {
            let (rw, rh) = (cur_w.div_ceil(factor_x), cur_h.div_ceil(factor_y));
            // Image.resize keeps the original full-image box after reduce and
            // scales its right/bottom edges by the integer factors. When the
            // source dimensions are not divisible by a factor, that box ends
            // inside the ceil-sized reduced image; using the whole reduced
            // image changes boundary pixels (Pillow's _get_safe_box path).
            resize_box = Some((
                0.0,
                0.0,
                cur_w as f64 / factor_x as f64,
                cur_h as f64 / factor_y as f64,
            ));
            match explicit_mode {
                // Pillow keeps F/I samples in their native scalar domain for
                // the reducing_gap pass. Averaging encoded RGBA bytes would
                // corrupt the representation before resize_f/resize_i runs.
                Some("F") if matches!(img, DynamicImage::ImageRgba8(_)) => {
                    reduce_f_thumbnail(img, rw, rh, factor_x, factor_y)?
                }
                #[cfg(not(feature = "parallel"))]
                Some("I")
                    if matches!(img, DynamicImage::ImageRgba8(_))
                        && factor_x == 2
                        && factor_y == 2
                        && cur_w % 2 == 0
                        && cur_h % 2 == 0 =>
                {
                    return Ok(preserve_mode(
                        img,
                        resize_i_thumbnail_reduce2x2(img, new_w, new_h, effective_filter)?,
                    ));
                }
                Some("I") if matches!(img, DynamicImage::ImageRgba8(_)) => {
                    reduce_i_thumbnail(img, rw, rh, factor_x, factor_y)?
                }
                _ => execute_reduce(img, factor_x, factor_y, explicit_mode)?,
            }
        } else {
            img.clone()
        }
    } else {
        img.clone()
    };
    // Only use F/I thumbnail paths when the image is already stored as Rgba8
    // (4 bytes per pixel), meaning it has been converted to F/I mode already.
    // If the image is still RGB or other format, use normal thumbnail regardless
    // of explicit_mode, because the F/I convert hasn't happened yet in the pipeline.
    let result = match (explicit_mode, &work_img, resize_box) {
        (Some("F"), DynamicImage::ImageRgba8(_), Some((left, top, right, bottom))) => {
            // Image.resize adjusts the source box after its reducing-gap pass.
            // Keep that fractional box for F as well; resizing the complete
            // ceil-sized reduction includes partial edge samples that Pillow
            // deliberately excludes from the final convolution.
            pil_resize_boxed(
                &work_img,
                new_w,
                new_h,
                left,
                top,
                right,
                bottom,
                effective_filter,
                explicit_mode,
            )
        }
        (Some("F"), DynamicImage::ImageRgba8(_), None) => {
            resize_f(&work_img, new_w, new_h, &effective_filter)?
        }
        (Some("I"), DynamicImage::ImageRgba8(_), Some((left, top, right, bottom))) => {
            resize_i_boxed(
                &work_img,
                new_w,
                new_h,
                left,
                top,
                right,
                bottom,
                effective_filter,
            )?
        }
        (Some("I"), DynamicImage::ImageRgba8(_), None) => {
            resize_i(&work_img, new_w, new_h, &effective_filter)?
        }
        (_, _, Some((left, top, right, bottom))) => pil_resize_boxed(
            &work_img,
            new_w,
            new_h,
            left,
            top,
            right,
            bottom,
            effective_filter,
            explicit_mode,
        ),
        _ => pil_resize(&work_img, new_w, new_h, effective_filter, explicit_mode),
    };
    Ok(preserve_mode(img, result))
}

/// Return whether a validated RGB pixel-write prefix can be folded into a
/// 2×2 non-nearest thumbnail reduction without materializing a full source clone.
pub(crate) fn rgb_putpixel_thumbnail_fusion_supported(
    img: &DynamicImage,
    putpixel_ops: &[PipelineOp],
    thumbnail: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    let DynamicImage::ImageRgb8(_) = img else {
        return false;
    };
    if !matches!(mode, None | Some("RGB"))
        || putpixel_ops.is_empty()
        || !putpixel_ops.iter().all(|op| {
            matches!(
                op,
                PipelineOp::PutPixel {
                    palette_index: false,
                    ..
                }
            )
        })
    {
        return false;
    }
    let PipelineOp::Thumbnail { w, h, filter } = thumbnail else {
        return false;
    };
    if matches!(filter, ResampleFilter::Nearest) {
        return false;
    }

    let (source_width, source_height) = img.dimensions();
    if source_width < 2
        || source_height < 2
        || source_width % 2 != 0
        || source_height % 2 != 0
        || (source_width as usize).saturating_mul(source_height as usize) < 512 * 512
    {
        return false;
    }
    let (new_width, new_height) = (
        (*w).max(1).min(source_width),
        (*h).max(1).min(source_height),
    );
    if new_width == 0 || new_height == 0 {
        return false;
    }
    let factor_x = ((source_width as f64 / new_width as f64 / 2.0) as u32).max(1);
    let factor_y = ((source_height as f64 / new_height as f64 / 2.0) as u32).max(1);
    factor_x == 2 && factor_y == 2
}

/// Apply validated RGB PutPixel operations at the exact integer-reduction
/// boundary, then run the same boxed resampling filter as `execute_thumbnail`.
/// This avoids cloning the full source image merely to change a few pixels.
pub(crate) fn execute_rgb_putpixel_thumbnail_fusion(
    img: &DynamicImage,
    putpixel_ops: &[PipelineOp],
    thumbnail: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if !rgb_putpixel_thumbnail_fusion_supported(img, putpixel_ops, thumbnail, mode) {
        return Err(PilError::InternalError(
            "unsupported RGB PutPixel thumbnail fusion request".into(),
        ));
    }
    let DynamicImage::ImageRgb8(source) = img else {
        return Err(PilError::InternalError(
            "RGB thumbnail fusion source changed after admission".into(),
        ));
    };
    let PipelineOp::Thumbnail { w, h, filter } = thumbnail else {
        return Err(PilError::InternalError(
            "RGB thumbnail fusion operation changed after admission".into(),
        ));
    };
    let (source_width, source_height) = img.dimensions();
    let new_width = (*w).max(1).min(source_width);
    let new_height = (*h).max(1).min(source_height);
    let reduced_width = source_width / 2;
    let reduced_height = source_height / 2;

    // Preserve ordered writes, including repeated coordinates, while keeping
    // only the final native RGB value for each source pixel.
    let mut pixel_overrides = std::collections::BTreeMap::<usize, [u8; 3]>::new();
    for op in putpixel_ops {
        let PipelineOp::PutPixel {
            x,
            y,
            color,
            palette_index: false,
        } = op
        else {
            return Err(PilError::InternalError(
                "RGB PutPixel fusion received a different operation".into(),
            ));
        };
        if *x >= source_width || *y >= source_height {
            return Err(PilError::IndexError("image index out of range".into()));
        }
        let pixel_index = *y as usize * source_width as usize + *x as usize;
        pixel_overrides.insert(pixel_index, [color.0, color.1, color.2]);
    }

    let Some((reduced_image, mut sparse_nonzero_rows)) =
        execute_reduce_rgb_with_sparse_rows(img, 2, 2, Some("RGB"))?
    else {
        return Err(PilError::InternalError(
            "RGB thumbnail reduction rejected its native source".into(),
        ));
    };
    let mut reduced_bytes = match reduced_image {
        DynamicImage::ImageRgb8(reduced) => reduced.into_raw(),
        _ => {
            return Err(PilError::InternalError(
                "RGB thumbnail reduction changed the native mode".into(),
            ));
        }
    };
    let affected_blocks = pixel_overrides
        .keys()
        .map(|&pixel_index| {
            let x = pixel_index % source_width as usize;
            let y = pixel_index / source_width as usize;
            (y / 2) * reduced_width as usize + x / 2
        })
        .collect::<std::collections::BTreeSet<_>>();
    let source_bytes = source.as_raw();
    for block_index in affected_blocks {
        let block_x = block_index % reduced_width as usize;
        let block_y = block_index / reduced_width as usize;
        let mut sums = [0u32; 3];
        for dy in 0..2usize {
            for dx in 0..2usize {
                let source_pixel_index =
                    (block_y * 2 + dy) * source_width as usize + block_x * 2 + dx;
                let sample = pixel_overrides
                    .get(&source_pixel_index)
                    .copied()
                    .unwrap_or_else(|| {
                        let source_start = source_pixel_index * 3;
                        [
                            source_bytes[source_start],
                            source_bytes[source_start + 1],
                            source_bytes[source_start + 2],
                        ]
                    });
                for channel in 0..3 {
                    sums[channel] += u32::from(sample[channel]);
                }
            }
        }

        let output_start = block_index * 3;
        for channel in 0..3 {
            reduced_bytes[output_start + channel] = ((sums[channel] + 2) >> 2) as u8;
        }
        if reduced_bytes[output_start..output_start + 3]
            .iter()
            .any(|&sample| sample != 0)
        {
            if let Some(nonzero_rows) = sparse_nonzero_rows.as_mut() {
                nonzero_rows[block_index / reduced_width as usize] = true;
            }
        }
    }

    let reduced =
        crate::raster::RgbImage::from_raw(reduced_width, reduced_height, reduced_bytes)
            .ok_or_else(|| PilError::InternalError("RGB reduction buffer shape mismatch".into()))?;
    let reduced = DynamicImage::ImageRgb8(reduced);
    let box_right = f64::from(source_width) / 2.0;
    let box_bottom = f64::from(source_height) / 2.0;
    let resized = if let Some(nonzero_rows) = sparse_nonzero_rows.as_deref() {
        crate::ops::pil_resize::pil_resize_boxed_with_sparse_rgb_rows(
            &reduced,
            new_width,
            new_height,
            0.0,
            0.0,
            box_right,
            box_bottom,
            *filter,
            Some("RGB"),
            nonzero_rows,
        )
    } else {
        pil_resize_boxed(
            &reduced,
            new_width,
            new_height,
            0.0,
            0.0,
            box_right,
            box_bottom,
            *filter,
            Some("RGB"),
        )
    };
    Ok(preserve_mode(img, resized))
}

fn reduce_f_thumbnail_2x2_samples(
    dst_w: u32,
    dst_h: u32,
    source_width: u32,
    output_bytes: usize,
    mut sample_f32: impl FnMut(usize) -> f32,
) -> Result<DynamicImage, PilError> {
    let source_width = source_width as usize;
    let output_row_bytes = (dst_w as usize)
        .checked_mul(4)
        .ok_or_else(|| PilError::InternalError("F thumbnail row size overflow".into()))?;
    let mut output = vec![0; output_bytes];
    for y in 0..dst_h as usize {
        let top_row = y * 2 * source_width;
        let bottom_row = top_row + source_width;
        let output_row_start = y * output_row_bytes;
        let output_row_end = output_row_start + output_row_bytes;
        let output_row = &mut output[output_row_start..output_row_end];
        for (x, output_sample) in output_row.chunks_exact_mut(4).enumerate() {
            let source_x = x * 2;
            let top_left = sample_f32(top_row + source_x);
            let top_right = sample_f32(top_row + source_x + 1);
            let bottom_left = sample_f32(bottom_row + source_x);
            let bottom_right = sample_f32(bottom_row + source_x + 1);
            // Pillow adds this quartet in f32 order before promoting it to f64.
            let quartet = ((top_left + top_right) + bottom_left) + bottom_right;
            let mut sum = 0.0f64;
            sum += f64::from(quartet);
            let value = (sum * 0.25) as f32;
            output_sample.copy_from_slice(&value.to_le_bytes());
        }
    }
    raw_bytes_to_image(dst_w, dst_h, output, 4)
}

fn reduce_f_thumbnail(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    factor_x: u32,
    factor_y: u32,
) -> Result<DynamicImage, PilError> {
    // F's four-byte image is a scalar carrier here; borrow it to avoid a copy.
    let (src_w, src_h, source_image) = match img {
        DynamicImage::ImageRgba8(image) => (image.width(), image.height(), image),
        _ => {
            return Err(PilError::ValueError(
                "F-mode thumbnail reduction requires four-byte scalar storage".into(),
            ));
        }
    };
    // `thumbnail` commonly applies a 2×2 reducing-gap pass before its F32
    // resize. Keep that frequent interior case branch-free: read the four
    // packed scalar samples by row offset, preserve Pillow's f32 quartet
    // order, then promote only the completed quartet to the f64 accumulator.
    if factor_x == 2
        && factor_y == 2
        && src_w % 2 == 0
        && src_h % 2 == 0
        && dst_w == src_w / 2
        && dst_h == src_h / 2
    {
        let source_dims = CheckedDims::new_allow_empty(src_w, src_h, 4)?;
        let source_bytes = source_image.as_raw();
        if source_bytes.len() != source_dims.total_bytes() {
            return Err(PilError::InternalError(
                "F-mode thumbnail source buffer shape mismatch".into(),
            ));
        }
        let output_dims = CheckedDims::new_allow_empty(dst_w, dst_h, 4)?;
        let output_bytes = output_dims.total_bytes();
        if let Some(samples) = crate::ops::pil_resize::f32_samples_borrowed_from_le_bytes(
            source_bytes,
            source_dims.total_bytes() / 4,
        ) {
            return reduce_f_thumbnail_2x2_samples(dst_w, dst_h, src_w, output_bytes, |index| {
                samples[index]
            });
        }
        return reduce_f_thumbnail_2x2_samples(dst_w, dst_h, src_w, output_bytes, |index| {
            let byte_index = index * 4;
            f32::from_le_bytes([
                source_bytes[byte_index],
                source_bytes[byte_index + 1],
                source_bytes[byte_index + 2],
                source_bytes[byte_index + 3],
            ])
        });
    }

    let sample_f32 = |x: u32, y: u32| {
        let sample = source_image.get_pixel(x, y);
        f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]])
    };
    let mut out = Vec::with_capacity((dst_w * dst_h * 4) as usize);
    let main_width = src_w / factor_x;
    let main_height = src_h / factor_y;
    for y in 0..dst_h {
        let source_y = y * factor_y;
        let block_h = factor_y.min(src_h - source_y);
        for x in 0..dst_w {
            let source_x = x * factor_x;
            let block_w = factor_x.min(src_w - source_x);
            // `ImagingReduceNxN_32bpc` uses a double accumulator, but its
            // interior 2x2 groups are formed by float additions before being
            // promoted to double. The corner helper handles partial right,
            // bottom, and bottom-right blocks as scalar float values. Keep
            // those two paths distinct: a flat f32 sum drifts on constants,
            // while a flat f64 sum differs on heterogeneous blocks.
            let mut sum = 0.0f64;
            if x < main_width && y < main_height {
                let mut dy = 0;
                while dy + 1 < block_h {
                    let mut dx = 0;
                    while dx + 1 < block_w {
                        let value = |offset_x: u32, offset_y: u32| {
                            sample_f32(source_x + offset_x, source_y + offset_y)
                        };
                        let top_left = value(dx, dy);
                        let top_right = value(dx + 1, dy);
                        let bottom_left = value(dx, dy + 1);
                        let bottom_right = value(dx + 1, dy + 1);
                        let quartet = ((top_left + top_right) + bottom_left) + bottom_right;
                        sum += f64::from(quartet);
                        dx += 2;
                    }
                    if dx < block_w {
                        let value = |offset_y: u32| sample_f32(source_x + dx, source_y + offset_y);
                        sum += f64::from(value(dy) + value(dy + 1));
                    }
                    dy += 2;
                }
                if dy < block_h {
                    let mut dx = 0;
                    while dx + 1 < block_w {
                        let value = |offset_x: u32| sample_f32(source_x + offset_x, source_y + dy);
                        sum += f64::from(value(dx) + value(dx + 1));
                        dx += 2;
                    }
                    if dx < block_w {
                        sum += f64::from(sample_f32(source_x + dx, source_y + dy));
                    }
                }
            } else {
                for dy in 0..block_h {
                    for dx in 0..block_w {
                        sum += f64::from(sample_f32(source_x + dx, source_y + dy));
                    }
                }
            }
            let value = (sum * (1.0 / f64::from(block_w * block_h))) as f32;
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    raw_bytes_to_image(dst_w, dst_h, out, 4)
}

#[cfg(not(feature = "parallel"))]
fn resize_i_thumbnail_reduce2x2(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    filter: ResampleFilter,
) -> Result<DynamicImage, PilError> {
    #[cfg(target_arch = "x86_64")]
    if let Some(fma) = X86FmaToken::detect() {
        return resize_i_thumbnail_reduce2x2_with_fma(img, dst_w, dst_h, filter, &fma);
    }
    resize_i_thumbnail_reduce2x2_with_fma(img, dst_w, dst_h, filter, &PortableFma)
}

/// Stream native-I 2×2 reducing-gap rows directly into the existing serial
/// resize ring. Pillow still rounds and stores each reduced sample and each
/// horizontal intermediate as INT32; this only removes the full reduced image
/// buffer between those exact stages.
#[cfg(not(feature = "parallel"))]
fn resize_i_thumbnail_reduce2x2_with_fma<F: F64MulAdd>(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    filter: ResampleFilter,
    fma: &F,
) -> Result<DynamicImage, PilError> {
    let (source_width, source_height, source_bytes) = i32_sample_bytes_from_native_storage(img)?;
    if source_width % 2 != 0 || source_height % 2 != 0 {
        return Err(PilError::InternalError(
            "streamed I thumbnail reduction requires even source dimensions".into(),
        ));
    }
    let source_pixels =
        CheckedDims::new_allow_empty(source_width, source_height, 4)?.total_pixels();
    let source = i32_samples_from_le_bytes(source_bytes, source_pixels);
    let reduced_width = source_width / 2;
    let reduced_height = source_height / 2;
    let (kernel, support) = resample_kernel(&filter);
    let horizontal = precompute_coeffs_f64(dst_w, reduced_width, kernel, support);
    let vertical = precompute_coeffs_f64(dst_h, reduced_height, kernel, support);
    let fixed_horizontal_weights = resize_i_contiguous_eight_tap_weights(&horizontal.weights);
    let max_vertical_taps = vertical.weights.iter().map(Vec::len).max().unwrap_or(0);
    let ring_rows = max_vertical_taps
        .checked_next_power_of_two()
        .ok_or_else(|| {
            PilError::ValueError("resize_i: vertical coefficient span is too large".into())
        })?;
    let ring_samples = ring_rows
        .checked_mul(dst_w as usize)
        .ok_or_else(|| PilError::ValueError("resize_i: vertical ring is too large".into()))?;
    let mut ring = vec![0_i32; ring_samples];
    let mut reduced_row = vec![0_i32; reduced_width as usize];
    let output_dims = CheckedDims::new(dst_w, dst_h, 4)?;
    let output_stride = output_dims.row_stride();
    let mut output_bytes = output_dims.alloc_buffer();
    let ring_mask = ring_rows - 1;
    let source_width = source_width as usize;
    let mut next_source_row = 0_usize;

    for (output_y, output_row) in output_bytes.chunks_mut(output_stride).enumerate() {
        let first_vertical_row = vertical.xmin[output_y] as usize;
        let vertical_weights = &vertical.weights[output_y];
        let source_end = first_vertical_row + vertical_weights.len();
        while next_source_row < source_end {
            let top_start = next_source_row * 2 * source_width;
            let bottom_start = top_start + source_width;
            let top = &source[top_start..top_start + source_width];
            let bottom = &source[bottom_start..bottom_start + source_width];
            for (output_x, reduced) in reduced_row.iter_mut().enumerate() {
                let source_x = output_x * 2;
                let quartet = top[source_x]
                    .wrapping_add(top[source_x + 1])
                    .wrapping_add(bottom[source_x])
                    .wrapping_add(bottom[source_x + 1]);
                *reduced = round_up(f64::from(quartet) / 4.0) as i32;
            }

            let ring_start = (next_source_row & ring_mask) * dst_w as usize;
            let horizontal_row = &mut ring[ring_start..ring_start + dst_w as usize];
            resize_i_horizontal_row(
                &reduced_row,
                horizontal_row,
                &horizontal,
                fixed_horizontal_weights.as_ref(),
                fma,
            );
            next_source_row += 1;
        }

        if let Ok(weights) = <&[f64; 8]>::try_from(vertical_weights.as_slice()) {
            for (output_x, output) in output_row.chunks_exact_mut(4).enumerate() {
                let accumulator = resize_i_sum_eight(
                    weights,
                    first_vertical_row,
                    |source_y| ring[(source_y & ring_mask) * dst_w as usize + output_x],
                    fma,
                );
                output.copy_from_slice(&resize_i_round_up_to_i32(accumulator).to_le_bytes());
            }
        } else {
            for (output_x, output) in output_row.chunks_exact_mut(4).enumerate() {
                let accumulator = resize_i_sum_general(
                    vertical_weights,
                    first_vertical_row,
                    |source_y| ring[(source_y & ring_mask) * dst_w as usize + output_x],
                    fma,
                );
                output.copy_from_slice(&resize_i_round_up_to_i32(accumulator).to_le_bytes());
            }
        }
    }

    let output = crate::raster::RgbaImage::from_raw(dst_w, dst_h, output_bytes)
        .ok_or_else(|| PilError::ValueError("resize_i: failed to create output buffer".into()))?;
    Ok(DynamicImage::ImageRgba8(output))
}

fn reduce_i_thumbnail(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    factor_x: u32,
    factor_y: u32,
) -> Result<DynamicImage, PilError> {
    // I's four-byte image is a scalar carrier here; borrow it to avoid a copy.
    let (src_w, src_h, source_image) = match img {
        DynamicImage::ImageRgba8(image) => (image.width(), image.height(), image),
        _ => {
            return Err(PilError::ValueError(
                "I-mode thumbnail reduction requires four-byte scalar storage".into(),
            ));
        }
    };
    let source_bytes = source_image.as_raw();
    let source_samples = i32_samples_from_le_bytes(
        source_bytes,
        source_bytes.len() / std::mem::size_of::<i32>(),
    );
    let source_stride = src_w as usize;
    let sample_i32 = |x: u32, y: u32| source_samples[y as usize * source_stride + x as usize];
    let mut out = Vec::with_capacity((dst_w * dst_h * 4) as usize);

    // Pillow's common native-I thumbnail reducing-gap uses complete 2×2
    // quartets. Traverse each pair of source rows contiguously and preserve
    // Reduce.c's wrapping INT32 additions before the f64 average. Partial
    // blocks and other factors stay on the generic path below.
    if factor_x == 2 && factor_y == 2 && src_w > 0 && src_h > 0 && src_w % 2 == 0 && src_h % 2 == 0
    {
        let row_pair_len = source_stride
            .checked_mul(2)
            .ok_or_else(|| PilError::InternalError("I thumbnail row size overflow".into()))?;
        for rows in source_samples.chunks_exact(row_pair_len) {
            let (top, bottom) = rows.split_at(source_stride);
            for (top_pair, bottom_pair) in top.chunks_exact(2).zip(bottom.chunks_exact(2)) {
                let quartet = top_pair[0]
                    .wrapping_add(top_pair[1])
                    .wrapping_add(bottom_pair[0])
                    .wrapping_add(bottom_pair[1]);
                let value = round_up(f64::from(quartet) / 4.0) as i32;
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
        return raw_bytes_to_image(dst_w, dst_h, out, 4);
    }

    let main_width = src_w / factor_x;
    let main_height = src_h / factor_y;
    for y in 0..dst_h {
        let source_y = y * factor_y;
        let block_h = factor_y.min(src_h - source_y);
        for x in 0..dst_w {
            let source_x = x * factor_x;
            let block_w = factor_x.min(src_w - source_x);
            // ImagingReduceNxN_32bpc adds pairs/quartets while the samples
            // are still INT32, so each intermediate addition wraps at 32
            // bits before the result is promoted to the double accumulator.
            // Its corner helper instead adds each partial-edge sample
            // directly to the double accumulator. Preserve those two paths
            // separately; summing every sample as i64 changes overflow cases.
            let mut sum = 0.0f64;
            if x < main_width && y < main_height {
                let value = |offset_x: u32, offset_y: u32| {
                    sample_i32(source_x + offset_x, source_y + offset_y)
                };
                let mut dy = 0;
                while dy + 1 < block_h {
                    let mut dx = 0;
                    while dx + 1 < block_w {
                        let quartet = value(dx, dy)
                            .wrapping_add(value(dx + 1, dy))
                            .wrapping_add(value(dx, dy + 1))
                            .wrapping_add(value(dx + 1, dy + 1));
                        sum += f64::from(quartet);
                        dx += 2;
                    }
                    if dx < block_w {
                        let pair = value(dx, dy).wrapping_add(value(dx, dy + 1));
                        sum += f64::from(pair);
                    }
                    dy += 2;
                }
                if dy < block_h {
                    let mut dx = 0;
                    while dx + 1 < block_w {
                        let pair = value(dx, dy).wrapping_add(value(dx + 1, dy));
                        sum += f64::from(pair);
                        dx += 2;
                    }
                    if dx < block_w {
                        sum += f64::from(value(dx, dy));
                    }
                }
            } else {
                for dy in 0..block_h {
                    for dx in 0..block_w {
                        sum += f64::from(sample_i32(source_x + dx, source_y + dy));
                    }
                }
            }
            let value = round_up(sum / f64::from(block_w * block_h)) as i32;
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    raw_bytes_to_image(dst_w, dst_h, out, 4)
}

/// Reduce native RGB blocks without the generic per-channel mode checks.
///
/// Keep Pillow's 24-bit reciprocal-and-amend rounding unchanged. Interior,
/// right-edge, bottom-edge, and corner blocks each retain their own divisor.
fn execute_reduce_rgb(
    img: &DynamicImage,
    x_factor: u32,
    y_factor: u32,
    explicit_mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    execute_reduce_rgb_with_sparse_rows(img, x_factor, y_factor, explicit_mode)
        .map(|reduced| reduced.map(|(image, _)| image))
}

/// Reduce RGB while retaining exact active rows when the sparse 2×2 path is
/// selected. Thumbnail can use that proof to skip zero rows in both resample
/// passes without rescanning the reduced image.
fn execute_reduce_rgb_with_sparse_rows(
    img: &DynamicImage,
    x_factor: u32,
    y_factor: u32,
    explicit_mode: Option<&str>,
) -> Result<Option<(DynamicImage, Option<Vec<bool>>)>, PilError> {
    if explicit_mode != Some("RGB") || !matches!(img, DynamicImage::ImageRgb8(_)) {
        return Ok(None);
    }

    let fx = x_factor.max(1);
    let fy = y_factor.max(1);
    let Some(full_divider) = fx.checked_mul(fy) else {
        return Ok(None);
    };
    // Keep both the channel sums plus amend and their fixed-point multiply
    // within u32 for the hot path. Larger factors retain the generic u64 code.
    if full_divider > u32::MAX / 256 {
        return Ok(None);
    }
    let (width, height) = img.dimensions();
    let new_width = width.div_ceil(fx);
    let new_height = height.div_ceil(fy);
    let source_stride = (width as usize).checked_mul(3);
    let expected_source_len = source_stride.and_then(|stride| stride.checked_mul(height as usize));
    let source = img.as_bytes();
    if expected_source_len != Some(source.len()) {
        return Ok(None);
    }

    if fx == 2 && fy == 2 && width % 2 == 0 && height % 2 == 0 {
        if let Some((output, nonzero_rows)) =
            execute_reduce_rgb_sparse_2x2_with_rows(img, new_width, new_height)?
        {
            return Ok(Some((output, Some(nonzero_rows))));
        }
    }

    let mut output = CheckedDims::new(new_width, new_height, 3)?.alloc_buffer();
    if new_width == 0 || new_height == 0 {
        return raw_bytes_to_image(new_width, new_height, output, 3)
            .map(|image| Some((image, None)));
    }

    let division_multiplier = |divider: u32| -> u32 {
        // Pillow's division_UINT32 reciprocal: 2^32 / (256 * divider).
        ((1u128 << 32) / (u128::from(divider) * 256)) as u32
    };
    let main_width = width / fx;
    let main_height = height / fy;
    let right_width = width % fx;
    let bottom_height = height % fy;
    let full_multiplier = division_multiplier(full_divider);
    let full_amend = full_divider / 2;
    let right_divider = right_width * fy;
    let right_multiplier = division_multiplier(right_divider.max(1));
    let right_amend = right_divider / 2;
    let bottom_divider = fx * bottom_height;
    let bottom_multiplier = division_multiplier(bottom_divider.max(1));
    let bottom_amend = bottom_divider / 2;
    let corner_divider = right_width * bottom_height;
    let corner_multiplier = division_multiplier(corner_divider.max(1));
    let corner_amend = corner_divider / 2;
    let source_stride = source_stride.expect("RGB source stride was checked");

    // Pillow's reducing-gap thumbnail path commonly reduces RGB by 2×2
    // before its final resample. Avoid four dynamic loops and per-block
    // coordinate work for this dense interior case; keep odd dimensions on
    // the general path so their partial right/bottom divisors are unchanged.
    if fx == 2 && fy == 2 && width % 2 == 0 && height % 2 == 0 {
        let output_stride = new_width as usize * 3;
        let process_row = |output_y: usize, output_row: &mut [u8]| {
            let source_top = output_y * 2 * source_stride;
            let source_bottom = source_top + source_stride;
            let top_row = &source[source_top..source_top + source_stride];
            let bottom_row = &source[source_bottom..source_bottom + source_stride];

            for (block_x, output_pixel) in output_row.chunks_exact_mut(3).enumerate() {
                let source_x = block_x * 6;
                let top_right = source_x + 3;
                for channel in 0..3 {
                    let sum = u32::from(top_row[source_x + channel])
                        + u32::from(top_row[top_right + channel])
                        + u32::from(bottom_row[source_x + channel])
                        + u32::from(bottom_row[top_right + channel]);
                    // For a 2×2 block Pillow's reciprocal is exactly
                    // `(sum + 2) / 4`; keep the rounding bias and replace the
                    // general per-channel reciprocal multiply with a shift.
                    #[expect(
                        clippy::arithmetic_side_effects,
                        reason = "four byte samples plus the Pillow rounding bias fit u32"
                    )]
                    let average = (sum + 2) >> 2;
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "the Pillow 2x2 average is bounded to one byte"
                    )]
                    {
                        output_pixel[channel] = average as u8;
                    }
                }
            }
        };

        #[cfg(feature = "parallel")]
        {
            const REDUCE_PARALLEL_PIXEL_THRESHOLD: usize = 512 * 512;
            let input_pixels = (width as usize).saturating_mul(height as usize);
            if input_pixels >= REDUCE_PARALLEL_PIXEL_THRESHOLD {
                crate::par_rows_mut!(
                    &mut output,
                    output_stride,
                    new_height as usize,
                    |_row_start, _row_end, y, row| { process_row(y as usize, row) }
                );
            } else {
                for y in 0..new_height as usize {
                    let start = y * output_stride;
                    process_row(y, &mut output[start..start + output_stride]);
                }
            }
        }

        #[cfg(not(feature = "parallel"))]
        for y in 0..new_height as usize {
            let start = y * output_stride;
            process_row(y, &mut output[start..start + output_stride]);
        }

        return raw_bytes_to_image(new_width, new_height, output, 3)
            .map(|image| Some((image, None)));
    }

    let write_rgb_block = |row: &mut [u8],
                           output_x: usize,
                           source_x: usize,
                           source_y: usize,
                           block_width: usize,
                           block_height: usize,
                           multiplier: u32,
                           amend: u32| {
        let mut red_sum = 0u32;
        let mut green_sum = 0u32;
        let mut blue_sum = 0u32;
        let mut source_row_start = source_y * source_stride + source_x * 3;
        for _ in 0..block_height {
            let mut red_row_sum = 0u32;
            let mut green_row_sum = 0u32;
            let mut blue_row_sum = 0u32;
            let mut source_index = source_row_start;
            for _ in 0..block_width {
                red_row_sum += u32::from(source[source_index]);
                green_row_sum += u32::from(source[source_index + 1]);
                blue_row_sum += u32::from(source[source_index + 2]);
                source_index += 3;
            }
            red_sum += red_row_sum;
            green_sum += green_row_sum;
            blue_sum += blue_row_sum;
            source_row_start += source_stride;
        }

        let destination_index = output_x * 3;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "the factor-area guard proves sums plus amend and the 24-bit reciprocal multiply fit u32"
        )]
        let red = ((red_sum + amend) * multiplier) >> 24;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "the factor-area guard proves sums plus amend and the 24-bit reciprocal multiply fit u32"
        )]
        let green = ((green_sum + amend) * multiplier) >> 24;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "the factor-area guard proves sums plus amend and the 24-bit reciprocal multiply fit u32"
        )]
        let blue = ((blue_sum + amend) * multiplier) >> 24;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "Pillow's RGB box average is an 8-bit result"
        )]
        let red = red as u8;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "Pillow's RGB box average is an 8-bit result"
        )]
        let green = green as u8;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "Pillow's RGB box average is an 8-bit result"
        )]
        let blue = blue as u8;
        row[destination_index] = red;
        row[destination_index + 1] = green;
        row[destination_index + 2] = blue;
    };

    let process_row = |y: u32, row: &mut [u8]| {
        let full_y = y < main_height;
        let source_y = if full_y {
            (y * fy) as usize
        } else {
            (main_height * fy) as usize
        };
        let block_height = if full_y {
            fy as usize
        } else {
            bottom_height as usize
        };
        let (interior_multiplier, interior_amend) = if full_y {
            (full_multiplier, full_amend)
        } else {
            (bottom_multiplier, bottom_amend)
        };
        for x in 0..main_width {
            write_rgb_block(
                row,
                x as usize,
                (x * fx) as usize,
                source_y,
                fx as usize,
                block_height,
                interior_multiplier,
                interior_amend,
            );
        }
        if right_width != 0 {
            let (edge_multiplier, edge_amend) = if full_y {
                (right_multiplier, right_amend)
            } else {
                (corner_multiplier, corner_amend)
            };
            write_rgb_block(
                row,
                main_width as usize,
                (main_width * fx) as usize,
                source_y,
                right_width as usize,
                block_height,
                edge_multiplier,
                edge_amend,
            );
        }
    };

    let output_stride = (new_width as usize) * 3;
    #[cfg(feature = "parallel")]
    {
        const REDUCE_PARALLEL_PIXEL_THRESHOLD: usize = 512 * 512;
        let input_pixels = (width as usize).saturating_mul(height as usize);
        if input_pixels >= REDUCE_PARALLEL_PIXEL_THRESHOLD {
            crate::par_rows_mut!(
                &mut output,
                output_stride,
                new_height as usize,
                |_row_start, _row_end, y, row| {
                    process_row(y, row);
                }
            );
        } else {
            for y in 0..new_height {
                let start = y as usize * output_stride;
                process_row(y, &mut output[start..start + output_stride]);
            }
        }
    }

    #[cfg(not(feature = "parallel"))]
    for y in 0..new_height {
        let start = y as usize * output_stride;
        process_row(y, &mut output[start..start + output_stride]);
    }

    raw_bytes_to_image(new_width, new_height, output, 3).map(|image| Some((image, None)))
}

/// Reduce large sparse RGB sources by updating only blocks with nonzero input.
/// Dense sources stop at the density bound and use the ordinary row kernel.
fn execute_reduce_rgb_sparse_2x2_with_rows(
    img: &DynamicImage,
    new_width: u32,
    new_height: u32,
) -> Result<Option<(DynamicImage, Vec<bool>)>, PilError> {
    let (width, height) = img.dimensions();
    let source = img.as_bytes();
    let source_pixels = source.len() / 3;
    if source_pixels < 512 * 512 || width == 0 || height == 0 {
        return Ok(None);
    }
    // Scanning remains worthwhile only when very few input pixels contribute
    // nonzero samples. Stop early on denser images so they pay only a bounded
    // prefix scan before continuing through the established reducer.
    let maximum_nonzero_pixels = source_pixels / 1024;
    let mut nonzero_pixels = 0usize;
    let mut block_sums = std::collections::HashMap::<usize, [u16; 3]>::new();
    let mut accumulate_pixel = |source_index: usize, pixel: &[u8]| {
        if pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 0 {
            return true;
        }
        nonzero_pixels += 1;
        if nonzero_pixels > maximum_nonzero_pixels {
            return false;
        }

        let source_x = source_index % width as usize;
        let source_y = source_index / width as usize;
        let block_index = (source_y / 2) * new_width as usize + source_x / 2;
        let sums = block_sums.entry(block_index).or_insert([0; 3]);
        sums[0] += u16::from(pixel[0]);
        sums[1] += u16::from(pixel[1]);
        sums[2] += u16::from(pixel[2]);
        true
    };

    // The common sparse case contains long runs of black RGB pixels. Check
    // three native words at a time, which covers eight complete pixels, before
    // entering the per-pixel accounting loop. Keep the pixel loop for any
    // nonzero block so channel order, density bounds, and edge coordinates
    // remain identical to the ordinary sparse scan.
    let grouped_pixels = source.len() / 24 * 8;
    for block_index in 0..source.len() / 24 {
        let byte_start = block_index * 24;
        let block = &source[byte_start..byte_start + 24];
        let words_are_zero = [0usize, 8, 16].into_iter().all(|offset| {
            u64::from_ne_bytes(
                block[offset..offset + 8]
                    .try_into()
                    .expect("each sparse RGB scan word has eight bytes"),
            ) == 0
        });
        if words_are_zero {
            continue;
        }
        for (pixel_offset, pixel) in block.chunks_exact(3).enumerate() {
            let source_index = block_index * 8 + pixel_offset;
            if !accumulate_pixel(source_index, pixel) {
                return Ok(None);
            }
        }
    }
    for (pixel_offset, pixel) in source[grouped_pixels * 3..].chunks_exact(3).enumerate() {
        if !accumulate_pixel(grouped_pixels + pixel_offset, pixel) {
            return Ok(None);
        }
    }

    let mut output = CheckedDims::new(new_width, new_height, 3)?.alloc_buffer();
    let mut nonzero_rows = vec![false; new_height as usize];
    for (block_index, sums) in block_sums {
        let output_index = block_index * 3;
        let pixel = [
            ((sums[0] + 2) >> 2) as u8,
            ((sums[1] + 2) >> 2) as u8,
            ((sums[2] + 2) >> 2) as u8,
        ];
        output[output_index..output_index + 3].copy_from_slice(&pixel);
        if pixel != [0; 3] {
            nonzero_rows[block_index / new_width as usize] = true;
        }
    }

    raw_bytes_to_image(new_width, new_height, output, 3).map(|image| Some((image, nonzero_rows)))
}

#[cfg(test)]
fn execute_reduce_rgb_sparse_2x2(
    img: &DynamicImage,
    new_width: u32,
    new_height: u32,
) -> Result<Option<DynamicImage>, PilError> {
    execute_reduce_rgb_sparse_2x2_with_rows(img, new_width, new_height)
        .map(|reduced| reduced.map(|(image, _)| image))
}

/// Sum one RGBA reduction block after Pillow's per-input premultiplication.
///
/// Keep the four stored RGBA channels explicit here: the generic byte-mode
/// loop checks the channel count and alpha condition for every sample, and
/// reloads alpha while visiting each color channel. Alpha is read once per
/// pixel and the RGB multiply/divide is performed directly on the native
/// four-byte carrier.
#[inline]
fn reduce_rgba_premultiplied_block_sums(
    source: &[u8],
    width: usize,
    source_x: usize,
    source_y: usize,
    block_width: usize,
    block_height: usize,
) -> [u64; 4] {
    let mut sums = [0u64; 4];
    for dy in 0..block_height {
        let mut source_index = ((source_y + dy) * width + source_x) * 4;
        for _ in 0..block_width {
            let alpha = u32::from(source[source_index + 3]);
            let red = u32::from(source[source_index]);
            let green = u32::from(source[source_index + 1]);
            let blue = u32::from(source[source_index + 2]);
            sums[0] += u64::from((red * alpha + 127) / 255);
            sums[1] += u64::from((green * alpha + 127) / 255);
            sums[2] += u64::from((blue * alpha + 127) / 255);
            sums[3] += u64::from(alpha);
            source_index += 4;
        }
    }
    sums
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
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if x_factor < 2 && y_factor < 2 {
        return Ok(img.clone());
    }
    let fx = x_factor.max(1);
    let fy = y_factor.max(1);
    let (w, h) = (img.width(), img.height());
    if matches!(img, DynamicImage::ImageRgba8(_)) && explicit_mode == Some("I") {
        return reduce_i_thumbnail(img, w.div_ceil(fx), h.div_ceil(fy), fx, fy);
    }
    if matches!(img, DynamicImage::ImageRgba8(_)) && explicit_mode == Some("F") {
        return reduce_f_thumbnail(img, w.div_ceil(fx), h.div_ceil(fy), fx, fy);
    }
    if let Some(output) = execute_reduce_rgb(img, fx, fy, explicit_mode)? {
        return Ok(output);
    }
    let channels = img.color().channel_count() as usize;
    let new_w = w.div_ceil(fx);
    let new_h = h.div_ceil(fy);
    let raw = img.as_bytes();
    let premultiplied_alpha = matches!(
        img.color(),
        crate::raster::ColorType::La8 | crate::raster::ColorType::Rgba8
    ) && !matches!(
        explicit_mode,
        Some("CMYK" | "RGBa" | "La" | "PA" | "RGBX" | "F" | "I")
    );
    let native_rgba_reduce = premultiplied_alpha
        && explicit_mode == Some("RGBA")
        && matches!(img, DynamicImage::ImageRgba8(_));
    let mut out = CheckedDims::new(new_w, new_h, channels as u8)?.alloc_buffer();
    if new_w == 0 || new_h == 0 {
        return raw_bytes_to_image(new_w, new_h, out, channels);
    }

    let division_multiplier = |divider: u32| -> u64 {
        // division_UINT32(divider, 8): 2^32 / (256 * divider), truncated.
        ((1u128 << 32) / (u128::from(divider) * 256)) as u64
    };

    // Every reduced output row reads immutable source pixels and owns a
    // disjoint destination slice. Keep the partial right/bottom blocks in the
    // same row function as the full blocks so the parallel and serial lanes
    // share one exact rounding order.
    let main_w = w / fx;
    let main_h = h / fy;
    let right_width = w % fx;
    let bottom_height = h % fy;
    let full_divider = fx * fy;
    let full_multiplier = division_multiplier(full_divider);
    let full_amend = full_divider / 2;
    let right_divider = right_width * fy;
    let right_multiplier = division_multiplier(right_divider.max(1));
    let right_amend = right_divider / 2;
    let bottom_divider = fx * bottom_height;
    let bottom_multiplier = division_multiplier(bottom_divider.max(1));
    let bottom_amend = bottom_divider / 2;
    let corner_divider = right_width * bottom_height;
    let corner_multiplier = division_multiplier(corner_divider.max(1));
    let corner_amend = corner_divider / 2;
    let process_row = |y: u32, row: &mut [u8]| {
        let full_y = y < main_h;
        let y_count = if full_y { fy } else { bottom_height };
        let source_y = if y < main_h { y * fy } else { main_h * fy };
        for x in 0..new_w {
            let full_x = x < main_w;
            let x_count = if full_x { fx } else { right_width };
            let source_x = if x < main_w { x * fx } else { main_w * fx };
            let (multiplier, amend) = match (full_x, full_y) {
                (true, true) => (full_multiplier, full_amend),
                (false, true) => (right_multiplier, right_amend),
                (true, false) => (bottom_multiplier, bottom_amend),
                (false, false) => (corner_multiplier, corner_amend),
            };
            let dst_idx = x as usize * channels;
            if native_rgba_reduce {
                let sums = reduce_rgba_premultiplied_block_sums(
                    raw,
                    w as usize,
                    source_x as usize,
                    source_y as usize,
                    x_count as usize,
                    y_count as usize,
                );
                let average = |sum: u64| (((sum + u64::from(amend)) * multiplier) >> 24) as u8;
                let alpha = average(sums[3]);
                let mut red = average(sums[0]);
                let mut green = average(sums[1]);
                let mut blue = average(sums[2]);
                if alpha != 0 {
                    red = (u16::from(red) * 255 / u16::from(alpha)) as u8;
                    green = (u16::from(green) * 255 / u16::from(alpha)) as u8;
                    blue = (u16::from(blue) * 255 / u16::from(alpha)) as u8;
                }
                row[dst_idx] = red;
                row[dst_idx + 1] = green;
                row[dst_idx + 2] = blue;
                row[dst_idx + 3] = alpha;
            } else {
                let mut sums = [0u64; 4];
                for dy in 0..y_count {
                    for dx in 0..x_count {
                        let src_idx = ((source_y + dy) * w + source_x + dx) as usize * channels;
                        for c in 0..channels {
                            let sample = if premultiplied_alpha && c + 1 < channels {
                                ((u16::from(raw[src_idx + c])
                                    * u16::from(raw[src_idx + channels - 1])
                                    + 127)
                                    / 255) as u8
                            } else {
                                raw[src_idx + c]
                            };
                            sums[c] += u64::from(sample);
                        }
                    }
                }
                for c in 0..channels {
                    let mut value = (((sums[c] + u64::from(amend)) * multiplier) >> 24) as u8;
                    if premultiplied_alpha && c + 1 < channels {
                        let alpha =
                            (((sums[channels - 1] + u64::from(amend)) * multiplier) >> 24) as u8;
                        if alpha != 0 {
                            value = (u16::from(value) * 255 / u16::from(alpha)) as u8;
                        }
                    }
                    row[dst_idx + c] = value;
                }
            }
        }
    };

    let output_stride = new_w as usize * channels;
    #[cfg(feature = "parallel")]
    {
        // Pillow's Reduce.c has no row-task boundary; each destination row is
        // independent.  Match the other CPU geometry kernels by keeping tiny
        // reductions serial: Rayon setup costs more than the complete 32x24
        // benchmark reduction, while large images still use row-level
        // parallelism.  Use source pixels for the guard so a large source
        // remains parallel even when its reduced output is smaller than 512².
        const REDUCE_PARALLEL_PIXEL_THRESHOLD: usize = 512 * 512;
        let input_pixels = (w as usize).saturating_mul(h as usize);
        if input_pixels >= REDUCE_PARALLEL_PIXEL_THRESHOLD {
            crate::par_rows_mut!(
                &mut out,
                output_stride,
                new_h as usize,
                |_row_start, _row_end, y, row| {
                    process_row(y, row);
                }
            );
        } else {
            for y in 0..new_h {
                let start = y as usize * output_stride;
                process_row(y, &mut out[start..start + output_stride]);
            }
        }
    }

    #[cfg(not(feature = "parallel"))]
    for y in 0..new_h {
        let start = y as usize * output_stride;
        process_row(y, &mut out[start..start + output_stride]);
    }

    let result = raw_bytes_to_image(new_w, new_h, out, channels)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::PortableFma;
    #[cfg(not(feature = "parallel"))]
    use super::resize_i;
    #[cfg(not(feature = "parallel"))]
    use super::resize_i_contiguous_eight_tap_weights;
    use super::resize_i_sum_eight;
    #[cfg(target_arch = "x86_64")]
    use super::{F64MulAdd, X86FmaToken, resize_i_with_fma};
    use super::{
        execute_reduce, execute_rgb_putpixel_thumbnail_fusion, execute_thumbnail,
        reduce_f_thumbnail, reduce_i_thumbnail, resize_f, rgb_putpixel_thumbnail_fusion_supported,
    };
    #[cfg(target_arch = "x86_64")]
    use super::{f_resize_samples_allow_finite_fma, resize_f_with_fma};
    #[cfg(not(feature = "parallel"))]
    use super::{resize_i_round_up_to_i32, round_up};
    use crate::pipeline::{PipelineOp, ResampleFilter};
    use crate::raster::{DynamicImage, GenericImageView, GrayImage, RgbImage, RgbaImage};

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn runtime_x86_fma_matches_portable_fused_operation() {
        let Some(x86_fma) = X86FmaToken::detect() else {
            return;
        };
        assert!(x86_fma.supports_finite_mul_add());
        let portable = PortableFma;
        for (weight, sample, accumulator) in [
            (0.1, 0.2, 0.3),
            (1.0 + f64::EPSILON, 1.0 - f64::EPSILON, -1.0),
            (f64::MAX, 2.0, -f64::MAX),
            (f64::MIN_POSITIVE, f64::EPSILON, -f64::MIN_POSITIVE),
            (-0.0, 1.0, -0.0),
            (1.0, -0.0, -0.0),
            (f64::INFINITY, 2.0, f64::NEG_INFINITY),
            (f64::NAN, 1.0, 0.0),
        ] {
            assert_eq!(
                x86_fma.mul_add(weight, sample, accumulator).to_bits(),
                portable.mul_add(weight, sample, accumulator).to_bits(),
                "FMA result differs for {weight:?} * {sample:?} + {accumulator:?}",
            );
            if weight.is_finite() && sample.is_finite() && accumulator.is_finite() {
                assert_eq!(
                    x86_fma
                        .mul_add_finite(weight, sample, accumulator)
                        .to_bits(),
                    portable.mul_add(weight, sample, accumulator).to_bits(),
                    "finite FMA result differs for {weight:?} * {sample:?} + {accumulator:?}",
                );
            }
        }
    }

    #[cfg(target_endian = "little")]
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn finite_x86_f_resize_path_matches_checked_reference() {
        let Some(x86_fma) = X86FmaToken::detect() else {
            return;
        };
        let (source_width, source_height) = (32u32, 24u32);
        let mut sample_sets = vec![
            (0..source_width as usize * source_height as usize)
                .map(|index| (((index * 73 % 4093) as i32 - 2046) as f32) / 17.0)
                .collect::<Vec<_>>(),
            (0..source_width as usize * source_height as usize)
                .map(|index| if index % 2 == 0 { f32::MAX } else { -f32::MAX })
                .collect::<Vec<_>>(),
        ];
        let mut nonfinite = sample_sets[0].clone();
        nonfinite[7] = f32::from_bits(0x7fc1_2345);
        nonfinite[19] = f32::INFINITY;
        nonfinite[41] = f32::NEG_INFINITY;
        sample_sets.push(nonfinite);

        for (set_index, samples) in sample_sets.iter().enumerate() {
            assert_eq!(
                f_resize_samples_allow_finite_fma(&x86_fma, samples),
                set_index < 2,
                "finite-sample admission differs for sample set {set_index}",
            );
            let source_bytes = samples
                .iter()
                .flat_map(|sample| sample.to_le_bytes())
                .collect();
            let source = DynamicImage::ImageRgba8(
                RgbaImage::from_raw(source_width, source_height, source_bytes)
                    .expect("F sample source shape must be valid"),
            );
            let actual = resize_f_with_fma(&source, 16, 12, &ResampleFilter::Bicubic, &x86_fma)
                .expect("x86 F resize must succeed");
            let portable =
                resize_f_with_fma(&source, 16, 12, &ResampleFilter::Bicubic, &PortableFma)
                    .expect("portable F resize must succeed");
            assert_eq!(
                actual.as_bytes(),
                portable.as_bytes(),
                "F resize differs from checked reference for sample set {set_index}",
            );
        }
    }

    #[test]
    fn transpose_tiled_rows_preserve_native_pixels_edges_and_trailing_storage() {
        use crate::pipeline::TransposeMethod;
        use crate::raster::GrayAlphaImage;

        for (width, height) in [
            (1, 1),
            (1, 257),
            (257, 1),
            (3, 7),
            (9, 31),
            (31, 9),
            (31, 33),
            (32, 32),
            (33, 31),
            (63, 65),
            (127, 129),
            (255, 257),
            (511, 513),
            (512, 512),
            (513, 515),
            (515, 513),
        ] {
            for channels in 1..=4 {
                let length = width as usize * height as usize * channels;
                // ImageBuffer permits extra samples after its logical raster.
                // Both paths must copy only pixels within the declared shape.
                let source: Vec<u8> = (0..length + channels * 3 + 1)
                    .map(|index| ((index * 73 + index / 17 * 29) % 256) as u8)
                    .collect();
                let image = match channels {
                    1 => DynamicImage::ImageLuma8(
                        GrayImage::from_raw(width, height, source.clone()).unwrap(),
                    ),
                    2 => DynamicImage::ImageLumaA8(
                        GrayAlphaImage::from_raw(width, height, source.clone()).unwrap(),
                    ),
                    3 => DynamicImage::ImageRgb8(
                        RgbImage::from_raw(width, height, source.clone()).unwrap(),
                    ),
                    4 => DynamicImage::ImageRgba8(
                        RgbaImage::from_raw(width, height, source.clone()).unwrap(),
                    ),
                    _ => unreachable!(),
                };
                for method in [
                    TransposeMethod::Transpose,
                    TransposeMethod::Transverse,
                    TransposeMethod::Rotate90,
                    TransposeMethod::Rotate270,
                ] {
                    let mut expected = vec![0u8; length];
                    for source_y in 0..height as usize {
                        for source_x in 0..width as usize {
                            let (target_x, target_y) = match method {
                                TransposeMethod::Transpose => (source_y, source_x),
                                TransposeMethod::Transverse => (
                                    height as usize - 1 - source_y,
                                    width as usize - 1 - source_x,
                                ),
                                TransposeMethod::Rotate90 => {
                                    (source_y, width as usize - 1 - source_x)
                                }
                                TransposeMethod::Rotate270 => {
                                    (height as usize - 1 - source_y, source_x)
                                }
                                _ => unreachable!(),
                            };
                            let input = (source_y * width as usize + source_x) * channels;
                            let output = (target_y * height as usize + target_x) * channels;
                            expected[output..output + channels]
                                .copy_from_slice(&source[input..input + channels]);
                        }
                    }
                    // Directly exercise partial tiles even below admission,
                    // and prove every output byte is written independently of
                    // initialized storage contents.
                    for sentinel in [0u8, 0xA5] {
                        let mut output = vec![sentinel; length];
                        macro_rules! transpose {
                            ($channels:literal) => {
                                super::transpose_bytes_tiled::<$channels>(
                                    &source,
                                    &mut output,
                                    width,
                                    height,
                                    &method,
                                    super::TRANSPOSE_TILE_SIZE,
                                )
                            };
                        }
                        match channels {
                            1 => transpose!(1),
                            2 => transpose!(2),
                            3 => transpose!(3),
                            4 => transpose!(4),
                            _ => unreachable!(),
                        }
                        assert_eq!(
                            output, expected,
                            "tiled {width}x{height} C{channels} {method:?}"
                        );
                    }
                    let actual =
                        super::execute_transpose(&image, &method).expect("native transpose");
                    assert_eq!(actual.dimensions(), (height, width));
                    assert_eq!(actual.color(), image.color());
                    assert_eq!(
                        actual.as_bytes(),
                        expected,
                        "dispatch {width}x{height} C{channels} {method:?}"
                    );
                    assert_eq!(image.as_bytes(), source, "source remains unchanged");
                }
            }
        }
    }

    #[test]
    fn transpose_row_copies_preserve_native_pixels_and_edges() {
        use crate::pipeline::TransposeMethod;
        for (width, height) in [
            (0, 3),
            (3, 0),
            (1, 9),
            (9, 1),
            (9, 31),
            (511, 513),
            (512, 512),
            (513, 515),
            (1, 262145),
            (262145, 1),
        ] {
            for channels in 1..=4 {
                let length = width as usize * height as usize * channels;
                let source: Vec<u8> = (0..length)
                    .map(|index| ((index * 73 + index / 17 * 29) % 256) as u8)
                    .collect();
                let image = crate::image_utils::raw_bytes_to_image_allow_empty(
                    width,
                    height,
                    source.clone(),
                    channels,
                )
                .expect("native source shape");
                for method in [
                    TransposeMethod::FlipLeftRight,
                    TransposeMethod::FlipTopBottom,
                    TransposeMethod::Rotate180,
                ] {
                    let mut expected = vec![0u8; length];
                    for y in 0..height as usize {
                        for x in 0..width as usize {
                            let (destination_x, destination_y) = match method {
                                TransposeMethod::FlipLeftRight => (width as usize - 1 - x, y),
                                TransposeMethod::FlipTopBottom => (x, height as usize - 1 - y),
                                TransposeMethod::Rotate180 => {
                                    (width as usize - 1 - x, height as usize - 1 - y)
                                }
                                _ => unreachable!(),
                            };
                            let input = (y * width as usize + x) * channels;
                            let output =
                                (destination_y * width as usize + destination_x) * channels;
                            expected[output..output + channels]
                                .copy_from_slice(&source[input..input + channels]);
                        }
                    }
                    let actual = super::execute_transpose(&image, &method).expect("native flip");
                    assert_eq!(actual.dimensions(), (width, height));
                    assert_eq!(actual.color(), image.color());
                    assert_eq!(
                        actual.as_bytes(),
                        expected,
                        "{width}x{height} C{channels} {method:?}"
                    );
                    assert_eq!(image.as_bytes(), source, "source remains unchanged");
                }
            }
        }
    }

    #[test]
    fn transpose_row_copies_discard_trailing_storage() {
        use crate::pipeline::TransposeMethod;
        let image = DynamicImage::ImageRgb8(
            RgbImage::from_raw(
                2,
                2,
                vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 99, 98, 97],
            )
            .expect("trailing samples are permitted by ImageBuffer"),
        );
        for (method, expected) in [
            (
                TransposeMethod::FlipLeftRight,
                [4, 5, 6, 1, 2, 3, 10, 11, 12, 7, 8, 9],
            ),
            (
                TransposeMethod::FlipTopBottom,
                [7, 8, 9, 10, 11, 12, 1, 2, 3, 4, 5, 6],
            ),
            (
                TransposeMethod::Rotate180,
                [10, 11, 12, 7, 8, 9, 4, 5, 6, 1, 2, 3],
            ),
        ] {
            let output = super::execute_transpose(&image, &method).expect("native flip");
            assert_eq!(output.as_bytes(), expected);
        }
    }

    #[test]
    fn rotate_near_right_angle_uses_affine_sampling() {
        let rgb = DynamicImage::ImageRgb8(
            RgbImage::from_raw(2, 1, vec![10, 20, 30, 40, 50, 60])
                .expect("RGB source shape must be valid"),
        );
        let rgba = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(2, 1, vec![10, 20, 30, 40, 50, 60, 70, 80])
                .expect("RGBA source shape must be valid"),
        );

        // Pillow only selects its transpose fast path for an exact 90°
        // multiple.  Rounding 89.9° or 90.1° into that path moves the source
        // pixel selected at the edge and diverges from Geometry.c's affine
        // nearest sampler.
        let rgb_output = super::execute_rotate(
            &rgb,
            89.9,
            false,
            None,
            None,
            None,
            ResampleFilter::Nearest,
            true,
            Some("RGB"),
        )
        .expect("near-right RGB rotation");
        assert_eq!(rgb_output.dimensions(), (2, 1));
        assert_eq!(rgb_output.as_bytes(), &[10, 20, 30, 0, 0, 0]);

        let rgba_output = super::execute_rotate(
            &rgba,
            90.1,
            false,
            None,
            None,
            None,
            ResampleFilter::Nearest,
            true,
            Some("RGBA"),
        )
        .expect("near-right RGBA rotation");
        assert_eq!(rgba_output.dimensions(), (2, 1));
        assert_eq!(rgba_output.as_bytes(), &[50, 60, 70, 80, 0, 0, 0, 0]);
    }

    #[cfg(target_endian = "little")]
    #[test]
    fn f_rotate_bicubic_uses_scalar_filter() {
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(
                2,
                2,
                [0.0f32, 1.0, 2.0, 3.0]
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect(),
            )
            .expect("F source shape must be valid"),
        );
        let output = super::execute_rotate(
            &source,
            45.0,
            true,
            None,
            None,
            None,
            ResampleFilter::Bicubic,
            false,
            Some("F"),
        )
        .expect("F bicubic rotation must succeed");
        assert_eq!(output.dimensions(), (4, 4));
        let expected_words = [
            0x0000_0000,
            0x0000_0000,
            0x0000_0000,
            0x0000_0000,
            0x0000_0000,
            0x3e75_57b3,
            0x4008_5542,
            0x0000_0000,
            0x0000_0000,
            0x3f5e_aaf6,
            0x4030_aa85,
            0x0000_0000,
            0x0000_0000,
            0x0000_0000,
            0x0000_0000,
            0x0000_0000,
        ];
        let expected: Vec<u8> = expected_words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        assert_eq!(output.as_bytes(), expected);
    }

    #[test]
    fn byte_rotate_bicubic_uses_geometry_c_horner() {
        let source = DynamicImage::ImageLuma8(
            GrayImage::from_raw(2, 2, vec![0, 1, 2, 3]).expect("L source shape must be valid"),
        );
        let output = super::execute_rotate(
            &source,
            45.0,
            true,
            None,
            None,
            None,
            ResampleFilter::Bicubic,
            false,
            Some("L"),
        )
        .expect("L bicubic rotation must succeed");
        assert_eq!(output.dimensions(), (4, 4));
        assert_eq!(
            output.as_bytes(),
            &[0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 2, 0, 0, 0, 0, 0]
        );
    }

    #[cfg(target_endian = "little")]
    #[test]
    fn f_bicubic_preserves_pillow_f64_rounding() {
        let source_words = [0xd9f6def9, 0x3210f3c9];
        let source_bytes = source_words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(1, 2, source_bytes).expect("source shape must be valid"),
        );

        let output = resize_f(&source, 2, 6, &ResampleFilter::Bicubic)
            .expect("finite F-mode bicubic resize must succeed");
        let DynamicImage::ImageRgba8(output) = output else {
            panic!("F-mode resize must retain packed float storage");
        };
        let expected_words = [
            0xda086dc0, 0xda086dc0, 0xd9f6def9, 0xd9f6def9, 0xd9accf48, 0xd9accf48, 0xd9141f62,
            0xd9141f62, 0x3210f3c9, 0x3210f3c9, 0x584fe430, 0x584fe430,
        ];
        let expected_bytes: Vec<u8> = expected_words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        assert_eq!(output.as_raw(), &expected_bytes);
    }

    #[cfg(target_endian = "little")]
    #[test]
    fn f_resize_tall_uses_pillow_vertical_first_order() {
        // Pillow's Image.resize switches a source taller than 100x its width
        // to vertical-first passes.  This 2x256 source is intentionally
        // heterogeneous so the materialized FLOAT32 intermediate is part of
        // the observable result rather than an algebraically interchangeable
        // detail.
        let source_bytes: Vec<u8> = (0..512)
            .map(|index| {
                let value = 0.25f32 + ((index * 29 % 120) as f32) * 0.01f32;
                value.to_bits()
            })
            .flat_map(u32::to_le_bytes)
            .collect();
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(2, 256, source_bytes).expect("tall F source shape must be valid"),
        );

        for (filter, expected_word) in [
            (ResampleFilter::Bilinear, 0x3f57_eb1eu32),
            (ResampleFilter::Bicubic, 0x3f57_ee45),
            (ResampleFilter::Lanczos, 0x3f57_e8a6),
            (ResampleFilter::Box, 0x3f57_b852),
            (ResampleFilter::Hamming, 0x3f58_0fba),
        ] {
            let output = resize_f(&source, 1, 1, &filter)
                .expect("tall F resize must preserve Pillow's pass order");
            let DynamicImage::ImageRgba8(output) = output else {
                panic!("F-mode resize must retain packed float storage");
            };
            assert_eq!(output.as_raw(), &expected_word.to_le_bytes());
        }
    }

    #[cfg(target_endian = "little")]
    #[test]
    fn f_nearest_uses_pillow_cumulative_row_mapping() {
        let source_words = [1.0f32.to_bits(), 2.0f32.to_bits()];
        let source_bytes = source_words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(1, 2, source_bytes).expect("source shape must be valid"),
        );

        let output = resize_f(&source, 1, 7, &ResampleFilter::Nearest)
            .expect("finite F-mode nearest resize must succeed");
        let DynamicImage::ImageRgba8(output) = output else {
            panic!("F-mode resize must retain packed float storage");
        };
        let expected_words = [
            1.0f32.to_bits(),
            1.0f32.to_bits(),
            1.0f32.to_bits(),
            1.0f32.to_bits(),
            2.0f32.to_bits(),
            2.0f32.to_bits(),
            2.0f32.to_bits(),
        ];
        let expected_bytes: Vec<u8> = expected_words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        assert_eq!(output.as_raw(), &expected_bytes);
    }

    #[cfg(target_endian = "little")]
    #[test]
    fn f_convolution_preserves_pillow_signed_zero() {
        let source_words = [0x8000_0000, 0x0000_0001];
        let source_bytes = source_words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(1, 2, source_bytes).expect("source shape must be valid"),
        );

        let output = resize_f(&source, 1, 3, &ResampleFilter::Bicubic)
            .expect("mixed signed-zero F-mode bicubic resize must succeed");
        let DynamicImage::ImageRgba8(output) = output else {
            panic!("F-mode resize must retain packed float storage");
        };
        let expected_words = [0x8000_0000, 0x0000_0000, 0x0000_0001];
        let expected_bytes: Vec<u8> = expected_words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        assert_eq!(output.as_raw(), &expected_bytes);
    }

    #[test]
    fn rgb_reduce_specialized_rows_match_generic_reference_at_edges() {
        for (width, height, x_factor, y_factor) in [
            (5, 4, 3, 2),
            (7, 11, 3, 5),
            (4, 7, 1, 3),
            (13, 8, 4, 1),
            (2, 2, 2, 2),
            (4, 2, 2, 2),
            (8, 6, 2, 2),
            (32, 24, 2, 2),
            (9, 5, 2, 2),
        ] {
            let source = (0..width as usize * height as usize * 3)
                .map(|index| ((index * 37 + index / 11 * 17 + 29) % 256) as u8)
                .collect();
            let image = DynamicImage::ImageRgb8(
                RgbImage::from_raw(width, height, source).expect("RGB source shape must be valid"),
            );
            let optimized = execute_reduce(&image, x_factor, y_factor, Some("RGB"))
                .expect("specialized RGB Reduce must succeed");
            let reference = execute_reduce(&image, x_factor, y_factor, None)
                .expect("generic RGB Reduce reference must succeed");

            assert_eq!(
                optimized.as_bytes(),
                reference.as_bytes(),
                "RGB Reduce mismatch for {width}×{height} by {x_factor}×{y_factor}"
            );
        }
    }

    #[test]
    fn rgb_sparse_2x2_reduction_matches_generic_and_rejects_dense_input() {
        let (width, height) = (1024, 768);
        let zero = DynamicImage::ImageRgb8(RgbImage::new(width, height));
        let zero_sparse = super::execute_reduce_rgb_sparse_2x2(&zero, width / 2, height / 2)
            .expect("all-zero RGB reduction must succeed")
            .expect("all-zero RGB should use the sparse path");
        let zero_reference =
            execute_reduce(&zero, 2, 2, None).expect("generic zero RGB reduction must succeed");
        assert_eq!(zero_sparse.as_bytes(), zero_reference.as_bytes());

        let mut source = vec![0; width as usize * height as usize * 3];
        for (x, y, color) in [
            (0, 0, [1, 2, 3]),
            (1, 1, [255, 17, 9]),
            (700, 300, [0, 129, 0]),
            (1023, 767, [12, 34, 56]),
        ] {
            let index = (y * width + x) as usize * 3;
            source[index..index + 3].copy_from_slice(&color);
        }
        let image = DynamicImage::ImageRgb8(
            RgbImage::from_raw(width, height, source).expect("RGB source shape must be valid"),
        );

        let sparse = super::execute_reduce_rgb_sparse_2x2(&image, width / 2, height / 2)
            .expect("sparse RGB reduction must succeed")
            .expect("few nonzero RGB pixels should use the sparse path");
        let routed = execute_reduce(&image, 2, 2, Some("RGB")).expect("RGB Reduce must succeed");
        let reference =
            execute_reduce(&image, 2, 2, None).expect("generic RGB Reduce reference must succeed");
        assert_eq!(sparse.as_bytes(), reference.as_bytes());
        assert_eq!(routed.as_bytes(), reference.as_bytes());

        let (tail_width, tail_height) = (1022, 514);
        let mut tail_source = vec![0; tail_width as usize * tail_height as usize * 3];
        let last_pixel = (tail_width as usize * tail_height as usize - 1) * 3;
        tail_source[last_pixel..last_pixel + 3].copy_from_slice(&[19, 37, 251]);
        let tail_image = DynamicImage::ImageRgb8(
            RgbImage::from_raw(tail_width, tail_height, tail_source)
                .expect("tail RGB source shape must be valid"),
        );
        let tail_sparse = super::execute_reduce_rgb_sparse_2x2(
            &tail_image,
            tail_width.div_ceil(2),
            tail_height.div_ceil(2),
        )
        .expect("sparse RGB scan tail must succeed")
        .expect("sparse RGB scan with a partial eight-pixel tail should be selected");
        let tail_reference = execute_reduce(&tail_image, 2, 2, None)
            .expect("generic odd-size RGB reduction must succeed");
        assert_eq!(tail_sparse.as_bytes(), tail_reference.as_bytes());

        let dense = DynamicImage::ImageRgb8(
            RgbImage::from_raw(width, height, vec![1; width as usize * height as usize * 3])
                .expect("dense RGB source shape must be valid"),
        );
        assert!(
            super::execute_reduce_rgb_sparse_2x2(&dense, width / 2, height / 2)
                .expect("density probe must succeed")
                .is_none(),
            "dense RGB inputs must retain the established reducer"
        );
    }

    #[test]
    fn rgba_reduce_native_channel_sums_match_generic_reference_at_edges() {
        for (width, height, x_factor, y_factor) in [
            (32, 24, 4, 3),
            (34, 27, 4, 3),
            (19, 23, 3, 5),
            (11, 9, 2, 2),
        ] {
            let source = (0..width as usize * height as usize * 4)
                .map(|index| ((index * 73 + index / 13 * 41 + 19) % 256) as u8)
                .collect();
            let image = DynamicImage::ImageRgba8(
                RgbaImage::from_raw(width, height, source)
                    .expect("RGBA source shape must be valid"),
            );
            let optimized = execute_reduce(&image, x_factor, y_factor, Some("RGBA"))
                .expect("native RGBA Reduce must succeed");
            let reference = execute_reduce(&image, x_factor, y_factor, None)
                .expect("generic RGBA Reduce reference must succeed");

            assert_eq!(
                optimized.as_bytes(),
                reference.as_bytes(),
                "RGBA Reduce mismatch for {width}×{height} by {x_factor}×{y_factor}"
            );
        }
    }

    #[cfg(target_endian = "little")]
    #[test]
    fn f_thumbnail_reduce_matches_pillow_32bpc_grouping() {
        let source_words = [
            0x0e65_a54au32,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
            0x0e65_a54a,
        ];
        let source_bytes = source_words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(4, 4, source_bytes).expect("source shape must be valid"),
        );
        let output = reduce_f_thumbnail(&source, 1, 1, 4, 4)
            .expect("finite F-mode thumbnail reduction must succeed");
        let DynamicImage::ImageRgba8(output) = output else {
            panic!("F-mode thumbnail reduction must retain packed float storage");
        };
        assert_eq!(output.as_raw(), &0x0e65_a54au32.to_le_bytes());
    }

    #[cfg(target_endian = "little")]
    #[test]
    fn f_thumbnail_reduce_matches_pillow_float_group_order() {
        let source_words = [
            0xc42a_2b37,
            0xc3a2_c889,
            0xc3a6_341f,
            0xc432_e997,
            0x4411_0932,
            0xc46f_61cc,
        ];
        let source_bytes = source_words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(3, 2, source_bytes).expect("source shape must be valid"),
        );
        let output = reduce_f_thumbnail(&source, 1, 1, 3, 2)
            .expect("finite F-mode thumbnail reduction must succeed");
        let DynamicImage::ImageRgba8(output) = output else {
            panic!("F-mode thumbnail reduction must retain packed float storage");
        };
        assert_eq!(output.as_raw(), &0xc3ca_a3eau32.to_le_bytes());
    }

    #[cfg(target_endian = "little")]
    #[test]
    fn f_thumbnail_2x2_row_output_keeps_scalar_rounding_and_bits() {
        let (width, height) = (16u32, 10u32);
        let mut source_words = (0..width as usize * height as usize)
            .map(|index| (((index * 37 % 257) as i32 - 128) as f32) / 31.0)
            .map(f32::to_bits)
            .collect::<Vec<_>>();
        source_words[0] = (-0.0f32).to_bits();
        source_words[1] = (-0.0f32).to_bits();
        source_words[width as usize] = (-0.0f32).to_bits();
        source_words[width as usize + 1] = (-0.0f32).to_bits();
        source_words[4] = f32::from_bits(0x7fc1_2345).to_bits();
        source_words[width as usize * 2 + 7] = f32::INFINITY.to_bits();
        source_words[width as usize * 4 + 13] = f32::NEG_INFINITY.to_bits();
        let source_bytes = source_words
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>();
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(width, height, source_bytes).expect("F source shape must be valid"),
        );

        let actual = reduce_f_thumbnail(&source, width / 2, height / 2, 2, 2)
            .expect("even 2x2 F thumbnail reduction must succeed");
        let DynamicImage::ImageRgba8(actual) = actual else {
            panic!("F-mode thumbnail reduction must retain packed float storage");
        };
        let mut expected = Vec::with_capacity(width as usize * height as usize);
        for y in 0..height as usize / 2 {
            let top_row = y * 2 * width as usize;
            let bottom_row = top_row + width as usize;
            for x in 0..width as usize / 2 {
                let source_x = x * 2;
                let top_left = f32::from_bits(source_words[top_row + source_x]);
                let top_right = f32::from_bits(source_words[top_row + source_x + 1]);
                let bottom_left = f32::from_bits(source_words[bottom_row + source_x]);
                let bottom_right = f32::from_bits(source_words[bottom_row + source_x + 1]);
                let quartet = ((top_left + top_right) + bottom_left) + bottom_right;
                let mut sum = 0.0f64;
                sum += f64::from(quartet);
                expected.extend_from_slice(&((sum * 0.25) as f32).to_le_bytes());
            }
        }
        assert_eq!(actual.as_raw(), expected.as_slice());
    }

    #[cfg(target_endian = "little")]
    #[test]
    fn i_thumbnail_reduce_matches_pillow_int32_grouping() {
        let source_words = [i32::MAX; 4];
        let source_bytes: Vec<u8> = source_words
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect();
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(2, 2, source_bytes).expect("source shape must be valid"),
        );
        let output = reduce_i_thumbnail(&source, 1, 1, 2, 2)
            .expect("I-mode thumbnail reduction must succeed");
        let DynamicImage::ImageRgba8(output) = output else {
            panic!("I-mode thumbnail reduction must retain packed integer storage");
        };
        // Reduce.c forms the interior quartet while still INT32: four
        // INT32_MAX values wrap to -4 before promotion and averaging.
        assert_eq!(output.as_raw(), &(-1i32).to_le_bytes());
    }

    #[cfg(target_endian = "little")]
    #[test]
    fn i_thumbnail_2x2_row_path_matches_multiple_wrapping_quartets() {
        let source_words = [
            i32::MAX,
            1,
            i32::MIN,
            -1,
            -3,
            i32::MAX,
            9,
            11,
            4,
            -4,
            5,
            -5,
            6,
            8,
            i32::MIN,
            i32::MAX,
        ];
        let source_bytes = source_words
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(4, 4, source_bytes).expect("source shape must be valid"),
        );
        let output =
            reduce_i_thumbnail(&source, 2, 2, 2, 2).expect("I-mode 2x2 reduction must succeed");

        let mut expected = Vec::with_capacity(2 * 2 * 4);
        for y in 0..2usize {
            for x in 0..2usize {
                let top = y * 2 * 4 + x * 2;
                let bottom = top + 4;
                let quartet = source_words[top]
                    .wrapping_add(source_words[top + 1])
                    .wrapping_add(source_words[bottom])
                    .wrapping_add(source_words[bottom + 1]);
                let value = super::round_up(f64::from(quartet) / 4.0) as i32;
                expected.extend_from_slice(&value.to_le_bytes());
            }
        }
        assert_eq!(output.as_bytes(), expected);
    }

    #[cfg(not(feature = "parallel"))]
    #[test]
    fn i_thumbnail_streaming_2x2_matches_materialized_reduce_and_resize() {
        let samples: Vec<i32> = (0..32 * 24)
            .map(|index| match index % 8 {
                0 => i32::MAX,
                1 => i32::MIN,
                2 => -1,
                3 => 1,
                4 => index * 7919,
                5 => -(index * 3571),
                6 => 0,
                _ => i32::MAX / 2,
            })
            .collect();
        let source_bytes: Vec<u8> = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(32, 24, source_bytes).expect("source shape must be valid"),
        );
        let filters = [
            ResampleFilter::Box,
            ResampleFilter::Bilinear,
            ResampleFilter::Hamming,
            ResampleFilter::Bicubic,
            ResampleFilter::Lanczos,
        ];

        for filter in filters {
            let reduced = reduce_i_thumbnail(&source, 16, 12, 2, 2)
                .expect("I-mode 2x2 reduction must succeed");
            for (destination_width, destination_height) in [(8, 6), (7, 5)] {
                let expected = resize_i(&reduced, destination_width, destination_height, &filter)
                    .expect("materialized I-mode resize must succeed");
                let actual = execute_thumbnail(
                    &source,
                    destination_width,
                    destination_height,
                    &filter,
                    Some("I"),
                )
                .expect("I-mode thumbnail must succeed");
                let (DynamicImage::ImageRgba8(actual), DynamicImage::ImageRgba8(expected)) =
                    (actual, expected)
                else {
                    panic!("I-mode thumbnail must retain packed integer storage");
                };
                assert_eq!(
                    actual.as_raw(),
                    expected.as_raw(),
                    "filter={filter:?}, size={destination_width}x{destination_height}"
                );
            }
        }
    }

    #[cfg(target_endian = "little")]
    #[test]
    fn i_thumbnail_odd_2x2_edges_keep_partial_double_accumulation() {
        let source_words = [
            i32::MAX,
            1,
            i32::MIN,
            -1,
            17,
            -3,
            i32::MAX,
            9,
            11,
            -29,
            4,
            -4,
            5,
            -5,
            i32::MIN,
        ];
        let source_bytes = source_words
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(5, 3, source_bytes).expect("source shape must be valid"),
        );
        let output = reduce_i_thumbnail(&source, 3, 2, 2, 2)
            .expect("I-mode odd-edge reduction must succeed");

        let mut expected = Vec::with_capacity(3 * 2 * 4);
        let main_width = 5 / 2;
        let main_height = 3 / 2;
        for y in 0..2usize {
            for x in 0..3usize {
                let source_x = x * 2;
                let source_y = y * 2;
                let block_width = 2.min(5 - source_x);
                let block_height = 2.min(3 - source_y);
                let mut sum = 0.0f64;
                if x < main_width && y < main_height {
                    let top = source_y * 5 + source_x;
                    let bottom = top + 5;
                    let quartet = source_words[top]
                        .wrapping_add(source_words[top + 1])
                        .wrapping_add(source_words[bottom])
                        .wrapping_add(source_words[bottom + 1]);
                    sum = f64::from(quartet);
                } else {
                    for dy in 0..block_height {
                        for dx in 0..block_width {
                            sum += f64::from(source_words[(source_y + dy) * 5 + source_x + dx]);
                        }
                    }
                }
                let divisor =
                    u32::try_from(block_width * block_height).expect("test block size fits u32");
                let value = super::round_up(sum / f64::from(divisor)) as i32;
                expected.extend_from_slice(&value.to_le_bytes());
            }
        }
        assert_eq!(output.as_bytes(), expected);
    }

    #[cfg(target_endian = "little")]
    #[test]
    fn f_hamming_matches_pillow_fma_window_on_cancellation() {
        let source_words = [
            1.0f32.to_bits(),
            (-1.0f32).to_bits(),
            3.0f32.to_bits(),
            (-3.0f32).to_bits(),
            1.0f32.to_bits(),
            (-1.0f32).to_bits(),
            3.0f32.to_bits(),
            (-3.0f32).to_bits(),
        ];
        let source_bytes: Vec<u8> = source_words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(8, 1, source_bytes).expect("source shape must be valid"),
        );

        let output = resize_f(&source, 4, 1, &ResampleFilter::Hamming)
            .expect("finite F-mode Hamming resize must succeed");
        let DynamicImage::ImageRgba8(output) = output else {
            panic!("F-mode resize must retain packed float storage");
        };
        // Pillow's Resample.c Hamming window contracts its `0.46f * cos +
        // 0.54f` expression before the sinc product.  The exact cancellation
        // residuals here catch either a separated window expression or a
        // changed accumulation order.
        let expected_words = [0x3df4_077e, 0xa3c0_0000, 0xa300_0000, 0xbd22_afaa];
        let expected_bytes: Vec<u8> = expected_words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        assert_eq!(output.as_raw(), &expected_bytes);
    }

    #[cfg(target_endian = "little")]
    #[test]
    fn f_hamming_preserves_near_zero_kernel_residual() {
        let source_values = [
            -2.0f32, -1.0, -0.5, -2.0, -1.0, -0.5, -2.0, -1.0, -0.5, -2.0, -1.0, -0.5, -2.0, -1.0,
            -0.5, -2.0, -1.0, -0.5, -2.0, -1.0, -0.5, -2.0, -1.0, -0.5, -2.0,
        ];
        let source_bytes: Vec<u8> = source_values
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect();
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(1, 25, source_bytes).expect("source shape must be valid"),
        );

        let output = resize_f(&source, 1, 11, &ResampleFilter::Hamming)
            .expect("finite F-mode Hamming resize must succeed");
        let DynamicImage::ImageRgba8(output) = output else {
            panic!("F-mode resize must retain packed float storage");
        };
        // For the centered output row, Resample.c evaluates a tiny non-zero
        // kernel argument rather than taking its exact-zero branch.  The
        // float Hamming constants therefore contribute a residual that is
        // visible in the serialized f32 word.
        let expected_words = [
            0xbfab_ebf1,
            0xbfb0_c781,
            0xbf86_fa08,
            0xbf6f_206e,
            0xbfa1_ad5c,
            0xbfb3_56af,
            0xbf8c_8ddf,
            0xbf6b_37f1,
            0xbf9b_c741,
            0xbfb4_e066,
            0xbf93_63f8,
        ];
        let expected_bytes: Vec<u8> = expected_words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        assert_eq!(output.as_raw(), &expected_bytes);
    }

    #[test]
    fn i_resize_eight_tap_accumulation_matches_scalar_fma_order() {
        let weights: [f64; 8] = [
            -0.03125, 0.09375, -0.15625, 0.59375, 0.59375, -0.15625, 0.09375, -0.03125,
        ];
        for source in [
            [i32::MIN, i32::MAX, -1, 0, 1, i32::MIN + 1, i32::MAX - 1, 17],
            [-1_000_003, 923_771, 0, 17, -31, 63, -127, 255],
        ] {
            let expected = weights
                .iter()
                .zip(source)
                .fold(0.0_f64, |acc, (&weight, sample)| {
                    weight.mul_add(f64::from(sample), acc)
                });
            let actual = resize_i_sum_eight(&weights, 0, |index| source[index], &PortableFma);
            assert_eq!(actual.to_bits(), expected.to_bits());
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn runtime_x86_fma_i_resize_matches_portable_path() {
        let Some(x86_fma) = X86FmaToken::detect() else {
            return;
        };
        let (source_width, source_height) = (32u32, 24u32);
        let samples: Vec<i32> = (0..source_width as usize * source_height as usize)
            .map(|index| match index % 8 {
                0 => i32::MIN,
                1 => i32::MAX,
                2 => -1,
                3 => 0,
                4 => i32::MIN + 1,
                5 => i32::MAX - 1,
                _ => (index as i32).wrapping_mul(73_856_093),
            })
            .collect();
        let source_bytes: Vec<u8> = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(source_width, source_height, source_bytes)
                .expect("I-mode source shape must be valid"),
        );

        for filter in [ResampleFilter::Bicubic, ResampleFilter::Lanczos] {
            let actual = resize_i_with_fma(&source, 16, 12, &filter, &x86_fma)
                .expect("x86 I-mode resize must succeed");
            let portable = resize_i_with_fma(&source, 16, 12, &filter, &PortableFma)
                .expect("portable I-mode resize must succeed");
            assert_eq!(
                actual.as_bytes(),
                portable.as_bytes(),
                "I-mode {filter:?} resize differs from the portable fused reference",
            );
        }
    }

    #[cfg(not(feature = "parallel"))]
    #[test]
    fn i_resize_rounding_fast_path_matches_pillow_conversion() {
        let mut values = vec![
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NAN,
            f64::MAX,
            f64::MIN,
            -0.0,
            0.0,
            f64::from(i32::MIN) - 0.5,
            f64::from(i32::MIN),
            f64::from(i32::MAX),
            f64::from(i32::MAX) + 0.5,
            f64::from_bits(0.5_f64.to_bits() - 1),
            0.5,
            f64::from_bits(0.5_f64.to_bits() + 1),
            f64::from_bits((-0.5_f64).to_bits() - 1),
            -0.5,
            f64::from_bits((-0.5_f64).to_bits() + 1),
        ];

        for integer in -1024_i32..=1024 {
            let positive_half = f64::from(integer) + 0.5;
            let negative_half = -positive_half;
            for boundary in [positive_half, negative_half] {
                values.push(f64::from_bits(boundary.to_bits() - 1));
                values.push(boundary);
                values.push(f64::from_bits(boundary.to_bits() + 1));
            }
        }

        let mut state = 0x8a5c_13d7_6b24_e901_u64;
        for _ in 0..10_000 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let bytes = state.to_le_bytes();
            let integer = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            let fraction_bits = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
            let fraction = f64::from(fraction_bits) / f64::from(u32::MAX);
            values.push(f64::from(integer) + fraction - 0.5);
        }

        for value in values {
            assert_eq!(
                resize_i_round_up_to_i32(value),
                round_up(value) as i32,
                "rounding mismatch for {value:?}"
            );
        }
    }

    #[cfg(not(feature = "parallel"))]
    #[test]
    fn i_resize_packs_only_a_contiguous_eight_tap_interior() {
        let coefficients = vec![
            vec![0.0; 4],
            (0..8).map(f64::from).collect(),
            (8..16).map(f64::from).collect(),
            vec![0.0; 3],
        ];
        let Some((first, packed)) = resize_i_contiguous_eight_tap_weights(&coefficients) else {
            panic!("the adjacent eight-tap spans must be packed");
        };
        assert_eq!(first, 1);
        assert_eq!(packed[0], [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]);
        assert_eq!(packed[1], [8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0]);

        let disjoint = vec![vec![1.0; 8], vec![1.0; 3], vec![1.0; 8]];
        assert!(resize_i_contiguous_eight_tap_weights(&disjoint).is_none());
    }

    #[test]
    fn rgb_putpixel_thumbnail_fusion_matches_materialized_putpixel_and_thumbnail() {
        let (source_width, source_height) = (1024, 768);
        let (output_width, output_height) = (256, 192);
        let filters = [
            ResampleFilter::Bilinear,
            ResampleFilter::Bicubic,
            ResampleFilter::Lanczos,
            ResampleFilter::Box,
            ResampleFilter::Hamming,
        ];
        let putpixel_ops = vec![
            PipelineOp::PutPixel {
                x: 2,
                y: 3,
                color: (180, 120, 60, 255),
                palette_index: false,
            },
            PipelineOp::PutPixel {
                x: 2,
                y: 3,
                color: (10, 20, 30, 255),
                palette_index: false,
            },
            PipelineOp::PutPixel {
                x: source_width - 1,
                y: source_height - 1,
                color: (7, 129, 251, 255),
                palette_index: false,
            },
        ];

        for dense in [false, true] {
            let bytes = if dense {
                (0..source_width as usize * source_height as usize * 3)
                    .map(|index| (index.wrapping_mul(73).wrapping_add(index / 17 * 29)) as u8)
                    .collect()
            } else {
                vec![0; source_width as usize * source_height as usize * 3]
            };
            let source = DynamicImage::ImageRgb8(
                RgbImage::from_raw(source_width, source_height, bytes)
                    .expect("test RGB source dimensions must match its bytes"),
            );
            let nearest_thumbnail = PipelineOp::Thumbnail {
                w: output_width,
                h: output_height,
                filter: ResampleFilter::Nearest,
            };
            assert!(!rgb_putpixel_thumbnail_fusion_supported(
                &source,
                &putpixel_ops,
                &nearest_thumbnail,
                Some("RGB"),
            ));
            let materialized_source =
                crate::compute::pool_cpu::ops::effects::op_put_pixel_batch(&source, &putpixel_ops)
                    .expect("validated RGB pixel writes must succeed");
            for filter in filters {
                let thumbnail = PipelineOp::Thumbnail {
                    w: output_width,
                    h: output_height,
                    filter,
                };
                assert!(rgb_putpixel_thumbnail_fusion_supported(
                    &source,
                    &putpixel_ops,
                    &thumbnail,
                    Some("RGB"),
                ));
                let expected = execute_thumbnail(
                    &materialized_source,
                    output_width,
                    output_height,
                    &filter,
                    Some("RGB"),
                )
                .expect("reference RGB thumbnail must succeed");
                let actual = execute_rgb_putpixel_thumbnail_fusion(
                    &source,
                    &putpixel_ops,
                    &thumbnail,
                    Some("RGB"),
                )
                .expect("fused RGB thumbnail must succeed");
                assert_eq!(
                    actual.as_bytes(),
                    expected.as_bytes(),
                    "dense={dense} filter={filter:?}"
                );
            }
        }
    }
}
