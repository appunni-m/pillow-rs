use crate::error::PilError;

pub(crate) fn execute(
    image: &crate::raster::RgbImage,
    mask_raw: Option<&[u8]>,
) -> Result<Vec<u32>, PilError> {
    Ok(crate::compute::pool_cpu::ops::analysis::helpers::histogram_samples::byte_bands::<_, false>(
        image, mask_raw,
    ))
}
