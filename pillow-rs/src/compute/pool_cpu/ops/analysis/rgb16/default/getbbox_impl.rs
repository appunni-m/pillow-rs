use crate::compute::pool_cpu::ops::analysis::helpers::bbox::{self, BBox};
use crate::compute::pool_cpu::ops::analysis::helpers::get_data::Rgb16Image;

// NO_PILLOW_SUPPORT: this internal raster identity uses a native GetBBox scan.
pub(crate) fn execute(image: &Rgb16Image, _alpha_only: bool) -> BBox {
    bbox::u16_samples::<3>(
        image.as_raw(),
        image.width(),
        image.height(),
        None,
    )
}
