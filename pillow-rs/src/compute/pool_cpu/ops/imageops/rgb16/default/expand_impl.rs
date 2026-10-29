//! Native ImageRgb16 samples for ImageOps.expand's Default strategy.
use crate::raster::{ImageBuffer, Rgb};

use crate::error::PilError;

pub(super) type Input = ImageBuffer<Rgb<u16>, Vec<u16>>;


pub(super) struct Kernel<'a> {
    input: &'a Input,
    border: u32,
    fill: [u16; 3],
}

pub(super) fn prepare(
    input: &Input,
    border: u32,
    fill: (u8, u8, u8, u8),
) -> Kernel<'_> {
    Kernel { input, border, fill: [u16::from(fill.0), u16::from(fill.1), u16::from(fill.2)] }
}

pub(super) fn execute(kernel: Kernel<'_>) -> Result<Input, PilError> {
    crate::compute::pool_cpu::ops::imageops::helpers::expand::expand_buffer(
        kernel.input,
        kernel.border,
        6,
        3,
        &kernel.fill,
    )
}
