//! Selects typed I/F vector kernels after exact-mode strategy admission.

use crate::compute::pool_cpu::ops::rotate::ops::RotateRequest;
use crate::error::PilError;
use crate::raster::RgbaImage;

pub(crate) fn execute<const FLOAT: bool>(
    image: &RgbaImage,
    request: RotateRequest,
) -> Result<RgbaImage, PilError> {
    if request.nearest {
        super::vector_nearest::execute::<crate::raster::Rgba<u8>, 4>(image, request)
    } else {
        super::vector_typed_bilinear::execute::<FLOAT>(image, request)
    }
}
