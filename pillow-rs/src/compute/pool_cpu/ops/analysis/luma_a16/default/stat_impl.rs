//! NO_PILLOW_SUPPORT: Stat consumes native-sample CoreOnly histogram bins.
use crate::compute::pool_cpu::ops::analysis::helpers::get_data::LumaA16Image;
use crate::compute::pool_cpu::ops::analysis::helpers::stat::{self, StatOutput, StatRequest};
use crate::error::PilError;

pub(crate) fn execute(
    _image: &LumaA16Image,
    request: StatRequest<'_>,
) -> Result<StatOutput, PilError> {
    stat::histogram(request)
}
