//! Direct unpremultiplication from native La samples.

use crate::compute::pool_cpu::ops::color::helpers::convert::pixel::unpremultiply_byte;
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage, LumaA, Mode};

pub(crate) fn execute(source: &GrayAlphaImage, target: Mode) -> Result<DynamicImage, PilError> {
    if target == Mode::ImageLab8 {
        // NO_PILLOW_SUPPORT: convert native premultiplied luma to straight
        // luma samples, then write LAB directly without an intermediate mode.
        return crate::compute::pool_cpu::ops::color::helpers::convert::lab_target::execute(
            source.width(),
            source.height(),
            source.pixels().map(|pixel| {
                let luma = unpremultiply_byte(pixel[0], pixel[1]);
                [luma; 3]
            }),
        );
    }
    if target != Mode::ImageLumaA8 {
        return Err(PilError::ValueError(
            "conversion from La to L not supported".into(),
        ));
    }
    let mut output = GrayAlphaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        *destination = LumaA([
            crate::compute::pool_cpu::ops::color::helpers::convert::pixel::unpremultiply_byte(
                pixel[0], pixel[1],
            ),
            pixel[1],
        ]);
    }
    Ok(DynamicImage::ImageLumaA8(output))
}
