//! WASM SIMD128 implementation for exact ImageF32 Reduce.

use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn execute(
    input: &RgbaImage,
    x_factor: u32,
    y_factor: u32,
) -> Result<DynamicImage, PilError> {
    super::super::kernels::vector::execute(input, x_factor, y_factor)
}
