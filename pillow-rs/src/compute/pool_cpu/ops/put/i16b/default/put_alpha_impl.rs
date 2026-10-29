//! Original constant alpha semantics for `ImageI16B`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use super::super::super::helpers::put_alpha::{self as helpers, AlphaInput};

pub(crate) type Input = crate::raster::ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>;
pub(crate) type Kernel<'a> = AlphaInput<'a, crate::raster::Luma<u16>>;

#[inline]
pub(crate) fn prepare(image: &Input, alpha: u8) -> Kernel<'_> {
    AlphaInput { image, alpha }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    let rgba: crate::raster::RgbaImage = super::super::super::helpers::put_data::clipped(input.image, |sample| {
        crate::raster::Rgba([sample, sample, sample, u8::MAX])
    });
    Ok(DynamicImage::ImageRgba8(helpers::replace_rgba_alpha(rgba, input.alpha)))
}
