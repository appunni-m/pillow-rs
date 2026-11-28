use crate::error::PilError;
use crate::raster::{Rgba, buffer::ImageBuffer};

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute(
    image: &ImageBuffer<Rgba<u16>, Vec<u16>>,
) -> Result<ImageBuffer<Rgba<u16>, Vec<u16>>, PilError> {
    crate::compute::pool_cpu::helpers::sample::avx512f_invert::invert_u16(image)
}
