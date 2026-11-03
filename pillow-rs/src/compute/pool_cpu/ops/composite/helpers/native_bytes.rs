//! Exact-mode construction for native one-to-four-byte operation outputs.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{DynamicImage, Luma, LumaA, Mode, Rgb, Rgba};

pub(crate) fn construct(
    width: u32,
    height: u32,
    mode: Mode,
    bytes: Vec<u8>,
) -> Result<DynamicImage, PilError> {
    match mode {
        Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 => {
            let buffer = crate::image_utils::typed_bytes_to_buffer::<Luma<u8>>(
                CheckedDims::new_allow_empty(width, height, 1)?,
                bytes,
            )?;
            match mode {
                Mode::One => Ok(DynamicImage::One(buffer)),
                Mode::ImageLuma8 => Ok(DynamicImage::ImageLuma8(buffer)),
                Mode::ImagePalette8 => Ok(DynamicImage::ImagePalette8(buffer)),
                _ => Err(invalid_target("one-channel")),
            }
        }
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => {
            let buffer = crate::image_utils::typed_bytes_to_buffer::<LumaA<u8>>(
                CheckedDims::new_allow_empty(width, height, 2)?,
                bytes,
            )?;
            match mode {
                Mode::ImageLumaA8 => Ok(DynamicImage::ImageLumaA8(buffer)),
                Mode::ImageLaPremultiplied8 => Ok(DynamicImage::ImageLaPremultiplied8(buffer)),
                Mode::ImagePaletteAlpha8 => Ok(DynamicImage::ImagePaletteAlpha8(buffer)),
                _ => Err(invalid_target("two-channel")),
            }
        }
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => {
            let buffer = crate::image_utils::typed_bytes_to_buffer::<Rgb<u8>>(
                CheckedDims::new_allow_empty(width, height, 3)?,
                bytes,
            )?;
            match mode {
                Mode::ImageRgb8 => Ok(DynamicImage::ImageRgb8(buffer)),
                Mode::ImageYCbCr8 => Ok(DynamicImage::ImageYCbCr8(buffer)),
                Mode::ImageLab8 => Ok(DynamicImage::ImageLab8(buffer)),
                Mode::ImageHsv8 => Ok(DynamicImage::ImageHsv8(buffer)),
                _ => Err(invalid_target("three-channel")),
            }
        }
        Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8 => {
            let buffer = crate::image_utils::typed_bytes_to_buffer::<Rgba<u8>>(
                CheckedDims::new_allow_empty(width, height, 4)?,
                bytes,
            )?;
            match mode {
                Mode::ImageRgba8 => Ok(DynamicImage::ImageRgba8(buffer)),
                Mode::ImageRgbPremultiplied8 => {
                    Ok(DynamicImage::ImageRgbPremultiplied8(buffer))
                }
                Mode::ImageRgbx8 => Ok(DynamicImage::ImageRgbx8(buffer)),
                Mode::ImageCmyk8 => Ok(DynamicImage::ImageCmyk8(buffer)),
                _ => Err(invalid_target("four-channel")),
            }
        }
        // Pillow's I and F Composite paths operate on their four raw carrier
        // bytes. Preserve that arithmetic while retaining their exact modes.
        Mode::ImageI32 | Mode::ImageF32 => {
            let buffer = crate::image_utils::typed_bytes_to_buffer::<Rgba<u8>>(
                CheckedDims::new_allow_empty(width, height, 4)?,
                bytes,
            )?;
            match mode {
                Mode::ImageI32 => Ok(DynamicImage::ImageI32(buffer)),
                Mode::ImageF32 => Ok(DynamicImage::ImageF32(buffer)),
                _ => Err(invalid_target("four-byte scalar carrier")),
            }
        }
        _ => Err(PilError::InternalError(
            "non-byte mode reached native byte construction".into(),
        )),
    }
}

fn invalid_target(layout: &'static str) -> PilError {
    PilError::InternalError(format!("invalid {layout} byte-mode output identity"))
}
