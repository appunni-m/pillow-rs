//! Default native ImageRgbPremultiplied8 Sharpness implementation.

// NO_PILLOW_SUPPORT: native Sharpness execution extends Pillow for this mode.
use crate::error::PilError;

pub(crate) fn execute(
    source: &crate::raster::RgbaImage,
    factor: f64,
) -> Result<crate::raster::RgbaImage, PilError> {
    crate::compute::pool_cpu::ops::enhance::helpers::sharpness::execute_u8_image::<_, 4, 3>(
        source, factor,
    )
}
