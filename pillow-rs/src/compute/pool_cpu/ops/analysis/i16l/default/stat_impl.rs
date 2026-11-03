use crate::compute::pool_cpu::ops::analysis::helpers::stat::{self, StatOutput, StatRequest};
use crate::error::PilError;
use crate::compute::pool_cpu::ops::analysis::helpers::get_data::Luma16Image;

pub(crate) fn execute(
    _image: &Luma16Image,
    request: StatRequest<'_>,
) -> Result<StatOutput, PilError> {
    stat::histogram(request)
}
