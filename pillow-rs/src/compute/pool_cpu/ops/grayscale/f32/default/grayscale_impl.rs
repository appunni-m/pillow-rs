//! Original scalar sample conversion for f32.

use crate::raster::{RgbaImage, GrayImage};

pub(crate) type Input = RgbaImage;
pub(crate) type Kernel<'a> = &'a Input;

pub(crate) fn prepare(input: &Input) -> Kernel<'_> { input }
pub(crate) fn execute(input: Kernel<'_>) -> GrayImage {
    let output = input.as_raw().chunks_exact(4).map(|pixel| {
        let value = f32::from_le_bytes([pixel[0], pixel[1], pixel[2], pixel[3]]);
        value.clamp(0.0, 255.0) as u8
    }).collect();
    GrayImage::from_raw(input.width(), input.height(), output)
        .expect("F to L output matches the source dimensions")
}
