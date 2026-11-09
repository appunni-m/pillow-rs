//! LA AlphaComposite route selected by the SSE2 architecture strategy.

use crate::error::PilError;
use crate::raster::GrayAlphaImage;

pub(crate) fn execute(
    source: &GrayAlphaImage,
    destination: &GrayAlphaImage,
) -> Result<GrayAlphaImage, PilError> {
    let (width, height) = destination.dimensions();
    let output = crate::compute::pool_cpu::ops::composite::helpers::alpha_composite_vector::apply::<
        2,
    >(source.as_raw(), destination.as_raw(), width, height, "LA")?;
    GrayAlphaImage::from_raw(width, height, output)
        .ok_or_else(|| PilError::InternalError("LA AlphaComposite output shape mismatch".into()))
}
