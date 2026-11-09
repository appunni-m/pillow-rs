use crate::compute::pool_cpu::ops::analysis::helpers::get_data::Luma16Image;
use crate::error::PilError;

pub(crate) fn execute(image: &Luma16Image, mask_raw: Option<&[u8]>) -> Result<Vec<u32>, PilError> {
    Ok(crate::compute::pool_cpu::ops::analysis::helpers::histogram_samples::l16_byte_prefix_big_endian(
        image, mask_raw,
    ))
}
