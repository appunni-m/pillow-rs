//! Default native ImageLumaA8 Sharpness implementation.

use crate::error::PilError;

pub(crate) fn execute(source: &crate::raster::GrayAlphaImage, factor: f64) -> Result<crate::raster::GrayAlphaImage, PilError> {
    crate::compute::pool_cpu::ops::enhance::helpers::sharpness::execute_u8_image::<
        _, 2, 1,
    >(source, factor)
}
