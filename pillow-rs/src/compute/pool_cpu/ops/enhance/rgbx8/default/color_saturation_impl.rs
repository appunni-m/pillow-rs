use crate::compute::pool_cpu::ops::enhance::helpers::color_saturation::{self, BASE_RGBX};
use crate::error::PilError;
use crate::raster::RgbaImage;

pub(crate) fn execute(source: &RgbaImage, factor: f64) -> Result<RgbaImage, PilError> {
    color_saturation::execute::<_, 4, BASE_RGBX>(source, factor)
}
