//! Archived serial grayscale loop for the exact luma8 identity.

use crate::error::PilError;
use crate::raster::{GrayImage, Luma};
use super::super::super::helpers::luma::{Checked, finish};

pub(crate) type Input = GrayImage;
pub(crate) type Kernel<'a> = Checked<'a, Luma<u8>>;

pub(crate) fn prepare(input: &Input) -> Result<Kernel<'_>, PilError> {
    Checked::new(input)
}

pub(crate) fn execute(input: Kernel<'_>) -> Result<GrayImage, PilError> {
    let bytes = input.input.as_raw().to_vec();
    finish(input.input.width(), input.input.height(), bytes)
}
