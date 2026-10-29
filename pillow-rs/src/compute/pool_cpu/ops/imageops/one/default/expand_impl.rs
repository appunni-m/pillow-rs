//! Native One samples for ImageOps.expand's Default strategy.
use crate::raster::GrayImage;

use crate::error::PilError;

pub(super) type Input = GrayImage;


pub(super) struct Kernel<'a> {
    input: &'a Input,
    border: u32,
    fill: [u8; 1],
}

pub(super) fn prepare(
    input: &Input,
    border: u32,
    fill: (u8, u8, u8, u8),
) -> Kernel<'_> {
    Kernel { input, border, fill: [fill.0] }
}

pub(super) fn execute(kernel: Kernel<'_>) -> Result<Input, PilError> {
    crate::compute::pool_cpu::ops::imageops::helpers::expand::expand_buffer(
        kernel.input,
        kernel.border,
        1,
        1,
        &kernel.fill,
    )
}
