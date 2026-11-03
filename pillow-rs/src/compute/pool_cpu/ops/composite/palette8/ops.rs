//! Exact-mode strategy entry for ImagePalette8.

use crate::compute::registry::support::{OperationId, StrategySelection};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use std::sync::Arc;

pub(crate) const MODE: Mode = Mode::ImagePalette8;

pub(crate) fn merge(
    source: &DynamicImage,
    remaining: &[Arc<DynamicImage>],
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    super::super::helpers::strategy::require_default(selection, OperationId::Merge, MODE)?;
    super::default::operations::merge(source, remaining)
}

pub(crate) fn composite(
    source: &DynamicImage,
    destination: &DynamicImage,
    mask: &DynamicImage,
    mask_alpha: bool,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    super::super::helpers::composite::execute_byte_selected(
        source,
        destination,
        mask,
        mask_alpha,
        MODE,
        1,
        selection,
        |plan| crate::compute::pool_cpu::ops::paste::palette8::ops::execute(plan),
    )
}
