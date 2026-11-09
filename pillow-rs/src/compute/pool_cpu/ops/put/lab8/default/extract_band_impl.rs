//! Original serial ExtractBand algorithm for `ImageLab8`.

use super::super::super::helpers::extract_band::{self as helpers, CheckedBand};
use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) type Input = crate::raster::RgbImage;
pub(crate) type Kernel<'a> = CheckedBand<'a, crate::raster::Rgb<u8>>;

#[inline]
pub(crate) fn prepare(input: &Input, index: u8) -> Result<Kernel<'_>, PilError> {
    CheckedBand::new(input, index)
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    let channel = input.index;
    let mut output = helpers::rgb(input)?;
    if channel == 1 || channel == 2 {
        // Pillow exposes LAB A/B as normalized L bands; Merge adds the stored
        // 128 bias back after filtering the extracted band.
        let samples = output.as_bytes_mut().ok_or_else(|| {
            PilError::InternalError("LAB ExtractBand output has no byte storage".into())
        })?;
        for sample in samples {
            *sample = sample.wrapping_sub(128);
        }
    }
    Ok(output)
}
