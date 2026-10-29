//! Serial Default Rotate implementation for exact mode ImageRgba32F.

use super::super::ops::Input;
use crate::compute::pool_cpu::ops::rotate::ops::RotateRequest;
use crate::error::PilError;

// NO_PILLOW_SUPPORT: this strategy rotates native f32 samples without re-tagging or converting the image.
pub(crate) fn execute(input: &Input, request: RotateRequest) -> Result<Input, PilError> {
    crate::compute::pool_cpu::ops::rotate::helpers::numeric::execute::<
        crate::raster::Rgba<f32>, f32, 4, false, true,
    >(input, request)
}
