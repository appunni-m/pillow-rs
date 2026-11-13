//! Restore straight RGB using the original integer alpha division, then take luma.

use super::super::super::helpers::luma::{Checked, finish};
use crate::compute::pool_cpu::helpers::alpha::unpremultiply_channel;
use crate::error::PilError;
use crate::raster::{GrayImage, Rgba, RgbaImage};

pub(crate) type Input = RgbaImage;
pub(crate) type Kernel<'a> = Checked<'a, Rgba<u8>>;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn prepare(input: &Input) -> Result<Kernel<'_>, PilError> {
    Checked::new(input)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(input: Kernel<'_>) -> Result<GrayImage, PilError> {
    let mut output: Vec<u8> = input
        .input
        .as_raw()
        .chunks_exact(4)
        .take(input.dimensions.total_pixels())
        .map(|pixel| {
            let alpha = pixel[3];
            let straight = [
                unpremultiply_channel(pixel[0], alpha),
                unpremultiply_channel(pixel[1], alpha),
                unpremultiply_channel(pixel[2], alpha),
            ];
            crate::color::grayscale_rgb_pixel(&straight, 32768)
        })
        .collect();
    output.resize(input.dimensions.total_pixels(), 0);
    finish(input.input.width(), input.input.height(), output)
}
