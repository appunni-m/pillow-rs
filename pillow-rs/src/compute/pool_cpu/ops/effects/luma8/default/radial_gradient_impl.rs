//! Default RadialGradient implementation and precomputed L raster.

use crate::compute::pool_cpu::ops::effects::helpers::generator::GeneratedImage;
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage};

const WIDTH: u32 = 256;
const HEIGHT: u32 = 256;

const fn integer_sqrt(value: u32) -> u32 {
    let mut low = 0u32;
    let mut high = 256u32;
    while low < high {
        let middle = (low + high).div_ceil(2);
        if middle * middle <= value {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    low
}

const fn build_template() -> [u8; 256 * 256] {
    let mut output = [0u8; 256 * 256];
    let mut y = 0usize;
    while y < 256 {
        let dy = y as i32 - 128;
        let mut x = 0usize;
        while x < 256 {
            let dx = x as i32 - 128;
            let squared_radius = (dx * dx + dy * dy) as u32 * 2;
            let distance = integer_sqrt(squared_radius);
            output[y * 256 + x] = if distance >= 255 { 255 } else { distance as u8 };
            x += 1;
        }
        y += 1;
    }
    output
}

pub(crate) static TEMPLATE: [u8; 256 * 256] = build_template();

pub(crate) fn execute() -> Result<GeneratedImage, PilError> {
    let image = GrayImage::from_raw(WIDTH, HEIGHT, TEMPLATE.to_vec())
        .ok_or_else(|| PilError::InternalError("RadialGradient L buffer shape mismatch".into()))?;
    Ok(GeneratedImage {
        image: DynamicImage::ImageLuma8(image),
        vector_blocks: 0,
        scalar_tail: 0,
    })
}

pub(crate) fn execute_vector() -> Result<GeneratedImage, PilError> {
    super::super::helpers::radial_gradient_vector::execute(&TEMPLATE)
}
