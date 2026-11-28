use crate::error::PilError;
use crate::raster::{Rgba, buffer::ImageBuffer};

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute(
    image: &ImageBuffer<Rgba<f32>, Vec<f32>>,
) -> Result<ImageBuffer<Rgba<f32>, Vec<f32>>, PilError> {
    crate::compute::pool_cpu::helpers::sample::avx512f_invert::invert_f32(image)
}
