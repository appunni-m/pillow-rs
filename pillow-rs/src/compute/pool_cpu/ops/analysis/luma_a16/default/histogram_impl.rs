use crate::compute::pool_cpu::ops::analysis::helpers::get_data::LumaA16Image;
use crate::compute::pool_cpu::ops::analysis::helpers::histogram_samples;
use crate::error::PilError;

// NO_PILLOW_SUPPORT: scans native u16 samples into the 256-bin extension.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(image: &LumaA16Image, mask_raw: Option<&[u8]>) -> Result<Vec<u32>, PilError> {
    Ok(histogram_samples::u16_bands(image, mask_raw))
}
