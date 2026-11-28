//! NEON-selected Equalize data kernel after P palette expansion.

use crate::error::PilError;
use crate::raster::{DynamicImage, RgbImage};

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn execute(input: RgbImage, lut: &[u8]) -> Result<DynamicImage, PilError> {
    crate::compute::pool_cpu::ops::imageops::helpers::histogram::apply_rgb_architecture_in_place(
        input, lut,
    )
}
