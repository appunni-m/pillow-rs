// NO_PILLOW_SUPPORT: this Rust/JS byte-valued result is outside Pillow's typed extrema API.
use crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes;
use crate::compute::pool_cpu::ops::analysis::helpers::get_data::Luma16Image;
pub(crate) fn execute(image: &Luma16Image) -> Vec<(u8, u8)> {
    extrema_bytes::clipped_u16_luma(extrema_bytes::logical_samples(image))
}
