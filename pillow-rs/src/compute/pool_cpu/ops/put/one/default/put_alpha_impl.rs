//! Original constant alpha semantics for `One`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use super::super::super::helpers::put_alpha::{self as helpers, AlphaInput};

pub(crate) type Input = crate::raster::GrayImage;
pub(crate) type Kernel<'a> = AlphaInput<'a, crate::raster::Luma<u8>>;

#[inline]
pub(crate) fn prepare(image: &Input, alpha: u8) -> Kernel<'_> {
    AlphaInput { image, alpha }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    let luma = input.image.clone();
    Ok(DynamicImage::ImageLumaA8(helpers::luma_constant(&luma, input.alpha)))
}
