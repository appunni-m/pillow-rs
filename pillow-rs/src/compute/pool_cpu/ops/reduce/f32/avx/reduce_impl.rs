//! AVX implementation for exact ImageF32 Reduce.

use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(
    input: &RgbaImage,
    x_factor: u32,
    y_factor: u32,
) -> Result<DynamicImage, PilError> {
    super::super::helpers::vector::execute(input, x_factor, y_factor)
}
