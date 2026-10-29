//! Serial F-mode vertical resampling and packed-byte materialization.

use super::{F64MulAdd, FilterCoeffsF64};

use crate::checked_dims::CheckedDims;

use crate::error::PilError;

use crate::pipeline::ResampleFilter;
use crate::raster::DynamicImage;

#[inline]
pub(super) fn accumulate_row<const FINITE_OPERANDS: bool, F: F64MulAdd>(
    source: &[f32],
    source_row_stride: usize,
    source_y: i64,
    weights: &[f64],
    accumulators: &mut [f64],
    fma: &F,
) {
    accumulators.fill(0.0);
    for (offset, &weight) in weights.iter().enumerate() {
        let source_row_start = (source_y + offset as i64) as usize * source_row_stride;
        let source_row = &source[source_row_start..source_row_start + source_row_stride];
        for (accumulator, &sample) in accumulators.iter_mut().zip(source_row) {
            *accumulator = super::f_resize_mul_add::<FINITE_OPERANDS, _>(
                fma,
                weight,
                f64::from(sample),
                *accumulator,
            );
        }
    }
}

pub(super) fn vertical_pass_packed<const FINITE_OPERANDS: bool, F: F64MulAdd>(
    source: &[f32],
    output: &mut [[u8; 4]],
    destination_width: usize,
    coefficients: &FilterCoeffsF64,
    fma: &F,
) {
    let mut accumulators = vec![0.0f64; destination_width];
    for (destination_y, row) in output.chunks_mut(destination_width).enumerate() {
        let source_y = coefficients.xmin[destination_y];
        let weights = &coefficients.weights[destination_y];
        accumulate_row::<FINITE_OPERANDS, _>(
            source,
            destination_width,
            source_y,
            weights,
            &mut accumulators,
            fma,
        );
        for (output, &accumulator) in row.iter_mut().zip(&accumulators) {
            *output = (accumulator as f32).to_le_bytes();
        }
    }
}

pub(super) fn packed_vertical_output<F: F64MulAdd>(
    intermediate: &[f32],
    source_height: u32,
    destination_width: u32,
    destination_height: u32,
    needs_horizontal: bool,
    finite_source_samples: bool,
    filter: &ResampleFilter,
    fma: &F,
) -> Result<DynamicImage, PilError> {
    let (kernel, support) = super::resample_kernel(filter);
    let coefficients =
        super::precompute_coeffs_f64(destination_height, source_height, kernel, support);
    let finite_samples = if needs_horizontal {
        super::f_resize_samples_allow_finite_fma(fma, intermediate)
    } else {
        finite_source_samples
    };
    let pixel_count =
        CheckedDims::new_allow_empty(destination_width, destination_height, 4)?.total_pixels();
    let mut output = vec![[0u8; 4]; pixel_count];
    if finite_samples && super::f_resize_coefficients_allow_finite_fma(&coefficients) {
        vertical_pass_packed::<true, _>(
            intermediate,
            &mut output,
            destination_width as usize,
            &coefficients,
            fma,
        );
    } else {
        vertical_pass_packed::<false, _>(
            intermediate,
            &mut output,
            destination_width as usize,
            &coefficients,
            fma,
        );
    }
    let output = crate::raster::RgbaImage::from_raw(
        destination_width,
        destination_height,
        output.into_flattened(),
    )
    .expect("resize_f output shape must match its dimensions");
    Ok(DynamicImage::ImageRgba8(output))
}

/// Pillow's F-mode NEAREST resize advances affine coordinates cumulatively.
#[inline(always)]
pub(super) fn nearest(
    source: &[f32],
    source_size: (u32, u32),
    destination_size: (u32, u32),
) -> DynamicImage {
    let (source_width, source_height) = source_size;
    let (destination_width, destination_height) = destination_size;
    let scale_x = source_width as f64 / destination_width as f64;
    let scale_y = source_height as f64 / destination_height as f64;
    let mut xintab = Vec::with_capacity(destination_width as usize);
    let mut source_x = scale_x * 0.5;
    for _ in 0..destination_width {
        let sx = source_x as u32;
        xintab.push(sx.min(source_width - 1));
        source_x += scale_x;
    }

    // `ImagingScaleAffine` initializes y once then advances after each row.
    let mut yintab = Vec::with_capacity(destination_height as usize);
    let mut source_y = scale_y * 0.5;
    for _ in 0..destination_height {
        let sy = source_y as u32;
        yintab.push(sy.min(source_height - 1));
        source_y += scale_y;
    }

    let mut output = vec![0.0f32; (destination_width * destination_height) as usize];

    for (y, row) in output.chunks_mut(destination_width as usize).enumerate() {
        let sy = yintab[y];
        for (out, &sx) in row.iter_mut().zip(&xintab) {
            *out = source[(sy * source_width + sx) as usize];
        }
    }
    let rgba_bytes = output
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect();
    let output =
        crate::raster::RgbaImage::from_raw(destination_width, destination_height, rgba_bytes)
            .expect("resize_f nearest output shape must match its dimensions");
    DynamicImage::ImageRgba8(output)
}
