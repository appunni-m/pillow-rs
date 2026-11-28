use crate::compute::pool_cpu::ops::enhance::helpers::color_saturation::{self, BASE_LUMA};
use crate::error::PilError;
use crate::raster::GrayAlphaImage;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute(source: &GrayAlphaImage, factor: f64) -> Result<GrayAlphaImage, PilError> {
    color_saturation::avx512f::execute::<_, 2, BASE_LUMA>(source, factor)
}
