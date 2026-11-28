//! SSE2-selected native Luma8 Convert strategy.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage, Mode};

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(crate) fn execute(source: &GrayImage, target: Mode) -> Result<DynamicImage, PilError> {
    super::super::helpers::vector::execute(source, target)
}
