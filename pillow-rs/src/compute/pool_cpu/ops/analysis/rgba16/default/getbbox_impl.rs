use crate::compute::pool_cpu::ops::analysis::helpers::bbox::{self, BBox};
use crate::compute::pool_cpu::ops::analysis::helpers::get_data::Rgba16Image;

// NO_PILLOW_SUPPORT: this internal raster identity uses a native GetBBox scan.
pub(crate) fn execute(image: &Rgba16Image, alpha_only: bool) -> BBox {
    bbox::u16_samples::<4>(
        image.as_raw(),
        image.width(),
        image.height(),
        alpha_only.then_some(3),
    )
}
