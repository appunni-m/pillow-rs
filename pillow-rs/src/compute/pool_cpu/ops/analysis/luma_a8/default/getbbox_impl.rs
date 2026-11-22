use crate::compute::pool_cpu::ops::analysis::helpers::bbox::{self, BBox};
use crate::raster::GrayAlphaImage;

// VERIFY: source=reviewed@2026-11-20; parity=pending; benchmark=pending
pub(crate) fn execute(image: &GrayAlphaImage, alpha_only: bool) -> BBox {
    bbox::luma_alpha8(image, alpha_only)
}
