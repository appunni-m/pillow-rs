//! Archived serial grayscale loop for the exact rgb8 identity.

use crate::error::PilError;
use crate::raster::{RgbImage, Rgb, GrayImage};
use super::super::super::helpers::luma::{Checked, finish};

pub(crate) type Input = RgbImage;
pub(crate) type Kernel<'a> = Checked<'a, Rgb<u8>>;

pub(crate) fn prepare(input: &Input) -> Result<Kernel<'_>, PilError> {
    Checked::new(input)
}

pub(crate) fn execute(input: Kernel<'_>) -> Result<GrayImage, PilError> {
    let bytes = crate::color::grayscale_rgb_bytes::<3>(input.input.as_raw(), input.dimensions, 32768);
    finish(input.input.width(), input.input.height(), bytes)
}
