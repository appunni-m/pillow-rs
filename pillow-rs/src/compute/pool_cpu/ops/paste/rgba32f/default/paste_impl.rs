// NO_PILLOW_SUPPORT: internal f32 Paste uses a native typed sample blend.
use crate::compute::pool_cpu::ops::paste::helpers::{plan::NativeF32Plan, typed};
use crate::error::PilError;
use crate::raster::{DynamicImage, Rgba32FImage};

pub(crate) fn execute(strategy: NativeF32Plan<'_>) -> Result<DynamicImage, PilError> {
    let (width, height) = strategy.destination_size;
    let source = std::sync::Arc::clone(&strategy.source);
    let source = match source.as_ref() {
        DynamicImage::ImageRgba32F(image) => image.as_raw(),
        _ => return Err(PilError::InternalError("native f32 Paste source mode changed after preparation".into())),
    };
    let samples = typed::execute_f32::<4>(strategy, source)?;
    let image = Rgba32FImage::from_raw(width, height, samples)
        .ok_or_else(|| PilError::InternalError("native f32 Paste output dimensions were invalid".into()))?;
    Ok(DynamicImage::ImageRgba32F(image))
}
