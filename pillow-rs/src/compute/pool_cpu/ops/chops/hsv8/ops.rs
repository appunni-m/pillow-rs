//! Exact hsv8 Chops support and mode strategy.

use crate::compute::registry::support::{ImplementationSupport, ModeSupport, OperationId};
use crate::compute::Architecture;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use super::super::helpers::strategy::declare_byte_binary_architectures;

pub(crate) const MODE: Mode = Mode::ImageHsv8;
const CHANNELS: u8 = 3;

declare_byte_binary_architectures!(MODE, CHANNELS);

pub(crate) fn support(operation: OperationId) -> ModeSupport {
    let architectures = binary_architectures_for(operation);
    super::super::ops::mode_support_with_architectures(MODE, operation, architectures)
}

pub(crate) fn execute(
    image: &DynamicImage,
    request: super::super::ops::ChopsRequest,
    other: Option<&DynamicImage>,
    architecture: Architecture,
) -> Result<DynamicImage, PilError> {
    match image {
        DynamicImage::ImageHsv8(_) if architecture == Architecture::Default => {
            default::ops::execute(image, request, other)
        }
        DynamicImage::ImageHsv8(_) => {
            super::super::helpers::vector::execute(image, request, other, MODE, architecture)
        }
        _ => Err(PilError::InternalError("Chops mode strategy received a different raster variant".into())),
    }
}
