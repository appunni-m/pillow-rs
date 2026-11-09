//! Original neon native-band implementation for `palette_alpha8`.

use super::super::super::helpers::extract_band_vector as vector;
use super::super::default::extract_band_impl as scalar;
use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) type Kernel<'a> = vector::NeonBand<'a, crate::raster::LumaA<u8>>;

#[inline]
pub(crate) fn prepare(input: scalar::Kernel<'_>) -> Kernel<'_> {
    vector::prepare_neon(input)
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    vector::neon::<crate::raster::LumaA<u8>, 2>(input)
}
