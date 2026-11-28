//! AVX-512F nearest Resize strategy for signed ImageI32 samples.

use crate::compute::ExecutionContext;
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::{Mode, RgbaImage};

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute(
    context: &ExecutionContext,
    input: &RgbaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: super::super::super::helpers::typed_nearest::BoxBounds,
    selection: &StrategySelection,
) -> Result<RgbaImage, PilError> {
    if !matches!(filter, ResampleFilter::Nearest) || bounds.is_some() {
        return Err(PilError::InternalError(
            "ImageI32 AVX-512F Resize received an unsupported request".into(),
        ));
    }
    super::super::super::helpers::avx512f_nearest::execute(
        context,
        selection,
        Mode::ImageI32,
        input,
        (width, height),
    )
}
