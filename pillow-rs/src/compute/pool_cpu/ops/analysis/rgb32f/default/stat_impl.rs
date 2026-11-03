//! NO_PILLOW_SUPPORT: Stat consumes native-sample CoreOnly histogram bins.
use crate::compute::pool_cpu::ops::analysis::helpers::stat::{self, StatOutput, StatRequest};
use crate::error::PilError;
use crate::raster::Rgb32FImage;

pub(crate) fn execute(
    _image: &Rgb32FImage,
    request: StatRequest<'_>,
) -> Result<StatOutput, PilError> {
    stat::histogram(request)
}
