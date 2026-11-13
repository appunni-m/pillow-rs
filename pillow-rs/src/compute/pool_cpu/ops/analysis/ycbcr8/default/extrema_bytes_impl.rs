use crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes;
use crate::raster::RgbImage;
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(image: &RgbImage) -> Vec<(u8, u8)> {
    extrema_bytes::byte_bands_3(extrema_bytes::logical_samples(image))
}
