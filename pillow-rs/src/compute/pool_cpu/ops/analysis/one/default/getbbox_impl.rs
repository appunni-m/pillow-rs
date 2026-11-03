use crate::compute::pool_cpu::ops::analysis::helpers::bbox::{self, BBox};
use crate::raster::GrayImage;

pub(crate) fn execute(image: &GrayImage, _alpha_only: bool) -> BBox {
    bbox::luma8(image)
}
