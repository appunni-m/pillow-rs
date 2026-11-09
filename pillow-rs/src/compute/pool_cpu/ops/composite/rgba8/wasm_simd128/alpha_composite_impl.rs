//! RGBA AlphaComposite route selected by the WASM_SIMD128 architecture strategy.

use crate::error::PilError;
use crate::raster::RgbaImage;

pub(crate) fn execute(source: &RgbaImage, destination: &RgbaImage) -> Result<RgbaImage, PilError> {
    let (width, height) = destination.dimensions();
    let output = crate::compute::pool_cpu::ops::composite::helpers::alpha_composite_vector::apply::<
        4,
    >(source.as_raw(), destination.as_raw(), width, height, "RGBA")?;
    RgbaImage::from_raw(width, height, output)
        .ok_or_else(|| PilError::InternalError("RGBA AlphaComposite output shape mismatch".into()))
}
