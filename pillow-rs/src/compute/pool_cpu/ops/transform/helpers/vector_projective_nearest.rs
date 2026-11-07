//! Existing eight-lane perspective and quadrilateral nearest sampling.

use crate::error::PilError;
use super::vector_samples;
use wide::f64x8;

const LANES: usize = 8;

pub(crate) fn execute<const CHANNELS: usize>(
    source: &[u8],
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    coefficients: &[f64],
    fill: [u8; 4],
    quad: bool,
) -> Result<Vec<u8>, PilError> {
    if !(1..=4).contains(&CHANNELS) {
        return Err(PilError::InternalError(
            "Transform vector kernel has an invalid native channel count".into(),
        ));
    }
    let source_len = checked_len::<CHANNELS>(source_dimensions.0, source_dimensions.1)?;
    let output_len = checked_len::<CHANNELS>(output_dimensions.0, output_dimensions.1)?;
    if source.len() != source_len || coefficients.len() != 8 {
        return Err(PilError::InternalError(
            "Transform vector projective input shape mismatch".into(),
        ));
    }
    if output_dimensions.0 == 0 || output_dimensions.1 == 0 {
        return Ok(vec![0; output_len]);
    }
    let mut output = vec![0; output_len];
    execute_into::<CHANNELS>(
        source,
        source_dimensions,
        output_dimensions,
        coefficients,
        fill,
        quad,
        &mut output,
    )?;
    Ok(output)
}

pub(crate) fn execute_into<const CHANNELS: usize>(
    source: &[u8],
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    coefficients: &[f64],
    fill: [u8; 4],
    quad: bool,
    output: &mut [u8],
) -> Result<(), PilError> {
    let (source_width, source_height) = source_dimensions;
    let (output_width, output_height) = output_dimensions;
    if !(1..=4).contains(&CHANNELS) {
        return Err(PilError::InternalError(
            "Transform vector kernel has an invalid native channel count".into(),
        ));
    }
    let source_len = checked_len::<CHANNELS>(source_width, source_height)?;
    let output_len = checked_len::<CHANNELS>(output_width, output_height)?;
    if output.len() != output_len {
        return Err(PilError::InternalError(
            "Transform vector projective destination buffer shape mismatch".into(),
        ));
    }
    if source.len() != source_len || coefficients.len() != 8 {
        return Err(PilError::InternalError(
            "Transform vector projective input shape mismatch".into(),
        ));
    }
    if output_width == 0 || output_height == 0 {
        return Ok(());
    }
    let coefficients: [f64; 8] = coefficients
        .try_into()
        .map_err(|_| PilError::InternalError("Transform coefficient count changed".into()))?;
    let mut blocks = 0u64;

    for y in 0..output_height as usize {
        for x in (0..output_width as usize).step_by(LANES) {
            let count = (output_width as usize - x).min(LANES);
            let (source_x, source_y) = source_points(
                coefficients,
                quad,
                x,
                y,
                count,
            );
            for lane in 0..count {
                let output_pixel = y * output_width as usize + x + lane;
                let output_start = output_pixel * CHANNELS;
                let input_x = nearest_coordinate(source_x[lane]);
                let input_y = nearest_coordinate(source_y[lane]);
                vector_samples::copy_nearest::<CHANNELS>(
                    source,
                    &mut output[output_start..output_start + CHANNELS],
                    source_dimensions,
                    (input_x, input_y),
                    fill,
                );
            }
            blocks = blocks.saturating_add(1);
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    Ok(())
}

fn checked_len<const CHANNELS: usize>(width: u32, height: u32) -> Result<usize, PilError> {
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(CHANNELS))
        .ok_or_else(|| PilError::ValueError("transform image is too large".into()))
}

fn nearest_coordinate(value: f64) -> i64 {
    if !value.is_finite() || value < 0.0 {
        -1
    } else {
        value as i64
    }
}

fn source_points(
    coefficients: [f64; 8],
    quad: bool,
    destination_x: usize,
    destination_y: usize,
    count: usize,
) -> ([f64; LANES], [f64; LANES]) {
    let [a, b, c, d, e, f, g, h] = coefficients;
    let x = f64x8::new(std::array::from_fn(|lane| {
        if lane < count {
            (destination_x + lane) as f64
        } else {
            0.0
        }
    }));
    let y = f64x8::splat(destination_y as f64);
    if quad {
        let x = x + f64x8::splat(0.5);
        let y = y + f64x8::splat(0.5);
        let source_x = f64x8::splat(a)
            + f64x8::splat(b) * x
            + f64x8::splat(c) * y
            + f64x8::splat(d) * x * y;
        let source_y = f64x8::splat(e)
            + f64x8::splat(f) * x
            + f64x8::splat(g) * y
            + f64x8::splat(h) * x * y;
        (source_x.to_array(), source_y.to_array())
    } else {
        let x = x + f64x8::splat(0.5);
        let y = y + f64x8::splat(0.5);
        let denominator = f64x8::splat(g) * x + f64x8::splat(h) * y + f64x8::splat(1.0);
        let source_x = (f64x8::splat(a) * x + f64x8::splat(b) * y + f64x8::splat(c))
            / denominator;
        let source_y = (f64x8::splat(d) * x + f64x8::splat(e) * y + f64x8::splat(f))
            / denominator;
        (source_x.to_array(), source_y.to_array())
    }
}
