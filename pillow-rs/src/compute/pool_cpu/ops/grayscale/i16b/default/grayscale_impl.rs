//! Process native unsigned-16 luma samples with Pillow's established clipping.

use crate::error::PilError;
use crate::raster::{ImageBuffer, Luma, GrayImage};
use super::super::super::helpers::luma::{Checked, native_luma16_clipped};

pub(crate) type Input = ImageBuffer<Luma<u16>, Vec<u16>>;
pub(crate) type Kernel<'a> = Checked<'a, Luma<u16>>;

pub(crate) fn prepare(input: &Input) -> Result<Kernel<'_>, PilError> {
    Checked::new(input)
}

pub(crate) fn execute(input: Kernel<'_>) -> Result<GrayImage, PilError> {
    native_luma16_clipped(input.input.as_raw(), input.input.width(), input.input.height(), input.dimensions)
}
