//! Original sse2 native-band implementation for `rgbx8`.

use super::super::super::helpers::extract_band_vector as vector;
use super::super::default::extract_band_impl as scalar;
use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) type Kernel<'a> = scalar::Kernel<'a>;

#[inline]
pub(crate) fn prepare(input: scalar::Kernel<'_>) -> Kernel<'_> {
    input
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    vector::portable::<crate::raster::Rgba<u8>, 4>(input)
}
