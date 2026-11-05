//! Existing fixed-point and floating-point SIMD affine-nearest samplers.

use crate::compute::pool_cpu::ops::transform::helpers::{
    scalar::affine_nearest_fixed, vector_samples,
};
use crate::error::PilError;
use wide::{i64x8, u8x16};

const VECTOR_PIXELS: usize = 8;
const VECTOR_BYTES: usize = 16;

pub(crate) fn execute<const CHANNELS: usize>(
    source: &[u8],
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    affine: [f64; 6],
    fill: [u8; 4],
) -> Result<Vec<u8>, PilError> {
    if !(1..=4).contains(&CHANNELS) {
        return Err(PilError::InternalError(
            "Transform vector kernel has an invalid native channel count".into(),
        ));
    }
    if output_dimensions.0 == 0 || output_dimensions.1 == 0 {
        return Ok(Vec::new());
    }
    let output_len = byte_len::<CHANNELS>(output_dimensions.0, output_dimensions.1)?;
    let mut output = vec![0; output_len];
    execute_into::<CHANNELS>(
        source,
        source_dimensions,
        output_dimensions,
        affine,
        fill,
        &mut output,
    )?;
    Ok(output)
}

pub(crate) fn execute_into<const CHANNELS: usize>(
    source: &[u8],
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    affine: [f64; 6],
    fill: [u8; 4],
    output: &mut [u8],
) -> Result<(), PilError> {
    let (source_width, source_height) = source_dimensions;
    let (output_width, output_height) = output_dimensions;
    if !(1..=4).contains(&CHANNELS) {
        return Err(PilError::InternalError(
            "Transform vector kernel has an invalid native channel count".into(),
        ));
    }
    let output_len = byte_len::<CHANNELS>(output_width, output_height)?;
    if output.len() != output_len {
        return Err(PilError::InternalError(
            "Transform affine nearest destination buffer shape mismatch".into(),
        ));
    }
    if output_width == 0 || output_height == 0 {
        return Ok(());
    }
    let source_len = byte_len::<CHANNELS>(source_width, source_height)?;
    if source.len() != source_len {
        return Err(PilError::InternalError(
            "Transform vector source buffer shape mismatch".into(),
        ));
    }
    match affine_nearest_fixed(affine, output_width, output_height) {
        Some(steps) => sample_fixed::<CHANNELS>(
            source,
            source_dimensions,
            output_dimensions,
            steps,
            fill,
            output,
        ),
        None => sample_floating::<CHANNELS>(
            source,
            source_dimensions,
            output_dimensions,
            affine,
            fill,
            output,
        )?,
    }
    Ok(())
}

fn byte_len<const CHANNELS: usize>(width: u32, height: u32) -> Result<usize, PilError> {
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(CHANNELS))
        .ok_or_else(|| PilError::ValueError("transform image is too large".into()))
}

fn sample_fixed<const CHANNELS: usize>(
    source: &[u8],
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    steps: [i64; 6],
    fill: [u8; 4],
    output: &mut [u8],
) {
    let (output_width, _) = output_dimensions;
    let [step_x, step_y, origin_x, step_x_y, step_y_y, origin_y] = steps;
    let offsets_x = i64x8::new(std::array::from_fn(|lane| lane as i64 * step_x));
    let offsets_y = i64x8::new(std::array::from_fn(|lane| lane as i64 * step_x_y));
    let step_x_vector = i64x8::splat(step_x * VECTOR_PIXELS as i64);
    let step_y_vector = i64x8::splat(step_x_y * VECTOR_PIXELS as i64);
    for (row_index, row) in output
        .chunks_exact_mut(output_width as usize * CHANNELS)
        .enumerate()
    {
        let mut x = i64x8::splat(origin_x + row_index as i64 * step_y) + offsets_x;
        let mut y = i64x8::splat(origin_y + row_index as i64 * step_y_y) + offsets_y;
        for block in row.chunks_mut(VECTOR_PIXELS * CHANNELS) {
            let source_x = (x >> 16_i32).to_array();
            let source_y = (y >> 16_i32).to_array();
            for (lane, pixel) in block.chunks_exact_mut(CHANNELS).enumerate() {
                vector_samples::copy_nearest::<CHANNELS>(
                    source,
                    pixel,
                    source_dimensions,
                    (source_x[lane], source_y[lane]),
                    fill,
                );
            }
            x += step_x_vector;
            y += step_y_vector;
        }
    }
    record_vector_work(output_dimensions, VECTOR_PIXELS);
}

fn sample_floating<const CHANNELS: usize>(
    source: &[u8],
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    affine: [f64; 6],
    fill: [u8; 4],
    output: &mut [u8],
) -> Result<(), PilError> {
    let (output_width, output_height) = output_dimensions;
    let [a, b, c, d, e, f] = affine;
    let pixels_per_vector = VECTOR_BYTES / CHANNELS;
    let mut blocks = 0u64;

    for destination_y in 0..output_height as usize {
        let row_start = destination_y * output_width as usize * CHANNELS;
        let mut destination_x = 0usize;
        while destination_x < output_width as usize {
            let count = pixels_per_vector.min(output_width as usize - destination_x);
            let mut block = [0; VECTOR_BYTES];
            for pixel in 0..count {
                let x = (destination_x + pixel) as f64 + 0.5;
                let y = destination_y as f64 + 0.5;
                let source_x = a * x + b * y + c;
                let source_y = d * x + e * y + f;
                let input_x = if source_x < 0.0 { -1 } else { source_x as i64 };
                let input_y = if source_y < 0.0 { -1 } else { source_y as i64 };
                let pixel_start = pixel * CHANNELS;
                vector_samples::copy_nearest::<CHANNELS>(
                    source,
                    &mut block[pixel_start..pixel_start + CHANNELS],
                    source_dimensions,
                    (input_x, input_y),
                    fill,
                );
            }
            let output_start = row_start + destination_x * CHANNELS;
            let bytes = count * CHANNELS;
            output[output_start..output_start + bytes]
                .copy_from_slice(&u8x16::new(block).to_array()[..bytes]);
            blocks = blocks.saturating_add(1);
            destination_x += count;
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    Ok(())
}

fn record_vector_work(dimensions: (u32, u32), pixels_per_block: usize) {
    let blocks = (dimensions.0 as usize).div_ceil(pixels_per_block) * dimensions.1 as usize;
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(blocks as u64);
    crate::compute::record_pipeline_operation_scalar_tail(0);
}
