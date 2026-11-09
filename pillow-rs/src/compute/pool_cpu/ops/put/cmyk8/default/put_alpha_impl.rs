//! Original constant alpha semantics for `ImageCmyk8`.

use super::super::super::helpers::put_alpha::AlphaInput;
use crate::error::PilError;
use crate::raster::{DynamicImage, Rgba};

pub(crate) type Input = crate::raster::RgbaImage;
pub(crate) type Kernel<'a> = AlphaInput<'a, crate::raster::Rgba<u8>>;

#[inline]
pub(crate) fn prepare(image: &Input, alpha: u8) -> Kernel<'_> {
    AlphaInput { image, alpha }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    // Preserve the shared CMYK pixel arithmetic while writing the required
    // RGBA result directly, without allocating an intermediate RGB frame.
    let mut output = crate::raster::RgbaImage::new(input.image.width(), input.image.height());
    for (destination, source) in output.pixels_mut().zip(input.image.pixels()) {
        let [red, green, blue] = crate::color::cmyk_pixel_to_rgb(source.0);
        *destination = Rgba([red, green, blue, input.alpha]);
    }
    Ok(DynamicImage::ImageRgba8(output))
}
