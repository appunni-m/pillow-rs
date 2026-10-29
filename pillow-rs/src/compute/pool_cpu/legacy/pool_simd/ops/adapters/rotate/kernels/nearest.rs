use crate::ops::rotate::nearest_contiguous_rgb_run_len;
use wide::u8x16;

const COPY_VECTOR_BYTES: usize = 16;
const RGB_CHANNELS: usize = 3;

/// Copy exact nearest-neighbor RGB runs using wide byte loads/stores.
///
/// Coordinate steps and fill handling retain Pillow's signed 16.16 sampler.
/// A short or unproven run is handled one pixel at a time. The caller admits
/// only shallow positive-angle geometry so arbitrary affine gathers stay on
/// the established kernel.
#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_contiguous_rgb_runs(
    source: &[u8],
    output: &mut [u8],
    source_width: usize,
    source_height: usize,
    destination_width: usize,
    destination_height: usize,
    origin_x: i64,
    origin_y: i64,
    step_x_x: i64,
    step_y_x: i64,
    step_x_y: i64,
    step_y_y: i64,
    fill: [u8; RGB_CHANNELS],
) -> Option<(u64, u64)> {
    if source_width == 0 || source_height == 0 || destination_width == 0 || destination_height == 0
    {
        return None;
    }
    if step_x_x <= 0 || step_x_y < 0 {
        return None;
    }

    let source_stride = source_width.checked_mul(RGB_CHANNELS)?;
    let destination_stride = destination_width.checked_mul(RGB_CHANNELS)?;
    let expected_source_len = source_stride.checked_mul(source_height)?;
    let expected_output_len = destination_stride.checked_mul(destination_height)?;
    if source.len() != expected_source_len || output.len() != expected_output_len {
        return None;
    }
    let source_width_i64 = i64::try_from(source_width).ok()?;
    let source_height_i64 = i64::try_from(source_height).ok()?;

    let mut vector_blocks = 0u64;
    let mut scalar_tail_bytes = 0u64;
    for destination_y in 0..destination_height {
        let destination_y_i64 = i64::try_from(destination_y).ok()?;
        let row_x = origin_x.checked_add(destination_y_i64.checked_mul(step_y_x)?)?;
        let row_y = origin_y.checked_add(destination_y_i64.checked_mul(step_y_y)?)?;
        let destination_start = destination_y.checked_mul(destination_stride)?;
        let destination_end = destination_start.checked_add(destination_stride)?;
        let output_row = output.get_mut(destination_start..destination_end)?;
        let mut source_x = row_x;
        let mut source_y = row_y;
        let mut destination_x = 0usize;

        while destination_x < destination_width {
            let input_x = source_x >> 16;
            let input_y = source_y >> 16;
            if input_x >= 0
                && input_x < source_width_i64
                && input_y >= 0
                && input_y < source_height_i64
            {
                let input_x = usize::try_from(input_x).ok()?;
                let input_y = usize::try_from(input_y).ok()?;
                let max_run = (destination_width - destination_x).min(source_width - input_x);
                let run_length = nearest_contiguous_rgb_run_len(
                    source_x, source_y, step_x_x, step_x_y, max_run,
                )?;
                if run_length >= 4 {
                    let source_start = input_y
                        .checked_mul(source_stride)?
                        .checked_add(input_x.checked_mul(RGB_CHANNELS)?)?;
                    let destination_byte = destination_x.checked_mul(RGB_CHANNELS)?;
                    let copy_bytes = run_length.checked_mul(RGB_CHANNELS)?;
                    let source_end = source_start.checked_add(copy_bytes)?;
                    let destination_byte_end = destination_byte.checked_add(copy_bytes)?;
                    let (blocks, tail_bytes) = copy_vector_bytes(
                        source.get(source_start..source_end)?,
                        output_row.get_mut(destination_byte..destination_byte_end)?,
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

            let destination_byte = destination_x.checked_mul(RGB_CHANNELS)?;
            if input_x >= 0
                && input_x < source_width_i64
                && input_y >= 0
                && input_y < source_height_i64
            {
                let source_index = usize::try_from(input_y)
                    .ok()?
                    .checked_mul(source_stride)?
                    .checked_add(usize::try_from(input_x).ok()?.checked_mul(RGB_CHANNELS)?)?;
                let source_pixel =
                    source.get(source_index..source_index.checked_add(RGB_CHANNELS)?)?;
                output_row
                    .get_mut(destination_byte..destination_byte.checked_add(RGB_CHANNELS)?)?
                    .copy_from_slice(source_pixel);
            } else {
                output_row
                    .get_mut(destination_byte..destination_byte.checked_add(RGB_CHANNELS)?)?
                    .copy_from_slice(&fill);
            }
            scalar_tail_bytes = scalar_tail_bytes.saturating_add(RGB_CHANNELS as u64);
            source_x = source_x.checked_add(step_x_x)?;
            source_y = source_y.checked_add(step_x_y)?;
            destination_x += 1;
        }
    }

    Some((vector_blocks, scalar_tail_bytes))
}

#[inline]
fn copy_vector_bytes(source: &[u8], destination: &mut [u8]) -> Option<(u64, u64)> {
    if source.len() != destination.len() {
        return None;
    }
    let vector_length = source.len() / COPY_VECTOR_BYTES * COPY_VECTOR_BYTES;
    for offset in (0..vector_length).step_by(COPY_VECTOR_BYTES) {
        let source_block: [u8; COPY_VECTOR_BYTES] = source
            .get(offset..offset.checked_add(COPY_VECTOR_BYTES)?)?
            .try_into()
            .ok()?;
        let vector_block = u8x16::new(source_block);
        destination
            .get_mut(offset..offset.checked_add(COPY_VECTOR_BYTES)?)?
            .copy_from_slice(&vector_block.to_array());
    }
    destination
        .get_mut(vector_length..)?
        .copy_from_slice(source.get(vector_length..)?);
    let tail_bytes = u64::try_from(source.len() - vector_length).ok()?;
    Some((
        u64::try_from(vector_length / COPY_VECTOR_BYTES).ok()?,
        tail_bytes,
    ))
}
