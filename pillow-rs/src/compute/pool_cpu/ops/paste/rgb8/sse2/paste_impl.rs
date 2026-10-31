use crate::compute::pool_cpu::ops::paste::helpers::{plan::BytePlan, vector};
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbImage};

pub(crate) fn execute(strategy: BytePlan<'_>) -> Result<DynamicImage, PilError> {
    let (width, height) = strategy.destination_size;
    let bytes = if super::super::helpers::rgba_to_rgb::supports(&strategy) {
        super::super::helpers::rgba_to_rgb::execute(&strategy)?
    } else {
        vector::execute::<3>(strategy)?
    };
    let image = RgbImage::from_raw(width, height, bytes).ok_or_else(|| {
        PilError::InternalError("Paste vector output dimensions were invalid".into())
    })?;
    Ok(DynamicImage::ImageRgb8(image))
}
