//! Existing masked native byte store compiled for `Sse2`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use super::super::default::put_pixel_impl as scalar;
use super::super::super::helpers::put_pixel_vector;

pub(crate) type Kernel<'a> = scalar::Kernel<'a>;

#[inline]
pub(crate) fn prepare(input: scalar::Kernel<'_>) -> Kernel<'_> { input }

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    put_pixel_vector::single::<crate::raster::Rgb<u8>, 3>(input)
        .map(DynamicImage::ImageYCbCr8)
}
