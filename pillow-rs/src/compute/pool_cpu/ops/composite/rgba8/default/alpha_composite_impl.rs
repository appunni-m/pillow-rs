//! Default serial RGBA AlphaComposite kernel.

use crate::compute::pool_cpu::ops::composite::helpers::alpha_composite;
use crate::error::PilError;
use crate::raster::RgbaImage;

pub(crate) fn execute(source: &RgbaImage, destination: &RgbaImage) -> Result<RgbaImage, PilError> {
    let (width, height) = destination.dimensions();
    let output =
        alpha_composite::apply::<4>(source.as_raw(), destination.as_raw(), width, height, "RGBA")?;
    RgbaImage::from_raw(width, height, output)
        .ok_or_else(|| PilError::InternalError("RGBA AlphaComposite output shape mismatch".into()))
}
