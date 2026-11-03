//! RGBX mode strategy: transform RGB and preserve X in LAB's auxiliary lane.

use crate::compute::pool_cpu::ops::color::helpers::lab_kernel;
use crate::compute::pool_cpu::ops::color::convert::helpers::physical_lanes;
use crate::error::PilError;
use crate::raster::{DynamicImage, ImageBuffer, Mode, RgbaImage};

pub(crate) fn execute(input: &RgbaImage) -> Result<DynamicImage, PilError> {
    let (width, height) = input.dimensions();
    let pixels = lab_kernel::pixel_count(width, height)?;
    let bytes = lab_kernel::convert_four_byte_rgb_bytes(input.as_raw(), pixels)?;
    let output = ImageBuffer::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::InternalError("LAB output buffer shape mismatch".into()))?;
    let mut output = DynamicImage::ImageLab8(output);
    physical_lanes::preserve_fourth_byte(
        input.as_raw(),
        width,
        height,
        Mode::ImageLab8,
        &mut output,
    )?;
    Ok(output)
}
