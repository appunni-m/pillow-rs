//! Exact rgb32f Chops support and mode strategy.

use crate::compute::registry::support::{ModeSupport, OperationId};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) const MODE: Mode = Mode::ImageRgb32F;

pub(crate) fn support(operation: OperationId) -> ModeSupport {
    super::super::ops::mode_support(MODE, operation)
}

pub(crate) fn execute(
    image: &DynamicImage,
    request: super::super::ops::ChopsRequest,
    other: Option<&DynamicImage>,
) -> Result<DynamicImage, PilError> {
    match image {
        DynamicImage::ImageRgb32F(_) => default::ops::execute(image, request, other),
        _ => Err(PilError::InternalError("Chops mode strategy received a different raster variant".into())),
    }
}
