//! NO_PILLOW_SUPPORT: direct native `u16` PutData for CoreOnly RGBA16.

use crate::error::PilError;
use crate::raster::{ImageBuffer, Rgba};

pub(crate) fn execute_value_at(
    image: &mut ImageBuffer<Rgba<u16>, Vec<u16>>,
    pixel_index: usize,
    pixel: [u16; 4],
) -> Result<(), PilError> {
    super::super::super::helpers::put_data_value_at::write_native_pixel(
        image.as_mut(),
        pixel_index,
        pixel,
    )
}

pub(crate) fn execute_values(
    image: &mut ImageBuffer<Rgba<u16>, Vec<u16>>,
    values: &[crate::image::PutDataValue],
    first: [u16; 4],
    completed: &mut usize,
) -> Result<(), PilError> {
    use super::super::super::helpers::put_data_value_at::{
        native_u16_components, write_native_values,
    };

    write_native_values(
        image.as_mut(),
        values,
        first,
        |value| native_u16_components::<4>(value, 4, u16::MAX),
        completed,
    )
}
