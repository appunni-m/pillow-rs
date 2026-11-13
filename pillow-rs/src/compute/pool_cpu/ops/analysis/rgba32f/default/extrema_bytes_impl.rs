// NO_PILLOW_SUPPORT: this Rust/JS byte-valued result is outside Pillow's typed extrema API.
use crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes;
use crate::raster::Rgba32FImage;
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(image: &Rgba32FImage) -> Vec<(u8, u8)> {
    extrema_bytes::f32_bands_4(extrema_bytes::logical_samples(image))
}
