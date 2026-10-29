//! Original serial ExtractBand algorithm for `ImageRgb32F`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use super::super::super::helpers::extract_band::{self as helpers, CheckedBand};

pub(crate) type Input = crate::raster::Rgb32FImage;
pub(crate) type Kernel<'a> = CheckedBand<'a, crate::raster::Rgb<f32>>;

#[inline]
pub(crate) fn prepare(input: &Input, index: u8) -> Result<Kernel<'_>, PilError> {
    CheckedBand::new(input, index)
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    // RGB-to-RGBA compatibility supplies an opaque fourth band.
    let channel = (input.index < 3).then_some(input.index);
    helpers::native(input, channel)
}
