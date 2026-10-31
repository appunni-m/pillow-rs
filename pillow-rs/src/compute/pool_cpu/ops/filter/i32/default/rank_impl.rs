//! Native signed-I rank filters over little-endian 32-bit samples.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};

pub(super) fn execute(
    image: &DynamicImage,
    size: u32,
    rank: u32,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageI32(image) = image else {
        return Err(PilError::InternalError(
            "I32 rank strategy received a non-I32 raster".into(),
        ));
    };
    let area = size
        .checked_mul(size)
        .ok_or_else(|| PilError::ValueError("bad filter size".into()))?;
    if size == 0 || size % 2 == 0 {
        return Err(PilError::ValueError("bad filter size".into()));
    }
    if rank >= area {
        return Err(PilError::ValueError("bad rank value".into()));
    }

    let width = image.width() as usize;
    let height = image.height() as usize;
    let raw = image.as_raw();
    let expected = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| PilError::ValueError("filter dimensions overflow".into()))?;
    if raw.len() != expected {
        return Err(PilError::InternalError(
            "I32 rank strategy received an invalid native buffer".into(),
        ));
    }

    let mut output = CheckedDims::new(image.width(), image.height(), 4)?.alloc_buffer();
    if width == 0 || height == 0 {
        return native_output(image.width(), image.height(), output);
    }

    let side = size as usize;
    let area = area as usize;
    let half = side / 2;
    let mut neighborhood = Vec::with_capacity(area);
    for y in 0..height {
        for x in 0..width {
            neighborhood.clear();
            for dy in 0..side {
                let source_y = y.saturating_add(dy).saturating_sub(half).min(height - 1);
                for dx in 0..side {
                    let source_x = x.saturating_add(dx).saturating_sub(half).min(width - 1);
                    neighborhood.push(read_sample(raw, source_y * width + source_x));
                }
            }
            let selected = *neighborhood.select_nth_unstable(rank as usize).1;
            let destination = (y * width + x) * 4;
            output[destination..destination + 4].copy_from_slice(&selected.to_le_bytes());
        }
    }
    native_output(image.width(), image.height(), output)
}

#[inline]
fn read_sample(raw: &[u8], index: usize) -> i32 {
    let start = index * 4;
    i32::from_le_bytes([raw[start], raw[start + 1], raw[start + 2], raw[start + 3]])
}

fn native_output(width: u32, height: u32, bytes: Vec<u8>) -> Result<DynamicImage, PilError> {
    RgbaImage::from_raw(width, height, bytes)
        .map(DynamicImage::ImageI32)
        .ok_or_else(|| PilError::ValueError("I32 rank filter: buffer error".into()))
}
