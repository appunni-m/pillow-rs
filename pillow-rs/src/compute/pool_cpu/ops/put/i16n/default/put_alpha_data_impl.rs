//! Original image-backed alpha semantics for `ImageI16N`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use super::super::super::helpers::{
    alpha_mask::MaskStrategy, put_alpha_data::{self as helpers, AlphaDataInput},
};

pub(crate) type Input = crate::raster::ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>;
pub(crate) type Kernel<'a> = AlphaDataInput<'a, crate::raster::Luma<u16>>;

#[inline]
pub(crate) fn prepare<'a>(image: &'a Input, mask: MaskStrategy<'a>) -> Kernel<'a> {
    AlphaDataInput { image, mask }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    let rgba: crate::raster::RgbaImage = super::super::super::helpers::put_data::clipped(input.image, |sample| {
        crate::raster::Rgba([sample, sample, sample, u8::MAX])
    });
    Ok(DynamicImage::ImageRgba8(helpers::rgba_mask(&rgba, input.mask)))
}
