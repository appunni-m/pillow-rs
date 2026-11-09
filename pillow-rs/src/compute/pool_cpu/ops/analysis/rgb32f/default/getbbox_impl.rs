use crate::compute::pool_cpu::ops::analysis::helpers::bbox::{self, BBox};
use crate::raster::Rgb32FImage;

// NO_PILLOW_SUPPORT: this internal raster identity uses a native GetBBox scan.
pub(crate) fn execute(image: &Rgb32FImage, _alpha_only: bool) -> BBox {
    bbox::f32_samples::<3>(image.as_raw(), image.width(), image.height(), None)
}
