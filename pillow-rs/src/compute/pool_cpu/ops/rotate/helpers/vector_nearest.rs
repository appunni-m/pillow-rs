//! Native-byte fixed-point nearest-neighbor Rotate kernel.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::rotate::helpers::geometry::{rotate_geometry, Geometry};
use crate::compute::pool_cpu::ops::rotate::ops::RotateRequest;
use crate::error::PilError;
use crate::raster::{GenericImageView, ImageBuffer, Pixel};
use wide::u8x16;

pub(crate) fn execute<P, const CHANNELS: usize>(
    image: &ImageBuffer<P, Vec<u8>>,
    request: RotateRequest,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    if !(1..=4).contains(&CHANNELS) {
        return Err(PilError::InternalError(
            "Rotate vector strategy requires one to four native byte channels".into(),
        ));
    }
    let geometry = rotate_geometry(image.width(), image.height(), request);
    let dimensions = CheckedDims::new(geometry.width, geometry.height, CHANNELS as u8)?;
    let mut output = dimensions.alloc_buffer();
    let counts = copy_nearest_rows::<CHANNELS>(
        image.as_raw(),
        image.dimensions(),
        geometry,
        request.fill.unwrap_or((0, 0, 0, 0)),
        &mut output,
    );
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(counts.0);
    crate::compute::record_pipeline_operation_scalar_tail(counts.1);
    crate::image_utils::typed_bytes_to_buffer::<P>(dimensions, output)
}

fn copy_nearest_rows<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    geometry: Geometry,
    fill: (u8, u8, u8, u8),
    output: &mut [u8],
) -> (u64, u64) {
    let [a, b, c, d, e, f] = geometry.affine;
    let fixed = |value: f64| (value.mul_add(65_536.0, 0.5).floor()) as i64;
    let (step_x_x, step_y_x, origin_x) = (fixed(a), fixed(b), fixed(c + a * 0.5 + b * 0.5));
    let (step_x_y, step_y_y, origin_y) = (fixed(d), fixed(e), fixed(f + d * 0.5 + e * 0.5));
    let pixels_per_vector = (16 / CHANNELS).max(1);
    let block_bytes = pixels_per_vector * CHANNELS;
    let output_width = geometry.width as usize;
    if CHANNELS == 3
        && step_x_x >= (65_536_i64 * 9 / 10)
        && step_x_x <= (65_536_i64 * 11 / 10)
        && (0_i64..=65_536_i64 / 8).contains(&step_x_y)
        && let Some(counts) = copy_contiguous_three_byte_runs(
            source,
            output,
            source_size,
            geometry,
            fill,
            (origin_x, origin_y, step_x_x, step_y_x, step_x_y, step_y_y),
        )
    {
        return counts;
    }
    let mut vector_blocks = 0_u64;
    let mut scalar_tail = 0_u64;
    for (destination_y, row) in output.chunks_exact_mut(output_width * CHANNELS).enumerate() {
        let (mut source_x, mut source_y) = (
            origin_x + destination_y as i64 * step_y_x,
            origin_y + destination_y as i64 * step_y_y,
        );
        let mut destination_x = 0;
        while destination_x + pixels_per_vector <= output_width {
            let mut block = [0_u8; 16];
            for pixel in 0..pixels_per_vector {
                let offset = pixel * CHANNELS;
                copy_nearest_pixel::<CHANNELS>(
                    source,
                    source_size,
                    (source_x, source_y),
                    fill,
                    &mut block[offset..offset + CHANNELS],
                );
                source_x += step_x_x;
                source_y += step_x_y;
            }
            let vector = u8x16::new(block).to_array();
            let start = destination_x * CHANNELS;
            row[start..start + block_bytes].copy_from_slice(&vector[..block_bytes]);
            destination_x += pixels_per_vector;
            vector_blocks = vector_blocks.saturating_add(1);
        }
        while destination_x < output_width {
            let start = destination_x * CHANNELS;
            copy_nearest_pixel::<CHANNELS>(
                source,
                source_size,
                (source_x, source_y),
                fill,
                &mut row[start..start + CHANNELS],
            );
            source_x += step_x_x;
            source_y += step_x_y;
            destination_x += 1;
            scalar_tail = scalar_tail.saturating_add(1);
        }
    }
    (vector_blocks, scalar_tail)
}

fn copy_contiguous_three_byte_runs(
    source: &[u8],
    output: &mut [u8],
    source_size: (u32, u32),
    geometry: Geometry,
    fill: (u8, u8, u8, u8),
    fixed: (i64, i64, i64, i64, i64, i64),
) -> Option<(u64, u64)> {
    let source_width = usize::try_from(source_size.0).ok()?;
    let source_height = usize::try_from(source_size.1).ok()?;
    let destination_width = usize::try_from(geometry.width).ok()?;
    let destination_height = usize::try_from(geometry.height).ok()?;
    let source_stride = source_width.checked_mul(3)?;
    let destination_stride = destination_width.checked_mul(3)?;
    if source.len() != source_stride.checked_mul(source_height)?
        || output.len() != destination_stride.checked_mul(destination_height)?
        || destination_width == 0
        || destination_height == 0
    {
        return None;
    }

    let (origin_x, origin_y, step_x_x, step_y_x, step_x_y, step_y_y) = fixed;
    let mut vector_blocks = 0_u64;
    let mut scalar_tail_bytes = 0_u64;
    for destination_y in 0..destination_height {
        let destination_y_i64 = i64::try_from(destination_y).ok()?;
        let mut source_x = origin_x.checked_add(destination_y_i64.checked_mul(step_y_x)?)?;
        let mut source_y = origin_y.checked_add(destination_y_i64.checked_mul(step_y_y)?)?;
        let destination_start = destination_y.checked_mul(destination_stride)?;
        let destination_end = destination_start.checked_add(destination_stride)?;
        let row = output.get_mut(destination_start..destination_end)?;
        let mut destination_x = 0_usize;

        while destination_x < destination_width {
            let input_x = source_x >> 16;
            let input_y = source_y >> 16;
            if input_x >= 0
                && input_x < i64::try_from(source_width).ok()?
                && input_y >= 0
                && input_y < i64::try_from(source_height).ok()?
            {
                let input_x = usize::try_from(input_x).ok()?;
                let input_y = usize::try_from(input_y).ok()?;
                let max_run = (destination_width - destination_x).min(source_width - input_x);
                let run_length = super::nearest::contiguous_three_byte_run_len(
                    source_x, source_y, step_x_x, step_x_y, max_run,
                )?;
                if run_length >= 4 {
                    let source_start = input_y
                        .checked_mul(source_stride)?
                        .checked_add(input_x.checked_mul(3)?)?;
                    let destination_byte = destination_x.checked_mul(3)?;
                    let copy_bytes = run_length.checked_mul(3)?;
                    let (blocks, tail_bytes) = copy_vector_bytes(
                        source.get(source_start..source_start.checked_add(copy_bytes)?)?,
                        row.get_mut(destination_byte..destination_byte.checked_add(copy_bytes)?)?,
                    )?;
                    vector_blocks = vector_blocks.saturating_add(blocks);
                    scalar_tail_bytes = scalar_tail_bytes.saturating_add(tail_bytes);
                    let run_length_i64 = i64::try_from(run_length).ok()?;
                    source_x = source_x.checked_add(step_x_x.checked_mul(run_length_i64)?)?;
                    source_y = source_y.checked_add(step_x_y.checked_mul(run_length_i64)?)?;
                    destination_x += run_length;
                    continue;
                }
            }

            let destination_byte = destination_x.checked_mul(3)?;
            let pixel = row.get_mut(destination_byte..destination_byte.checked_add(3)?)?;
            if input_x >= 0
                && input_x < i64::try_from(source_width).ok()?
                && input_y >= 0
                && input_y < i64::try_from(source_height).ok()?
            {
                let source_index = usize::try_from(input_y)
                    .ok()?
                    .checked_mul(source_stride)?
                    .checked_add(usize::try_from(input_x).ok()?.checked_mul(3)?)?;
                pixel.copy_from_slice(source.get(source_index..source_index.checked_add(3)?)?);
            } else {
                for (channel, value) in pixel.iter_mut().enumerate() {
                    *value = super::byte::fill_sample::<3>(fill, channel);
                }
            }
            source_x = source_x.checked_add(step_x_x)?;
            source_y = source_y.checked_add(step_x_y)?;
            destination_x += 1;
            scalar_tail_bytes = scalar_tail_bytes.saturating_add(3);
        }
    }
    Some((vector_blocks, scalar_tail_bytes))
}

fn copy_vector_bytes(source: &[u8], destination: &mut [u8]) -> Option<(u64, u64)> {
    if source.len() != destination.len() {
        return None;
    }
    let vector_length = source.len() / 16 * 16;
    for offset in (0..vector_length).step_by(16) {
        let end = offset.checked_add(16)?;
        let source_block: [u8; 16] = source.get(offset..end)?.try_into().ok()?;
        destination.get_mut(offset..end)?.copy_from_slice(&u8x16::new(source_block).to_array());
    }
    destination
        .get_mut(vector_length..)?
        .copy_from_slice(source.get(vector_length..)?);
    Some((
        u64::try_from(vector_length / 16).ok()?,
        u64::try_from(source.len() - vector_length).ok()?,
    ))
}

fn copy_nearest_pixel<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    fixed: (i64, i64),
    fill: (u8, u8, u8, u8),
    destination: &mut [u8],
) {
    let (source_x, source_y) = (fixed.0 >> 16, fixed.1 >> 16);
    if source_x >= 0
        && source_x < i64::from(source_size.0)
        && source_y >= 0
        && source_y < i64::from(source_size.1)
    {
        let start = (source_y as usize * source_size.0 as usize + source_x as usize) * CHANNELS;
        destination.copy_from_slice(&source[start..start + CHANNELS]);
    } else {
        for (channel, sample) in destination.iter_mut().enumerate() {
            *sample = super::byte::fill_sample::<CHANNELS>(fill, channel);
        }
    }
}
