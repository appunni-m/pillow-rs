use crate::compute::pool_cpu::ops::paste::helpers::{bytes, plan::BytePlan};
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbImage};

// VERIFY: source=reviewed@2026-11-15; parity=pending; benchmark=pending
pub(crate) fn execute<I>(strategy: BytePlan<'_, I>) -> Result<DynamicImage, PilError>
where
    I: std::ops::Deref<Target = crate::raster::DynamicImage>,
{
    let (width, height) = strategy.destination_size;
    let bytes = bytes::execute::<3, I>(strategy)?;
    let image = RgbImage::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::InternalError("Paste output dimensions were invalid".into()))?;
    Ok(DynamicImage::ImageRgb8(image))
}
