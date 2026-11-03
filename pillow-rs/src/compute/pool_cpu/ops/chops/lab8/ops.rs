//! Exact lab8 Chops support and mode strategy.

use crate::compute::registry::support::{ModeSupport, OperationId};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) const MODE: Mode = Mode::ImageLab8;

pub(crate) fn support(operation: OperationId) -> ModeSupport {
    super::super::ops::mode_support(MODE, operation)
}

pub(crate) fn execute(
    image: &DynamicImage,
    request: super::super::ops::ChopsRequest,
    other: Option<&DynamicImage>,
) -> Result<DynamicImage, PilError> {
    match image {
        DynamicImage::ImageLab8(_) => super::default::ops::execute(image, request, other),
        _ => Err(PilError::InternalError("Chops mode strategy received a different raster variant".into())),
    }
}
