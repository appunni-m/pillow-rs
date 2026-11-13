//! Ordered f64 accumulation with Pillow's f32 store after each separable pass.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::ops::pil_resize::{
    F64MulAdd, FilterCoeffsF64, f_resize_coefficients_allow_finite_fma,
    f_resize_samples_allow_finite_fma,
};
use crate::pipeline::ResampleFilter;
use crate::raster::RgbaImage;

type Bounds = super::super::super::helpers::typed_nearest::BoxBounds;

// Pillow 13.0's AArch64 F-mode output matches four separately rounded
// products followed by fused tail accumulation. Other architectures retain
// their existing block width pending oracle verification.
#[cfg(target_arch = "aarch64")]
const F_RESIZE_VECTOR_WIDTH: usize = 4;
#[cfg(not(target_arch = "aarch64"))]
const F_RESIZE_VECTOR_WIDTH: usize = 16;
// TODO(pillow-rs#3ptb): verify the F-mode product block width on x86_64 and wasm.

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute<F: F64MulAdd>(
    source: &[f32],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    filter: ResampleFilter,
    bounds: Bounds,
    vertical_first: bool,
    fma: &F,
) -> Result<RgbaImage, PilError> {
    if vertical_first {
        return execute_vertical_first(source, source_size, destination_size, filter, bounds, fma);
    }
    let (source_width, source_height) = source_size;
    let (width, height) = destination_size;
    let (horizontal, vertical) = super::super::super::helpers::coefficients::for_resize(
        source_size,
        destination_size,
        filter,
        bounds,
    );
    let (need_horizontal, need_vertical) =
        super::super::super::helpers::coefficients::resample_pass_requirements(
            source_size,
            destination_size,
            bounds,
        );
    let intermediate_dims = CheckedDims::new_allow_empty(width, source_height, 1)?;
    let mut intermediate = vec![0.0; intermediate_dims.total_pixels()];
    if need_horizontal {
        let finite_samples = f_resize_samples_allow_finite_fma(fma, source);
        if finite_samples && f_resize_coefficients_allow_finite_fma(&horizontal) {
            horizontal_pass::<true, _>(
                source,
                &mut intermediate,
                source_width as usize,
                width as usize,
                source_height as usize,
                &horizontal,
                fma,
            );
        } else {
            horizontal_pass::<false, _>(
                source,
                &mut intermediate,
                source_width as usize,
                width as usize,
                source_height as usize,
                &horizontal,
                fma,
            );
        }
    } else {
        intermediate.copy_from_slice(source);
    }
    if !need_vertical {
        return encode(width, height, &intermediate);
    }
    let vertical_samples_are_finite = if need_horizontal {
        f_resize_samples_allow_finite_fma(fma, &intermediate)
    } else {
        f_resize_samples_allow_finite_fma(fma, source)
    };
    if vertical_samples_are_finite && f_resize_coefficients_allow_finite_fma(&vertical) {
        vertical_output_packed::<true, _>(&intermediate, width, height, &vertical, fma)
    } else {
        vertical_output_packed::<false, _>(&intermediate, width, height, &vertical, fma)
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn execute_vertical_first<F: F64MulAdd>(
    source: &[f32],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    filter: ResampleFilter,
    bounds: Bounds,
    fma: &F,
) -> Result<RgbaImage, PilError> {
    let (source_width, _) = source_size;
    let (width, height) = destination_size;
    let (_, vertical) = super::super::super::helpers::coefficients::for_resize(
        source_size,
        (source_width, height),
        filter,
        bounds,
    );
    let vertical_dims = CheckedDims::new_allow_empty(source_width, height, 1)?;
    let mut vertical_output = vec![0.0; vertical_dims.total_pixels()];
    vertical_pass::<false, _>(
        source,
        &mut vertical_output,
        source_width as usize,
        height as usize,
        &vertical,
        fma,
    );

    let (horizontal, _) = super::super::super::helpers::coefficients::for_resize(
        (source_width, height),
        destination_size,
        filter,
        bounds,
    );
    let output_dims = CheckedDims::new_allow_empty(width, height, 1)?;
    let mut output = vec![0.0; output_dims.total_pixels()];
    horizontal_pass::<false, _>(
        &vertical_output,
        &mut output,
        source_width as usize,
        width as usize,
        height as usize,
        &horizontal,
        fma,
    );
    encode(width, height, &output)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn horizontal_pass<const FINITE_OPERANDS: bool, F: F64MulAdd>(
    source: &[f32],
    output: &mut [f32],
    source_width: usize,
    destination_width: usize,
    source_height: usize,
    coefficients: &FilterCoeffsF64,
    fma: &F,
) {
    let eight_tap_coefficients = contiguous_eight_tap_coefficients(coefficients);
    for (source_y, row) in output
        .chunks_mut(destination_width)
        .enumerate()
        .take(source_height)
    {
        let source_row_start = source_y * source_width;
        let source_row = &source[source_row_start..source_row_start + source_width];
        if let Some((first_eight_tap, fixed_weights)) = &eight_tap_coefficients {
            let end_eight_tap = *first_eight_tap + fixed_weights.len();
            for (destination_x, output) in row[..*first_eight_tap].iter_mut().enumerate() {
                *output = horizontal_sample::<FINITE_OPERANDS, _>(
                    source_row,
                    coefficients.xmin[destination_x],
                    &coefficients.weights[destination_x],
                    fma,
                );
            }
            for (offset, output) in row[*first_eight_tap..end_eight_tap].iter_mut().enumerate() {
                let destination_x = *first_eight_tap + offset;
                let (source_x, weights) = fixed_weights[offset];
                *output =
                    horizontal_eight_tap::<FINITE_OPERANDS, _>(source_row, source_x, &weights, fma)
                        .unwrap_or_else(|| {
                            horizontal_sample::<FINITE_OPERANDS, _>(
                                source_row,
                                coefficients.xmin[destination_x],
                                &coefficients.weights[destination_x],
                                fma,
                            )
                        });
            }
            for (offset, output) in row[end_eight_tap..].iter_mut().enumerate() {
                let destination_x = end_eight_tap + offset;
                *output = horizontal_sample::<FINITE_OPERANDS, _>(
                    source_row,
                    coefficients.xmin[destination_x],
                    &coefficients.weights[destination_x],
                    fma,
                );
            }
        } else {
            for (destination_x, output) in row.iter_mut().enumerate() {
                *output = horizontal_sample::<FINITE_OPERANDS, _>(
                    source_row,
                    coefficients.xmin[destination_x],
                    &coefficients.weights[destination_x],
                    fma,
                );
            }
        }
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn horizontal_sample<const FINITE_OPERANDS: bool, F: F64MulAdd>(
    source_row: &[f32],
    first_source_x: i64,
    weights: &[f64],
    fma: &F,
) -> f32 {
    if let Ok(weights) = <&[f64; 8]>::try_from(weights)
        && let Ok(source_x) = usize::try_from(first_source_x)
        && let Some(sample) =
            horizontal_eight_tap::<FINITE_OPERANDS, _>(source_row, source_x, weights, fma)
    {
        return sample;
    }
    let vector_product_count = weights.len() / F_RESIZE_VECTOR_WIDTH * F_RESIZE_VECTOR_WIDTH;
    let mut accumulator = 0.0_f64;
    for (offset, &weight) in weights.iter().enumerate() {
        let source_x = (first_source_x + offset as i64) as usize;
        let sample = source_row[source_x];
        accumulator = if offset < vector_product_count {
            accumulator + std::hint::black_box(weight * f64::from(sample))
        } else {
            mul_add::<FINITE_OPERANDS, _>(fma, weight, f64::from(sample), accumulator)
        };
    }
    accumulator as f32
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
#[inline]
fn horizontal_eight_tap<const FINITE_OPERANDS: bool, F: F64MulAdd>(
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
    let mut accumulator = mul_add::<FINITE_OPERANDS, _>(fma, *weight0, f64::from(*sample0), 0.0);
    accumulator = mul_add::<FINITE_OPERANDS, _>(fma, *weight1, f64::from(*sample1), accumulator);
    accumulator = mul_add::<FINITE_OPERANDS, _>(fma, *weight2, f64::from(*sample2), accumulator);
    accumulator = mul_add::<FINITE_OPERANDS, _>(fma, *weight3, f64::from(*sample3), accumulator);
    accumulator = mul_add::<FINITE_OPERANDS, _>(fma, *weight4, f64::from(*sample4), accumulator);
    accumulator = mul_add::<FINITE_OPERANDS, _>(fma, *weight5, f64::from(*sample5), accumulator);
    accumulator = mul_add::<FINITE_OPERANDS, _>(fma, *weight6, f64::from(*sample6), accumulator);
    accumulator = mul_add::<FINITE_OPERANDS, _>(fma, *weight7, f64::from(*sample7), accumulator);
    Some(accumulator as f32)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn contiguous_eight_tap_coefficients(
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

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn vertical_pass<const FINITE_OPERANDS: bool, F: F64MulAdd>(
    source: &[f32],
    output: &mut [f32],
    destination_width: usize,
    destination_height: usize,
    coefficients: &FilterCoeffsF64,
    fma: &F,
) {
    let mut accumulators = vec![0.0f64; destination_width];
    for (destination_y, row) in output
        .chunks_mut(destination_width)
        .enumerate()
        .take(destination_height)
    {
        accumulate_row::<FINITE_OPERANDS, _>(
            source,
            destination_width,
            coefficients.xmin[destination_y],
            &coefficients.weights[destination_y],
            &mut accumulators,
            fma,
        );
        for (output, &accumulator) in row.iter_mut().zip(&accumulators) {
            *output = accumulator as f32;
        }
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn vertical_output_packed<const FINITE_OPERANDS: bool, F: F64MulAdd>(
    source: &[f32],
    width: u32,
    height: u32,
    coefficients: &FilterCoeffsF64,
    fma: &F,
) -> Result<RgbaImage, PilError> {
    let dims = CheckedDims::new_allow_empty(width, height, 4)?;
    let mut output = vec![[0u8; 4]; dims.total_pixels()];
    let mut accumulators = vec![0.0f64; width as usize];
    for (destination_y, row) in output.chunks_mut(width as usize).enumerate() {
        accumulate_row::<FINITE_OPERANDS, _>(
            source,
            width as usize,
            coefficients.xmin[destination_y],
            &coefficients.weights[destination_y],
            &mut accumulators,
            fma,
        );
        for (output, &accumulator) in row.iter_mut().zip(&accumulators) {
            *output = (accumulator as f32).to_le_bytes();
        }
    }
    RgbaImage::from_raw(width, height, output.into_flattened()).ok_or_else(|| {
        PilError::ValueError("resize_f output shape must match its dimensions".into())
    })
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn accumulate_row<const FINITE_OPERANDS: bool, F: F64MulAdd>(
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
            *accumulator =
                mul_add::<FINITE_OPERANDS, _>(fma, weight, f64::from(sample), *accumulator);
        }
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
#[inline(always)]
fn mul_add<const FINITE_OPERANDS: bool, F: F64MulAdd>(
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

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn encode(width: u32, height: u32, samples: &[f32]) -> Result<RgbaImage, PilError> {
    let dims = CheckedDims::new_allow_empty(width, height, 4)?;
    if samples.len() != dims.total_pixels() {
        return Err(PilError::InternalError(
            "F32 Resize result sample count does not match its dimensions".into(),
        ));
    }
    let bytes = samples
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect();
    RgbaImage::from_raw(width, height, bytes).ok_or_else(|| {
        PilError::ValueError("resize_f output shape must match its dimensions".into())
    })
}
