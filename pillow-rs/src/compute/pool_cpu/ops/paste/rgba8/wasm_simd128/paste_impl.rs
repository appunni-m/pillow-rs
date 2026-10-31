use crate::compute::pool_cpu::ops::paste::helpers::{plan::BytePlan, vector};
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};

pub(crate) fn execute(strategy: BytePlan<'_>) -> Result<DynamicImage, PilError> {
    let (width, height) = strategy.destination_size;
    let bytes = if super::super::helpers::rgb_to_rgba::supports(&strategy) {
        super::super::helpers::rgb_to_rgba::execute(&strategy)?
    } else {
        vector::execute::<4>(strategy)?
    };
    let image = RgbaImage::from_raw(width, height, bytes).ok_or_else(|| {
        PilError::InternalError("Paste vector output dimensions were invalid".into())
    })?;
    Ok(DynamicImage::ImageRgba8(image))
}
