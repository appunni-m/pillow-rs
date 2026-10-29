//! Native ImageYCbCr8 samples for ImageOps.expand's Default strategy.
use crate::raster::RgbImage;

use crate::error::PilError;

pub(super) type Input = RgbImage;


pub(super) struct Kernel<'a> {
    input: &'a Input,
    border: u32,
    fill: [u8; 3],
}

pub(super) fn prepare(
    input: &Input,
    border: u32,
    fill: (u8, u8, u8, u8),
) -> Kernel<'_> {
    Kernel { input, border, fill: [fill.0, fill.1, fill.2] }
}

pub(super) fn execute(kernel: Kernel<'_>) -> Result<Input, PilError> {
    crate::compute::pool_cpu::ops::imageops::helpers::expand::expand_buffer(
        kernel.input,
        kernel.border,
        3,
        3,
        &kernel.fill,
    )
}
