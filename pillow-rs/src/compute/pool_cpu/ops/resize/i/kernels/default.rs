/// Resize an I-mode image (32-bit signed integers stored as RGBA8 bytes LE).
use crate::checked_dims::CheckedDims;
use crate::error::PilError;

use crate::ops::pil_resize::FilterCoeffsF64;
use crate::ops::pil_resize::{
    F64MulAdd, PortableFma, i32_samples_from_le_bytes, precompute_coeffs_f64,
    precompute_coeffs_f64_boxed, round_up,
};
use crate::pipeline::ResampleFilter;
use crate::raster::{DynamicImage, GenericImageView};

use super::super::super::resample_kernel;
/// Uses PIL's two-pass separable approach matching ImagingResample.
/// The common eight-tap interior keeps Pillow's sequential FMA order; clipped
/// edge spans retain the general tap loop.
#[inline(always)]
pub(crate) fn resize_i_mul_add<F: F64MulAdd>(
    fma: &F,
    weight: f64,
    sample: i32,
    accumulator: f64,
) -> f64 {
    fma.mul_add(weight, f64::from(sample), accumulator)
}

#[inline(always)]
pub(crate) fn resize_i_sum_eight<F: F64MulAdd>(
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
pub(crate) fn resize_i_sum_eight_contiguous<F: F64MulAdd>(
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
pub(crate) fn resize_i_sum_general<F: F64MulAdd>(
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
pub(crate) fn resize_i_round_up_to_i32(value: f64) -> i32 {
    (value + 0.5_f64.copysign(value)) as i32
}

/// Copy one contiguous run of eight-tap coefficients into a dense fixed-width
/// table so row resampling does not reclassify the same horizontal spans.
pub(crate) fn resize_i_contiguous_eight_tap_weights(
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

pub(crate) fn resize_i_horizontal_row<F: F64MulAdd>(
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

pub(crate) fn resize_i_streaming_vertical_pass<F: F64MulAdd>(
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

pub(crate) fn resize_i_streaming_with_fma<F: F64MulAdd>(
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

pub(crate) fn resize_i_default(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    filter: &ResampleFilter,
) -> Result<DynamicImage, PilError> {
    resize_i_with_fma(img, dst_w, dst_h, filter, &PortableFma)
}

#[inline(always)]
fn resize_i_nearest(
    src_ints: &[i32],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    output_dims: CheckedDims,
    coordinate_scales: (f64, f64, f64, f64),
) -> DynamicImage {
    let (sw, sh) = source_size;
    let (dst_w, dst_h) = destination_size;
    let (sw_f, sh_f, dw_f, dh_f) = coordinate_scales;
    let output_stride = output_dims.row_stride();
    let mut output_bytes = output_dims.alloc_buffer();

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
    DynamicImage::ImageRgba8(out)
}

pub(crate) fn resize_i_with_fma<F: F64MulAdd>(
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

    // PIL-compatible scale factor for kernel widening during downscaling.
    let _sx_scale = (sw_f / dw_f).max(1.0);
    let _sy_scale = (sh_f / dh_f).max(1.0);

    let output_dims = CheckedDims::new(dst_w, dst_h, 4)?;

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
        return Ok(resize_i_nearest(
            &src_ints,
            (sw, sh),
            (dst_w, dst_h),
            output_dims,
            (sw_f, sh_f, dw_f, dh_f),
        ));
    }

    // PIL: for INT32 images, coefficients stay as double-precision (not fixed-point).
    // Use f64 accumulation + ROUND_UP matching PIL's ImagingResample for 32-bit types.
    let h_coeffs_f64 = precompute_coeffs_f64(dst_w, sw, kernel, support);
    let v_coeffs_f64 = precompute_coeffs_f64(dst_h, sh, kernel, support);

    let h_eight_tap_weights = resize_i_contiguous_eight_tap_weights(&h_coeffs_f64.weights);

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
}

/// Borrow I-mode's little-endian four-byte carrier after validating its
/// concrete storage. `I` samples are signed scalar words, not RGBA channels.
pub(crate) fn i32_sample_bytes_from_native_storage(
    img: &DynamicImage,
) -> Result<(u32, u32, &[u8]), PilError> {
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
pub(crate) fn i32_samples_from_native_storage<'a>(
    img: &'a DynamicImage,
) -> Result<(u32, u32, std::borrow::Cow<'a, [i32]>), PilError> {
    let (width, height, source_bytes) = i32_sample_bytes_from_native_storage(img)?;
    let source_pixels = CheckedDims::new_allow_empty(width, height, 4)?.total_pixels();
    let samples = i32_samples_from_le_bytes(source_bytes, source_pixels);
    Ok((width, height, samples))
}

/// Native scalar boxed-resize kernels for the signed-I mode strategy.
pub(crate) fn resize_i_boxed_native(
    image: &DynamicImage,
    output_size: (u32, u32),
    bounds: (f64, f64, f64, f64),
    filter: ResampleFilter,
) -> Result<DynamicImage, PilError> {
    let (width, height) = output_size;
    let (source_width, source_height, source) = i32_samples_from_native_storage(image)?;
    let horizontal = precompute_coeffs_f64_boxed(width, source_width, bounds.0, bounds.2, filter);
    let vertical = precompute_coeffs_f64_boxed(height, source_height, bounds.1, bounds.3, filter);
    let intermediate = resize_i_boxed_horizontal(&source, (source_width, width), &horizontal);
    resize_i_boxed_vertical(&intermediate, (width, height), &vertical)
}

fn resize_i_boxed_horizontal(
    source: &[i32],
    dimensions: (u32, u32),
    coefficients: &FilterCoeffsF64,
) -> Vec<i32> {
    let (source_width, output_width) = dimensions;
    let source_height = source.len() / source_width as usize;
    let mut intermediate = vec![0i32; source_height * output_width as usize];
    for source_y in 0..source_height {
        let source_row = source_y * source_width as usize;
        let output_row = source_y * output_width as usize;
        for output_x in 0..output_width as usize {
            let first_source_x = coefficients.xmin[output_x];
            let mut sum = 0.0f64;
            for (tap, &weight) in coefficients.weights[output_x].iter().enumerate() {
                let source_x = (first_source_x + tap as i64) as usize;
                sum = weight.mul_add(f64::from(source[source_row + source_x]), sum);
            }
            intermediate[output_row + output_x] = round_up(sum) as i32;
        }
    }
    intermediate
}

fn resize_i_boxed_vertical(
    intermediate: &[i32],
    dimensions: (u32, u32),
    coefficients: &FilterCoeffsF64,
) -> Result<DynamicImage, PilError> {
    let (width, height) = dimensions;
    let output_dimensions = CheckedDims::new_allow_empty(width, height, 4)?;
    let mut output_bytes = Vec::with_capacity(output_dimensions.total_bytes());
    for output_y in 0..height as usize {
        let first_source_y = coefficients.xmin[output_y];
        for output_x in 0..width as usize {
            let mut sum = 0.0f64;
            for (tap, &weight) in coefficients.weights[output_y].iter().enumerate() {
                let source_y = (first_source_y + tap as i64) as usize;
                sum = weight.mul_add(
                    f64::from(intermediate[source_y * width as usize + output_x]),
                    sum,
                );
            }
            output_bytes.extend_from_slice(&(round_up(sum) as i32).to_le_bytes());
        }
    }
    let output = crate::raster::RgbaImage::from_raw(width, height, output_bytes)
        .ok_or_else(|| PilError::ValueError("resize_i boxed: failed to create output buffer".into()))?;
    Ok(DynamicImage::ImageRgba8(output))
}

