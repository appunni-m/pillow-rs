//! WASM_SIMD128 Contrast implementation for exact mode ImageRgb8.

use crate::compute::helpers::enhance::ContrastBase;
use crate::error::PilError;
use crate::raster::RgbImage;

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(crate) fn execute(
    source: &RgbImage,
    base: &ContrastBase,
    factor: f64,
) -> Result<RgbImage, PilError> {
    crate::compute::pool_cpu::ops::enhance::helpers::contrast::execute_vector::<_, 3>(
        source, base, factor,
    )
}
