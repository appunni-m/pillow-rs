//! Pillow mode-1 Thumbnail filter policy.

use crate::compute::registry::support::{ModeSupport, OperationId};
use crate::pipeline::ResampleFilter;
use crate::raster::Mode;

pub(crate) const SUPPORT: ModeSupport =
    ModeSupport::serial(OperationId::Thumbnail, Mode::One, super::super::ops::INPUTS);

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(in crate::compute::pool_cpu::ops::thumbnail) const fn effective_filter(
    _requested: ResampleFilter,
) -> ResampleFilter {
    ResampleFilter::Nearest
}
