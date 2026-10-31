//! Original image-backed alpha semantics for `ImageRgba32F`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use super::super::super::helpers::{
    alpha_mask::MaskStrategy, put_alpha_data::{self as helpers, AlphaDataInput},
};

pub(crate) type Input = crate::raster::Rgba32FImage;
pub(crate) type Kernel<'a> = AlphaDataInput<'a, crate::raster::Rgba<f32>>;

#[inline]
pub(crate) fn prepare<'a>(image: &'a Input, mask: MaskStrategy<'a>) -> Kernel<'a> {
    AlphaDataInput { image, mask }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    let mask = input.mask.samples();
    Ok(DynamicImage::ImageRgba32F(helpers::rgba32f_mask(
        input.image,
        mask,
    )))
}
