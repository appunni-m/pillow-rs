//! WebAssembly SIMD128-selected native LA Convert strategy.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage, Mode};

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(crate) fn execute(source: &GrayAlphaImage, target: Mode) -> Result<DynamicImage, PilError> {
    super::super::helpers::vector::execute(source, target)
}
