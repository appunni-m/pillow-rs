//! Default LinearGradient implementation for Pillow P index bytes.

use crate::compute::pool_cpu::ops::effects::helpers::generator::GeneratedImage;
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage};

const WIDTH: usize = 256;
const HEIGHT: usize = 256;

pub(crate) fn execute() -> Result<GeneratedImage, PilError> {
    let mut pixels = vec![0u8; WIDTH * HEIGHT];
    for (y, row) in pixels.chunks_exact_mut(WIDTH).enumerate() {
        row.fill(y as u8);
    }
    from_pixels(pixels, 0, 0)
}

pub(crate) fn execute_vector() -> Result<GeneratedImage, PilError> {
    let mut pixels = vec![0u8; WIDTH * HEIGHT];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for (y, row) in pixels.chunks_exact_mut(WIDTH).enumerate() {
        let value = [y as u8];
        let (blocks, tail) =
            super::super::super::helpers::linear_gradient_vector::fill_pattern(row, &value)?;
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
    let image = GrayImage::from_raw(WIDTH as u32, HEIGHT as u32, pixels)
        .ok_or_else(|| PilError::InternalError("LinearGradient P buffer shape mismatch".into()))?;
    Ok(GeneratedImage {
        image: DynamicImage::ImagePalette8(image),
        vector_blocks,
        scalar_tail,
    })
}
