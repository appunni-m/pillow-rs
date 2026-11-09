//! Original image-backed alpha semantics for `ImageLumaA16`.

use super::super::super::helpers::{
    alpha_mask::MaskStrategy,
    put_alpha_data::{self as helpers, AlphaDataInput},
};
use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) type Input = crate::raster::ImageBuffer<crate::raster::LumaA<u16>, Vec<u16>>;
pub(crate) type Kernel<'a> = AlphaDataInput<'a, crate::raster::LumaA<u16>>;

#[inline]
pub(crate) fn prepare<'a>(image: &'a Input, mask: MaskStrategy<'a>) -> Kernel<'a> {
    AlphaDataInput { image, mask }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    Ok(DynamicImage::ImageLumaA16(
        helpers::replace_luma_alpha16_mask(input.image, input.mask),
    ))
}
