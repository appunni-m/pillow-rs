//! Original neon native-band implementation for `hsv8`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use super::super::default::extract_band_impl as scalar;
use super::super::super::helpers::extract_band_vector as vector;

pub(crate) type Kernel<'a> = vector::NeonBand<'a, crate::raster::Rgb<u8>>;

#[inline]
pub(crate) fn prepare(input: scalar::Kernel<'_>) -> Kernel<'_> {
    vector::prepare_neon(input)
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    vector::neon::<crate::raster::Rgb<u8>, 3>(input)
}
