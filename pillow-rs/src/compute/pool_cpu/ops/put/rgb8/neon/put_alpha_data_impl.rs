//! Original image-backed alpha implementation compiled for `Neon`.

use super::super::super::helpers::put_alpha_data_vector::{self, NativeInput};
use super::super::default::put_alpha_data_impl as scalar;
use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) type Kernel<'a> = NativeInput<'a, crate::raster::Rgb<u8>>;

#[inline]
pub(crate) fn prepare(input: scalar::Kernel<'_>) -> Result<Kernel<'_>, PilError> {
    NativeInput::new(input)
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    put_alpha_data_vector::rgb_mask(input).map(DynamicImage::ImageRgba8)
}
