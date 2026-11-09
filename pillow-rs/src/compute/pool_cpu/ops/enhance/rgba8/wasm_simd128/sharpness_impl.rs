//! WebAssembly SIMD128 Sharpness implementation for exact mode ImageRgba8.

use crate::error::PilError;

pub(crate) fn execute(
    source: &crate::raster::RgbaImage,
    factor: f64,
) -> Result<crate::raster::RgbaImage, PilError> {
    crate::compute::pool_cpu::ops::enhance::helpers::sharpness_vector::execute_image::<
        _,
        4,
        3,
        false,
        false,
    >(source, factor)
}
