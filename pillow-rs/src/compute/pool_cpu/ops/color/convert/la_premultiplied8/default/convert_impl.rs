//! Direct unpremultiplication from native La samples.

use crate::error::PilError;
use crate::raster::{DynamicImage, GenericImageView, GrayAlphaImage, LumaA, Mode};
use super::super::super::helpers::pixel::unpremultiply_byte;

pub(crate) fn execute(
    source: &GrayAlphaImage,
    target: Mode,
) -> Result<DynamicImage, PilError> {
    if target == Mode::ImageLab8 {
        // NO_PILLOW_SUPPORT: convert native premultiplied luma to straight
        // luma samples, then write LAB directly without an intermediate mode.
        return super::super::super::helpers::lab_target::execute(
            source.width(),
            source.height(),
            source.pixels().map(|pixel| {
                let luma = unpremultiply_byte(pixel[0], pixel[1]);
                [luma; 3]
            }),
        );
    }
    if target == Mode::ImageLaPremultiplied8 {
        return Ok(DynamicImage::ImageLaPremultiplied8(source.clone()));
    }
    if target != Mode::ImageLumaA8 {
        return Err(PilError::ValueError(
            "conversion from La to L not supported".into(),
        ));
    }
    let mut output = GrayAlphaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        *destination = LumaA([
            crate::compute::pool_cpu::ops::color::convert::helpers::pixel::unpremultiply_byte(
                pixel[0], pixel[1],
            ),
            pixel[1],
        ]);
    }
    Ok(DynamicImage::ImageLumaA8(output))
}

