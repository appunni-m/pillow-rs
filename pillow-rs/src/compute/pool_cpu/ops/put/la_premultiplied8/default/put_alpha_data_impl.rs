//! Original image-backed alpha semantics for `ImageLaPremultiplied8`.

use super::super::super::helpers::{alpha_mask::MaskStrategy, put_alpha_data::AlphaDataInput};
use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) type Input = crate::raster::GrayAlphaImage;
pub(crate) type Kernel<'a> = AlphaDataInput<'a, crate::raster::LumaA<u8>>;

#[inline]
pub(crate) fn prepare<'a>(image: &'a Input, mask: MaskStrategy<'a>) -> Kernel<'a> {
    AlphaDataInput { image, mask }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    super::put_alpha_helper::replace_alpha_mask(input.image, input.mask)
        .map(DynamicImage::ImageLumaA8)
}
