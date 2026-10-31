use crate::compute::pool_cpu::ops::enhance::helpers::color_saturation::{
    self, BASE_LUMA,
};
use crate::error::PilError;
use crate::raster::GrayAlphaImage;

pub(crate) fn execute(source: &GrayAlphaImage, factor: f64) -> Result<GrayAlphaImage, PilError> {
    color_saturation::execute_vector::<_, 2, BASE_LUMA>(source, factor)
}
