//! Original constant alpha semantics for `ImagePaletteAlpha8`.

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
    // Preserve indices independently of the wrapper's retained palette.
    let luma: crate::raster::GrayImage = input.image.convert();
    Ok(DynamicImage::ImagePaletteAlpha8(helpers::luma_constant(&luma, input.alpha)))
}
