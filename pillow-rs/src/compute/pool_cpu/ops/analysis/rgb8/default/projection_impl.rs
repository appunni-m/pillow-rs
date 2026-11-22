use crate::compute::pool_cpu::ops::analysis::helpers::projection::ProjectionAxes;
use crate::raster::RgbImage;

// VERIFY: source=reviewed@2026-11-20; parity=pending; benchmark=pending
pub(crate) fn execute(image: &RgbImage) -> ProjectionAxes {
    crate::compute::pool_cpu::ops::analysis::helpers::projection::rgb_projection(image)
}
