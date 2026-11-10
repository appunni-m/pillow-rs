use crate::compute::pool_cpu::ops::analysis::helpers::bbox::{self, BBox};
use crate::compute::pool_cpu::ops::analysis::helpers::get_data::Luma16Image;

pub(crate) fn execute(image: &Luma16Image, _alpha_only: bool) -> BBox {
    bbox::u16_samples::<1>(image.as_raw(), image.width(), image.height(), None)
}
