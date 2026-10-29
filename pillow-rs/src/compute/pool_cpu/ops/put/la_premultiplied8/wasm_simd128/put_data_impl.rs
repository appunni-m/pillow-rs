//! Existing native-byte prefix copy compiled for `WasmSimd128`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use super::super::default::put_data_impl as scalar;
use super::super::super::helpers::put_data_vector;

pub(crate) type Kernel<'a> = scalar::Kernel<'a>;

#[inline]
pub(crate) fn prepare(input: scalar::Kernel<'_>) -> Kernel<'_> { input }

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    put_data_vector::execute(input).map(DynamicImage::ImageLaPremultiplied8)
}
