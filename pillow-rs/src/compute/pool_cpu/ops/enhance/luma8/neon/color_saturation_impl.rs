use crate::compute::pool_cpu::ops::enhance::helpers::color_saturation::{
    self, BASE_LUMA,
};
use crate::error::PilError;
use crate::raster::GrayImage;

pub(crate) fn execute(source: &GrayImage, factor: f64) -> Result<GrayImage, PilError> {
    color_saturation::execute_vector::<_, 1, BASE_LUMA>(source, factor)
}
