//! Archived serial grayscale loop for the exact rgb8 identity.

use super::super::super::helpers::luma::{Checked, finish};
use crate::error::PilError;
use crate::raster::{GrayImage, Rgb, RgbImage};

pub(crate) type Input = RgbImage;
pub(crate) type Kernel<'a> = Checked<'a, Rgb<u8>>;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn prepare(input: &Input) -> Result<Kernel<'_>, PilError> {
    Checked::new(input)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(input: Kernel<'_>) -> Result<GrayImage, PilError> {
    let bytes =
        crate::color::grayscale_rgb_bytes::<3>(input.input.as_raw(), input.dimensions, 32768);
    finish(input.input.width(), input.input.height(), bytes)
}
