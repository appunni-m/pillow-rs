//! SIMD proof scan and exact BLUR ring kernel for native RGB Filter5x5.

use wide::{u8x16, u16x8};

const CHANNELS: usize = 3;
const INPUT_LANES: usize = 16;
const OUTPUT_LANES: usize = 8;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ScanEvidence {
    pub(crate) is_uniform: bool,
    pub(crate) vector_blocks: u64,
    pub(crate) scalar_tail_pixels: u64,
}

/// Prove a large RGB image is constant, returning the vector work performed.
pub(crate) fn scan_uniform(raw: &[u8], width: usize, height: usize) -> Option<ScanEvidence> {
    let pixel_count = width.checked_mul(height)?;
    let expected_bytes = pixel_count.checked_mul(CHANNELS)?;
    if width == 0 || height == 0 || raw.len() != expected_bytes {
        return None;
    }
    let first_pixel: [u8; CHANNELS] = raw.get(..CHANNELS)?.try_into().ok()?;
    if !sampled_pixels_match(raw, width, height, first_pixel) {
        return None;
    }

    let vector_pixels = pixel_count / INPUT_LANES * INPUT_LANES;
    let vector_bytes = vector_pixels.checked_mul(CHANNELS)?;
    let patterns: [u8x16; CHANNELS] = std::array::from_fn(|phase| {
        u8x16::new(std::array::from_fn(|lane| {
            first_pixel[(phase + lane) % CHANNELS]
        }))
    });
    let mut vector_blocks = 0u64;
    for group in raw[..vector_bytes].chunks_exact(CHANNELS * INPUT_LANES) {
        for (phase, start) in (0..CHANNELS).map(|phase| (phase, phase * INPUT_LANES)) {
            let bytes: [u8; INPUT_LANES] = group[start..start + INPUT_LANES].try_into().ok()?;
            vector_blocks = vector_blocks.checked_add(1)?;
            if !u8x16::new(bytes).simd_eq(patterns[phase]).all() {
                return Some(ScanEvidence {
                    is_uniform: false,
                    vector_blocks,
                    scalar_tail_pixels: 0,
                });
            }
        }
    }

    let is_uniform = raw[vector_bytes..]
        .chunks_exact(CHANNELS)
        .all(|pixel| pixel == first_pixel);
    Some(ScanEvidence {
        is_uniform,
        vector_blocks,
        scalar_tail_pixels: u64::try_from(pixel_count - vector_pixels).ok()?,
    })
}

fn sampled_pixels_match(raw: &[u8], width: usize, height: usize, first: [u8; CHANNELS]) -> bool {
    let last_x = width - 1;
    let last_y = height - 1;
    for row in 0..=4 {
        let y = (last_y / 4) * row + (last_y % 4) * row / 4;
        for column in 0..=6 {
            let x = (last_x / 6) * column + (last_x % 6) * column / 6;
            let start = (y * width + x) * CHANNELS;
            if raw.get(start..start + CHANNELS) != Some(first.as_slice()) {
                return false;
            }
        }
    }
    true
}

fn load_u8x16(raw: &[u8], start: usize) -> Option<u8x16> {
    let end = start.checked_add(INPUT_LANES)?;
    Some(u8x16::new(raw.get(start..end)?.try_into().ok()?))
}

fn load_u16x8(values: &[u16], start: usize) -> Option<u16x8> {
    let end = start.checked_add(OUTPUT_LANES)?;
    Some(u16x8::new(values.get(start..end)?.try_into().ok()?))
}

fn sum_rows_into_columns(
    raw: &[u8],
    row_stride: usize,
    first_row: usize,
    row_count: usize,
    destination: &mut [u16],
) -> Option<()> {
    if destination.len() != row_stride || row_count == 0 {
        return None;
    }
    let vector_end = row_stride / INPUT_LANES * INPUT_LANES;
    for byte in (0..vector_end).step_by(INPUT_LANES) {
        let first_start = first_row.checked_mul(row_stride)?.checked_add(byte)?;
        let first = load_u8x16(raw, first_start)?;
        let mut low = u16x8::from_u8x16_low(first);
        let mut high = u16x8::from_u8x16_high(first);
        for row_delta in 1..row_count {
            let row = first_row.checked_add(row_delta)?;
            let start = row.checked_mul(row_stride)?.checked_add(byte)?;
            let samples = load_u8x16(raw, start)?;
            low += u16x8::from_u8x16_low(samples);
            high += u16x8::from_u8x16_high(samples);
        }
        let middle = byte.checked_add(OUTPUT_LANES)?;
        destination
            .get_mut(byte..middle)?
            .copy_from_slice(&low.to_array());
        let end = byte.checked_add(INPUT_LANES)?;
        destination
            .get_mut(middle..end)?
            .copy_from_slice(&high.to_array());
    }
    for byte in vector_end..row_stride {
        let mut sum = 0u16;
        for row_delta in 0..row_count {
            let row = first_row.checked_add(row_delta)?;
            let start = row.checked_mul(row_stride)?.checked_add(byte)?;
            sum += u16::from(*raw.get(start)?);
        }
        *destination.get_mut(byte)? = sum;
    }
    Some(())
}

fn slide_columns(
    raw: &[u8],
    row_stride: usize,
    columns: &mut [u16],
    leaving_row: usize,
    entering_row: usize,
) -> Option<()> {
    if columns.len() != row_stride {
        return None;
    }
    let vector_end = row_stride / INPUT_LANES * INPUT_LANES;
    for byte in (0..vector_end).step_by(INPUT_LANES) {
        let leaving_start = leaving_row.checked_mul(row_stride)?.checked_add(byte)?;
        let entering_start = entering_row.checked_mul(row_stride)?.checked_add(byte)?;
        let leaving = load_u8x16(raw, leaving_start)?;
        let entering = load_u8x16(raw, entering_start)?;
        let middle = byte.checked_add(OUTPUT_LANES)?;
        let low = load_u16x8(columns, byte)? - u16x8::from_u8x16_low(leaving)
            + u16x8::from_u8x16_low(entering);
        let high = load_u16x8(columns, middle)? - u16x8::from_u8x16_high(leaving)
            + u16x8::from_u8x16_high(entering);
        columns
            .get_mut(byte..middle)?
            .copy_from_slice(&low.to_array());
        let end = byte.checked_add(INPUT_LANES)?;
        columns
            .get_mut(middle..end)?
            .copy_from_slice(&high.to_array());
    }
    for byte in vector_end..row_stride {
        let leaving = leaving_row.checked_mul(row_stride)?.checked_add(byte)?;
        let entering = entering_row.checked_mul(row_stride)?.checked_add(byte)?;
        let column = columns.get_mut(byte)?;
        *column = column
            .checked_sub(u16::from(*raw.get(leaving)?))?
            .checked_add(u16::from(*raw.get(entering)?))?;
    }
    Some(())
}

/// Apply the exact RGB BLUR ring and preserve its copied two-pixel border.
pub(crate) fn apply_blur_ring(
    raw: &[u8],
    width: usize,
    height: usize,
) -> Option<(Vec<u8>, u64, u64)> {
    let row_stride = width.checked_mul(CHANNELS)?;
    let byte_count = row_stride.checked_mul(height)?;
    if raw.len() != byte_count || width < 5 || height < 5 {
        return None;
    }

    let mut outer_columns = Vec::<u16>::new();
    outer_columns.try_reserve_exact(row_stride).ok()?;
    outer_columns.resize(row_stride, 0);
    let mut inner_columns = Vec::<u16>::new();
    inner_columns.try_reserve_exact(row_stride).ok()?;
    inner_columns.resize(row_stride, 0);
    let mut output = raw.to_vec();
    let tracked_capacity = outer_columns
        .capacity()
        .checked_add(inner_columns.capacity())?
        .checked_mul(std::mem::size_of::<u16>())?
        .checked_add(output.capacity())?;
    crate::compute::record_pipeline_allocation(tracked_capacity);

    sum_rows_into_columns(raw, row_stride, 0, 5, &mut outer_columns)?;
    sum_rows_into_columns(raw, row_stride, 1, 3, &mut inner_columns)?;
    let output_start = 2usize.checked_mul(CHANNELS)?;
    let output_end = width.checked_sub(2)?.checked_mul(CHANNELS)?;
    let output_components = output_end.checked_sub(output_start)?;
    let output_vector_end =
        output_start.checked_add(output_components / OUTPUT_LANES * OUTPUT_LANES)?;
    let blocks_per_row = output_components / OUTPUT_LANES;
    let interior_height = height.checked_sub(4)?;
    let mut vector_blocks = 0usize;

    for y in 2..height - 2 {
        let output_row = y.checked_mul(row_stride)?;
        for byte in (output_start..output_vector_end).step_by(OUTPUT_LANES) {
            let outer = load_u16x8(&outer_columns, byte.checked_sub(6)?)?
                + load_u16x8(&outer_columns, byte.checked_sub(3)?)?
                + load_u16x8(&outer_columns, byte)?
                + load_u16x8(&outer_columns, byte.checked_add(3)?)?
                + load_u16x8(&outer_columns, byte.checked_add(6)?)?;
            let inner = load_u16x8(&inner_columns, byte.checked_sub(3)?)?
                + load_u16x8(&inner_columns, byte)?
                + load_u16x8(&inner_columns, byte.checked_add(3)?)?;
            let result = ((outer - inner + u16x8::splat(8)) >> 4u32)
                .to_array()
                .map(|value| value as u8);
            let start = output_row.checked_add(byte)?;
            let end = start.checked_add(OUTPUT_LANES)?;
            output.get_mut(start..end)?.copy_from_slice(&result);
            vector_blocks = vector_blocks.checked_add(1)?;
        }

        for byte in output_vector_end..output_end {
            let outer = outer_columns[byte - 6]
                + outer_columns[byte - 3]
                + outer_columns[byte]
                + outer_columns[byte + 3]
                + outer_columns[byte + 6];
            let inner = inner_columns[byte - 3] + inner_columns[byte] + inner_columns[byte + 3];
            output[output_row + byte] = ((outer - inner + 8) >> 4) as u8;
        }

        if y + 1 < height - 2 {
            slide_columns(raw, row_stride, &mut outer_columns, y - 2, y + 3)?;
            slide_columns(raw, row_stride, &mut inner_columns, y - 1, y + 2)?;
        }
    }

    let vector_blocks = u64::try_from(vector_blocks).ok()?;
    let scalar_tail = u64::try_from(output_components - blocks_per_row * OUTPUT_LANES)
        .ok()?
        .checked_mul(u64::try_from(interior_height).ok()?)?;
    Some((output, vector_blocks, scalar_tail))
}
