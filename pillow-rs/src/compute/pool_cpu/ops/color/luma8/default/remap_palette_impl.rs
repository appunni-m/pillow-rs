use crate::raster::GrayImage;

/// Runs L's serial remap strategy through the shared native-byte kernel.
#[inline]
pub(crate) fn execute(source: &GrayImage, destination: &[u8]) -> Vec<u8> {
    super::super::super::helpers::remap_palette::scalar(source.as_raw(), destination)
}
