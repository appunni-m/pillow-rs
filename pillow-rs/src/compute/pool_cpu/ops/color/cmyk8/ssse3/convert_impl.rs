//! SSSE3-selected CMYK-to-RGB vector implementation.

use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn execute(source: &RgbaImage) -> Result<DynamicImage, PilError> {
    super::super::helpers::to_rgb::execute_ssse3(source).map(DynamicImage::ImageRgb8)
}
