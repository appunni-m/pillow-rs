//! Binary Chops secondary storage mapping for three-band exact modes.

use crate::error::PilError;
use crate::raster::{DynamicImage, RgbImage};

use super::binary::BinaryOperation;

// VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
/// Applies raw channel arithmetic to the primary mode's typed pixel buffer.
pub(crate) fn execute(
    image: &RgbImage,
    other: Option<&DynamicImage>,
    operation: BinaryOperation,
) -> Result<RgbImage, PilError> {
    let Some(other) = other else {
        return Err(PilError::InternalError(
            "Chops binary plan lost its secondary image".into(),
        ));
    };
    let right = three_band_image(other)?;
    super::binary::execute(image, right, operation)
}

// VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
fn three_band_image(image: &DynamicImage) -> Result<&RgbImage, PilError> {
    match image {
        DynamicImage::ImageRgb8(buffer)
        | DynamicImage::ImageYCbCr8(buffer)
        | DynamicImage::ImageLab8(buffer)
        | DynamicImage::ImageHsv8(buffer) => Ok(buffer),
        _ => Err(PilError::InternalError(
            "three-band Chops received a different raster variant".into(),
        )),
    }
}
