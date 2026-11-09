//! Native ImageI16L samples for ImageOps.expand's Default strategy.
use crate::raster::{ImageBuffer, Luma};

use crate::error::PilError;
use crate::pipeline::NativeColorInput;

pub(super) type Input = ImageBuffer<Luma<u16>, Vec<u16>>;

pub(crate) struct Kernel<'a> {
    input: &'a Input,
    border: crate::pipeline::ExpandMargins,
    fill: [u16; 1],
}

pub(super) fn prepare(
    input: &Input,
    border: crate::pipeline::ExpandMargins,
    fill: (u8, u8, u8, u8),
    native_fill: Option<NativeColorInput>,
) -> Kernel<'_> {
    let fill = match native_fill {
        Some(input) => [input.component(0, i64::from(fill.0)) as u16],
        None => [u16::from(fill.0)],
    };
    Kernel {
        input,
        border,
        fill,
    }
}

pub(super) fn execute(kernel: Kernel<'_>) -> Result<Input, PilError> {
    crate::compute::pool_cpu::ops::imageops::helpers::expand::expand_buffer(
        kernel.input,
        kernel.border,
        2,
        1,
        &kernel.fill,
    )
}
