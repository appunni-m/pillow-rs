//! Native Rgb32FImage samples for ImageOps.expand's Default strategy.
use crate::raster::Rgb32FImage;

use crate::error::PilError;
use crate::pipeline::NativeColorInput;

pub(super) type Input = Rgb32FImage;

pub(crate) struct Kernel<'a> {
    input: &'a Input,
    border: crate::pipeline::ExpandMargins,
    fill: [f32; 3],
}

pub(super) fn prepare(
    input: &Input,
    border: crate::pipeline::ExpandMargins,
    fill: (u8, u8, u8, u8),
    native_fill: Option<NativeColorInput>,
) -> Kernel<'_> {
    let fill = match native_fill {
        Some(input) => [
            input.float_component(0, f64::from(fill.0)) as f32,
            input.float_component(1, f64::from(fill.1)) as f32,
            input.float_component(2, f64::from(fill.2)) as f32,
        ],
        None => [f32::from(fill.0), f32::from(fill.1), f32::from(fill.2)],
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
        12,
        3,
        &kernel.fill,
    )
}
