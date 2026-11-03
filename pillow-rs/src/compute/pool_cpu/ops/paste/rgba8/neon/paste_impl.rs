use crate::compute::pool_cpu::ops::paste::helpers::{plan::BytePlan, vector};
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};

pub(crate) fn execute<I>(strategy: BytePlan<'_, I>) -> Result<DynamicImage, PilError>
where
    I: std::ops::Deref<Target = crate::raster::DynamicImage>,
{
    let (width, height) = strategy.destination_size;
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    let bytes = match super::rgba_row::try_execute(&strategy)? {
        Some(bytes) => bytes,
        None if super::super::helpers::rgb_to_rgba::supports(&strategy) => {
            super::super::helpers::rgb_to_rgba::execute(&strategy)?
        }
        None => vector::execute::<4, I>(strategy)?,
    };
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    let bytes = if super::super::helpers::rgb_to_rgba::supports(&strategy) {
        super::super::helpers::rgb_to_rgba::execute(&strategy)?
    } else {
        vector::execute::<4, I>(strategy)?
    };
    let image = RgbaImage::from_raw(width, height, bytes).ok_or_else(|| {
        PilError::InternalError("Paste vector output dimensions were invalid".into())
    })?;
    Ok(DynamicImage::ImageRgba8(image))
}
