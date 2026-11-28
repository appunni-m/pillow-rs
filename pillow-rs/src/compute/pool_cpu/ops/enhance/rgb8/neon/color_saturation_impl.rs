use crate::compute::pool_cpu::ops::enhance::helpers::color_saturation::{self, BASE_RGB};
use crate::error::PilError;
use crate::raster::RgbImage;

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(crate) fn execute(source: &RgbImage, factor: f64) -> Result<RgbImage, PilError> {
    color_saturation::execute_vector::<_, 3, BASE_RGB>(source, factor)
}
