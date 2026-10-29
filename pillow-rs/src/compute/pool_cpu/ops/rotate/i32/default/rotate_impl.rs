//! Serial Default Rotate implementation for exact mode ImageI32.

use super::super::ops::Input;
use crate::compute::pool_cpu::ops::rotate::ops::RotateRequest;
use crate::error::PilError;

pub(crate) fn execute(input: &Input, request: RotateRequest) -> Result<Input, PilError> {
    crate::compute::pool_cpu::ops::rotate::helpers::scalar::execute(input, request, false)
}
