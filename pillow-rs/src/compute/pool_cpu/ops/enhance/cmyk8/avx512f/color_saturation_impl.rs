use crate::compute::pool_cpu::ops::enhance::helpers::color_saturation::{self, BASE_CMYK};
use crate::error::PilError;
use crate::raster::RgbaImage;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute(source: &RgbaImage, factor: f64) -> Result<RgbaImage, PilError> {
    color_saturation::avx512f::execute::<_, 4, BASE_CMYK>(source, factor)
}
