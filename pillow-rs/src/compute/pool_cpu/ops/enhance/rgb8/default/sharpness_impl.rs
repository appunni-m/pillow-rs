//! Default native ImageRgb8 Sharpness implementation.

use crate::error::PilError;

pub(crate) fn execute(
    source: &crate::raster::RgbImage,
    factor: f64,
) -> Result<crate::raster::RgbImage, PilError> {
    crate::compute::pool_cpu::ops::enhance::helpers::sharpness::execute_u8_image::<_, 3, 3>(
        source, factor,
    )
}
