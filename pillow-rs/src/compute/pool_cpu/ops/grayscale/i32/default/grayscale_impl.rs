//! Original scalar sample conversion for i32.

use crate::raster::{RgbaImage, GrayImage};

pub(crate) type Input = RgbaImage;
pub(crate) type Kernel<'a> = &'a Input;

pub(crate) fn prepare(input: &Input) -> Kernel<'_> { input }
pub(crate) fn execute(input: Kernel<'_>) -> GrayImage {
    let source = input.as_raw();
    let output = match bytemuck::try_cast_slice::<u8, i32>(source.as_slice()) {
        Ok(values) => values.iter().map(|&value| i32::from_le(value).clamp(0, 255) as u8).collect(),
        Err(_) => source.chunks_exact(4).map(|pixel| {
            let value = i32::from_le_bytes([pixel[0], pixel[1], pixel[2], pixel[3]]);
            value.clamp(0, 255) as u8
        }).collect(),
    };
    GrayImage::from_raw(input.width(), input.height(), output)
        .expect("I to L output matches the source dimensions")
}
