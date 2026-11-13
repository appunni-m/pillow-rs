use crate::error::PilError;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(
    image: &crate::raster::GrayAlphaImage,
    mask_raw: Option<&[u8]>,
) -> Result<Vec<u32>, PilError> {
    Ok(
        crate::compute::pool_cpu::ops::analysis::helpers::histogram_samples::byte_bands::<_, false>(
            image, mask_raw,
        ),
    )
}
