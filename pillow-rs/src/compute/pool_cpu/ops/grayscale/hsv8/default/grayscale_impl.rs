//! Preserve HSV-to-RGB conversion before the original rounded luma loop.

use crate::error::PilError;
use crate::raster::{GrayImage, Rgb, RgbImage};
use super::super::super::helpers::luma::{Checked, rgb};

pub(crate) type Input = RgbImage;
pub(crate) type Kernel<'a> = Checked<'a, Rgb<u8>>;
pub(crate) fn prepare(input: &Input) -> Result<Kernel<'_>, PilError> { Checked::new(input) }
pub(crate) fn execute(input: Kernel<'_>) -> Result<GrayImage, PilError> {
    let source = crate::color::hsv_buffer_to_rgb(input.input);
    rgb(&source, input.dimensions)
}
