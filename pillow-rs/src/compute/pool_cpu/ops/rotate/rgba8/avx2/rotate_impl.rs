use crate::compute::pool_cpu::ops::rotate::ops::RotateRequest;
use crate::error::PilError;

pub(crate) fn execute(
    input: &super::super::ops::Input,
    request: RotateRequest,
) -> Result<super::super::ops::Input, PilError> {
    crate::compute::pool_cpu::ops::rotate::helpers::vector::execute::<
        crate::raster::Rgba<u8>, 4, false, true,
    >(input, request)
}
