// NO_PILLOW_SUPPORT: internal u16 Paste uses a native typed sample blend.
use crate::compute::pool_cpu::ops::paste::helpers::{plan::NativeU16Plan, typed};
use crate::error::PilError;
use crate::raster::{DynamicImage, ImageBuffer, Rgb};

pub(crate) fn execute(strategy: NativeU16Plan<'_>) -> Result<DynamicImage, PilError> {
    let (width, height) = strategy.destination_size;
    let source = std::sync::Arc::clone(&strategy.source);
    let source = match source.as_ref() {
        DynamicImage::ImageRgb16(image) => image.as_raw(),
        _ => {
            return Err(PilError::InternalError(
                "native u16 Paste source mode changed after preparation".into(),
            ));
        }
    };
    let samples = typed::execute_u16::<3>(strategy, source)?;
    let image =
        ImageBuffer::<Rgb<u16>, Vec<u16>>::from_raw(width, height, samples).ok_or_else(|| {
            PilError::InternalError("native u16 Paste output dimensions were invalid".into())
        })?;
    Ok(DynamicImage::ImageRgb16(image))
}
