//! Native ImageRgb16 samples for ImageOps.expand's Default strategy.
use crate::raster::{ImageBuffer, Rgb};

use crate::error::PilError;
use crate::pipeline::NativeColorInput;

pub(super) type Input = ImageBuffer<Rgb<u16>, Vec<u16>>;


pub(super) struct Kernel<'a> {
    input: &'a Input,
    border: crate::pipeline::ExpandMargins,
    fill: [u16; 3],
}

pub(super) fn prepare(
    input: &Input,
    border: crate::pipeline::ExpandMargins,
    fill: (u8, u8, u8, u8),
    native_fill: Option<NativeColorInput>,
) -> Kernel<'_> {
    let fill = match native_fill {
        Some(input) => [
            input.component(0, i64::from(fill.0)) as u16,
            input.component(1, i64::from(fill.1)) as u16,
            input.component(2, i64::from(fill.2)) as u16,
        ],
        None => [u16::from(fill.0), u16::from(fill.1), u16::from(fill.2)],
    };
    Kernel { input, border, fill }
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
