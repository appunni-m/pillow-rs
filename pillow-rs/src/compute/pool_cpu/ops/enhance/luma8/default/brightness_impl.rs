use crate::compute::pool_cpu::ops::enhance::helpers::native;
use crate::error::PilError;
use crate::raster::GrayImage;

pub(crate) fn execute(source: &GrayImage, factor: f64) -> Result<GrayImage, PilError> {
    if factor == 1.0 {
        return Ok(source.clone());
    }
    let bytes = native::scale_u8(source.as_raw(), 1, 1, factor);
    GrayImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("Brightness output dimensions were invalid".into()))
}
