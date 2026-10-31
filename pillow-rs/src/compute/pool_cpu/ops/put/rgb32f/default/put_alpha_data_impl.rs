//! Original image-backed alpha semantics for `ImageRgb32F`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use super::super::super::helpers::{
    alpha_mask::MaskStrategy, put_alpha_data::{self as helpers, AlphaDataInput},
};

pub(crate) type Input = crate::raster::Rgb32FImage;
pub(crate) type Kernel<'a> = AlphaDataInput<'a, crate::raster::Rgb<f32>>;

#[inline]
pub(crate) fn prepare<'a>(image: &'a Input, mask: MaskStrategy<'a>) -> Kernel<'a> {
    AlphaDataInput { image, mask }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    let mask = input.mask.to_luma8();
    Ok(DynamicImage::ImageRgba32F(helpers::rgb32f_mask(
        input.image,
        &mask,
    )))
}
