//! SIMD Luma16 Resize kernel for Pillow's typed `I;16*` modes.

use super::super::{SIMD_RESIZE_LANES, simd_unsupported};
use super::resize_nearest_indices;
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::ops::pil_resize::{
    FilterCoeffsF64, filter_from_resample, luma16_resample_big_endian, luma16_resample_read,
    luma16_resample_write, precompute_coeffs_f64,
};
use crate::pipeline::ResampleFilter;
use crate::raster::{DynamicImage, ImageBuffer};
use wide::{f64x8, u16x8};

/// Produce an all-zero resize result for an empty source dimension.
///
/// Pillow accepts a zero-width/zero-height source for the typed resize paths
/// and returns a positive-sized zero image. The destination is written through
/// padded `u16x8` blocks, so this is still a SIMD data plane rather than a
/// scalar CPU fallback.
pub(super) fn simd_resize_luma16(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    filter: &ResampleFilter,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageLuma16(source) = img else {
        return Err(simd_unsupported("Resize"));
    };
    let source_width = usize::try_from(source.width()).map_err(|_| simd_unsupported("Resize"))?;
    let source_height = usize::try_from(source.height()).map_err(|_| simd_unsupported("Resize"))?;
    let output_width = usize::try_from(output_width).map_err(|_| simd_unsupported("Resize"))?;
    let output_height = usize::try_from(output_height).map_err(|_| simd_unsupported("Resize"))?;
    let source_len = source_width
        .checked_mul(source_height)
        .and_then(|pixels| pixels.checked_mul(2))
        .ok_or_else(|| simd_unsupported("Resize"))?;
    if img.as_bytes().len() != source_len {
        return Err(PilError::InternalError(
            "SIMD I;16 resize source buffer shape mismatch".into(),
        ));
    }
    let output_len = output_width
        .checked_mul(output_height)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let big_endian = luma16_resample_big_endian(mode);
    let (output, vector_blocks) = if source_width == 0 || source_height == 0 {
        luma16_zero_output(output_len)
    } else if (source_width, source_height) == (output_width, output_height) {
        luma16_copy_identity(source.as_raw())?
    } else if matches!(filter, ResampleFilter::Nearest) {
        let x_indices = resize_nearest_indices(source.width(), output_width as u32)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let y_indices = resize_nearest_indices(source.height(), output_height as u32)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        luma16_nearest_output(
            source.as_raw(),
            &x_indices,
            &y_indices,
            source_width,
            output_width,
        )?
    } else {
        let (kernel, support) = filter_from_resample(*filter);
        let horizontal =
            precompute_coeffs_f64(output_width as u32, source_width as u32, kernel, support);
        let vertical =
            precompute_coeffs_f64(output_height as u32, source_height as u32, kernel, support);
        let (intermediate, horizontal_blocks) = luma16_horizontal_pass(
            source.as_raw(),
            source_width,
            source_height,
            output_width,
            &horizontal,
            big_endian,
        )?;
        let (output, vertical_blocks) = luma16_vertical_pass(
            &intermediate,
            output_width,
            source_height,
            output_height,
            &vertical,
            big_endian,
        )?;
        (output, horizontal_blocks.saturating_add(vertical_blocks))
    };

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    let result = ImageBuffer::from_raw(output_width as u32, output_height as u32, output)
        .ok_or_else(|| PilError::InternalError("SIMD I;16 resize buffer shape mismatch".into()))?;
    Ok(preserve_mode(img, DynamicImage::ImageLuma16(result)))
}

fn luma16_zero_output(output_len: usize) -> (Vec<u16>, u64) {
    let zero = u16x8::splat(0).to_array();
    let mut output = vec![0u16; output_len];
    let mut vector_blocks = 0u64;
    for block in output.chunks_exact_mut(zero.len()) {
        block.copy_from_slice(&zero);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let remainder = output.len() % zero.len();
    if remainder != 0 {
        let start = output.len() - remainder;
        output[start..].copy_from_slice(&zero[..remainder]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    (output, vector_blocks)
}

fn luma16_copy_identity(source: &[u16]) -> Result<(Vec<u16>, u64), PilError> {
    let mut output = vec![0u16; source.len()];
    let mut vector_blocks = 0u64;
    for (chunk, values) in output
        .chunks_exact_mut(SIMD_RESIZE_LANES)
        .zip(source.chunks_exact(SIMD_RESIZE_LANES))
    {
        chunk.copy_from_slice(
            &u16x8::new(<[u16; 8]>::try_from(values).map_err(|_| simd_unsupported("Resize"))?)
                .to_array(),
        );
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let remainder = output.len() % SIMD_RESIZE_LANES;
    if remainder != 0 {
        let start = output.len() - remainder;
        let mut padded = [0u16; SIMD_RESIZE_LANES];
        padded[..remainder].copy_from_slice(&source[start..]);
        output[start..].copy_from_slice(&u16x8::new(padded).to_array()[..remainder]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    Ok((output, vector_blocks))
}

fn luma16_nearest_output(
    source: &[u16],
    x_indices: &[usize],
    y_indices: &[usize],
    source_width: usize,
    output_width: usize,
) -> Result<(Vec<u16>, u64), PilError> {
    let output_len = output_width
        .checked_mul(y_indices.len())
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let mut output = vec![0u16; output_len];
    let mut vector_blocks = 0u64;
    for (output_y, &source_y) in y_indices.iter().enumerate() {
        let source_row = source_y
            .checked_mul(source_width)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let output_row = output_y
            .checked_mul(output_width)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        for output_x in (0..output_width).step_by(SIMD_RESIZE_LANES) {
            let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
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

fn luma16_horizontal_pass(
    source: &[u16],
    source_width: usize,
    source_height: usize,
    output_width: usize,
    coeffs: &FilterCoeffsF64,
    big_endian: bool,
) -> Result<(Vec<u16>, u64), PilError> {
    let intermediate_len = source_height
        .checked_mul(output_width)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let mut intermediate = vec![0u16; intermediate_len];
    let mut vector_blocks = 0u64;
    for source_y in 0..source_height {
        let source_row = source_y
            .checked_mul(source_width)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let intermediate_row = source_y
            .checked_mul(output_width)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        for output_x in (0..output_width).step_by(SIMD_RESIZE_LANES) {
            let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
            let max_count = (0..count)
                .map(|lane| coeffs.weights[output_x + lane].len())
                .max()
                .unwrap_or(0);
            let mut sums = f64x8::splat(0.0);
            for tap in 0..max_count {
                let mut values = [0.0; SIMD_RESIZE_LANES];
                let mut weights = [0.0; SIMD_RESIZE_LANES];
                for lane in 0..count {
                    let output_index = output_x + lane;
                    let lane_weights = coeffs
                        .weights
                        .get(output_index)
                        .ok_or_else(|| simd_unsupported("Resize"))?;
                    let Some(&weight) = lane_weights.get(tap) else {
                        continue;
                    };
                    let source_x = usize::try_from(
                        *coeffs
                            .xmin
                            .get(output_index)
                            .ok_or_else(|| simd_unsupported("Resize"))?,
                    )
                    .map_err(|_| simd_unsupported("Resize"))?
                    .checked_add(tap)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                    let source_index = source_row
                        .checked_add(source_x)
                        .ok_or_else(|| simd_unsupported("Resize"))?;
                    values[lane] = f64::from(luma16_resample_read(
                        *source
                            .get(source_index)
                            .ok_or_else(|| simd_unsupported("Resize"))?,
                        big_endian,
                    ));
                    weights[lane] = weight;
                }
                sums = f64x8::new(weights).mul_add(f64x8::new(values), sums);
            }
            let rounded = sums
                .to_array()
                .map(|value| luma16_resample_write(value, big_endian));
            for lane in 0..count {
                let index = intermediate_row
                    .checked_add(output_x + lane)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                *intermediate
                    .get_mut(index)
                    .ok_or_else(|| simd_unsupported("Resize"))? = rounded[lane];
            }
            vector_blocks = vector_blocks.saturating_add(1);
        }
    }
    Ok((intermediate, vector_blocks))
}

fn luma16_vertical_pass(
    intermediate: &[u16],
    output_width: usize,
    source_height: usize,
    output_height: usize,
    coeffs: &FilterCoeffsF64,
    big_endian: bool,
) -> Result<(Vec<u16>, u64), PilError> {
    let output_len = output_width
        .checked_mul(output_height)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let mut output = vec![0u16; output_len];
    let mut vector_blocks = 0u64;
    for output_y in 0..output_height {
        let y0 = usize::try_from(
            *coeffs
                .xmin
                .get(output_y)
                .ok_or_else(|| simd_unsupported("Resize"))?,
        )
        .map_err(|_| simd_unsupported("Resize"))?;
        let weights = coeffs
            .weights
            .get(output_y)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let output_row = output_y
            .checked_mul(output_width)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        for output_x in (0..output_width).step_by(SIMD_RESIZE_LANES) {
            let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
            let mut sums = f64x8::splat(0.0);
            for (tap, &weight) in weights.iter().enumerate() {
                let source_y = y0
                    .checked_add(tap)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                if source_y >= source_height {
                    return Err(simd_unsupported("Resize"));
                }
                let source_row = source_y
                    .checked_mul(output_width)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                let values = std::array::from_fn(|lane| {
                    if lane < count {
                        f64::from(luma16_resample_read(
                            intermediate[source_row + output_x + lane],
                            big_endian,
                        ))
                    } else {
                        0.0
                    }
                });
                sums = f64x8::splat(weight).mul_add(f64x8::new(values), sums);
            }
            let rounded = sums
                .to_array()
                .map(|value| luma16_resample_write(value, big_endian));
            output[output_row + output_x..output_row + output_x + count]
                .copy_from_slice(&u16x8::new(rounded).to_array()[..count]);
            vector_blocks = vector_blocks.saturating_add(1);
        }
    }
    Ok((output, vector_blocks))
}
