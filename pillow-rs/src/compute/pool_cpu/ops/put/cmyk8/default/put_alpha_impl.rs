//! Original constant alpha semantics for `ImageCmyk8`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use super::super::super::helpers::put_alpha::{self as helpers, AlphaInput};

pub(crate) type Input = crate::raster::RgbaImage;
pub(crate) type Kernel<'a> = AlphaInput<'a, crate::raster::Rgba<u8>>;

#[inline]
pub(crate) fn prepare(image: &Input, alpha: u8) -> Kernel<'_> {
    AlphaInput { image, alpha }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    // Retain the original conversion result and RGB-copy allocation boundary.
    // The typed serial conversion never enters the optional Rayon adapter.
    let converted = DynamicImage::ImageRgb8(crate::color::cmyk_buffer_to_rgb(input.image));
    let rgb = converted.to_rgb8();
    let mut output = crate::raster::RgbaImage::new(rgb.width(), rgb.height());
    for (destination, source) in output.pixels_mut().zip(rgb.pixels()) {
        *destination = crate::raster::Rgba([source[0], source[1], source[2], input.alpha]);
    }
    Ok(DynamicImage::ImageRgba8(output))
}
