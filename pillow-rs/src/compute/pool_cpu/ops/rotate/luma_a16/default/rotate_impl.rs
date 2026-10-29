//! Serial Default Rotate implementation for exact mode ImageLumaA16.

use super::super::ops::Input;
use crate::compute::pool_cpu::ops::rotate::ops::RotateRequest;
use crate::error::PilError;

// NO_PILLOW_SUPPORT: this strategy rotates native u16 samples without re-tagging or converting the image.
pub(crate) fn execute(input: &Input, request: RotateRequest) -> Result<Input, PilError> {
    crate::compute::pool_cpu::ops::rotate::helpers::numeric::execute::<
        crate::raster::LumaA<u16>, u16, 2, false, true,
    >(input, request)
}
