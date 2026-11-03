use crate::error::PilError;

pub(crate) fn execute(
    image: &crate::raster::GrayAlphaImage,
    mask_raw: Option<&[u8]>,
) -> Result<Vec<u32>, PilError> {
    Ok(crate::compute::pool_cpu::ops::analysis::helpers::histogram_samples::byte_bands::<_, true>(
        image, mask_raw,
    ))
}
