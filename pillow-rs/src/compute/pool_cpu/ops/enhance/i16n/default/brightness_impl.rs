use crate::compute::pool_cpu::ops::enhance::helpers::native;
use crate::error::PilError;
use crate::raster::ImageBuffer;

type Image = ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>;

pub(crate) fn execute(source: &Image, factor: f64) -> Result<Image, PilError> {
    if factor == 1.0 {
        return Ok(source.clone());
    }
    let mut output = source.clone();
    native::scale_u16(output.as_mut(), 1, 1, factor);
    Ok(output)
}
