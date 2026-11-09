//! Serial Default implementation for ImageRgb8-to-LAB conversion.

use crate::compute::pool_cpu::ops::color::helpers::lab_kernel;
use crate::error::PilError;
use crate::raster::{DynamicImage, ImageBuffer, Mode, RgbImage};

/// Executes the ImageRgb8 strategy and retains its physical padding lane.
pub(crate) fn execute(input: &RgbImage) -> Result<DynamicImage, PilError> {
    let (width, height) = input.dimensions();
    let pixels = lab_kernel::pixel_count(width, height)?;
    let bytes = lab_kernel::convert_rgb_bytes(input.as_raw(), pixels)?;
    let output = ImageBuffer::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::InternalError("LAB output buffer shape mismatch".into()))?;
    let mut output = DynamicImage::ImageLab8(output);
    super::super::super::convert::helpers::physical_lanes::preserve_compact_rgb_lane(
        input,
        Mode::ImageLab8,
        &mut output,
    )?;
    Ok(output)
}
