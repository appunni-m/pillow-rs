use crate::compute::pool_cpu::ops::analysis::helpers::get_data as helpers;
use crate::error::PilError;
use crate::raster::RgbImage;

pub(crate) fn get_data(image: &RgbImage) -> Result<Vec<u8>, PilError> {
    let mut output = image.as_raw().clone();
    for pixel in output.chunks_exact_mut(3) {
        pixel[1] = pixel[1].wrapping_sub(128);
        pixel[2] = pixel[2].wrapping_sub(128);
    }
    Ok(output)
}

pub(crate) fn write_band(image: &RgbImage, band: usize, output: &mut [u8]) -> Result<(), PilError> {
    helpers::write_lab_band(image.as_raw(), band, output);
    Ok(())
}
