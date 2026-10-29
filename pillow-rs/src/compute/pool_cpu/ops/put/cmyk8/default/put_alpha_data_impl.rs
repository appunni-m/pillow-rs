//! Original image-backed alpha semantics for `ImageCmyk8`.

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
    // Preserve the original RGB conversion, copy, and output allocation order.
    let converted = DynamicImage::ImageRgb8(crate::color::cmyk_buffer_to_rgb(input.image));
    let rgb = converted.to_rgb8();
    let mut output = crate::raster::RgbaImage::new(rgb.width(), rgb.height());
    for ((destination, source), alpha) in output.pixels_mut().zip(rgb.pixels())
        .zip(input.mask.to_luma8().pixels()) {
        *destination = crate::raster::Rgba([source[0], source[1], source[2], alpha[0]]);
    }
    Ok(DynamicImage::ImageRgba8(output))
}
