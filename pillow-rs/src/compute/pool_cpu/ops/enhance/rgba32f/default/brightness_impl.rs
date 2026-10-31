// NO_PILLOW_SUPPORT: ImageRgba32F is an internal native raster identity.
use crate::compute::pool_cpu::ops::enhance::helpers::native;
use crate::error::PilError;
use crate::raster::Rgba32FImage;

pub(crate) fn execute(source: &Rgba32FImage, factor: f64) -> Result<Rgba32FImage, PilError> {
    if factor == 1.0 {
        return Ok(source.clone());
    }
    let mut output = source.clone();
    native::scale_f32(output.as_mut(), 4, 3, factor);
    Ok(output)
}
