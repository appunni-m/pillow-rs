//! Wasm SIMD128-selected CMYK-to-RGB vector implementation.

use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};

pub(crate) fn execute(source: &RgbaImage) -> Result<DynamicImage, PilError> {
    super::super::helpers::to_rgb::execute_wide(source).map(DynamicImage::ImageRgb8)
}
