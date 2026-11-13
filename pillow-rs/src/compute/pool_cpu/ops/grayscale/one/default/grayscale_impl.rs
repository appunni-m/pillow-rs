//! Original scalar sample conversion for one.

use crate::raster::GrayImage;

pub(crate) type Input = GrayImage;
pub(crate) type Kernel<'a> = &'a Input;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn prepare(input: &Input) -> Kernel<'_> {
    input
}
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(input: Kernel<'_>) -> GrayImage {
    let mut gray = input.clone();
    for pixel in gray.pixels_mut() {
        pixel[0] = if pixel[0] == 0 { 0 } else { u8::MAX };
    }
    gray
}
