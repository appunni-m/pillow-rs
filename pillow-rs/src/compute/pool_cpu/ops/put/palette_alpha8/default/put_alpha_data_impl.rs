//! Original image-backed alpha semantics for `ImagePaletteAlpha8`.

use super::super::super::helpers::{
    alpha_mask::MaskStrategy,
    put_alpha_data::{self as helpers, AlphaDataInput},
};
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
    Ok(DynamicImage::ImagePaletteAlpha8(helpers::luma_alpha_mask(
        input.image,
        input.mask,
    )))
}
