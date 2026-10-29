//! Native ImageRgba16 samples for ImageOps.expand's Default strategy.
use crate::raster::{ImageBuffer, Rgba};

use crate::error::PilError;

pub(super) type Input = ImageBuffer<Rgba<u16>, Vec<u16>>;


pub(super) struct Kernel<'a> {
    input: &'a Input,
    border: u32,
    fill: [u16; 4],
}

pub(super) fn prepare(
    input: &Input,
    border: u32,
    fill: (u8, u8, u8, u8),
) -> Kernel<'_> {
    Kernel { input, border, fill: [u16::from(fill.0), u16::from(fill.1), u16::from(fill.2), u16::from(fill.3)] }
}

pub(super) fn execute(kernel: Kernel<'_>) -> Result<Input, PilError> {
    crate::compute::pool_cpu::ops::imageops::helpers::expand::expand_buffer(
        kernel.input,
        kernel.border,
        8,
        4,
        &kernel.fill,
    )
}
