//! AVX non-FMA strategy for ImageF32 Resize.

use super::super::super::helpers::strategy::require_architecture;
use crate::compute::registry::support::StrategySelection;
use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::RgbaImage;

// VERIFY: source=reviewed@2026-11-28; parity=pass@f32-vector-closure-20261128; benchmark=pending
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
    if architecture != Architecture::Avx {
        return Err(PilError::InternalError(
            "ImageF32 AVX strategy received another architecture".into(),
        ));
    }
    super::super::helpers::vector::execute_with_filter::<16>(
        input,
        width,
        height,
        filter,
        bounds,
        super::super::x86_64::filters::filter_from_resample,
    )
}
