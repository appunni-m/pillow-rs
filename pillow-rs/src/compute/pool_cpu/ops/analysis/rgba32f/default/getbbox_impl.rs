use crate::compute::pool_cpu::ops::analysis::helpers::bbox::{self, BBox};
use crate::raster::Rgba32FImage;

// NO_PILLOW_SUPPORT: this internal raster identity uses a native GetBBox scan.
pub(crate) fn execute(image: &Rgba32FImage, alpha_only: bool) -> BBox {
    bbox::f32_samples::<4>(
        image.as_raw(),
        image.width(),
        image.height(),
        alpha_only.then_some(3),
    )
}
