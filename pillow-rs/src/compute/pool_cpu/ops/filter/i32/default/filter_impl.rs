use crate::error::PilError;
use crate::raster::DynamicImage;
use crate::compute::pool_cpu::ops::filter::ops::FilterRequest;
use super::{filter3x3_impl, filter5x5_impl, rank_impl};

pub(super) fn execute(
    image: &DynamicImage,
    request: FilterRequest,
) -> Result<DynamicImage, PilError> {
    match request {
        FilterRequest::Filter3x3 { kernel, scale, offset } => {
            filter3x3_impl::execute(image, &kernel, scale, offset)
        }
        FilterRequest::Filter5x5 { kernel, scale, offset } => {
            filter5x5_impl::execute(image, &kernel, scale, offset)
        }
        FilterRequest::MedianFilter { size } => rank_impl::execute(image, size, size * size / 2),
        FilterRequest::MaxFilter { size } => rank_impl::execute(image, size, size * size - 1),
        FilterRequest::MinFilter { size } => rank_impl::execute(image, size, 0),
        FilterRequest::RankFilter { size, rank } => rank_impl::execute(image, size, rank),
        FilterRequest::GaussianBlur { .. }
        | FilterRequest::BoxBlur { .. }
        | FilterRequest::BoxBlurXY { .. } => {
            Err(PilError::ValueError("image has wrong mode".into()))
        }
    }
}
