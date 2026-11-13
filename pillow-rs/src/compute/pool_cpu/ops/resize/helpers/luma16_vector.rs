//! Portable wide-vector data plane for the exact Pillow `I;16*` Resize modes.

use crate::compute::helpers::resampling::luma16_samples::{read, write};
use crate::error::PilError;
use crate::ops::pil_resize::FilterCoeffsF64;
use crate::pipeline::ResampleFilter;
use crate::raster::{ImageBuffer, Luma};
use wide::{f64x8, u16x8};

const LANES: usize = 8;

/// Resizes the native u16 carrier using the byte interpretation owned by its mode.
// TODO(pillow-rs#3ptb): verify empty I;16 requests select the Default fallback.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn resize<const BIG_ENDIAN: bool>(
    input: &ImageBuffer<Luma<u16>, Vec<u16>>,
    output_width: u32,
    output_height: u32,
    filter: ResampleFilter,
) -> Result<ImageBuffer<Luma<u16>, Vec<u16>>, PilError> {
    let source_width = usize::try_from(input.width())
        .map_err(|_| unsupported("source width does not fit the vector index type"))?;
    let source_height = usize::try_from(input.height())
        .map_err(|_| unsupported("source height does not fit the vector index type"))?;
    let output_width_usize = usize::try_from(output_width)
        .map_err(|_| unsupported("output width does not fit the vector index type"))?;
    let output_height_usize = usize::try_from(output_height)
        .map_err(|_| unsupported("output height does not fit the vector index type"))?;
    let source_len = source_width
        .checked_mul(source_height)
        .ok_or_else(|| unsupported("source sample count overflowed"))?;
    if input.as_raw().len() != source_len {
        return Err(PilError::InternalError(
            "I;16 vector Resize source buffer shape mismatch".into(),
        ));
    }

    let output_len = output_width_usize
        .checked_mul(output_height_usize)
        .ok_or_else(|| unsupported("output sample count overflowed"))?;
    let (output, vector_blocks) = if source_width == 0
        || source_height == 0
        || output_width_usize == 0
        || output_height_usize == 0
    {
        zero_output(output_len)
    } else if (source_width, source_height) == (output_width_usize, output_height_usize) {
        copy_identity(input.as_raw())?
    } else if matches!(filter, ResampleFilter::Nearest) {
        nearest_output(
            input.as_raw(),
            input.width(),
            input.height(),
            output_width,
            output_height,
        )?
    } else {
        filtered_output::<BIG_ENDIAN>(
            input.as_raw(),
            (source_width, source_height),
            (output_width_usize, output_height_usize),
            filter,
        )?
    };

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    ImageBuffer::from_raw(output_width, output_height, output).ok_or_else(|| {
        PilError::InternalError("I;16 vector Resize output buffer shape mismatch".into())
    })
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn unsupported(reason: &str) -> PilError {
    PilError::InternalError(format!("I;16 vector Resize {reason}"))
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn zero_output(output_len: usize) -> (Vec<u16>, u64) {
    let zero = u16x8::splat(0).to_array();
    let mut output = vec![0u16; output_len];
    let mut vector_blocks = 0u64;
    for block in output.chunks_exact_mut(LANES) {
        block.copy_from_slice(&zero);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let remainder = output.len() % LANES;
    if remainder != 0 {
        let start = output.len() - remainder;
        output[start..].copy_from_slice(&zero[..remainder]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    (output, vector_blocks)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn copy_identity(source: &[u16]) -> Result<(Vec<u16>, u64), PilError> {
    let mut output = vec![0u16; source.len()];
    let mut vector_blocks = 0u64;
    for (destination, values) in output
        .chunks_exact_mut(LANES)
        .zip(source.chunks_exact(LANES))
    {
        let values = <[u16; LANES]>::try_from(values)
            .map_err(|_| unsupported("identity sample block has an invalid lane count"))?;
        destination.copy_from_slice(&u16x8::new(values).to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let remainder = output.len() % LANES;
    if remainder != 0 {
        let start = output.len() - remainder;
        let mut padded = [0u16; LANES];
        padded[..remainder].copy_from_slice(&source[start..]);
        output[start..].copy_from_slice(&u16x8::new(padded).to_array()[..remainder]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    Ok((output, vector_blocks))
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn nearest_output(
    source: &[u16],
    source_width: u32,
    source_height: u32,
    output_width: u32,
    output_height: u32,
) -> Result<(Vec<u16>, u64), PilError> {
    let x_indices = nearest_indices(source_width, output_width)?;
    let y_indices = nearest_indices(source_height, output_height)?;
    let output_width = usize::try_from(output_width)
        .map_err(|_| unsupported("nearest output width conversion failed"))?;
    let source_width = usize::try_from(source_width)
        .map_err(|_| unsupported("nearest source width conversion failed"))?;
    let output_len = output_width
        .checked_mul(y_indices.len())
        .ok_or_else(|| unsupported("nearest output sample count overflowed"))?;
    let mut output = vec![0u16; output_len];
    let mut vector_blocks = 0u64;
    for (output_y, &source_y) in y_indices.iter().enumerate() {
        let source_row = source_y
            .checked_mul(source_width)
            .ok_or_else(|| unsupported("nearest source row offset overflowed"))?;
        let output_row = output_y
            .checked_mul(output_width)
            .ok_or_else(|| unsupported("nearest output row offset overflowed"))?;
        for output_x in (0..output_width).step_by(LANES) {
            let count = (output_width - output_x).min(LANES);
            let values = std::array::from_fn(|lane| {
                if lane < count {
                    source[source_row + x_indices[output_x + lane]]
                } else {
                    0
                }
            });
            let packed = u16x8::new(values).to_array();
            output[output_row + output_x..output_row + output_x + count]
                .copy_from_slice(&packed[..count]);
            vector_blocks = vector_blocks.saturating_add(1);
        }
    }
    Ok((output, vector_blocks))
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn nearest_indices(source_size: u32, output_size: u32) -> Result<Vec<usize>, PilError> {
    if source_size == 0 || output_size == 0 {
        return Err(unsupported("nearest dimensions must be nonzero"));
    }
    let scale = f64::from(source_size) / f64::from(output_size);
    let mut position = scale * 0.5;
    let capacity = usize::try_from(output_size)
        .map_err(|_| unsupported("nearest output size conversion failed"))?;
    let mut indices = Vec::with_capacity(capacity);
    for _ in 0..output_size {
        let index = position as u32;
        indices.push(
            usize::try_from(if index >= source_size {
                source_size - 1
            } else {
                index
            })
            .map_err(|_| unsupported("nearest source index conversion failed"))?,
        );
        position += scale;
    }
    Ok(indices)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn filtered_output<const BIG_ENDIAN: bool>(
    source: &[u16],
    source_dimensions: (usize, usize),
    output_dimensions: (usize, usize),
    filter: ResampleFilter,
) -> Result<(Vec<u16>, u64), PilError> {
    let (horizontal, vertical) = super::coefficients::for_resize(
        (source_dimensions.0 as u32, source_dimensions.1 as u32),
        (output_dimensions.0 as u32, output_dimensions.1 as u32),
        filter,
        None,
    );
    let (intermediate, horizontal_blocks) =
        horizontal_pass::<BIG_ENDIAN>(source, source_dimensions, output_dimensions.0, &horizontal)?;
    let (output, vertical_blocks) = vertical_pass::<BIG_ENDIAN>(
        &intermediate,
        output_dimensions,
        source_dimensions.1,
        &vertical,
    )?;
    Ok((output, horizontal_blocks.saturating_add(vertical_blocks)))
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn horizontal_pass<const BIG_ENDIAN: bool>(
    source: &[u16],
    source_dimensions: (usize, usize),
    output_width: usize,
    coefficients: &FilterCoeffsF64,
) -> Result<(Vec<u16>, u64), PilError> {
    let intermediate_len = source_dimensions
        .1
        .checked_mul(output_width)
        .ok_or_else(|| unsupported("horizontal intermediate size overflowed"))?;
    let mut intermediate = vec![0u16; intermediate_len];
    let mut vector_blocks = 0u64;
    for source_y in 0..source_dimensions.1 {
        let source_row = source_y
            .checked_mul(source_dimensions.0)
            .ok_or_else(|| unsupported("horizontal source row offset overflowed"))?;
        let intermediate_row = source_y
            .checked_mul(output_width)
            .ok_or_else(|| unsupported("horizontal output row offset overflowed"))?;
        for output_x in (0..output_width).step_by(LANES) {
            let count = (output_width - output_x).min(LANES);
            let max_count = (0..count)
                .map(|lane| coefficients.weights[output_x + lane].len())
                .max()
                .unwrap_or(0);
            let mut sums = f64x8::splat(0.0);
            for tap in 0..max_count {
                let mut values = [0.0; LANES];
                let mut weights = [0.0; LANES];
                for lane in 0..count {
                    let output_index = output_x + lane;
                    let lane_weights = coefficients
                        .weights
                        .get(output_index)
                        .ok_or_else(|| unsupported("horizontal coefficient row is missing"))?;
                    let Some(&weight) = lane_weights.get(tap) else {
                        continue;
                    };
                    let source_x =
                        usize::try_from(*coefficients.xmin.get(output_index).ok_or_else(|| {
                            unsupported("horizontal coefficient origin is missing")
                        })?)
                        .map_err(|_| unsupported("horizontal source origin is negative"))?
                        .checked_add(tap)
                        .ok_or_else(|| unsupported("horizontal source index overflowed"))?;
                    let source_index = source_row
                        .checked_add(source_x)
                        .ok_or_else(|| unsupported("horizontal source offset overflowed"))?;
                    values[lane] =
                        f64::from(read::<BIG_ENDIAN>(*source.get(source_index).ok_or_else(
                            || unsupported("horizontal source sample is missing"),
                        )?));
                    weights[lane] = weight;
                }
                sums = f64x8::new(weights).mul_add(f64x8::new(values), sums);
            }
            let rounded = sums.to_array().map(write::<BIG_ENDIAN>);
            for lane in 0..count {
                let index = intermediate_row
                    .checked_add(output_x + lane)
                    .ok_or_else(|| unsupported("horizontal output offset overflowed"))?;
                *intermediate
                    .get_mut(index)
                    .ok_or_else(|| unsupported("horizontal output sample is missing"))? =
                    rounded[lane];
            }
            vector_blocks = vector_blocks.saturating_add(1);
        }
    }
    Ok((intermediate, vector_blocks))
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn vertical_pass<const BIG_ENDIAN: bool>(
    intermediate: &[u16],
    output_dimensions: (usize, usize),
    source_height: usize,
    coefficients: &FilterCoeffsF64,
) -> Result<(Vec<u16>, u64), PilError> {
    let output_len = output_dimensions
        .0
        .checked_mul(output_dimensions.1)
        .ok_or_else(|| unsupported("vertical output size overflowed"))?;
    let mut output = vec![0u16; output_len];
    let mut vector_blocks = 0u64;
    for output_y in 0..output_dimensions.1 {
        let y0 = usize::try_from(
            *coefficients
                .xmin
                .get(output_y)
                .ok_or_else(|| unsupported("vertical coefficient origin is missing"))?,
        )
        .map_err(|_| unsupported("vertical source origin is negative"))?;
        let weights = coefficients
            .weights
            .get(output_y)
            .ok_or_else(|| unsupported("vertical coefficient row is missing"))?;
        let output_row = output_y
            .checked_mul(output_dimensions.0)
            .ok_or_else(|| unsupported("vertical output row offset overflowed"))?;
        for output_x in (0..output_dimensions.0).step_by(LANES) {
            let count = (output_dimensions.0 - output_x).min(LANES);
            let mut sums = f64x8::splat(0.0);
            for (tap, &weight) in weights.iter().enumerate() {
                let source_y = y0
                    .checked_add(tap)
                    .ok_or_else(|| unsupported("vertical source index overflowed"))?;
                if source_y >= source_height {
                    return Err(unsupported("vertical source row is out of bounds"));
                }
                let source_row = source_y
                    .checked_mul(output_dimensions.0)
                    .ok_or_else(|| unsupported("vertical source row offset overflowed"))?;
                let values = std::array::from_fn(|lane| {
                    if lane < count {
                        f64::from(read::<BIG_ENDIAN>(
                            intermediate[source_row + output_x + lane],
                        ))
                    } else {
                        0.0
                    }
                });
                sums = f64x8::splat(weight).mul_add(f64x8::new(values), sums);
            }
            let rounded = sums.to_array().map(write::<BIG_ENDIAN>);
            output[output_row + output_x..output_row + output_x + count]
                .copy_from_slice(&u16x8::new(rounded).to_array()[..count]);
            vector_blocks = vector_blocks.saturating_add(1);
        }
    }
    Ok((output, vector_blocks))
}
