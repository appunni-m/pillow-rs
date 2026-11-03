use crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes;
use crate::raster::GrayAlphaImage;
pub(crate) fn execute(image: &GrayAlphaImage) -> Vec<(u8, u8)> {
    extrema_bytes::byte_bands_2(extrema_bytes::logical_samples(image))
}
