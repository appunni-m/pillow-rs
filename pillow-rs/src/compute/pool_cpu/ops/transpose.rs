//! CPU Transpose mode routing.

use crate::error::PilError;
use crate::pipeline::TransposeMethod;
use crate::raster::DynamicImage;

mod kernels;

fn native_channels(mode: Option<&str>, image: &DynamicImage) -> Option<usize> {
    match (mode, image) {
        (Some("1" | "L" | "P"), DynamicImage::ImageLuma8(_)) => Some(1),
        (Some("LA" | "La" | "PA"), DynamicImage::ImageLumaA8(_)) => Some(2),
        (Some("RGB" | "HSV" | "LAB" | "YCbCr"), DynamicImage::ImageRgb8(_)) => Some(3),
        (Some("RGBA" | "RGBa" | "RGBX" | "CMYK" | "I" | "F"), DynamicImage::ImageRgba8(_)) => {
            Some(4)
        }
        _ => None,
    }
}

/// Transpose the selected Pillow mode while retaining its wrapper-owned mode.
pub(crate) fn execute_transpose(
    image: &DynamicImage,
    method: &TransposeMethod,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if let Some(channels) = native_channels(mode, image) {
        return kernels::execute_transpose(image, method, channels);
    }

    execute_raster_transpose(image, method)
}

fn execute_raster_transpose(
    image: &DynamicImage,
    method: &TransposeMethod,
) -> Result<DynamicImage, PilError> {
    // PIL rotates counter-clockwise; image crate rotates clockwise.
    match method {
        TransposeMethod::FlipLeftRight => Ok(image.fliph()),
        TransposeMethod::FlipTopBottom => Ok(image.flipv()),
        TransposeMethod::Rotate90 => Ok(image.rotate270()),
        TransposeMethod::Rotate180 => Ok(image.rotate180()),
        TransposeMethod::Rotate270 => Ok(image.rotate90()),
        TransposeMethod::Transpose | TransposeMethod::Transverse => {
            Ok(image.transpose_diagonal(matches!(method, TransposeMethod::Transverse)))
        }
    }
}

#[cfg(test)]
#[path = "transpose/tests.rs"]
mod tests;
