//! NEON-selected RGB histogram LUT kernels.

use crate::error::PilError;
use crate::raster::{DynamicImage, RgbImage};

pub(crate) fn autocontrast(input: &RgbImage, lut: &[u8]) -> Result<DynamicImage, PilError> {
    crate::compute::pool_cpu::ops::imageops::helpers::histogram::apply_rgb_architecture(
        input, lut, false,
    )
}

pub(crate) fn equalize(input: &RgbImage, lut: &[u8]) -> Result<DynamicImage, PilError> {
    crate::compute::pool_cpu::ops::imageops::helpers::histogram::apply_rgb_architecture(
        input, lut, true,
    )
}
