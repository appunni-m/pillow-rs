//! Original serial ExtractBand algorithm for `ImageRgb8`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use super::super::super::helpers::extract_band::{self as helpers, CheckedBand};

pub(crate) type Input = crate::raster::RgbImage;
pub(crate) type Kernel<'a> = CheckedBand<'a, crate::raster::Rgb<u8>>;

#[inline]
pub(crate) fn prepare(input: &Input, index: u8) -> Result<Kernel<'_>, PilError> {
    CheckedBand::new(input, index)
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    helpers::rgb(input)
}
