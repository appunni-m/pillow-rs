//! AVX-512F RGBA Merge adapter.

use crate::compute::Architecture;
use crate::compute::registry::support::{OperationId, StrategyGuard, StrategySelection};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use std::sync::Arc;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn merge(
    source: &DynamicImage,
    remaining: &[Arc<DynamicImage>],
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    if selection.implementation.operation != OperationId::Merge
        || selection.implementation.mode != Mode::ImageRgba8
        || selection.implementation.architecture != Architecture::Avx512F
        || selection.requested_architecture != Architecture::Avx512F
        || selection.guard
            != (StrategyGuard::MergeAvx512FInterleave {
                target_mode: Mode::ImageRgba8,
                channels: 4,
                minimum_pixels: 16,
            })
    {
        return Err(PilError::InternalError(
            "RGBA Merge received an invalid AVX-512F strategy selection".into(),
        ));
    }
    crate::compute::pool_cpu::ops::composite::helpers::merge::avx512f::interleave_four(
        source,
        remaining,
        Mode::ImageRgba8,
    )
}
