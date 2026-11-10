//! Pillow RGBA Thumbnail reduction policy.
//!
//! Keep the separate Reduce child disabled; `mode_strategy` passes the gap to
//! ResizeBoxed's native straight-alpha strategy.

use crate::compute::registry::support::{ModeSupport, OperationId};
use crate::pipeline::ResampleFilter;
use crate::raster::Mode;

pub(crate) const SUPPORT: ModeSupport = ModeSupport::serial(
    OperationId::Thumbnail,
    Mode::ImageRgba8,
    super::super::ops::INPUTS,
);

pub(in crate::compute::pool_cpu::ops::thumbnail) const fn should_reduce(
    _filter: ResampleFilter,
) -> bool {
    false
}
