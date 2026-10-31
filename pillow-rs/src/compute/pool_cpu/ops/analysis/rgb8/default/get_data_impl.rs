use crate::compute::pool_cpu::ops::analysis::helpers::get_data as helpers;
use crate::error::PilError;
use crate::raster::RgbImage;

pub(crate) fn get_data(image: &RgbImage) -> Result<Vec<u8>, PilError> {
    Ok(image.as_raw().clone())
}

pub(crate) fn write_band(
    image: &RgbImage,
    band: usize,
    output: &mut [u8],
) -> Result<(), PilError> {
    helpers::write_interleaved_band(image.as_raw(), 3, band, output);
    Ok(())
}
