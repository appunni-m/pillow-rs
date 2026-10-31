//! Default LinearGradient implementation for exact Pillow L mode.

use crate::compute::pool_cpu::ops::effects::helpers::generator::GeneratedImage;
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage};

const WIDTH: usize = 256;
const HEIGHT: usize = 256;

pub(crate) fn execute() -> Result<GeneratedImage, PilError> {
    let mut pixels = Vec::with_capacity(WIDTH * HEIGHT);
    for y in 0..HEIGHT {
        pixels.resize((y + 1) * WIDTH, y as u8);
    }
    from_pixels(pixels, 0, 0)
}

pub(crate) fn execute_vector() -> Result<GeneratedImage, PilError> {
    let mut pixels = Vec::with_capacity(WIDTH * HEIGHT);
    let mut vector_blocks = 0u64;
    for y in 0..HEIGHT {
        let value = [y as u8];
        let mut row = [0u8; WIDTH];
        let (blocks, _) = super::super::super::helpers::linear_gradient_vector::fill_pattern(
            &mut row, &value,
        )?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        pixels.extend_from_slice(&row);
    }
    from_pixels(pixels, vector_blocks, 0)
}

fn from_pixels(
    pixels: Vec<u8>,
    vector_blocks: u64,
    scalar_tail: u64,
) -> Result<GeneratedImage, PilError> {
    let image = GrayImage::from_raw(WIDTH as u32, HEIGHT as u32, pixels)
        .ok_or_else(|| PilError::InternalError("LinearGradient L buffer shape mismatch".into()))?;
    Ok(GeneratedImage {
        image: DynamicImage::ImageLuma8(image),
        vector_blocks,
        scalar_tail,
    })
}
