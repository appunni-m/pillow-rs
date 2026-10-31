//! Original image-backed alpha semantics for `ImageRgb8`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use super::super::super::helpers::{
    alpha_mask::MaskStrategy, put_alpha_data::{self as helpers, AlphaDataInput},
};

pub(crate) type Input = crate::raster::RgbImage;
pub(crate) type Kernel<'a> = AlphaDataInput<'a, crate::raster::Rgb<u8>>;

#[inline]
pub(crate) fn prepare<'a>(image: &'a Input, mask: MaskStrategy<'a>) -> Kernel<'a> {
    AlphaDataInput { image, mask }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    if let Some(mask) = helpers::native_mask(&input) {
        return Ok(DynamicImage::ImageRgba8(helpers::native_rgb_mask(
            input.image,
            mask.as_raw(),
        )));
    }
    let mask = input.mask.luma_samples();
    Ok(DynamicImage::ImageRgba8(helpers::native_rgb_mask(input.image, mask.as_slice())))
}
