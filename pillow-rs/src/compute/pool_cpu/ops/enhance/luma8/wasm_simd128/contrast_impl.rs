//! WASM_SIMD128 Contrast implementation for exact mode ImageLuma8.

use crate::compute::helpers::enhance::ContrastBase;
use crate::error::PilError;
use crate::raster::GrayImage;

pub(crate) fn execute(
    source: &GrayImage,
    base: &ContrastBase,
    factor: f64,
) -> Result<GrayImage, PilError> {
    crate::compute::pool_cpu::ops::enhance::helpers::contrast::execute_vector::<_, 1>(
        source, base, factor,
    )
}
