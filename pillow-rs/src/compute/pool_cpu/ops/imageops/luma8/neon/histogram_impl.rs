//! NEON-selected L histogram LUT kernels.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage};

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn autocontrast(input: &GrayImage, lut: &[u8]) -> Result<DynamicImage, PilError> {
    crate::compute::pool_cpu::ops::imageops::helpers::histogram::apply_luma_architecture(
        input, lut, false,
    )
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn equalize(input: &GrayImage, lut: &[u8]) -> Result<DynamicImage, PilError> {
    crate::compute::pool_cpu::ops::imageops::helpers::histogram::apply_luma_architecture(
        input, lut, true,
    )
}
