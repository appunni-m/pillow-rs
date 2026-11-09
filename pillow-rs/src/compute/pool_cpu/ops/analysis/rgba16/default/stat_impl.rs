//! NO_PILLOW_SUPPORT: Stat consumes native-sample CoreOnly histogram bins.
use crate::compute::pool_cpu::ops::analysis::helpers::get_data::Rgba16Image;
use crate::compute::pool_cpu::ops::analysis::helpers::stat::{self, StatOutput, StatRequest};
use crate::error::PilError;

pub(crate) fn execute(
    _image: &Rgba16Image,
    request: StatRequest<'_>,
) -> Result<StatOutput, PilError> {
    stat::histogram(request)
}
