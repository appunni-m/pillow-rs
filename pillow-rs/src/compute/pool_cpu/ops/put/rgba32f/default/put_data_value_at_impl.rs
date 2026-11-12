//! NO_PILLOW_SUPPORT: direct native `f32` PutData for CoreOnly RGBA32F.

use crate::error::PilError;
use crate::raster::{ImageBuffer, Rgba};

pub(crate) fn execute_value_at(
    image: &mut ImageBuffer<Rgba<f32>, Vec<f32>>,
    pixel_index: usize,
    pixel: [f32; 4],
) -> Result<(), PilError> {
    super::super::super::helpers::put_data_value_at::write_native_pixel(
        image.as_mut(),
        pixel_index,
        pixel,
    )
}

pub(crate) fn execute_values(
    image: &mut ImageBuffer<Rgba<f32>, Vec<f32>>,
    values: &[crate::image::PutDataValue],
    first: [f32; 4],
    completed: &mut usize,
) -> Result<(), PilError> {
    use super::super::super::helpers::put_data_value_at::{
        native_f32_components, write_native_values,
    };

    write_native_values(
        image.as_mut(),
        values,
        first,
        |value| native_f32_components::<4>(value, 4),
        completed,
    )
}
