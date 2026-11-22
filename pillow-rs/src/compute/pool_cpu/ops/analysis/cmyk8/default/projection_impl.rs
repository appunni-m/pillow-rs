use crate::compute::pool_cpu::ops::analysis::helpers::projection::ProjectionAxes;
use crate::raster::RgbaImage;

// VERIFY: source=reviewed@2026-11-20; parity=pending; benchmark=pending
pub(crate) fn execute(image: &RgbaImage) -> ProjectionAxes {
    crate::compute::pool_cpu::ops::analysis::helpers::projection::cmyk_projection(image)
}
