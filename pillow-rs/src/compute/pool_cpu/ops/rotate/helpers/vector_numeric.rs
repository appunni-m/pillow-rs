//! Selects typed I/F vector kernels after exact-mode strategy admission.

use crate::compute::pool_cpu::ops::rotate::ops::RotateRequest;
use crate::error::PilError;
use crate::raster::{GenericImageView, RgbaImage};

pub(crate) fn execute<const FLOAT: bool>(
    image: &RgbaImage,
    request: RotateRequest,
) -> Result<RgbaImage, PilError> {
    if image.width() == 0
        || image.height() == 0
        || super::geometry::identity_contract(request)
        || super::geometry::discrete_fast_angle(request, image.width(), image.height()).is_some()
    {
        return super::vector::execute::<crate::raster::Rgba<u8>, 4, false, false>(image, request);
    }
    if request.nearest {
        super::vector_nearest::execute::<crate::raster::Rgba<u8>, 4>(image, request)
    } else {
        super::vector_typed_bilinear::execute::<FLOAT>(image, request)
    }
}
