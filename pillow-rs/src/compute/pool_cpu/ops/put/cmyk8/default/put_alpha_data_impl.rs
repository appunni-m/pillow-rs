//! Original image-backed alpha semantics for `ImageCmyk8`.

use crate::error::PilError;
use crate::raster::{DynamicImage, Rgba};
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
    // Preserve the original CMYK-to-RGB arithmetic without copying the
    // converted RGB frame before packing its output alpha samples.
    let rgb = crate::color::cmyk_buffer_to_rgb(input.image);
    let mask = input.mask.luma_samples();
    let mut output = crate::raster::RgbaImage::new(rgb.width(), rgb.height());
    for ((destination, source), alpha) in output.pixels_mut().zip(rgb.pixels())
        .zip(mask.as_slice()) {
        *destination = Rgba([source[0], source[1], source[2], *alpha]);
    }
    Ok(DynamicImage::ImageRgba8(output))
}
