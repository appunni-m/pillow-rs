use crate::compute::pool_cpu::ops::analysis::helpers::bbox::{self, BBox};
use crate::raster::RgbaImage;

// VERIFY: source=reviewed@2026-11-20; parity=pending; benchmark=pending
pub(crate) fn execute(image: &RgbaImage, _alpha_only: bool) -> BBox {
    bbox::i32_carrier(image.as_raw(), image.width(), image.height())
}
