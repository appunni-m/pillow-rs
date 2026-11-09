//! Default native ImageLuma8 Sharpness implementation.

use crate::error::PilError;

pub(crate) fn execute(
    source: &crate::raster::GrayImage,
    factor: f64,
) -> Result<crate::raster::GrayImage, PilError> {
    crate::compute::pool_cpu::ops::enhance::helpers::sharpness::execute_u8_image::<_, 1, 1>(
        source, factor,
    )
}
