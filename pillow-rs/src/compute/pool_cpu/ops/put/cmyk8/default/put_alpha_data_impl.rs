//! Original image-backed alpha semantics for `ImageCmyk8`.

use crate::error::PilError;
use crate::raster::{DynamicImage, Rgba};
use super::super::super::helpers::{
    alpha_mask::MaskStrategy, put_alpha_data::AlphaDataInput,
};

pub(crate) type Input = crate::raster::RgbaImage;
pub(crate) type Kernel<'a> = AlphaDataInput<'a, crate::raster::Rgba<u8>>;

#[inline]
pub(crate) fn prepare<'a>(image: &'a Input, mask: MaskStrategy<'a>) -> Kernel<'a> {
    AlphaDataInput { image, mask }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    let mask = input.mask.samples();
    // Apply the shared CMYK pixel arithmetic directly into the required RGBA
    // result so the scalar path does not allocate an intermediate RGB frame.
    let mut output = crate::raster::RgbaImage::new(input.image.width(), input.image.height());
    for ((destination, source), alpha) in output.pixels_mut()
        .zip(input.image.pixels())
        .zip(mask)
    {
        let [red, green, blue] = crate::color::cmyk_pixel_to_rgb(source.0);
        *destination = Rgba([red, green, blue, *alpha]);
    }
    Ok(DynamicImage::ImageRgba8(output))
}
