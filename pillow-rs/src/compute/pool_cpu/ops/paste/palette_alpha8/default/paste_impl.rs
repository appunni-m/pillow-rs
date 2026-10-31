use crate::compute::pool_cpu::ops::paste::helpers::{bytes, plan::BytePlan};
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage};

pub(crate) fn execute(strategy: BytePlan<'_>) -> Result<DynamicImage, PilError> {
    let (width, height) = strategy.destination_size;
    let bytes = bytes::execute::<2>(strategy)?;
    let image = GrayAlphaImage::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::InternalError("Paste output dimensions were invalid".into()))?;
    Ok(DynamicImage::ImagePaletteAlpha8(image))
}
