//! WebAssembly SIMD128 Sharpness implementation for exact mode ImageRgb8.

use crate::error::PilError;

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
