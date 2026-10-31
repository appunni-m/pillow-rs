//! Default Transform implementation for exact mode ImageRgba8.

use crate::compute::pool_cpu::ops::transform::helpers::byte_mode_default;

byte_mode_default!(
    ImageRgba8,
    crate::raster::RgbaImage,
    4,
    false,
    true,
    Generic,
    "transform ImageRgba8 output buffer shape mismatch"
);

