//! Default native ImageRgb32F Sharpness implementation.

// NO_PILLOW_SUPPORT: native Sharpness execution extends Pillow for this mode.
use crate::error::PilError;

pub(crate) fn execute(source: &crate::raster::Rgb32FImage, factor: f64) -> Result<crate::raster::Rgb32FImage, PilError> {
    crate::compute::pool_cpu::ops::enhance::helpers::sharpness::execute_f32_image::<
        _, 3, 3,
    >(source, factor)
}
