//! Serial Posterize implementation for Pillow's native L mode.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage};

pub(crate) fn execute(input: &GrayImage, bits: u8) -> Result<DynamicImage, PilError> {
    let mask = !((1u8 << (8 - bits)) - 1);
    let mut output = input.clone();
    for sample in output.as_mut() {
        *sample &= mask;
    }
    Ok(DynamicImage::ImageLuma8(output))
}
