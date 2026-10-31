//! AVX Sharpness implementation for exact mode ImageLuma8.

use crate::error::PilError;

pub(crate) fn execute(source: &crate::raster::GrayImage, factor: f64) -> Result<crate::raster::GrayImage, PilError> {
    crate::compute::pool_cpu::ops::enhance::helpers::sharpness_vector::execute_image::<
        _, 1, 1, false, false,
    >(source, factor)
}
