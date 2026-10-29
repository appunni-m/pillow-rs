//! Process native RGB16 samples directly for the L result.

use crate::error::PilError;
use crate::raster::{ImageBuffer, Rgb, GrayImage};
use super::super::super::helpers::luma::{Checked, native_rgb};

pub(crate) type Input = ImageBuffer<Rgb<u16>, Vec<u16>>;
pub(crate) type Kernel<'a> = Checked<'a, Rgb<u16>>;

pub(crate) fn prepare(input: &Input) -> Result<Kernel<'_>, PilError> {
    Checked::new(input)
}

pub(crate) fn execute(input: Kernel<'_>) -> Result<GrayImage, PilError> {
    native_rgb::<u16, 3>(input.input.as_raw(), input.input.width(), input.input.height(), input.dimensions)
}
