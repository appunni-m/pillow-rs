//! Native ImageLuma8 sample mapping for Eval's Default strategy.

use crate::compute::pool_cpu::ops::eval::helpers::lut::map_native_byte_lut;
use crate::error::PilError;
use crate::raster::GrayImage;

pub(crate) type Input = GrayImage;

pub(crate) struct Kernel<'a> {
    input: &'a Input,
    lut: &'a [u8],
}

pub(crate) fn prepare<'a>(input: &'a Input, lut: &'a [u8]) -> Kernel<'a> {
    Kernel { input, lut }
}

pub(crate) fn execute(kernel: Kernel<'_>) -> Result<Input, PilError> {
    let bytes = map_native_byte_lut::<1>(kernel.input.as_raw(), kernel.lut)
        .ok_or_else(|| PilError::InternalError("Eval LUT did not match native byte storage".into()))?;
    Input::from_raw(kernel.input.width(), kernel.input.height(), bytes)
        .ok_or_else(|| PilError::InternalError("Eval LUT output dimensions were invalid".into()))
}
