use crate::compute::pool_cpu::ops::analysis::helpers::stat::{self, StatOutput, StatRequest};
use crate::error::PilError;
use crate::raster::GrayImage;

pub(crate) fn execute(
    _image: &GrayImage,
    request: StatRequest<'_>,
) -> Result<StatOutput, PilError> {
    stat::histogram(request)
}
