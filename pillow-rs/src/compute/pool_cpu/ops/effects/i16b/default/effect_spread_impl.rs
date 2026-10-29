// NO_PILLOW_SUPPORT: This mode uses the native-preserving core extension.
//! EffectSpread Default implementation for exact mode ImageI16B.

use crate::error::PilError;
use super::super::ops::Input;

pub(crate) type Kernel<'a> = crate::compute::pool_cpu::ops::effects::helpers::spread::Kernel<'a, Input>;

pub(crate) fn prepare(input: &Input, distance: u32) -> Kernel<'_> {
    crate::compute::pool_cpu::ops::effects::helpers::spread::prepare(input, distance)
}

pub(crate) fn execute(kernel: Kernel<'_>) -> Result<Input, PilError> {
    crate::compute::pool_cpu::ops::effects::helpers::spread::execute(kernel)
}
