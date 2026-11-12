//! NO_PILLOW_SUPPORT: direct native `u16` PutData for CoreOnly LA16.

use crate::error::PilError;
use crate::raster::{ImageBuffer, LumaA};

pub(crate) fn execute_value_at(
    image: &mut ImageBuffer<LumaA<u16>, Vec<u16>>,
    pixel_index: usize,
    pixel: [u16; 2],
) -> Result<(), PilError> {
    super::super::super::helpers::put_data_value_at::write_native_pixel(
        image.as_mut(),
        pixel_index,
        pixel,
    )
}

pub(crate) fn execute_values(
    image: &mut ImageBuffer<LumaA<u16>, Vec<u16>>,
    values: &[crate::image::PutDataValue],
    first: [u16; 2],
    completed: &mut usize,
) -> Result<(), PilError> {
    use super::super::super::helpers::put_data_value_at::{
        native_u16_components, write_native_values,
    };

    write_native_values(
        image.as_mut(),
        values,
        first,
        |value| native_u16_components::<2>(value, 2, u16::MAX),
        completed,
    )
}
