use crate::compute::pool_cpu::ops::analysis::helpers::projection::ProjectionAxes;
use crate::raster::RgbImage;

pub(crate) fn execute(image: &RgbImage) -> ProjectionAxes {
    crate::compute::pool_cpu::ops::analysis::helpers::projection::rgb_projection(image)
}
