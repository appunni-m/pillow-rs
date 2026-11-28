use crate::error::PilError;
use crate::raster::GrayAlphaImage;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute(
    source: &GrayAlphaImage,
    destination: &GrayAlphaImage,
) -> Result<GrayAlphaImage, PilError> {
    let pixels =
        crate::compute::pool_cpu::ops::composite::helpers::alpha_composite_avx512f::apply_la(
            source.as_raw(),
            destination.as_raw(),
            destination.width(),
            destination.height(),
        )?;
    GrayAlphaImage::from_raw(destination.width(), destination.height(), pixels)
        .ok_or_else(|| PilError::InternalError("LA AlphaComposite output shape mismatch".into()))
}
