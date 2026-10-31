use crate::compute::pool_cpu::ops::paste::helpers::{plan::BytePlan, vector};
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage};

pub(crate) fn execute(strategy: BytePlan<'_>) -> Result<DynamicImage, PilError> {
    let (width, height) = strategy.destination_size;
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    let bytes = match super::luma_row::try_execute(&strategy)? {
        Some(bytes) => bytes,
        None => vector::execute::<1>(strategy)?,
    };
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    let bytes = vector::execute::<1>(strategy)?;
    let image = GrayImage::from_raw(width, height, bytes).ok_or_else(|| {
        PilError::InternalError("Paste vector output dimensions were invalid".into())
    })?;
    Ok(DynamicImage::ImageLuma8(image))
}
