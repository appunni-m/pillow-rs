//! Pillow palette-index Thumbnail filter policy.

use crate::pipeline::ResampleFilter;
use crate::compute::registry::support::{ModeSupport, OperationId};
use crate::raster::Mode;

pub(crate) const SUPPORT: ModeSupport = ModeSupport::serial(
    OperationId::Thumbnail,
    Mode::ImagePalette8,
    super::super::ops::INPUTS,
);

pub(in crate::compute::pool_cpu::ops::thumbnail) const fn effective_filter(
    _requested: ResampleFilter,
) -> ResampleFilter {
    ResampleFilter::Nearest
}
