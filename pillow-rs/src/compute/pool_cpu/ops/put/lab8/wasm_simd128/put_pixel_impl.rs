//! Existing masked native byte store compiled for `WasmSimd128`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use super::super::default::put_pixel_impl as scalar;
use super::super::super::helpers::put_pixel_vector;

pub(crate) type Kernel<'a> = scalar::Kernel<'a>;

#[inline]
pub(crate) fn prepare(input: scalar::Kernel<'_>) -> Kernel<'_> { input }

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    let color = input.write.color;
    put_pixel_vector::single_with_hidden_lanes::<crate::raster::Rgb<u8>, 3>(
        input,
        1,
        [color.3, 0],
        [u8::MAX, 0],
        |_| [u8::MAX, 0],
    )
    .map(DynamicImage::ImageLab8)
}
