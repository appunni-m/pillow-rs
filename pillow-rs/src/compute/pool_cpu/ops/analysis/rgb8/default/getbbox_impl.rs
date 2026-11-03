use crate::compute::pool_cpu::ops::analysis::helpers::bbox::{self, BBox};
use crate::raster::RgbImage;

pub(crate) fn execute(image: &RgbImage, _alpha_only: bool) -> BBox {
    bbox::rgb8(image)
}
