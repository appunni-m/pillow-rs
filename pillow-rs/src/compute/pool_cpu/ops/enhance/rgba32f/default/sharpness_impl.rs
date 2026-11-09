//! Default native ImageRgba32F Sharpness implementation.

// NO_PILLOW_SUPPORT: native Sharpness execution extends Pillow for this mode.
use crate::error::PilError;

pub(crate) fn execute(
    source: &crate::raster::Rgba32FImage,
    factor: f64,
) -> Result<crate::raster::Rgba32FImage, PilError> {
    crate::compute::pool_cpu::ops::enhance::helpers::sharpness::execute_f32_image::<_, 4, 3>(
        source, factor,
    )
}
