use crate::compute::pool_cpu::ops::paste::helpers::{bytes, plan::BytePlan};
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};

// Pillow F Paste blends stored byte lanes for masked input; retain all four
// lanes as one F pixel rather than interpreting them as a numeric float blend.
// VERIFY: source=reviewed@2026-11-20; parity=pending; benchmark=pending
pub(crate) fn execute<I>(strategy: BytePlan<'_, I>) -> Result<DynamicImage, PilError>
where
    I: std::ops::Deref<Target = crate::raster::DynamicImage>,
{
    let (width, height) = strategy.destination_size;
    let bytes = bytes::execute::<4, I>(strategy)?;
    let image = RgbaImage::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::InternalError("Paste output dimensions were invalid".into()))?;
    Ok(DynamicImage::ImageF32(image))
}
