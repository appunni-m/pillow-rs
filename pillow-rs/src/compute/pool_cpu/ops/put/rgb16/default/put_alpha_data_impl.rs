//! Original image-backed alpha semantics for `ImageRgb16`.

use super::super::super::helpers::{
    alpha_mask::MaskStrategy,
    put_alpha_data::{self as helpers, AlphaDataInput},
};
use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) type Input = crate::raster::ImageBuffer<crate::raster::Rgb<u16>, Vec<u16>>;
pub(crate) type Kernel<'a> = AlphaDataInput<'a, crate::raster::Rgb<u16>>;

#[inline]
pub(crate) fn prepare<'a>(image: &'a Input, mask: MaskStrategy<'a>) -> Kernel<'a> {
    AlphaDataInput { image, mask }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    Ok(DynamicImage::ImageRgba16(helpers::rgb16_mask(
        input.image,
        input.mask,
    )))
}
