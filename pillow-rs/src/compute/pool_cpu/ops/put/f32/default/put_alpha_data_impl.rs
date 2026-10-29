//! Original image-backed alpha semantics for `ImageF32`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use super::super::super::helpers::{
    alpha_mask::MaskStrategy, put_alpha_data::{self as helpers, AlphaDataInput},
};

pub(crate) type Input = crate::raster::RgbaImage;
pub(crate) type Kernel<'a> = AlphaDataInput<'a, crate::raster::Rgba<u8>>;

#[inline]
pub(crate) fn prepare<'a>(image: &'a Input, mask: MaskStrategy<'a>) -> Kernel<'a> {
    AlphaDataInput { image, mask }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    let rgba: crate::raster::RgbaImage = input.image.clone();
    Ok(DynamicImage::ImageRgba8(helpers::rgba_mask(&rgba, input.mask)))
}
