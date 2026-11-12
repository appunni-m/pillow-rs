//! Exact-mode strategy entry for ImageLumaA16.

use crate::compute::registry::support::{OperationId, StrategySelection};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) const MODE: Mode = Mode::ImageLumaA16;

// NO_PILLOW_SUPPORT: CompositeModule is an internal native high-depth extension.
super::super::helpers::declare_composite_module_support!(ImageLumaA16, core_only);

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
