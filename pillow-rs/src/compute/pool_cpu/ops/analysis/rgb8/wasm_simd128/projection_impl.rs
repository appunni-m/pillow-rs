use crate::compute::pool_cpu::ops::analysis::helpers::projection_vector::VectorProjection;
use crate::raster::RgbImage;
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(image: &RgbImage) -> VectorProjection {
    crate::compute::pool_cpu::ops::analysis::helpers::projection_vector::rgb_projection(image)
}
