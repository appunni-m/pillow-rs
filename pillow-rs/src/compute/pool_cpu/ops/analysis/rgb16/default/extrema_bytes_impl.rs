// NO_PILLOW_SUPPORT: this Rust/JS byte-valued result is outside Pillow's typed extrema API.
use crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes;
use crate::compute::pool_cpu::ops::analysis::helpers::get_data::Rgb16Image;
pub(crate) fn execute(image: &Rgb16Image) -> Vec<(u8, u8)> {
    extrema_bytes::u16_bands_3(extrema_bytes::logical_samples(image))
}
