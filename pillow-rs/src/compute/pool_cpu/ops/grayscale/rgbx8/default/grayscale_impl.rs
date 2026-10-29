//! Archived serial grayscale loop for the exact rgbx8 identity.

use crate::error::PilError;
use crate::raster::{RgbaImage, Rgba, GrayImage};
use super::super::super::helpers::luma::{Checked, finish};

pub(crate) type Input = RgbaImage;
pub(crate) type Kernel<'a> = Checked<'a, Rgba<u8>>;

pub(crate) fn prepare(input: &Input) -> Result<Kernel<'_>, PilError> {
    Checked::new(input)
}

pub(crate) fn execute(input: Kernel<'_>) -> Result<GrayImage, PilError> {
    let bytes = crate::color::grayscale_rgb_bytes::<4>(input.input.as_raw(), input.dimensions, 32768);
    finish(input.input.width(), input.input.height(), bytes)
}
