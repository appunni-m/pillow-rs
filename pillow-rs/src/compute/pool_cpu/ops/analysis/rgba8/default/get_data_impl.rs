use crate::compute::pool_cpu::ops::analysis::helpers::get_data as helpers;
use crate::error::PilError;
use crate::raster::RgbaImage;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn get_data(image: &RgbaImage) -> Result<Vec<u8>, PilError> {
    Ok(image.as_raw().clone())
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn write_band(
    image: &RgbaImage,
    band: usize,
    output: &mut [u8],
) -> Result<(), PilError> {
    helpers::write_interleaved_band(image.as_raw(), 4, band, output);
    Ok(())
}
