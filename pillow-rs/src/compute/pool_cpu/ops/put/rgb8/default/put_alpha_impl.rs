//! Original constant alpha semantics for `ImageRgb8`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use super::super::super::helpers::put_alpha::{self as helpers, AlphaInput};

pub(crate) type Input = crate::raster::RgbImage;
pub(crate) type Kernel<'a> = AlphaInput<'a, crate::raster::Rgb<u8>>;

#[inline]
pub(crate) fn prepare(image: &Input, alpha: u8) -> Kernel<'_> {
    AlphaInput { image, alpha }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    Ok(DynamicImage::ImageRgba8(helpers::rgb_constant(input.image, input.alpha)))
}
