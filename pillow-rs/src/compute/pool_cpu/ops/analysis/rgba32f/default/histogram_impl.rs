use crate::compute::pool_cpu::ops::analysis::helpers::histogram_samples;
use crate::error::PilError;
use crate::raster::Rgba32FImage;

// NO_PILLOW_SUPPORT: scans native f32 samples into the 256-bin extension.
pub(crate) fn execute(
    image: &Rgba32FImage,
    mask_raw: Option<&[u8]>,
) -> Result<Vec<u32>, PilError> {
    Ok(histogram_samples::f32_bands(image, mask_raw))
}
