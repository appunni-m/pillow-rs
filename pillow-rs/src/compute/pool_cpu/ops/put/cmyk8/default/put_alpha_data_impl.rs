//! Original image-backed alpha semantics for `ImageCmyk8`.

use super::super::super::helpers::{
    alpha_mask::{MaskStrategy, normalize_sample},
    put_alpha_data::AlphaDataInput,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Rgba};

pub(crate) type Input = crate::raster::RgbaImage;
pub(crate) type Kernel<'a> = AlphaDataInput<'a, crate::raster::Rgba<u8>>;

#[inline]
pub(crate) fn prepare<'a>(image: &'a Input, mask: MaskStrategy<'a>) -> Kernel<'a> {
    AlphaDataInput { image, mask }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    input.mask.with_samples(
        |samples| execute_samples::<true>(&input, samples),
        |samples| execute_samples::<false>(&input, samples),
    )
}

fn execute_samples<const ONE: bool>(
    input: &Kernel<'_>,
    mask: &[u8],
) -> Result<DynamicImage, PilError> {
    // Apply the shared CMYK pixel arithmetic directly into the required RGBA
    // result so the scalar path does not allocate an intermediate RGB frame.
    let mut output = crate::raster::RgbaImage::new(input.image.width(), input.image.height());
    for ((destination, source), alpha) in output.pixels_mut().zip(input.image.pixels()).zip(mask) {
        let [red, green, blue] = crate::color::cmyk_pixel_to_rgb(source.0);
        *destination = Rgba([red, green, blue, normalize_sample::<ONE>(*alpha)]);
    }
    Ok(DynamicImage::ImageRgba8(output))
}
