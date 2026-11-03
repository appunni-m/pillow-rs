use crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes;
use crate::raster::RgbaImage;
pub(crate) fn execute(image: &RgbaImage) -> Vec<(u8, u8)> {
    extrema_bytes::byte_bands_4(extrema_bytes::logical_samples(image))
}
