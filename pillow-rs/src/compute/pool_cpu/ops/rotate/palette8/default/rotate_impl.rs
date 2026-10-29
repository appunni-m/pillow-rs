//! Serial Default Rotate implementation for exact mode ImagePalette8.

use super::super::ops::Input;
use crate::compute::pool_cpu::ops::rotate::ops::RotateRequest;
use crate::error::PilError;

pub(crate) fn execute(input: &Input, request: RotateRequest) -> Result<Input, PilError> {
    crate::compute::pool_cpu::ops::rotate::helpers::byte::execute::<
        crate::raster::Luma<u8>, 1, true, false,
    >(input, request)
}
