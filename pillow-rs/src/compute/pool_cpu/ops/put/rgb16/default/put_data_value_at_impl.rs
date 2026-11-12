//! NO_PILLOW_SUPPORT: direct native `u16` PutData for CoreOnly RGB16.

use crate::error::PilError;
use crate::raster::{ImageBuffer, Rgb};

pub(crate) fn execute_value_at(
    image: &mut ImageBuffer<Rgb<u16>, Vec<u16>>,
    pixel_index: usize,
    pixel: [u16; 3],
) -> Result<(), PilError> {
    super::super::super::helpers::put_data_value_at::write_native_pixel(
        image.as_mut(),
        pixel_index,
        pixel,
    )
}

pub(crate) fn execute_values(
    image: &mut ImageBuffer<Rgb<u16>, Vec<u16>>,
    values: &[crate::image::PutDataValue],
    first: [u16; 3],
    completed: &mut usize,
) -> Result<(), PilError> {
    use super::super::super::helpers::put_data_value_at::{
        native_u16_components, write_native_values,
    };

    write_native_values(
        image.as_mut(),
        values,
        first,
        |value| native_u16_components::<3>(value, 3, u16::MAX),
        completed,
    )
}
