use crate::compute::pool_cpu::ops::paste::helpers::{i16, plan::Luma16Plan};
use crate::error::PilError;
use crate::raster::{DynamicImage, ImageBuffer, Luma};

pub(crate) fn execute(strategy: Luma16Plan<'_>) -> Result<DynamicImage, PilError> {
    let (width, height) = strategy.destination_size;
    let source = std::sync::Arc::clone(&strategy.source);
    let source = i16::source_samples(source.as_ref()).ok_or_else(|| {
        PilError::InternalError("I;16 Paste source mode changed after preparation".into())
    })?;
    let samples = i16::execute(strategy, source)?;
    let image =
        ImageBuffer::<Luma<u16>, Vec<u16>>::from_raw(width, height, samples).ok_or_else(|| {
            PilError::InternalError("I;16 Paste output dimensions were invalid".into())
        })?;
    Ok(DynamicImage::ImageI16(image))
}
