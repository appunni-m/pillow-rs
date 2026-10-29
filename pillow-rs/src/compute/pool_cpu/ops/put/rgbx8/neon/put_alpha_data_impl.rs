//! Original image-backed alpha implementation compiled for `Neon`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use super::super::default::put_alpha_data_impl as scalar;
use super::super::super::helpers::put_alpha_data_vector::{self, NativeInput};

pub(crate) type Kernel<'a> = NativeInput<'a, crate::raster::Rgba<u8>>;

#[inline]
pub(crate) fn prepare(input: scalar::Kernel<'_>) -> Result<Kernel<'_>, PilError> {
    NativeInput::new(input)
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    put_alpha_data_vector::native_mask::<4, 4, false, _, _>(input).map(DynamicImage::ImageRgba8)
}
