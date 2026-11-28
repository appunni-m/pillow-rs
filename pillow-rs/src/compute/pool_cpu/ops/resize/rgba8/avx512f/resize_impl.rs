//! AVX-512F nearest Resize strategy for RGBA8 samples.

use crate::compute::ExecutionContext;
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::{Mode, RgbaImage};

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute(
    context: &ExecutionContext,
    selection: &StrategySelection,
    input: &RgbaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> Result<RgbaImage, PilError> {
    if !matches!(filter, ResampleFilter::Nearest) || bounds.is_some() {
        return Err(PilError::InternalError(
            "RGBA8 AVX-512F Resize received an unsupported request".into(),
        ));
    }
    super::super::super::helpers::avx512f_nearest::execute(
        context,
        selection,
        Mode::ImageRgba8,
        input,
        (width, height),
    )
}
