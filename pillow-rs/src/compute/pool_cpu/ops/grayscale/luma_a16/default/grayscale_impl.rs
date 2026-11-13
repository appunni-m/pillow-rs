//! Process native LA16 luma samples directly for the L result.

use super::super::super::helpers::luma::{Checked, native_luma_a16};
use crate::error::PilError;
use crate::raster::{GrayImage, ImageBuffer, LumaA};

pub(crate) type Input = ImageBuffer<LumaA<u16>, Vec<u16>>;
pub(crate) type Kernel<'a> = Checked<'a, LumaA<u16>>;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn prepare(input: &Input) -> Result<Kernel<'_>, PilError> {
    Checked::new(input)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(input: Kernel<'_>) -> Result<GrayImage, PilError> {
    native_luma_a16(
        input.input.as_raw(),
        input.input.width(),
        input.input.height(),
        input.dimensions,
    )
}
