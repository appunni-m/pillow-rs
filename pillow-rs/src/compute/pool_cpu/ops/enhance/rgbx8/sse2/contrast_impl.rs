//! SSE2 Contrast implementation for exact mode ImageRgbx8.

use crate::compute::helpers::enhance::ContrastBase;
use crate::error::PilError;
use crate::raster::RgbaImage;

pub(crate) fn execute(
    source: &RgbaImage,
    base: &ContrastBase,
    factor: f64,
) -> Result<RgbaImage, PilError> {
    crate::compute::pool_cpu::ops::enhance::helpers::contrast::execute_vector::<_, 4>(
        source, base, factor,
    )
}
