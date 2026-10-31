//! Original constant alpha semantics for `ImageRgba32F`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use super::super::super::helpers::put_alpha::{self as helpers, AlphaInput};

pub(crate) type Input = crate::raster::Rgba32FImage;
pub(crate) type Kernel<'a> = AlphaInput<'a, crate::raster::Rgba<f32>>;

#[inline]
pub(crate) fn prepare(image: &Input, alpha: u8) -> Kernel<'_> {
    AlphaInput { image, alpha }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    let alpha = helpers::alpha_u8_to_f32(input.alpha);
    Ok(DynamicImage::ImageRgba32F(helpers::rgba32f_with_alpha(
        input.image,
        alpha,
    )))
}
