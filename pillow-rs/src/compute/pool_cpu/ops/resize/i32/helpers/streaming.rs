//! Serial I32 resampling with a bounded vertical ring and exact pass rounding.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::ops::pil_resize::{F64MulAdd, FilterCoeffsF64};
use crate::raster::RgbaImage;

#[inline(always)]
fn mul_add<F: F64MulAdd>(fma: &F, weight: f64, sample: i32, accumulator: f64) -> f64 {
    fma.mul_add(weight, f64::from(sample), accumulator)
}

#[inline(always)]
pub(crate) fn sum_eight<F: F64MulAdd>(
    weights: &[f64; 8],
    first_sample: usize,
    sample: impl Fn(usize) -> i32,
    fma: &F,
) -> f64 {
    let mut accumulator = mul_add(fma, weights[0], sample(first_sample), 0.0);
    accumulator = mul_add(fma, weights[1], sample(first_sample + 1), accumulator);
    accumulator = mul_add(fma, weights[2], sample(first_sample + 2), accumulator);
    accumulator = mul_add(fma, weights[3], sample(first_sample + 3), accumulator);
    accumulator = mul_add(fma, weights[4], sample(first_sample + 4), accumulator);
    accumulator = mul_add(fma, weights[5], sample(first_sample + 5), accumulator);
    accumulator = mul_add(fma, weights[6], sample(first_sample + 6), accumulator);
    mul_add(fma, weights[7], sample(first_sample + 7), accumulator)
}

#[inline(always)]
fn sum_eight_contiguous<F: F64MulAdd>(
    weights: &[f64; 8],
    source: &[i32],
    first_sample: usize,
    fma: &F,
) -> f64 {
    let samples: &[i32; 8] = source[first_sample..first_sample + 8]
        .try_into()
        .expect("eight-tap resize span must contain eight samples");
    let mut accumulator = mul_add(fma, weights[0], samples[0], 0.0);
    accumulator = mul_add(fma, weights[1], samples[1], accumulator);
    accumulator = mul_add(fma, weights[2], samples[2], accumulator);
    accumulator = mul_add(fma, weights[3], samples[3], accumulator);
    accumulator = mul_add(fma, weights[4], samples[4], accumulator);
    accumulator = mul_add(fma, weights[5], samples[5], accumulator);
    accumulator = mul_add(fma, weights[6], samples[6], accumulator);
    mul_add(fma, weights[7], samples[7], accumulator)
}

#[inline(always)]
pub(crate) fn sum_general<F: F64MulAdd>(
    weights: &[f64],
    first_sample: usize,
    sample: impl Fn(usize) -> i32,
    fma: &F,
) -> f64 {
    let mut accumulator = 0.0_f64;
    for (offset, &weight) in weights.iter().enumerate() {
        accumulator = mul_add(fma, weight, sample(first_sample + offset), accumulator);
    }
    accumulator
}

#[inline(always)]
pub(crate) fn round_to_i32(value: f64) -> i32 {
    (value + 0.5_f64.copysign(value)) as i32
}

pub(crate) fn contiguous_eight_tap_weights(
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

pub(crate) fn horizontal_row<F: F64MulAdd>(
    source_row: &[i32],
    output_row: &mut [i32],
    coefficients: &FilterCoeffsF64,
    fixed_weights: Option<&(usize, Vec<[f64; 8]>)>,
    fma: &F,
) {
    if let Some((first_eight_tap, fixed_weights)) = fixed_weights {
        for dx in 0..*first_eight_tap {
            let first = coefficients.xmin[dx] as usize;
            let accumulator =
                sum_general(&coefficients.weights[dx], first, |sx| source_row[sx], fma);
            output_row[dx] = round_to_i32(accumulator);
        }

        let eight_tap_end = first_eight_tap + fixed_weights.len();
        for (dx, weights) in (*first_eight_tap..eight_tap_end).zip(fixed_weights) {
            let accumulator =
                sum_eight_contiguous(weights, source_row, coefficients.xmin[dx] as usize, fma);
            output_row[dx] = round_to_i32(accumulator);
        }

        for dx in eight_tap_end..output_row.len() {
            let first = coefficients.xmin[dx] as usize;
            let accumulator =
                sum_general(&coefficients.weights[dx], first, |sx| source_row[sx], fma);
            output_row[dx] = round_to_i32(accumulator);
        }
        return;
    }

    for (dx, output) in output_row.iter_mut().enumerate() {
        let first = coefficients.xmin[dx] as usize;
        let weights = &coefficients.weights[dx];
        let accumulator = if let Ok(weights) = <&[f64; 8]>::try_from(weights.as_slice()) {
            sum_eight_contiguous(weights, source_row, first, fma)
        } else {
            sum_general(weights, first, |sx| source_row[sx], fma)
        };
        *output = round_to_i32(accumulator);
    }
}

fn vertical_pass<F: F64MulAdd>(
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
        let first_source_y = vertical.xmin[dy] as usize;
        let weights = &vertical.weights[dy];
        let source_end = first_source_y + weights.len();
        while next_source_row < source_end {
            let source_start = next_source_row * source_width;
            let source_row = &source[source_start..source_start + source_width];
            let ring_start = (next_source_row & ring_mask) * destination_width;
            let intermediate_row = &mut ring[ring_start..ring_start + destination_width];
            horizontal_row(
                source_row,
                intermediate_row,
                horizontal,
                fixed_horizontal_weights,
                fma,
            );
            next_source_row += 1;
        }

        if let Ok(weights) = <&[f64; 8]>::try_from(weights.as_slice()) {
            for (dx, output) in output_row.chunks_exact_mut(4).enumerate() {
                let accumulator = sum_eight(
                    weights,
                    first_source_y,
                    |sy| ring[(sy & ring_mask) * destination_width + dx],
                    fma,
                );
                output.copy_from_slice(&round_to_i32(accumulator).to_le_bytes());
            }
        } else {
            for (dx, output) in output_row.chunks_exact_mut(4).enumerate() {
                let accumulator = sum_general(
                    weights,
                    first_source_y,
                    |sy| ring[(sy & ring_mask) * destination_width + dx],
                    fma,
                );
                output.copy_from_slice(&round_to_i32(accumulator).to_le_bytes());
            }
        }
    }
}

pub(crate) fn execute<F: F64MulAdd>(
    source: &[i32],
    source_width: u32,
    destination_width: u32,
    destination_height: u32,
    horizontal: &FilterCoeffsF64,
    vertical: &FilterCoeffsF64,
    fma: &F,
) -> Result<RgbaImage, PilError> {
    let max_vertical_taps = vertical.weights.iter().map(Vec::len).max().unwrap_or(0);
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
    let fixed_horizontal_weights = contiguous_eight_tap_weights(&horizontal.weights);
    vertical_pass(
        source,
        source_width as usize,
        destination_width as usize,
        vertical,
        horizontal,
        fixed_horizontal_weights.as_ref(),
        fma,
        &mut ring,
        ring_rows - 1,
        &mut output_bytes,
        output_stride,
    );
    RgbaImage::from_raw(destination_width, destination_height, output_bytes)
        .ok_or_else(|| PilError::ValueError("resize_i: failed to create output buffer".into()))
}
