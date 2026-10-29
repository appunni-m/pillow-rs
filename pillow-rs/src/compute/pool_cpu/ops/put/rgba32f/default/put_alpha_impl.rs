//! Original constant alpha semantics for `ImageRgba32F`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use crate::raster::ConvertBuffer;
use super::super::super::helpers::put_alpha::{self as helpers, AlphaInput};

pub(crate) type Input = crate::raster::Rgba32FImage;
pub(crate) type Kernel<'a> = AlphaInput<'a, crate::raster::Rgba<f32>>;

#[inline]
pub(crate) fn prepare(image: &Input, alpha: u8) -> Kernel<'_> {
    AlphaInput { image, alpha }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    let rgba: crate::raster::RgbaImage = input.image.convert();
    Ok(DynamicImage::ImageRgba8(helpers::replace_rgba_alpha(rgba, input.alpha)))
}
