//! Restore straight RGB using the original integer alpha division, then take luma.

use super::super::super::helpers::luma::{Checked, finish};
use crate::error::PilError;
use crate::raster::{GrayImage, Rgba, RgbaImage};

pub(crate) type Input = RgbaImage;
pub(crate) type Kernel<'a> = Checked<'a, Rgba<u8>>;
pub(crate) fn prepare(input: &Input) -> Result<Kernel<'_>, PilError> {
    Checked::new(input)
}
pub(crate) fn execute(input: Kernel<'_>) -> Result<GrayImage, PilError> {
    let source = crate::ops::pil_resize::unpremultiply_rgba(input.input);
    let output = crate::color::grayscale_rgb_bytes::<4>(source.as_raw(), input.dimensions, 32768);
    finish(source.width(), source.height(), output)
}
