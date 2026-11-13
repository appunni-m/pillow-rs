//! Apply Pillow's HSV-to-RGB and rounded luma formulas per pixel.

use super::super::super::helpers::luma::{Checked, finish};
use crate::error::PilError;
use crate::raster::{GrayImage, Rgb, RgbImage};

pub(crate) type Input = RgbImage;
pub(crate) type Kernel<'a> = Checked<'a, Rgb<u8>>;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn prepare(input: &Input) -> Result<Kernel<'_>, PilError> {
    Checked::new(input)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(input: Kernel<'_>) -> Result<GrayImage, PilError> {
    let mut output: Vec<u8> = input
        .input
        .as_raw()
        .chunks_exact(3)
        .take(input.dimensions.total_pixels())
        .map(|pixel| {
            let rgb = crate::color::hsv_pixel_to_rgb(pixel[0], pixel[1], pixel[2]);
            crate::color::grayscale_rgb_pixel(&rgb, 32768)
        })
        .collect();
    output.resize(input.dimensions.total_pixels(), 0);
    finish(input.input.width(), input.input.height(), output)
}
