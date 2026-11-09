//! Archived serial grayscale loop for the exact luma_a8 identity.

use super::super::super::helpers::luma::{Checked, finish};
use crate::error::PilError;
use crate::raster::{GrayAlphaImage, GrayImage, LumaA};

pub(crate) type Input = GrayAlphaImage;
pub(crate) type Kernel<'a> = Checked<'a, LumaA<u8>>;

pub(crate) fn prepare(input: &Input) -> Result<Kernel<'_>, PilError> {
    Checked::new(input)
}

pub(crate) fn execute(input: Kernel<'_>) -> Result<GrayImage, PilError> {
    let bytes = input
        .input
        .as_raw()
        .chunks_exact(2)
        .map(|pixel| pixel[0])
        .collect();
    finish(input.input.width(), input.input.height(), bytes)
}
