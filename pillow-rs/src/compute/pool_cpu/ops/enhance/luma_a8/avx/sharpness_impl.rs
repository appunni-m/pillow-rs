//! AVX Sharpness implementation for exact mode ImageLumaA8.

use crate::error::PilError;

pub(crate) fn execute(
    source: &crate::raster::GrayAlphaImage,
    factor: f64,
) -> Result<crate::raster::GrayAlphaImage, PilError> {
    crate::compute::pool_cpu::ops::enhance::helpers::sharpness_vector::execute_image::<
        _,
        2,
        1,
        false,
        false,
    >(source, factor)
}
