use crate::compute::pool_cpu::ops::enhance::helpers::native;
use crate::error::PilError;
use crate::raster::RgbaImage;

pub(crate) fn execute(source: &RgbaImage, factor: f64) -> Result<RgbaImage, PilError> {
    if factor == 1.0 {
        return Ok(source.clone());
    }
    // RGBX has no uppercase `A` band, so Pillow blends its X byte too.
    let bytes = native::scale_u8(source.as_raw(), 4, 4, factor);
    RgbaImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("Brightness output dimensions were invalid".into()))
}
