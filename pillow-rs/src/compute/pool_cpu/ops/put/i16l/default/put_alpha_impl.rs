//! Original constant alpha semantics for `ImageI16L`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use super::super::super::helpers::put_alpha::{self as helpers, AlphaInput};

pub(crate) type Input = crate::raster::ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>;
pub(crate) type Kernel<'a> = AlphaInput<'a, crate::raster::Luma<u16>>;

#[inline]
pub(crate) fn prepare(image: &Input, alpha: u8) -> Kernel<'_> {
    AlphaInput { image, alpha }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    let alpha = helpers::alpha_u8_to_u16(input.alpha);
    Ok(DynamicImage::ImageLumaA16(helpers::luma16_with_alpha(
        input.image,
        alpha,
    )))
}
