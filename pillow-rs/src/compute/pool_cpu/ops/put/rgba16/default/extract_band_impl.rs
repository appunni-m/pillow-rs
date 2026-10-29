//! Original serial ExtractBand algorithm for `ImageRgba16`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use super::super::super::helpers::extract_band::{self as helpers, CheckedBand};

pub(crate) type Input = crate::raster::ImageBuffer<crate::raster::Rgba<u16>, Vec<u16>>;
pub(crate) type Kernel<'a> = CheckedBand<'a, crate::raster::Rgba<u16>>;

#[inline]
pub(crate) fn prepare(input: &Input, index: u8) -> Result<Kernel<'_>, PilError> {
    CheckedBand::new(input, index)
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    let channel = Some(input.index.min(3));
    helpers::native(input, channel)
}
