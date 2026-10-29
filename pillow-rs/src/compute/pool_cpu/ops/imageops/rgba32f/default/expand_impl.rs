//! Native Rgba32FImage samples for ImageOps.expand's Default strategy.
use crate::raster::Rgba32FImage;

use crate::error::PilError;

pub(super) type Input = Rgba32FImage;


pub(super) struct Kernel<'a> {
    input: &'a Input,
    border: u32,
    fill: [f32; 4],
}

pub(super) fn prepare(
    input: &Input,
    border: u32,
    fill: (u8, u8, u8, u8),
) -> Kernel<'_> {
    Kernel { input, border, fill: [f32::from(fill.0), f32::from(fill.1), f32::from(fill.2), f32::from(fill.3)] }
}

pub(super) fn execute(kernel: Kernel<'_>) -> Result<Input, PilError> {
    crate::compute::pool_cpu::ops::imageops::helpers::expand::expand_buffer(
        kernel.input,
        kernel.border,
        16,
        4,
        &kernel.fill,
    )
}
