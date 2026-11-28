//! NEON architecture strategy for ImageF32 Resize.

use super::super::super::helpers::strategy::require_architecture;
use crate::compute::registry::support::StrategySelection;
use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::RgbaImage;

// VERIFY: source=reviewed@2026-11-27; parity=pending; benchmark=pending
pub(crate) fn execute(
    context: &ExecutionContext,
    input: &RgbaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: super::super::super::helpers::typed_nearest::BoxBounds,
    selection: &StrategySelection,
) -> Result<RgbaImage, PilError> {
    let architecture = require_architecture(context, selection)?;
    if architecture != Architecture::Neon {
        return Err(PilError::InternalError(
            "ImageF32 NEON strategy received another architecture".into(),
        ));
    }
    super::super::helpers::vector::execute::<4>(input, width, height, filter, bounds)
}
