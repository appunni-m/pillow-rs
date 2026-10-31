//! Default native ImageI16L Sharpness implementation.

// NO_PILLOW_SUPPORT: native Sharpness execution extends Pillow for this mode.
use crate::error::PilError;

pub(crate) fn execute(source: &crate::raster::ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>, factor: f64) -> Result<crate::raster::ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>, PilError> {
    crate::compute::pool_cpu::ops::enhance::helpers::sharpness::execute_u16_image::<
        _, 1, 1,
    >(source, factor)
}
