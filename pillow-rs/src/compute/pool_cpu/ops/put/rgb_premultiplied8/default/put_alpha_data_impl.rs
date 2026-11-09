//! Native RGBa-to-RGBA conversion and image-mask alpha replacement.

use super::super::super::helpers::{
    alpha_mask::{MaskStrategy, normalize_sample},
    put_alpha_data::AlphaDataInput,
};
use crate::compute::pool_cpu::helpers::bytes::unpremultiply::unpremultiply_byte;
use crate::error::PilError;
use crate::raster::DynamicImage;

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
    // Pillow converts RGBa to straight RGBA before replacing its alpha band.
    // Read the premultiplied source and mask directly into the one required
    // RGBA result allocation.
    let mut output = crate::raster::RgbaImage::new(input.image.width(), input.image.height());
    for ((destination, source), alpha) in output
        .as_mut()
        .chunks_exact_mut(4)
        .zip(input.image.as_raw().chunks_exact(4))
        .zip(mask)
    {
        let source_alpha = source[3];
        destination[0] = unpremultiply_byte(source[0], source_alpha);
        destination[1] = unpremultiply_byte(source[1], source_alpha);
        destination[2] = unpremultiply_byte(source[2], source_alpha);
        destination[3] = normalize_sample::<ONE>(*alpha);
    }
    Ok(DynamicImage::ImageRgba8(output))
}
