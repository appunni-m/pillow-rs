//! ImageF32 Thumbnail resize policy.

use crate::compute::registry::support::{ModeSupport, OperationId};
use crate::raster::Mode;

pub(crate) const SUPPORT: ModeSupport = ModeSupport::serial(
    OperationId::Thumbnail,
    Mode::ImageF32,
    super::super::ops::INPUTS,
);

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(in crate::compute::pool_cpu::ops::thumbnail) fn uses_unboxed_resize(
    source: (u32, u32),
    output: (u32, u32),
    factors: (u32, u32),
) -> bool {
    factors == (2, 2)
        && source.0 % 2 == 0
        && source.1 % 2 == 0
        && output.0 < source.0 / 2
        && output.1 < source.1 / 2
}
