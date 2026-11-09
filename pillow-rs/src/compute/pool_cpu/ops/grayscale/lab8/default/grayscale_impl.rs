//! Serial grayscale strategy for Pillow's LAB mode.

use super::super::super::helpers::luma::{Checked, finish};
use crate::error::PilError;
use crate::raster::{GrayImage, Rgb, RgbImage};

pub(crate) type Input = RgbImage;
pub(crate) type Kernel<'a> = Checked<'a, Rgb<u8>>;

pub(crate) fn prepare(input: &Input) -> Result<Kernel<'_>, PilError> {
    Checked::new(input)
}

pub(crate) fn execute(input: Kernel<'_>) -> Result<GrayImage, PilError> {
    // Stream the shared LAB-to-sRGB transform directly into luma, so the
    // Rust-only LAB grayscale extension does not materialize an RGB raster.
    let bytes =
        crate::compute::pool_cpu::ops::color::helpers::lab_to_srgb::srgb_pixels(input.input)
            .map(|[red, green, blue]| crate::color::grayscale_rgb_pixel(&[red, green, blue], 32768))
            .collect();
    finish(input.input.width(), input.input.height(), bytes)
}
