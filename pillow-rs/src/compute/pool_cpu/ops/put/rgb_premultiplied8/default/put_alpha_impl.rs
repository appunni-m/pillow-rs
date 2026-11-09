//! Native RGBa-to-RGBA conversion and alpha replacement for `ImageRgbPremultiplied8`.

use super::super::super::helpers::put_alpha::AlphaInput;
use crate::compute::pool_cpu::helpers::bytes::unpremultiply::unpremultiply_byte;
use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) type Input = crate::raster::RgbaImage;
pub(crate) type Kernel<'a> = AlphaInput<'a, crate::raster::Rgba<u8>>;

#[inline]
pub(crate) fn prepare(image: &Input, alpha: u8) -> Kernel<'_> {
    AlphaInput { image, alpha }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    // Pillow converts RGBa to straight RGBA before replacing its alpha band.
    // Fuse that conversion with the required output write; do not stage RGBA.
    let mut output = crate::raster::RgbaImage::new(input.image.width(), input.image.height());
    for (destination, source) in output
        .as_mut()
        .chunks_exact_mut(4)
        .zip(input.image.as_raw().chunks_exact(4))
    {
        let source_alpha = source[3];
        destination[0] = unpremultiply_byte(source[0], source_alpha);
        destination[1] = unpremultiply_byte(source[1], source_alpha);
        destination[2] = unpremultiply_byte(source[2], source_alpha);
        destination[3] = input.alpha;
    }
    Ok(DynamicImage::ImageRgba8(output))
}
