use crate::compute::pool_cpu::ops::analysis::helpers::get_data as helpers;
use crate::compute::pool_cpu::ops::analysis::helpers::get_data::Luma16Image;
use crate::error::PilError;
use crate::raster::Mode;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn get_data(image: &Luma16Image) -> Result<Vec<u8>, PilError> {
    helpers::narrow_luma16_samples(
        image.as_raw(),
        image.width(),
        image.height(),
        Mode::ImageI16L,
    )
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn write_band(
    image: &Luma16Image,
    band: usize,
    output: &mut [u8],
) -> Result<(), PilError> {
    helpers::write_native_sample_band(
        image.as_raw(),
        image.width(),
        image.height(),
        1,
        band,
        Mode::ImageI16L,
        output,
        |sample| sample.min(u16::from(u8::MAX)) as u8,
    )
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn integer_samples(image: &Luma16Image) -> Result<Vec<i32>, PilError> {
    Ok(image.as_raw().iter().copied().map(i32::from).collect())
}
