// NO_PILLOW_SUPPORT: this Rust/JS byte-valued result is outside Pillow's typed extrema API.
use crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes;
use crate::raster::Rgb32FImage;
pub(crate) fn execute(image: &Rgb32FImage) -> Vec<(u8, u8)> {
    extrema_bytes::f32_bands_3_with_opaque_alpha(extrema_bytes::logical_samples(image))
}
