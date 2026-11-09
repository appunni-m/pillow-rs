//! Exact-mode strategy entry for ImageLaPremultiplied8.

use crate::compute::registry::support::{OperationId, StrategySelection};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use std::sync::Arc;

pub(crate) const MODE: Mode = Mode::ImageLaPremultiplied8;

pub(crate) fn merge(
    source: &DynamicImage,
    remaining: &[Arc<DynamicImage>],
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    super::super::helpers::strategy::require_default(selection, OperationId::Merge, MODE)?;
    super::default::operations::merge(source, remaining)
}

pub(crate) fn blend(
    source: &DynamicImage,
    other: &DynamicImage,
    alpha: f64,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    super::super::helpers::blend::execute_selected(source, other, MODE, 2, alpha, selection)
}

pub(crate) fn composite(
    source: &DynamicImage,
    destination: &DynamicImage,
    mask: &DynamicImage,
    mask_alpha: bool,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    super::super::helpers::strategy::require_default(
        selection,
        OperationId::CompositeModule,
        MODE,
    )?;
    super::default::operations::composite(source, destination, mask, mask_alpha)
}
