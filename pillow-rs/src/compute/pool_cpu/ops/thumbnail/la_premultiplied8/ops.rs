//! Pillow premultiplied luma-alpha Thumbnail reducing-gap policy.

use crate::pipeline::ResampleFilter;
use crate::compute::registry::support::{ModeSupport, OperationId};
use crate::raster::Mode;

pub(crate) const SUPPORT: ModeSupport = ModeSupport::serial(
    OperationId::Thumbnail,
    Mode::ImageLaPremultiplied8,
    super::super::ops::INPUTS,
);

pub(in crate::compute::pool_cpu::ops::thumbnail) const fn should_reduce(
    _filter: ResampleFilter,
) -> bool {
    false
}
