use crate::compute::pool_cpu::ops::analysis::helpers::projection_vector::VectorProjection;
use crate::raster::RgbaImage;

pub(crate) fn execute(image: &RgbaImage) -> VectorProjection {
    crate::compute::pool_cpu::ops::analysis::helpers::projection_vector::cmyk_projection(image)
}
