//! Native synchronous PutPixel writes for `ImageI16B`.

use crate::error::PilError;
use crate::raster::{ImageBuffer, Luma};

pub(crate) type Input = ImageBuffer<Luma<u16>, Vec<u16>>;

pub(crate) fn execute_value_at(
    input: &mut Input,
    pixel_index: usize,
    sample: u16,
) -> Result<(), PilError> {
    super::super::super::helpers::put_pixel::write_native_luma16(
        input.as_mut(),
        pixel_index,
        sample,
    )
}
