//! Original constant alpha semantics for `ImageLaPremultiplied8`.

use super::super::super::helpers::put_alpha::AlphaInput;
use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) type Input = crate::raster::GrayAlphaImage;
pub(crate) type Kernel<'a> = AlphaInput<'a, crate::raster::LumaA<u8>>;

#[inline]
pub(crate) fn prepare(image: &Input, alpha: u8) -> Kernel<'_> {
    AlphaInput { image, alpha }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    // Pillow converts La to straight LA before replacing alpha. Fuse both
    // steps while retaining the physical bytes of Pillow's converted carrier.
    let pixel_count = input.image.as_raw().len() / 2;
    let alpha = std::iter::repeat(input.alpha).take(pixel_count);
    super::put_alpha_helper::replace_alpha(input.image, alpha).map(DynamicImage::ImageLumaA8)
}
