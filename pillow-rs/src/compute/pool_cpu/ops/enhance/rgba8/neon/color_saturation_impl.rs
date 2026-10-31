use crate::compute::pool_cpu::ops::enhance::helpers::color_saturation::{
    self, BASE_RGB,
};
use crate::error::PilError;
use crate::raster::RgbaImage;

pub(crate) fn execute(source: &RgbaImage, factor: f64) -> Result<RgbaImage, PilError> {
    color_saturation::execute_vector::<_, 4, BASE_RGB>(source, factor)
}
