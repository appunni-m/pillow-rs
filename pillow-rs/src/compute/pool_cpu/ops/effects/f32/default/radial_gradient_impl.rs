//! Default RadialGradient implementation for native F samples.

use crate::compute::pool_cpu::ops::effects::helpers::{generator::GeneratedImage, radial_gradient};
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};

const WIDTH: usize = 256;
const HEIGHT: usize = 256;
const ROW_BYTES: usize = WIDTH * 4;

pub(crate) fn execute() -> Result<GeneratedImage, PilError> {
    let mut pixels = vec![0u8; ROW_BYTES * HEIGHT];
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let sample = f32::from(radial_gradient::sample(x, y)).to_le_bytes();
            let offset = y * ROW_BYTES + x * sample.len();
            pixels[offset..offset + sample.len()].copy_from_slice(&sample);
        }
    }
    let image = RgbaImage::from_raw(WIDTH as u32, HEIGHT as u32, pixels)
        .ok_or_else(|| PilError::InternalError("RadialGradient F buffer shape mismatch".into()))?;
    Ok(GeneratedImage {
        image: DynamicImage::ImageF32(image),
        vector_blocks: 0,
        scalar_tail: 0,
    })
}
