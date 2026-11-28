//! Binary Chops secondary storage mapping for three-band exact modes.

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbImage};

use super::binary::BinaryOperation;

// VERIFY: source=reviewed@2026-11-26; parity=pass@migration-parity-9151618946984508b6673146ca4f53d0; benchmark=pending
// Scope: three-band byte Chops injects both raster identities into its kernel.
/// Applies raw channel arithmetic to the primary mode's typed pixel buffer.
pub(crate) fn execute(
    image: &RgbImage,
    left_mode: Mode,
    other: Option<&DynamicImage>,
    operation: BinaryOperation,
) -> Result<RgbImage, PilError> {
    let Some(other) = other else {
        return Err(PilError::InternalError(
            "Chops binary plan lost its secondary image".into(),
        ));
    };
    let (right, right_mode) = three_band_image(other)?;
    super::binary::execute_with_modes(image, right, operation, left_mode, right_mode)
}

// VERIFY: source=reviewed@2026-11-26; parity=pass@migration-parity-9151618946984508b6673146ca4f53d0; benchmark=pending
// Scope: byte Chops admits RGB, YCbCr, LAB, and HSV secondary carriers.
fn three_band_image(image: &DynamicImage) -> Result<(&RgbImage, Mode), PilError> {
    match image {
        DynamicImage::ImageRgb8(buffer) => Ok((buffer, Mode::ImageRgb8)),
        DynamicImage::ImageYCbCr8(buffer) => Ok((buffer, Mode::ImageYCbCr8)),
        DynamicImage::ImageLab8(buffer) => Ok((buffer, Mode::ImageLab8)),
        DynamicImage::ImageHsv8(buffer) => Ok((buffer, Mode::ImageHsv8)),
        _ => Err(PilError::InternalError(
            "three-band Chops received a different raster variant".into(),
        )),
    }
}
