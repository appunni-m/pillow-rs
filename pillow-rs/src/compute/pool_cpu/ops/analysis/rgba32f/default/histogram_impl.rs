use crate::compute::pool_cpu::ops::analysis::helpers::histogram_samples;
use crate::error::PilError;
use crate::raster::Rgba32FImage;

// NO_PILLOW_SUPPORT: scans native f32 samples into the 256-bin extension.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(image: &Rgba32FImage, mask_raw: Option<&[u8]>) -> Result<Vec<u32>, PilError> {
    Ok(histogram_samples::f32_bands(image, mask_raw))
}
