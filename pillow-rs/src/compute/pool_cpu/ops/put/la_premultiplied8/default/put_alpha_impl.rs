//! Original constant alpha semantics for `ImageLaPremultiplied8`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use super::super::super::helpers::put_alpha::{self as helpers, AlphaInput};

pub(crate) type Input = crate::raster::GrayAlphaImage;
pub(crate) type Kernel<'a> = AlphaInput<'a, crate::raster::LumaA<u8>>;

#[inline]
pub(crate) fn prepare(image: &Input, alpha: u8) -> Kernel<'_> {
    AlphaInput { image, alpha }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    // The old L→RGBA→LA route copied L unchanged and then replaced alpha.
    // Write that exact result in its native two-byte carrier.
    Ok(DynamicImage::ImageLumaA8(helpers::replace_luma_alpha(input.image, input.alpha)))
}
