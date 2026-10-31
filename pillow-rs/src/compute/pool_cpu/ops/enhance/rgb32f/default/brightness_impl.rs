// NO_PILLOW_SUPPORT: ImageRgb32F is an internal native raster identity.
use crate::compute::pool_cpu::ops::enhance::helpers::native;
use crate::error::PilError;
use crate::raster::Rgb32FImage;

pub(crate) fn execute(source: &Rgb32FImage, factor: f64) -> Result<Rgb32FImage, PilError> {
    if factor == 1.0 {
        return Ok(source.clone());
    }
    let mut output = source.clone();
    native::scale_f32(output.as_mut(), 3, 3, factor);
    Ok(output)
}
