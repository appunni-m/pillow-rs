//! Exact luma_a16 Chops support and mode strategy.

use crate::compute::registry::support::{ModeSupport, OperationId};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) const MODE: Mode = Mode::ImageLumaA16;

pub(crate) fn support(operation: OperationId) -> ModeSupport {
    super::super::ops::mode_support(MODE, operation)
}

pub(crate) fn execute(
    image: &DynamicImage,
    request: super::super::ops::ChopsRequest,
    other: Option<&DynamicImage>,
) -> Result<DynamicImage, PilError> {
    match image {
        DynamicImage::ImageLumaA16(_) => super::default::ops::execute(image, request, other),
        _ => Err(PilError::InternalError("Chops mode strategy received a different raster variant".into())),
    }
}
