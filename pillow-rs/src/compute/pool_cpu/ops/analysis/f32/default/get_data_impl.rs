use crate::compute::pool_cpu::ops::analysis::helpers::get_data as helpers;
use crate::error::PilError;
use crate::image::ScalarImageSamples;
use crate::raster::{Mode, RgbaImage};

pub(crate) fn get_data(image: &RgbaImage) -> Result<Vec<u8>, PilError> {
    Ok(image.as_raw().clone())
}

pub(crate) fn write_band(
    image: &RgbaImage,
    _band: usize,
    output: &mut [u8],
) -> Result<(), PilError> {
    helpers::write_interleaved_band(image.as_raw(), 4, 0, output);
    Ok(())
}

pub(crate) fn scalar_samples(image: &RgbaImage) -> Result<ScalarImageSamples, PilError> {
    let raw = image.as_raw();
    helpers::validate_scalar_storage(
        raw,
        image.width(),
        image.height(),
        Mode::ImageF32,
    )?;
    let values = raw
        .chunks_exact(4)
        .map(|sample| {
            f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]) as f64
        })
        .collect();
    Ok(ScalarImageSamples::Float(values))
}
