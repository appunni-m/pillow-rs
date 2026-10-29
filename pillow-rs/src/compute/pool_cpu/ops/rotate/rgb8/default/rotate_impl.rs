//! Serial Default Rotate implementation for exact mode ImageRgb8.

use super::super::ops::Input;
use crate::compute::pool_cpu::ops::rotate::ops::RotateRequest;
use crate::error::PilError;

pub(crate) fn execute(input: &Input, request: RotateRequest) -> Result<Input, PilError> {
    crate::compute::pool_cpu::ops::rotate::helpers::byte::execute::<
        crate::raster::Rgb<u8>, 3, false, false,
    >(input, request)
}
