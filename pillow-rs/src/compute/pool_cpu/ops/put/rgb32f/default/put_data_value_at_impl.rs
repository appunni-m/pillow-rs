//! NO_PILLOW_SUPPORT: direct native `f32` PutData for CoreOnly RGB32F.

use crate::error::PilError;
use crate::raster::{ImageBuffer, Rgb};

pub(crate) fn execute_value_at(
    image: &mut ImageBuffer<Rgb<f32>, Vec<f32>>,
    pixel_index: usize,
    pixel: [f32; 3],
) -> Result<(), PilError> {
    super::super::super::helpers::put_data_value_at::write_native_pixel(
        image.as_mut(),
        pixel_index,
        pixel,
    )
}

pub(crate) fn execute_values(
    image: &mut ImageBuffer<Rgb<f32>, Vec<f32>>,
    values: &[crate::image::PutDataValue],
    first: [f32; 3],
    completed: &mut usize,
) -> Result<(), PilError> {
    use super::super::super::helpers::put_data_value_at::{
        native_f32_components, write_native_values,
    };

    write_native_values(
        image.as_mut(),
        values,
        first,
        |value| native_f32_components::<3>(value, 3),
        completed,
    )
}
