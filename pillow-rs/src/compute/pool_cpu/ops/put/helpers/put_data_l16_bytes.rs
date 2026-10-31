//! Pillow's row-shaped raw-byte input for the four exact I;16 identities.

use crate::checked_dims::CheckedDims;
use crate::compute::helpers::put_data::clip_u8;
use crate::error::PilError;

pub(crate) fn prepare_raw_bytes(
    data: &[u8],
    dimensions: CheckedDims,
    scale: f64,
    offset: f64,
) -> Result<Vec<u8>, PilError> {
    let mut raw = dimensions.alloc_buffer();
    let width = dimensions.width as usize;
    let row_stride = width
        .checked_mul(2)
        .ok_or_else(|| PilError::DimensionError("putdata row stride overflow".into()))?;
    for (index, byte) in data.iter().copied().enumerate() {
        let row = index / width;
        let column = index % width;
        raw[row * row_stride + column] = if scale == 1.0 && offset == 0.0 {
            byte
        } else {
            clip_u8(f64::from(byte) * scale + offset)
        };
    }
    Ok(raw)
}

pub(crate) fn write_little_endian(samples: &mut [u16], raw: &[u8]) {
    for (sample, bytes) in samples.iter_mut().zip(raw.chunks_exact(2)) {
        *sample = u16::from_le_bytes([bytes[0], bytes[1]]);
    }
}

pub(crate) fn write_native_endian(samples: &mut [u16], raw: &[u8]) {
    for (sample, bytes) in samples.iter_mut().zip(raw.chunks_exact(2)) {
        *sample = u16::from_ne_bytes([bytes[0], bytes[1]]);
    }
}

pub(crate) fn write_big_endian(samples: &mut [u16], raw: &[u8]) {
    for (sample, bytes) in samples.iter_mut().zip(raw.chunks_exact(2)) {
        *sample = u16::from_be_bytes([bytes[0], bytes[1]]);
    }
}
