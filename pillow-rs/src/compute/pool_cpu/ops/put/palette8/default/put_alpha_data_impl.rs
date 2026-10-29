//! Original image-backed alpha semantics for `ImagePalette8`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use super::super::super::helpers::{
    alpha_mask::MaskStrategy, put_alpha_data::{self as helpers, AlphaDataInput},
};

pub(crate) type Input = crate::raster::GrayImage;
pub(crate) type Kernel<'a> = AlphaDataInput<'a, crate::raster::Luma<u8>>;

#[inline]
pub(crate) fn prepare<'a>(image: &'a Input, mask: MaskStrategy<'a>) -> Kernel<'a> {
    AlphaDataInput { image, mask }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    let luma = input.image.clone();
    Ok(DynamicImage::ImagePaletteAlpha8(helpers::luma_mask(&luma, input.mask)))
}
