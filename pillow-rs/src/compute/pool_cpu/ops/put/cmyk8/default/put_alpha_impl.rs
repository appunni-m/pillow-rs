//! Original constant alpha semantics for `ImageCmyk8`.

use crate::error::PilError;
use crate::raster::{DynamicImage, Rgba};
use super::super::super::helpers::put_alpha::{self as helpers, AlphaInput};

pub(crate) type Input = crate::raster::RgbaImage;
pub(crate) type Kernel<'a> = AlphaInput<'a, crate::raster::Rgba<u8>>;

#[inline]
pub(crate) fn prepare(image: &Input, alpha: u8) -> Kernel<'_> {
    AlphaInput { image, alpha }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    // Preserve the original CMYK-to-RGB arithmetic, then write the required
    // RGBA result directly without cloning the converted RGB frame.
    let rgb = crate::color::cmyk_buffer_to_rgb(input.image);
    let mut output = crate::raster::RgbaImage::new(rgb.width(), rgb.height());
    for (destination, source) in output.pixels_mut().zip(rgb.pixels()) {
        *destination = Rgba([source[0], source[1], source[2], input.alpha]);
    }
    Ok(DynamicImage::ImageRgba8(output))
}
