use crate::compute::pool_cpu::ops::analysis::helpers::stat::{self, StatOutput, StatRequest};
use crate::error::PilError;
use crate::raster::RgbaImage;

pub(crate) fn execute(
    _image: &RgbaImage,
    request: StatRequest<'_>,
) -> Result<StatOutput, PilError> {
    stat::histogram(request)
}
