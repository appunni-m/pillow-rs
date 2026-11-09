use super::rank_impl;
use crate::compute::pool_cpu::ops::filter::ops::FilterRequest;
use crate::error::PilError;
use crate::raster::DynamicImage;

pub(in crate::compute::pool_cpu::ops::filter::f32) fn execute(
    image: &DynamicImage,
    request: FilterRequest,
) -> Result<DynamicImage, PilError> {
    match request {
        FilterRequest::MedianFilter { size } => rank_impl::execute(image, size, size * size / 2),
        FilterRequest::MaxFilter { size } => rank_impl::execute(image, size, size * size - 1),
        FilterRequest::MinFilter { size } => rank_impl::execute(image, size, 0),
        FilterRequest::RankFilter { size, rank } => rank_impl::execute(image, size, rank),
        FilterRequest::Filter3x3 { .. }
        | FilterRequest::Filter5x5 { .. }
        | FilterRequest::GaussianBlur { .. }
        | FilterRequest::BoxBlur { .. }
        | FilterRequest::BoxBlurXY { .. } => Err(PilError::InternalError(
            "F32 filter strategy received an unsupported operation".into(),
        )),
    }
}
