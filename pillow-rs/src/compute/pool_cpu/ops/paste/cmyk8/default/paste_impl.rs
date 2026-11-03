use crate::compute::pool_cpu::ops::paste::helpers::{bytes, plan::BytePlan};
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};

pub(crate) fn execute<I>(strategy: BytePlan<'_, I>) -> Result<DynamicImage, PilError>
where
    I: std::ops::Deref<Target = crate::raster::DynamicImage>,
{
    let (width, height) = strategy.destination_size;
    let bytes = bytes::execute::<4, I>(strategy)?;
    let image = RgbaImage::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::InternalError("Paste output dimensions were invalid".into()))?;
    Ok(DynamicImage::ImageCmyk8(image))
}
