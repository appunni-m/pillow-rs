use crate::compute::pool_cpu::ops::enhance::helpers::native;
use crate::error::PilError;
use crate::raster::RgbImage;

// NO_PILLOW_SUPPORT: Pillow's Brightness constructor fails while building its
// LAB blend base; this native byte strategy extends that operation.
pub(crate) fn execute(source: &RgbImage, factor: f64) -> Result<RgbImage, PilError> {
    if factor == 1.0 {
        return Ok(source.clone());
    }
    let bytes = native::scale_u8(source.as_raw(), 3, 3, factor);
    RgbImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("Brightness output dimensions were invalid".into()))
}
