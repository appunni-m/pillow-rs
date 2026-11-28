//! AVX2 Sharpness implementation for exact mode ImageRgb8.

use crate::error::PilError;

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(crate) fn execute(
    source: &crate::raster::RgbImage,
    factor: f64,
) -> Result<crate::raster::RgbImage, PilError> {
    crate::compute::pool_cpu::ops::enhance::helpers::sharpness_vector::execute_image::<
        _,
        3,
        3,
        true,
        false,
    >(source, factor)
}
