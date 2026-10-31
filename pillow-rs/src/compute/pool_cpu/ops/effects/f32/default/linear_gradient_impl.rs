//! Default LinearGradient implementation for native F samples.

use crate::compute::pool_cpu::ops::effects::helpers::generator::GeneratedImage;
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};

const WIDTH: usize = 256;
const HEIGHT: usize = 256;
const ROW_BYTES: usize = WIDTH * 4;

pub(crate) fn execute() -> Result<GeneratedImage, PilError> {
    let mut pixels = vec![0u8; ROW_BYTES * HEIGHT];
    for (y, row) in pixels.chunks_exact_mut(ROW_BYTES).enumerate() {
        let value = (y as f32).to_le_bytes();
        for sample in row.chunks_exact_mut(value.len()) {
            sample.copy_from_slice(&value);
        }
    }
    from_pixels(pixels, 0, 0)
}

pub(crate) fn execute_vector() -> Result<GeneratedImage, PilError> {
    let mut pixels = vec![0u8; ROW_BYTES * HEIGHT];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for (y, row) in pixels.chunks_exact_mut(ROW_BYTES).enumerate() {
        let value = (y as f32).to_le_bytes();
        let (blocks, tail) = super::super::super::helpers::linear_gradient_vector::fill_pattern(
            row, &value,
        )?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    from_pixels(pixels, vector_blocks, scalar_tail)
}

fn from_pixels(
    pixels: Vec<u8>,
    vector_blocks: u64,
    scalar_tail: u64,
) -> Result<GeneratedImage, PilError> {
    let image = RgbaImage::from_raw(WIDTH as u32, HEIGHT as u32, pixels)
        .ok_or_else(|| PilError::InternalError("LinearGradient F buffer shape mismatch".into()))?;
    Ok(GeneratedImage {
        image: DynamicImage::ImageF32(image),
        vector_blocks,
        scalar_tail,
    })
}
