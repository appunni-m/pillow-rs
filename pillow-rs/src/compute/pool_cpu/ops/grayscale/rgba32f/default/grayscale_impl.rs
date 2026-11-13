//! Process native RGBA32F color samples directly for the L result.

use super::super::super::helpers::luma::{Checked, native_rgb};
use crate::error::PilError;
use crate::raster::{GrayImage, ImageBuffer, Rgba};

pub(crate) type Input = ImageBuffer<Rgba<f32>, Vec<f32>>;
pub(crate) type Kernel<'a> = Checked<'a, Rgba<f32>>;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn prepare(input: &Input) -> Result<Kernel<'_>, PilError> {
    Checked::new(input)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(input: Kernel<'_>) -> Result<GrayImage, PilError> {
    native_rgb::<f32, 4>(
        input.input.as_raw(),
        input.input.width(),
        input.input.height(),
        input.dimensions,
    )
}
