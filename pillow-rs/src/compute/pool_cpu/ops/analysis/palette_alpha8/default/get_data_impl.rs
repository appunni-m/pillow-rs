use crate::compute::pool_cpu::ops::analysis::helpers::get_data as helpers;
use crate::error::PilError;
use crate::raster::GrayAlphaImage;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn get_data(image: &GrayAlphaImage) -> Result<Vec<u8>, PilError> {
    Ok(image.as_raw().clone())
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn write_band(
    image: &GrayAlphaImage,
    band: usize,
    output: &mut [u8],
) -> Result<(), PilError> {
    helpers::write_interleaved_band(image.as_raw(), 2, band, output);
    Ok(())
}
