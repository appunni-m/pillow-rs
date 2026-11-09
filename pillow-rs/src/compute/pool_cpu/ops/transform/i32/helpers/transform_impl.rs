//! Native-byte nearest Transform route for the exact mode.

use crate::compute::pool_cpu::ops::transform::helpers::byte_mode_nearest;

byte_mode_nearest!(
    ImageI32,
    crate::raster::RgbaImage,
    4,
    "ImageI32",
    "transform ImageI32 output shape mismatch"
);
