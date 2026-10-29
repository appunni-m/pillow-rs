//! Serial Equalize implementation for Pillow's native RGB mode.

use crate::error::PilError;
use crate::raster::{DynamicImage, RgbImage};

pub(crate) fn execute(input: &RgbImage, lut: &[u8]) -> Result<DynamicImage, PilError> {
    if super::super::super::helpers::histogram::lut_is_identity(lut, 3) {
        return Ok(DynamicImage::ImageRgb8(input.clone()));
    }
    let mut output = vec![0; input.as_raw().len()];
    for (source, destination) in input
        .as_raw()
        .chunks_exact(3)
        .zip(output.chunks_exact_mut(3))
    {
        destination[0] = lut[usize::from(source[0])];
        destination[1] = lut[256 + usize::from(source[1])];
        destination[2] = lut[512 + usize::from(source[2])];
    }
    crate::image_utils::raw_bytes_to_image(input.width(), input.height(), output, 3)
}
