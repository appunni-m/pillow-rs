use crate::compute::pool_cpu::ops::enhance::helpers::native;
use crate::error::PilError;
use crate::raster::GrayAlphaImage;

pub(crate) fn execute(source: &GrayAlphaImage, factor: f64) -> Result<GrayAlphaImage, PilError> {
    if factor == 1.0 {
        return Ok(source.clone());
    }
    // Pillow's Brightness wrapper preserves only an uppercase `A` band.
    // The premultiplied `a` band is blended with the luma sample.
    let bytes = native::scale_u8(source.as_raw(), 2, 2, factor);
    GrayAlphaImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("Brightness output dimensions were invalid".into()))
}
