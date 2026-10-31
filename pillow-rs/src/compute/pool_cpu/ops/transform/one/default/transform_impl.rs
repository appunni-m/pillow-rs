//! Default Transform implementation for exact mode One.

use crate::compute::pool_cpu::ops::transform::helpers::byte_mode_default;

byte_mode_default!(
    One,
    crate::raster::GrayImage,
    1,
    true,
    false,
    Generic,
    "transform One output buffer shape mismatch"
);

