//! Native Rgb32FImage samples for ImageOps.expand's Default strategy.
use crate::raster::Rgb32FImage;

use crate::error::PilError;

pub(super) type Input = Rgb32FImage;


pub(super) struct Kernel<'a> {
    input: &'a Input,
    border: u32,
    fill: [f32; 3],
}

pub(super) fn prepare(
    input: &Input,
    border: u32,
    fill: (u8, u8, u8, u8),
) -> Kernel<'_> {
    Kernel { input, border, fill: [f32::from(fill.0), f32::from(fill.1), f32::from(fill.2)] }
}

pub(super) fn execute(kernel: Kernel<'_>) -> Result<Input, PilError> {
    crate::compute::pool_cpu::ops::imageops::helpers::expand::expand_buffer(
        kernel.input,
        kernel.border,
        12,
        3,
        &kernel.fill,
    )
}
