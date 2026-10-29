//! NEON-selected native YCbCr Color3DLut kernel.

use crate::compute::pool_cpu::ops::color3dlut::helpers::lut_vector;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

// NO_PILLOW_SUPPORT: This applies the table to stored YCbCr samples directly,
// without converting the raster to RGB.
pub(crate) fn execute(
    image: &DynamicImage,
    size: (u32, u32, u32),
    table: &[f64],
    channels: u32,
    target_mode: Mode,
) -> Result<DynamicImage, PilError> {
    lut_vector::apply(image, Mode::ImageYCbCr8, size, table, channels, target_mode)
}
