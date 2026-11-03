//! Native ImageLumaA16 samples for ImageOps.expand's Default strategy.
use crate::raster::{ImageBuffer, LumaA};

use crate::error::PilError;
use crate::pipeline::NativeColorInput;

pub(super) type Input = ImageBuffer<LumaA<u16>, Vec<u16>>;


pub(super) struct Kernel<'a> {
    input: &'a Input,
    border: crate::pipeline::ExpandMargins,
    fill: [u16; 2],
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
            input.component(1, i64::from(fill.3)) as u16,
        ],
        None => [u16::from(fill.0), u16::from(fill.3)],
    };
    Kernel { input, border, fill }
}

pub(super) fn execute(kernel: Kernel<'_>) -> Result<Input, PilError> {
    crate::compute::pool_cpu::ops::imageops::helpers::expand::expand_buffer(
        kernel.input,
        kernel.border,
        4,
        2,
        &kernel.fill,
    )
}
