//! Exact lab8 Chops support and mode strategy.

use crate::compute::registry::support::{ModeSupport, OperationId, StrategySelection};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) const MODE: Mode = Mode::ImageLab8;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn support(operation: OperationId) -> ModeSupport {
    super::super::ops::mode_support(MODE, operation)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(
    image: &DynamicImage,
    request: super::super::ops::ChopsRequest,
    other: Option<&DynamicImage>,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    match image {
        DynamicImage::ImageLab8(_) => {
            super::super::helpers::strategy::require_default_selection(selection, Mode::ImageLab8)?;
            super::default::ops::execute(image, request, other)
        }
        _ => Err(PilError::InternalError(
            "Chops mode strategy received a different raster variant".into(),
        )),
    }
}
