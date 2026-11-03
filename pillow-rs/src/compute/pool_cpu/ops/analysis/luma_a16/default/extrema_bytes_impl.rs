// NO_PILLOW_SUPPORT: this Rust/JS byte-valued result is outside Pillow's typed extrema API.
use crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes;
use crate::compute::pool_cpu::ops::analysis::helpers::get_data::LumaA16Image;
pub(crate) fn execute(image: &LumaA16Image) -> Vec<(u8, u8)> {
    extrema_bytes::projected_u16_luma_alpha(extrema_bytes::logical_samples(image))
}
