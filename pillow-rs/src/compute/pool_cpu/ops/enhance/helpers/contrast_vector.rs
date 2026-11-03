//! Archived native-byte Contrast LUT kernel shared by exact-mode arch owners.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{ImageBuffer, Pixel};
use wide::u8x16;

type LutTables = [[u8x16; 16]; 4];

pub(crate) fn execute<P, const CHANNELS: usize>(
    source: &ImageBuffer<P, Vec<u8>>,
    lut: &[u8],
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    if !(1..=4).contains(&CHANNELS) {
        return Err(PilError::InternalError(
            "Contrast vector strategy received an invalid channel count".into(),
        ));
    }
    let channels = u8::try_from(CHANNELS)
        .map_err(|_| PilError::InternalError("Contrast channel count overflowed".into()))?;
    let dimensions =
        CheckedDims::new_allow_empty(source.width(), source.height(), channels)?;
    let source_bytes = source.as_raw();
    if source_bytes.len() != dimensions.total_bytes() || source_bytes.len() < 8 {
        return Err(PilError::InternalError(
            "Contrast vector strategy received an ineligible native buffer".into(),
        ));
    }
    let tables = native_lut_tables::<CHANNELS>(lut)?;
    let mut output = source_bytes.to_vec();
    let (vector_blocks, scalar_tail) = apply_native_lut::<CHANNELS>(
        &mut output,
        source.width() as usize,
        source.height() as usize,
        &tables,
    )
    .ok_or_else(|| PilError::InternalError("Contrast vector LUT dimensions were invalid".into()))?;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::compute::record_pipeline_operation_path("vector");
    ImageBuffer::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| PilError::InternalError("Contrast output dimensions were invalid".into()))
}

fn native_lut_tables<const CHANNELS: usize>(lut: &[u8]) -> Result<LutTables, PilError> {
    let expected = CHANNELS.checked_mul(256).ok_or_else(|| {
        PilError::InternalError("Contrast vector LUT dimensions overflowed".into())
    })?;
    if !(1..=4).contains(&CHANNELS) || lut.len() != expected {
        return Err(PilError::InternalError(
            "Contrast vector LUT did not match its native mode".into(),
        ));
    }
    let mut tables = [[u8x16::splat(0); 16]; 4];
    for channel in 0..CHANNELS {
        let start = channel * 256;
        for (index, table) in tables[channel].iter_mut().enumerate() {
            let offset = start + index * 16;
            let values = <[u8; 16]>::try_from(&lut[offset..offset + 16]).map_err(|_| {
                PilError::InternalError("Contrast vector LUT block was incomplete".into())
            })?;
            *table = u8x16::new(values);
        }
    }
    Ok(tables)
}

#[inline]
fn native_lut_chunk(input: u8x16, tables: &[u8x16; 16]) -> u8x16 {
    let low = input & u8x16::splat(0x0f);
    let high: u8x16 = input >> 4u32;
    let mut output = tables[0].swizzle_relaxed(low);
    for (index, table) in tables.iter().enumerate().skip(1) {
        let selected = high
            .simd_eq(u8x16::splat(index as u8))
            .select(table.swizzle_relaxed(low), output);
        output = selected;
    }
    output
}

fn map_native_lut_row<const CHANNELS: usize>(row: &mut [u8], tables: &LutTables) {
    if CHANNELS == 1 {
        for chunk in row.chunks_mut(16) {
            let mut input = [0u8; 16];
            input[..chunk.len()].copy_from_slice(chunk);
            let mapped = native_lut_chunk(u8x16::new(input), &tables[0]).to_array();
            chunk.copy_from_slice(&mapped[..chunk.len()]);
        }
    } else if CHANNELS == 3 {
        map_rgb_lut_row(row, tables);
    } else {
        map_interleaved_lut_row::<CHANNELS>(row, tables);
    }
}

fn map_rgb_lut_row(row: &mut [u8], tables: &LutTables) {
    for chunk in row.chunks_mut(48) {
        let mut input = [[0u8; 16]; 3];
        for (pixel_index, pixel) in chunk.chunks_exact(3).enumerate() {
            for channel in 0..3 {
                input[channel][pixel_index] = pixel[channel];
            }
        }
        let mapped: [[u8; 16]; 3] = std::array::from_fn(|channel| {
            native_lut_chunk(u8x16::new(input[channel]), &tables[channel]).to_array()
        });
        for (pixel_index, pixel) in chunk.chunks_exact_mut(3).enumerate() {
            for channel in 0..3 {
                pixel[channel] = mapped[channel][pixel_index];
            }
        }
    }
}

fn map_interleaved_lut_row<const CHANNELS: usize>(row: &mut [u8], tables: &LutTables) {
    for (block, chunk) in row.chunks_mut(16).enumerate() {
        let start = block * 16;
        let mut input = [[0u8; 16]; 4];
        let mut lanes = [0usize; 4];
        let mut locations = [(0usize, 0usize); 16];
        for (lane, &sample) in chunk.iter().enumerate() {
            let channel = (start + lane) % CHANNELS;
            let slot = lanes[channel];
            input[channel][slot] = sample;
            locations[lane] = (channel, slot);
            lanes[channel] += 1;
        }
        let mapped: [[u8; 16]; 4] = std::array::from_fn(|channel| {
            native_lut_chunk(u8x16::new(input[channel]), &tables[channel]).to_array()
        });
        for (lane, sample) in chunk.iter_mut().enumerate() {
            let (channel, slot) = locations[lane];
            *sample = mapped[channel][slot];
        }
    }
}

fn apply_native_lut<const CHANNELS: usize>(
    bytes: &mut [u8],
    width: usize,
    height: usize,
    tables: &LutTables,
) -> Option<(u64, u64)> {
    let row_stride = width.checked_mul(CHANNELS)?;
    let expected_len = row_stride.checked_mul(height)?;
    if row_stride == 0 || bytes.len() != expected_len {
        return None;
    }
    for row in bytes.chunks_exact_mut(row_stride) {
        map_native_lut_row::<CHANNELS>(row, tables);
    }
    let blocks = if CHANNELS == 1 || CHANNELS == 3 {
        width.div_ceil(16).saturating_mul(CHANNELS).saturating_mul(height)
    } else {
        row_stride.div_ceil(16).saturating_mul(height)
    };
    Some((u64::try_from(blocks).ok()?, 0))
}
