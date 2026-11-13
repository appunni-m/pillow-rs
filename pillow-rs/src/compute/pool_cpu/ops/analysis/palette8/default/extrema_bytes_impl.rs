use crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes;
use crate::raster::GrayImage;
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(image: &GrayImage) -> Vec<(u8, u8)> {
    extrema_bytes::byte_bands_1(extrema_bytes::logical_samples(image))
}
