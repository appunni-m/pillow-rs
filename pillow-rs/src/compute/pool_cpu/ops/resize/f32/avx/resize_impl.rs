//! Ordered scalar implementation for ImageF32 Resize on x86 AVX/FMA targets.

use super::super::super::helpers::strategy::require_architecture;
use crate::compute::registry::support::StrategySelection;
use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::ops::pil_resize::PortableSeparateMulAdd;
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
    if architecture != Architecture::AvxFma {
        return Err(PilError::InternalError(
            "ImageF32 AVX/FMA strategy received another architecture".into(),
        ));
    }
    super::super::helpers::resize::execute_with_fma_and_filter::<{ usize::MAX }, _>(
        input,
        width,
        height,
        filter,
        bounds,
        &PortableSeparateMulAdd,
        super::super::x86_64::filters::filter_from_resample,
    )
}
