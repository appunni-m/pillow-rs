//! Original constant alpha semantics for `ImageLaPremultiplied8`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use crate::raster::ConvertBuffer;
use super::super::super::helpers::put_alpha::{self as helpers, AlphaInput};

pub(crate) type Input = crate::raster::GrayAlphaImage;
pub(crate) type Kernel<'a> = AlphaInput<'a, crate::raster::LumaA<u8>>;

#[inline]
pub(crate) fn prepare(image: &Input, alpha: u8) -> Kernel<'_> {
    AlphaInput { image, alpha }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    let rgba: crate::raster::RgbaImage = input.image.convert();
    let mut output = crate::raster::GrayAlphaImage::new(rgba.width(), rgba.height());
    for (destination, source) in output.pixels_mut().zip(rgba.pixels()) {
        destination[0] = source[0];
        destination[1] = input.alpha;
    }
    Ok(DynamicImage::ImageLumaA8(output))
}
