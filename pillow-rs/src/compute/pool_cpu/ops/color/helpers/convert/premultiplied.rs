//! Direct output packing for opaque RGB samples converted to premultiplied modes.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage, LumaA, Mode, Rgba, RgbaImage};

// NO_PILLOW_SUPPORT: callers use this native sample path for Rust-only source
// modes whose RGB channels are computed per pixel without a staged RGB raster.
pub(crate) fn from_rgb_samples(
    width: u32,
    height: u32,
    samples: impl Iterator<Item = [u8; 3]>,
    target: Mode,
) -> Result<DynamicImage, PilError> {
    match target {
        Mode::ImageLaPremultiplied8 => {
            let mut output = GrayAlphaImage::new(width, height);
            for (destination, [red, green, blue]) in output.pixels_mut().zip(samples) {
                let luma = crate::color::rgb_to_luma_u8(red, green, blue);
                *destination = LumaA([luma, u8::MAX]);
            }
            Ok(DynamicImage::ImageLaPremultiplied8(output))
        }
        Mode::ImageRgbPremultiplied8 => {
            let mut output = RgbaImage::new(width, height);
            for (destination, [red, green, blue]) in output.pixels_mut().zip(samples) {
                *destination = Rgba([red, green, blue, u8::MAX]);
            }
            Ok(DynamicImage::ImageRgbPremultiplied8(output))
        }
        _ => Err(PilError::ValueError("image has wrong mode".into())),
    }
}
