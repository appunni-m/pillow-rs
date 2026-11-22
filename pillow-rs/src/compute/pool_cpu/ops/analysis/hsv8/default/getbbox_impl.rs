use crate::compute::pool_cpu::ops::analysis::helpers::bbox::{self, BBox};
use crate::raster::RgbImage;

// VERIFY: source=reviewed@2026-11-20; parity=pending; benchmark=pending
pub(crate) fn execute(image: &RgbImage, _alpha_only: bool) -> BBox {
    bbox::rgb8(image)
}
