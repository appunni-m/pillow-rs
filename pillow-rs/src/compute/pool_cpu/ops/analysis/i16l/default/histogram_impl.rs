use crate::compute::pool_cpu::ops::analysis::helpers::get_data::Luma16Image;
use crate::error::PilError;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(image: &Luma16Image, mask_raw: Option<&[u8]>) -> Result<Vec<u32>, PilError> {
    Ok(crate::compute::pool_cpu::ops::analysis::helpers::histogram_samples::l16_byte_prefix_little_endian(
        image, mask_raw,
    ))
}
