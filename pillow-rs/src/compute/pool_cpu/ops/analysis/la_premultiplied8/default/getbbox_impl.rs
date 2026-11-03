use crate::compute::pool_cpu::ops::analysis::helpers::bbox::{self, BBox};
use crate::raster::GrayAlphaImage;

pub(crate) fn execute(image: &GrayAlphaImage, alpha_only: bool) -> BBox {
    bbox::luma_alpha8(image, alpha_only)
}
