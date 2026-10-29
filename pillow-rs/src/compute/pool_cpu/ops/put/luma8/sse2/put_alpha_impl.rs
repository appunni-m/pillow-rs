//! Original constant-alpha vector implementation compiled for `Sse2`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use super::super::default::put_alpha_impl as scalar;
use super::super::super::helpers::put_alpha_vector;

pub(crate) type Kernel<'a> = scalar::Kernel<'a>;

#[inline]
pub(crate) fn prepare(input: scalar::Kernel<'_>) -> Kernel<'_> { input }

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    put_alpha_vector::native_constant::<1, 2, false, _, _>(input).map(DynamicImage::ImageLumaA8)
}
