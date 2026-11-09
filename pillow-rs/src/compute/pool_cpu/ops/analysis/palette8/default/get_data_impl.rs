use crate::compute::pool_cpu::ops::analysis::helpers::get_data as helpers;
use crate::error::PilError;
use crate::raster::GrayImage;
use crate::raster::Mode;

pub(crate) fn get_data(image: &GrayImage) -> Result<Vec<u8>, PilError> {
    Ok(image.as_raw().clone())
}

pub(crate) fn get_data_view(image: &GrayImage) -> &[u8] {
    image.as_raw()
}

pub(crate) fn palette_indices_view(indices: &[u8]) -> &[u8] {
    indices
}

pub(crate) fn write_band(
    image: &GrayImage,
    band: usize,
    output: &mut [u8],
) -> Result<(), PilError> {
    if band == 0 {
        output.copy_from_slice(image.as_raw());
        Ok(())
    } else {
        Err(helpers::mode_carrier_mismatch(Mode::ImagePalette8))
    }
}

/// Keep an owned P image's zero-copy index plane on the Palette8 strategy.
pub(crate) fn write_palette_indices_band(
    indices: &[u8],
    band: usize,
    output: &mut [u8],
) -> Result<(), PilError> {
    if band == 0 {
        output.copy_from_slice(indices);
        Ok(())
    } else {
        Err(helpers::mode_carrier_mismatch(Mode::ImagePalette8))
    }
}
