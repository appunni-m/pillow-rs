//! Process native RGBA32F color samples directly for the L result.

use crate::error::PilError;
use crate::raster::{ImageBuffer, Rgba, GrayImage};
use super::super::super::helpers::luma::{Checked, native_rgb};

pub(crate) type Input = ImageBuffer<Rgba<f32>, Vec<f32>>;
pub(crate) type Kernel<'a> = Checked<'a, Rgba<f32>>;

pub(crate) fn prepare(input: &Input) -> Result<Kernel<'_>, PilError> {
    Checked::new(input)
}

pub(crate) fn execute(input: Kernel<'_>) -> Result<GrayImage, PilError> {
    native_rgb::<f32, 4>(input.input.as_raw(), input.input.width(), input.input.height(), input.dimensions)
}
