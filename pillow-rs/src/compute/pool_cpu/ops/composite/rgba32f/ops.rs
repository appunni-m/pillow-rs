//! Exact-mode strategy entry for ImageRgba32F.

use crate::compute::registry::support::{OperationId, StrategySelection};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) const MODE: Mode = Mode::ImageRgba32F;

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
