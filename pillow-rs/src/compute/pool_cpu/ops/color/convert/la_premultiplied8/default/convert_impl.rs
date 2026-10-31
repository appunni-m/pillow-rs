//! Direct unpremultiplication from native La samples.

use crate::error::PilError;
use crate::raster::{DynamicImage, GenericImageView, GrayAlphaImage, LumaA, Mode};

pub(crate) fn execute(
    source: &GrayAlphaImage,
    target: Mode,
) -> Result<DynamicImage, PilError> {
    if target != Mode::ImageLumaA8 {
        return Err(PilError::ValueError("image has wrong mode".into()));
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
