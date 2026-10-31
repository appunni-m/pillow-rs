use crate::compute::pool_cpu::ops::analysis::helpers::projection::ProjectionAxes;
use crate::raster::RgbaImage;

pub(crate) fn execute(image: &RgbaImage) -> ProjectionAxes {
    crate::compute::pool_cpu::ops::analysis::helpers::projection::cmyk_projection(image)
}
