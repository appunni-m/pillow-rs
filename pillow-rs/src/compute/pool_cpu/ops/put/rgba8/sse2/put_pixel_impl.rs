//! Existing masked native byte store compiled for `Sse2`.

use super::super::super::helpers::put_pixel_vector;
use super::super::default::put_pixel_impl as scalar;
use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) type Kernel<'a> = scalar::Kernel<'a>;

#[inline]
pub(crate) fn prepare(input: scalar::Kernel<'_>) -> Kernel<'_> {
    input
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    put_pixel_vector::single::<crate::raster::Rgba<u8>, 4>(input).map(DynamicImage::ImageRgba8)
}

pub(crate) type BatchKernel<'a> = scalar::BatchKernel<'a>;

#[inline]
pub(crate) fn prepare_batch(input: scalar::BatchKernel<'_>) -> BatchKernel<'_> {
    input
}

#[inline]
pub(crate) fn execute_batch(
    input: BatchKernel<'_>,
    selection: crate::compute::registry::support::StrategySelection,
) -> Result<DynamicImage, PilError> {
    put_pixel_vector::batch(input, selection).map(DynamicImage::ImageRgba8)
}
