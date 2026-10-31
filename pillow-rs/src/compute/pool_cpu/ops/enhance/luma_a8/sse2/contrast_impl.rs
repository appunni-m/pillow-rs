//! SSE2 Contrast implementation for exact mode ImageLumaA8.

use crate::compute::helpers::enhance::ContrastBase;
use crate::error::PilError;
use crate::raster::GrayAlphaImage;

pub(crate) fn execute(
    source: &GrayAlphaImage,
    base: &ContrastBase,
    factor: f64,
) -> Result<GrayAlphaImage, PilError> {
    crate::compute::pool_cpu::ops::enhance::helpers::contrast::execute_vector::<
        _,
        2,
    >(source, base, factor)
}
