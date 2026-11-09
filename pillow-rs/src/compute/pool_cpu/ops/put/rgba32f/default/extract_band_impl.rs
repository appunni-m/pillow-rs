//! Original serial ExtractBand algorithm for `ImageRgba32F`.

use super::super::super::helpers::extract_band::{self as helpers, CheckedBand};
use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) type Input = crate::raster::Rgba32FImage;
pub(crate) type Kernel<'a> = CheckedBand<'a, crate::raster::Rgba<f32>>;

#[inline]
pub(crate) fn prepare(input: &Input, index: u8) -> Result<Kernel<'_>, PilError> {
    CheckedBand::new(input, index)
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    let channel = Some(input.index.min(3));
    helpers::native_f32(input, channel)
}
