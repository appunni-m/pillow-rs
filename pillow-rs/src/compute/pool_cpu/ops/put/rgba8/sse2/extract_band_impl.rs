//! Original sse2 native-band implementation for `rgba8`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use super::super::default::extract_band_impl as scalar;
use super::super::super::helpers::extract_band_vector as vector;

pub(crate) type Kernel<'a> = scalar::Kernel<'a>;

#[inline]
pub(crate) fn prepare(input: scalar::Kernel<'_>) -> Kernel<'_> {
    input
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    vector::portable::<crate::raster::Rgba<u8>, 4>(input)
}
