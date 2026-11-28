use crate::error::PilError;
use crate::raster::RgbaImage;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute(source: &RgbaImage, destination: &RgbaImage) -> Result<RgbaImage, PilError> {
    let pixels =
        crate::compute::pool_cpu::ops::composite::helpers::alpha_composite_avx512f::apply_rgba(
            source.as_raw(),
            destination.as_raw(),
            destination.width(),
            destination.height(),
        )?;
    RgbaImage::from_raw(destination.width(), destination.height(), pixels)
        .ok_or_else(|| PilError::InternalError("RGBA AlphaComposite output shape mismatch".into()))
}
