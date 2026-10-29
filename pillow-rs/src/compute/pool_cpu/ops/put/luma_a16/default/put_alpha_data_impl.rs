//! Original image-backed alpha semantics for `ImageLumaA16`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use crate::raster::ConvertBuffer;
use super::super::super::helpers::{
    alpha_mask::MaskStrategy, put_alpha_data::{self as helpers, AlphaDataInput},
};

pub(crate) type Input = crate::raster::ImageBuffer<crate::raster::LumaA<u16>, Vec<u16>>;
pub(crate) type Kernel<'a> = AlphaDataInput<'a, crate::raster::LumaA<u16>>;

#[inline]
pub(crate) fn prepare<'a>(image: &'a Input, mask: MaskStrategy<'a>) -> Kernel<'a> {
    AlphaDataInput { image, mask }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    let rgba: crate::raster::RgbaImage = input.image.convert();
    Ok(DynamicImage::ImageRgba8(helpers::rgba_mask(&rgba, input.mask)))
}
