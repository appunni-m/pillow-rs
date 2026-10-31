//! Default RadialGradient implementation for expanded Pillow `1` mode.

use crate::compute::pool_cpu::ops::effects::helpers::{generator::GeneratedImage, radial_gradient};
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage};

const WIDTH: usize = 256;
const HEIGHT: usize = 256;

pub(crate) fn execute() -> Result<GeneratedImage, PilError> {
    let mut pixels = vec![0u8; WIDTH * HEIGHT];
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            if radial_gradient::sample(x, y) != 0 {
                pixels[y * WIDTH + x] = 255;
            }
        }
    }
    let image = GrayImage::from_raw(WIDTH as u32, HEIGHT as u32, pixels)
        .ok_or_else(|| PilError::InternalError("RadialGradient One buffer shape mismatch".into()))?;
    Ok(GeneratedImage {
        image: DynamicImage::One(image),
        vector_blocks: 0,
        scalar_tail: 0,
    })
}
