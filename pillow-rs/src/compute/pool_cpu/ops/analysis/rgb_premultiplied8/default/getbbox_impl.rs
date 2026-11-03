use crate::compute::pool_cpu::ops::analysis::helpers::bbox::{self, BBox};
use crate::raster::RgbaImage;

pub(crate) fn execute(image: &RgbaImage, alpha_only: bool) -> BBox {
    bbox::rgba8(image, alpha_only)
}
