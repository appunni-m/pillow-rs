//! AVX-512F nearest Resize strategy for ImageF32 samples.

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
    match selection.guard {
        crate::compute::registry::support::StrategyGuard::ResizeAvx512Nearest => {
            if !matches!(filter, ResampleFilter::Nearest) || bounds.is_some() {
                return Err(PilError::InternalError(
                    "ImageF32 AVX-512F nearest Resize received an unsupported request".into(),
                ));
            }
            super::super::super::helpers::avx512f_nearest::execute(
                context,
                selection,
                Mode::ImageF32,
                input,
                (width, height),
            )
        }
        crate::compute::registry::support::StrategyGuard::ResizeFilteredAvx512F => {
            super::filtered_resize_impl::execute(
                context, input, width, height, filter, bounds, selection,
            )
        }
        _ => Err(PilError::InternalError(
            "ImageF32 AVX-512F Resize received a mismatched strategy guard".into(),
        )),
    }
}
